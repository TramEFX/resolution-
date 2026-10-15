//! [`Playback`] on FilmCraft: the session's project snapshot (an `Arc`, shared, never copied) with
//! the media pool that decodes it. FilmCraft's own UI renders playback frames and mixes sound
//! the same way, from worker threads (`ui-egui/src/frames.rs`, `playback_mix` in its `lib.rs`).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use filmcraft_engine::media_pool::PoolProvider;
use filmcraft_engine::previews::PreviewStore;
use filmcraft_engine::project::{ItemId, ItemKind, Project};
use filmcraft_engine::render::{RenderOptions, SourceProvider, render_item, render_sequence};
use filmcraft_engine::time::{FrameRate, Tick};
use filmcraft_engine::{MediaPool, Services, Session};
use filmcraft_frame::{AudioBuffer, VideoFrame};
use filmcraft_media::{FrameRequest, MediaInfo, MediaSource, SharedSource, cancel};
use resolution_engine_api::{EngineError, Frame, Mixer, PEAK_RATE, PEAK_SAMPLES, Playback, Rate, RenderHints, Result};

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

/// Runs `render` in the calling thread's own frame pool. FilmCraft spreads a frame's compositing
/// over rayon's global pool, which its audio mixer uses too: frames rendering there made the
/// sound's mixing wait behind them and drop out (LOG B-014). In pools of their own, frames compete
/// with the sound only for CPU time.
///
/// One pool per rendering thread (each frame worker), never one shared pool: FilmCraft's decoder
/// turns sideways phone video upright with rayon while holding the decoder's lock. In a shared
/// pool, a thread waiting for that work could pick up the other worker's frame, which then waited
/// for the same lock on the same thread, forever (LOG B-015).
fn in_frame_pool<R: Send>(render: impl FnOnce() -> R + Send) -> R {
    static POOLS: AtomicU64 = AtomicU64::new(0);
    thread_local! {
        // half the cores, less one, each: the two frame workers together leave a core for the sound
        static POOL: Option<rayon::ThreadPool> = {
            let threads = std::thread::available_parallelism().map_or(2, |n| (n.get().saturating_sub(1) / 2).max(2));
            let n = POOLS.fetch_add(1, Ordering::Relaxed);
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .thread_name(move |i| format!("frame-render-{n}-{i}"))
                .build()
                .ok()
        };
    }
    POOL.with(|pool| match pool {
        Some(pool) => pool.install(render),
        None => render(),
    })
}

impl SourceProvider for TimedProvider {
    fn source(&self, item: ItemId) -> Option<SharedSource> {
        let s = self.0.source(item)?;
        Some(Arc::new(TimedSource(s, self.1.clone())))
    }
}

pub struct Snapshot {
    project: Arc<Project>,
    /// The timeline as the GPU renderer draws it (LOG D-036).
    graded: crate::gpu::Graded,
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
            graded: crate::gpu::Graded::of(&session.project, seq),
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
        // the decoder hints are per thread: set on the thread that renders. Resolution's GPU
        // renderer draws the frame where it covers it (LOG D-036), else FilmCraft's CPU renderer.
        let gpu = in_frame_pool(|| {
            cancel::with_cancel(&hints.cancel, || {
                cancel::with_catch_up(catch_up, || {
                    cancel::with_draft(hints.draft, || {
                        self.graded.render(self.seq, t, opts, &provider, hints.gpu, hints.highlight)
                    })
                })
            })
        });
        let frame = match gpu {
            Some(f) => f,
            None => {
                let img = in_frame_pool(|| {
                    cancel::with_cancel(&hints.cancel, || {
                        cancel::with_catch_up(catch_up, || {
                            cancel::with_draft(hints.draft, || {
                                // grades graded at their source (LOG D-044), else as compiled
                                self.graded
                                    .render_cpu(self.seq, t, opts, &provider, hints.highlight)
                                    .unwrap_or_else(|| render_sequence(&self.project, self.seq, t, opts, &provider))
                            })
                        })
                    })
                });
                crate::gpu::counted_cpu_frame();
                Frame::new(img.w as u32, img.h as u32, img.over_black_rgba8())
            }
        };
        // a cancelled render may be missing layers whose decoder gave up
        if hints.cancel.load(Ordering::Relaxed) {
            return Err(EngineError::Engine("cancelled".into()));
        }
        Ok(frame)
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

    fn peaks(&self, item: u64) -> Option<Vec<(f32, f32)>> {
        let source = self.pool.provider(self.project.clone(), self.services.clone()).source(ItemId(item))?;
        source.info().has_audio().then(|| media_peaks(&*source))
    }
}

