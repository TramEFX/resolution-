//! [`Playback`] on FilmCraft: the session's project snapshot (an `Arc`, shared, never copied) with
//! the media pool that decodes it. FilmCraft's own UI renders playback frames and mixes sound
//! the same way, from worker threads (`ui-egui/src/frames.rs`, `playback_mix` in its `lib.rs`).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use filmcraft_engine::media_pool::PoolProvider;
use filmcraft_engine::previews::PreviewStore;
use filmcraft_engine::project::{ItemId, Project};
use filmcraft_engine::render::{RenderOptions, SourceProvider, render_sequence};
use filmcraft_engine::time::{FrameRate, Tick};
use filmcraft_engine::{MediaPool, Services, Session};
use filmcraft_frame::{AudioBuffer, VideoFrame};
use filmcraft_media::{FrameRequest, MediaInfo, MediaSource, SharedSource, cancel};
use resolution_engine_api::{EngineError, Frame, Mixer, Playback, Rate, RenderHints, Result};

/// A media source that adds the time its `video_frame` calls take to a counter: the decoding
/// part of a frame's render time ([`RenderHints::decode_ns`]). As FilmCraft's frame server does
/// (`TimedSource` in `ui-egui/src/frames.rs`); every other call is passed straight through.
struct TimedSource(SharedSource, Arc<AtomicU64>);

impl MediaSource for TimedSource {
    fn info(&self) -> &MediaInfo {
        self.0.info()
    }
    fn video_frame(&self, req: FrameRequest) -> filmcraft_media::Result<Arc<VideoFrame>> {
        let t = Instant::now();
        let frame = self.0.video_frame(req);
        self.1.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
        frame
    }
    fn audio(&self, start: i64, frames: usize, sample_rate: u32) -> filmcraft_media::Result<AudioBuffer> {
        self.0.audio(start, frames, sample_rate)
    }
    fn audio_stream(
        &self,
        stream: usize,
        start: i64,
        frames: usize,
        sample_rate: u32,
    ) -> filmcraft_media::Result<AudioBuffer> {
        self.0.audio_stream(stream, start, frames, sample_rate)
    }
}

struct TimedProvider(PoolProvider, Arc<AtomicU64>);

/// The thread pool playback frames render in. FilmCraft spreads a frame's compositing over
/// rayon's global pool, which its audio mixer uses too: frames rendering there made the sound's
/// mixing wait behind them and drop out (LOG B-014). In a pool of their own, they compete with
/// the sound only for CPU time.
fn frame_pool() -> Option<&'static rayon::ThreadPool> {
    static POOL: OnceLock<Option<rayon::ThreadPool>> = OnceLock::new();
    POOL.get_or_init(|| {
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get().saturating_sub(1).max(2));
        rayon::ThreadPoolBuilder::new().num_threads(threads).thread_name(|i| format!("frame-render-{i}")).build().ok()
    })
    .as_ref()
}

impl SourceProvider for TimedProvider {
    fn source(&self, item: ItemId) -> Option<SharedSource> {
        let s = self.0.source(item)?;
        Some(Arc::new(TimedSource(s, self.1.clone())))
    }
}

pub struct Snapshot {
    project: Arc<Project>,
    pool: Arc<MediaPool>,
    services: Arc<dyn Services>,
    previews: Arc<PreviewStore>,
    seq: ItemId,
    rate: FrameRate,
    frames: i64,
    size: (u32, u32),
    revision: u64,
}

impl Snapshot {
    /// The session's active timeline, if one is open.
    pub fn of(session: &Session) -> Option<Self> {
        let seq = session.state.active_sequence?;
        let q = session.project.sequence(seq)?;
        let rate = q.settings.frame_rate;
        let duration = q.duration();
        // frames that start before the end
        let last = rate.frame_at(duration);
        let frames = if rate.tick_of(last) < duration { last + 1 } else { last };
        Some(Self {
            project: session.project.clone(),
            pool: session.media.clone(),
            services: session.services.clone(),
            previews: session.previews.clone(),
            seq,
            rate,
            frames: frames.max(0),
            size: (q.settings.width, q.settings.height),
            revision: session.revision,
        })
    }
}