/// The waveform of a media source's first audio stream: (lowest, highest) sample of every
/// [`PEAK_SAMPLES`] samples at [`PEAK_RATE`], decoded four seconds at a time. Ported from
/// FilmCraft's `source_peaks` (`ui-egui/src/panels/timeline.rs`).
fn media_peaks(source: &dyn MediaSource) -> Vec<(f32, f32)> {
    let total = source.info().duration.to_units_floor(i64::from(PEAK_RATE)).max(0) as usize;
    let mut out = Vec::with_capacity(total / PEAK_SAMPLES + 1);
    let chunk = PEAK_RATE as usize * 4;
    let mut at = 0usize;
    while at < total {
        let n = chunk.min(total - at);
        let Ok(buf) = source.audio_stream(0, at as i64, n, PEAK_RATE) else { break };
        let first = buf.channels.first().map(Vec::as_slice).unwrap_or_default();
        out.extend(
            first.chunks(PEAK_SAMPLES).map(|c| c.iter().fold((0f32, 0f32), |(lo, hi), v| (lo.min(*v), hi.max(*v)))),
        );
        at += n;
    }
    out
}

/// The clip open in the source viewer, played like the timeline: frames from FilmCraft's
/// `render_item` (as its Source monitor shows media: SDR Rec. 709) and sound read straight from
/// the media. Frame 0 is the first frame of the clip's range (a subclip's start).
pub struct SourceSnapshot {
    project: Arc<Project>,
    pool: Arc<MediaPool>,
    services: Arc<dyn Services>,
    /// The item whose frames are shown: the clip, or a subclip's media.
    media: ItemId,
    start: Tick,
    rate: FrameRate,
    frames: i64,
    size: (u32, u32),
    revision: u64,
}

impl SourceSnapshot {
    /// The clip open in the source viewer, if there is one.
    pub fn of(session: &Session) -> Option<Self> {
        let item = session.state.source_item?;
        let view = filmcraft_engine::clip_ops::source_view(session, item)?;
        let rate = view.rate;
        let span = Tick(view.end.0 - view.start.0);
        let last = rate.frame_at(span);
        let frames = if rate.tick_of(last) < span { last + 1 } else { last };
        let size = session
            .project
            .item(view.media)
            .and_then(|pi| match &pi.kind {
                ItemKind::Media(m) => m.info.video.as_ref().map(|v| (v.width, v.height)),
                ItemKind::Sequence(q) => Some((q.settings.width, q.settings.height)),
                _ => None,
            })
            .unwrap_or((0, 0));
        Some(Self {
            project: session.project.clone(),
            pool: session.media.clone(),
            services: session.services.clone(),
            media: view.media,
            start: view.start,
            rate,
            frames: frames.max(1),
            size,
            revision: session.revision,
        })
    }
}

impl Playback for SourceSnapshot {
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
        let t = Tick(self.start.0 + self.rate.tick_of(frame.clamp(0, self.frames - 1)).0);
        let provider =
            TimedProvider(self.pool.provider(self.project.clone(), self.services.clone()), hints.decode_ns.clone());
        let catch_up = hints.catch_up.map(Tick::from_seconds_f64);
        let render = || {
            cancel::with_cancel(&hints.cancel, || {
                cancel::with_catch_up(catch_up, || {
                    cancel::with_draft(hints.draft, || render_item(&self.project, self.media, t, scale, &provider))
                })
            })
        };
        let img = in_frame_pool(render);
        if hints.cancel.load(Ordering::Relaxed) {
            return Err(EngineError::Engine("cancelled".into()));
        }
        let img = img.ok_or_else(|| EngineError::Engine("the source clip has no picture".into()))?;
        Ok(Frame::new(img.w as u32, img.h as u32, img.over_black_rgba8()))
    }

    fn mixer(&self, sample_rate: u32) -> Mixer {
        let source = self.pool.provider(self.project.clone(), self.services.clone()).source(self.media);
        let offset = (self.start.0 as f64 * f64::from(sample_rate) / filmcraft_engine::time::TICKS_PER_SECOND as f64)
            .round() as i64;
        Box::new(move |start: i64, buf: &mut [f32], ch: usize| {
            let n = buf.len() / ch.max(1);
            let sound = source
                .as_ref()
                .filter(|s| s.info().has_audio())
                .and_then(|s| s.audio(start + offset, n, sample_rate).ok());
            match sound.as_ref().and_then(|a| a.channels.first().map(|l| (l, a.channels.get(1).unwrap_or(l)))) {
                Some((left, right)) => filmcraft_engine::settings::map_output(left, right, buf, ch, [0, 1]),
                None => buf.fill(0.0),
            }
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