impl Playback for Snapshot {
    fn revision(&self) -> u64 {
        self.revision
    }

    fn rate(&self) -> Rate {
        Rate::new(self.rate.num, self.rate.den)
    }

    fn frames(&self) -> i64 {
        self.frames
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }

    fn render(&self, frame: i64, max_side: u32, hints: &RenderHints) -> Result<Frame> {
        let (w, h) = self.size;
        let scale = (max_side.max(1) as f32 / w.max(h).max(1) as f32).min(1.0);
        let t = self.rate.tick_of(frame.max(0));
        let provider =
            TimedProvider(self.pool.provider(self.project.clone(), self.services.clone()), hints.decode_ns.clone());
        let opts = RenderOptions { scale, captions: true, ..Default::default() };
        let catch_up = hints.catch_up.map(Tick::from_seconds_f64);
        // the decoder hints are per thread: set on the thread that renders
        let render = || {
            cancel::with_cancel(&hints.cancel, || {
                cancel::with_catch_up(catch_up, || {
                    cancel::with_draft(hints.draft, || render_sequence(&self.project, self.seq, t, opts, &provider))
                })
            })
        };
        let img = match frame_pool() {
            Some(pool) => pool.install(render),
            None => render(),
        };
        // a cancelled render may be missing layers whose decoder gave up
        if hints.cancel.load(Ordering::Relaxed) {
            return Err(EngineError::Engine("cancelled".into()));
        }
        Ok(Frame { width: img.w as u32, height: img.h as u32, rgba: img.over_black_rgba8() })
    }

    fn mixer(&self, sample_rate: u32) -> Mixer {
        let project = self.project.clone();
        let seq = self.seq;
        let provider = self.pool.provider(project.clone(), self.services.clone());
        let previews = self.previews.clone();
        previews.live.publish_project(project.clone());
        let mut resampler: Option<filmcraft_audio_dsp::resample::StreamResampler> = None;
        Box::new(move |start: i64, buf: &mut [f32], ch: usize| {
            // the engine publishes each edit here: what is heard is the newest project
            let project =
                previews.live.project().filter(|p| p.sequence(seq).is_some()).unwrap_or_else(|| project.clone());
            let Some(q) = project.sequence(seq) else {
                buf.fill(0.0);
                return;
            };
            let n = buf.len() / ch.max(1);
            let timeline_rate = q.settings.sample_rate;
            let channels = if timeline_rate == sample_rate {
                previews.mix(&project, seq, start, n, &provider).channels
            } else {
                // mixed at the timeline's rate as one continuous stream, interpolated to ours
                let r = resampler.get_or_insert_with(|| {
                    filmcraft_audio_dsp::resample::StreamResampler::new(timeline_rate, sample_rate)
                });
                if r.rates() != (timeline_rate.max(1), sample_rate.max(1)) {
                    *r = filmcraft_audio_dsp::resample::StreamResampler::new(timeline_rate, sample_rate);
                }
                r.process(start, n, |s0, m| previews.mix(&project, seq, s0, m, &provider).channels)
            };
            let (Some(left), Some(right)) = (channels.first(), channels.get(1).or(channels.first())) else {
                buf.fill(0.0);
                return;
            };
            filmcraft_engine::settings::map_output(left, right, buf, ch, [0, 1]);
        })
    }
}

/// Put the operating system's video decoders (Media Foundation on Windows, VideoToolbox on
/// macOS, VA-API or NVDEC on Linux) in front of FilmCraft's own, as FilmCraft's desktop app does
/// (LOG D-014). Process-wide; streams they decline still decode in software. Returns the
/// decoder's name, or why there is none.
pub fn use_hardware_decoders() -> std::result::Result<&'static str, &'static str> {
    match filmcraft_platform::register() {
        filmcraft_platform::Availability::Available(name) => Ok(name),
        filmcraft_platform::Availability::Unavailable(why) => Err(why),
    }
}
