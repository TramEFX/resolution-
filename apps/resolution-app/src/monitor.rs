//! A viewer's playback. The Edit page has two, as in Resolve: the source viewer plays the clip
//! open in it, the timeline viewer plays the timeline. Each has its own [`Playback`] from the
//! engine's snapshots, its own frame server and frame on screen, and its own transport; they
//! share the audio device ([`Sound`]), and only one plays at a time.

use std::sync::Arc;

use egui::{ColorImage, TextureHandle, TextureOptions};
use resolution_engine_api::Playback;
use resolution_playback::{AudioLost, AudioOut, AudioStats, FrameKey, FrameServer, Step, Transport, ahead};
use serde_json::json;

use crate::worker::Worker;

/// Frames shown while playing: the viewer's frame-rate readout and the `--play` report.
#[derive(Clone, Copy, Debug, Default)]
pub struct Meter {
    /// When the clock started (s, app clock).
    pub started: f64,
    pub shown: u64,
    window: (f64, u32),
    pub fps: Option<f64>,
}

impl Meter {
    fn new(now: f64) -> Self {
        Self { started: now, window: (now, 0), ..Self::default() }
    }

    fn frame_shown(&mut self, now: f64) {
        self.shown += 1;
        self.window.1 += 1;
        let dt = now - self.window.0;
        if dt >= 1.0 {
            self.fps = Some(f64::from(self.window.1) / dt);
            self.window = (now, 0);
        }
    }
}

/// The audio device, shared by the two viewers. Opened at the first play.
pub struct Sound {
    pub out: Option<AudioOut>,
    pub stats: Arc<AudioStats>,
    /// Play without opening a device (tests, machines without one).
    pub mute: bool,
}

impl Sound {
    pub fn new(mute: bool) -> Self {
        Self { out: None, stats: Arc::default(), mute }
    }

    pub fn stop(&mut self) {
        if let Some(a) = &mut self.out {
            a.stop();
        }
    }

    /// Start `p`'s sound from frame `from`. `Ok(true)` when it plays (and then drives the clock);
    /// reverse and shuttle faster than 4x play silent.
    fn start(&mut self, p: &Arc<dyn Playback>, from: i64, speed: f64) -> Result<bool, String> {
        if self.mute || resolution_playback::shuttle_mix_rate(48_000, speed).is_none() {
            return Ok(false);
        }
        let (p, stats) = (p.clone(), self.stats.clone());
        self.out
            .get_or_insert_with(AudioOut::new)
            .start(move |device_rate, channels| {
                let mix_rate = resolution_playback::shuttle_mix_rate(device_rate, speed).unwrap_or(device_rate);
                ahead::spawn(p.mixer(mix_rate), p.rate().sample_of(from, mix_rate), device_rate, channels, stats)
            })
            .map(|_| true)
    }
}

pub struct Monitor {
    /// What this viewer plays, from the latest snapshot.
    pub program: Option<Arc<dyn Playback>>,
    pub frames: FrameServer,
    pub frame: Option<TextureHandle>,
    /// The frame in `frame`.
    pub shown: Option<FrameKey>,
    /// Longest side, in pixels, of the frames asked for: the viewer's size on screen.
    pub view_side: u32,
    pub transport: Transport,
    /// A seek sent to the engine and not in a snapshot yet, shown meanwhile so the playhead does
    /// not jump back while the engine catches up; and how many seeks are in flight.
    pub pending_seek: Option<i64>,
    pub seeks_in_flight: u32,
    pub meter: Meter,
    /// The engine's playhead, from the latest snapshot.
    pub engine_frame: i64,
    /// The command that moves the engine's playhead: `playhead.set` or `source.seek`.
    seek_command: &'static str,
    texture: &'static str,
}

impl Monitor {
    pub fn new(ctx: &egui::Context, seek_command: &'static str, texture: &'static str) -> Self {
        let frames = FrameServer::new(FrameServer::default_workers());
        let ctx = ctx.clone();
        frames.on_ready(move || ctx.request_repaint());
        Self {
            program: None,
            frames,
            frame: None,
            shown: None,
            view_side: 960,
            transport: Transport::default(),
            pending_seek: None,
            seeks_in_flight: 0,
            meter: Meter::default(),
            engine_frame: 0,
            seek_command,
            texture,
        }
    }

    /// The playhead's frame: playback's while playing, a seek on its way to the engine, or the
    /// engine's.
    pub fn playhead_frame(&self) -> i64 {
        self.transport.frame().or(self.pending_seek).unwrap_or(self.engine_frame)
    }

    pub fn last_frame(&self) -> i64 {
        self.program.as_ref().map_or(0, |p| p.frames() - 1).max(0)
    }

    pub fn is_playing(&self) -> bool {
        self.transport.is_playing()
    }

    fn frame_key(&self, p: &Arc<dyn Playback>, frame: i64) -> FrameKey {
        FrameKey { revision: p.revision(), frame, side: self.view_side, draft: false }
    }

    /// A new snapshot: what to play, and where the engine's playhead is.
    pub fn update_program(&mut self, program: Option<Arc<dyn Playback>>, engine_frame: i64, sound: &mut Sound) {
        let had = self.program.is_some();
        self.program = program;
        self.engine_frame = engine_frame;
        if self.seeks_in_flight == 0 {
            self.pending_seek = None;
        }
        if self.program.is_none() {
            if had && self.transport.is_playing() {
                sound.stop();
            }
            self.transport.stop();
            self.frame = None;
            self.shown = None;
        }
    }

    /// The engine has answered one of this viewer's seeks.
    pub fn seek_answered(&mut self) {
        self.seeks_in_flight = self.seeks_in_flight.saturating_sub(1);
    }

    /// Play from the playhead at `speed` (from the start when it is on the last frame). Starts
    /// with the preroll; [`Monitor::advance`] starts the clock and the sound.
    pub fn play(&mut self, speed: f64, sound: &mut Sound) {
        let Some(p) = self.program.clone() else { return };
        let mut from = self.playhead_frame();
        if speed > 0.0 && from >= p.frames() - 1 {
            from = 0;
        }
        sound.stop();
        self.transport.play(from, speed, p.rate());
    }

    /// Stop playback and leave the engine's playhead where it stopped.
    pub fn stop(&mut self, worker: &Worker, sound: &mut Sound) {
        if let Some(frame) = self.transport.stop() {
            sound.stop();
            self.frames.stop_prefetch();
            self.send_seek(worker, frame);
        }
    }

    /// Move the playhead to `frame`, stopping playback.
    pub fn seek(&mut self, worker: &Worker, frame: i64, sound: &mut Sound) {
        self.stop(worker, sound);
        self.send_seek(worker, frame.clamp(0, self.last_frame()));
    }

    /// While playing: move the engine's playhead to the frame on screen, so a mark made now
    /// lands there; playback goes on.
    pub fn sync_engine(&self, worker: &Worker) {
        if let Some(frame) = self.transport.frame() {
            worker.run(self.seek_command, json!({ "frame": frame }));
        }
    }

    pub fn send_seek(&mut self, worker: &Worker, frame: i64) {
        if self.playhead_frame() == frame {
            return;
        }
        self.pending_seek = Some(frame);
        self.seeks_in_flight += 1;
        worker.run(self.seek_command, json!({ "frame": frame }));
    }

    /// Playing: wait for the first frames, then start the sound and the clock; afterwards move
    /// the playhead with the clock and keep the frames ahead of it rendering. Returns a message
    /// for the status bar when the sound fails.
    pub fn advance(
        &mut self,
        ctx: &egui::Context,
        worker: &Worker,
        now: impl Fn() -> f64,
        sound: &mut Sound,
    ) -> Option<String> {
        if !self.transport.is_playing() {
            return None;
        }
        let p = self.program.clone()?;
        ctx.request_repaint();
        let speed = self.transport.speed().unwrap_or(1.0);
        let last = p.frames() - 1;
        if self.transport.in_preroll() {
            let key = self.frame_key(&p, self.transport.frame().unwrap_or(0));
            self.frames.schedule_playback(&p, key, speed, true);
            if self.transport.preroll_done(now(), self.frames.preroll_ready(key, speed, last)) {
                let (audio, problem) = match sound.start(&p, key.frame, speed) {
                    Ok(on) => (on, None),
                    Err(e) => (false, Some(format!("No sound ({e}): playing without it."))),
                };
                let t = now();
                self.transport.start_clock(t, audio);
                self.meter = Meter::new(t);
                return problem;
            }
            return None;
        }
        let played = sound
            .out
            .as_ref()
            .filter(|_| self.transport.audio_clock())
            .and_then(|a| a.played_frames().map(|f| (f, a.sample_rate())));
        let (step, lost) = self.transport.advance(now(), played, p.rate(), last)?;
        let problem = lost.map(|lost| {
            sound.stop();
            let why = match lost {
                AudioLost::Failed => "Audio output failed",
                AudioLost::Stalled => "Audio output is not responding",
            };
            format!("{why}: playing without sound.")
        });
        match step {
            Step::Play(f) => self.frames.schedule_playback(&p, self.frame_key(&p, f), speed, false),
            Step::End(f) => {
                sound.stop();
                self.frames.stop_prefetch();
                self.send_seek(worker, f);
            }
            Step::Preroll(_) => {}
        }
        problem
    }

    /// Put the frame at the playhead on screen: the exact frame when it is rendered; until then
    /// the one already there (while playing, the newest rendered frame before the playhead).
    /// Returns a rendering error to report, when stopped.
    pub fn update(&mut self, ctx: &egui::Context, now: f64) -> Option<String> {
        let p = self.program.clone()?;
        let frame = self.playhead_frame().clamp(0, (p.frames() - 1).max(0));
        let key = self.frame_key(&p, frame);
        let found = if self.transport.is_playing() {
            let dir = if self.transport.speed().unwrap_or(1.0) < 0.0 { -1 } else { 1 };
            self.frames.nearest(key, 48, dir)
        } else {
            self.frames.request(&p, key);
            self.frames.get(&key).map(|f| (key, f))
        };
        match found {
            Some((k, f)) if self.shown != Some(k) => {
                let image = ColorImage::from_rgba_unmultiplied([f.width as usize, f.height as usize], &f.rgba);
                match &mut self.frame {
                    Some(t) => t.set(image, TextureOptions::LINEAR),
                    None => self.frame = Some(ctx.load_texture(self.texture, image, TextureOptions::LINEAR)),
                }
                self.shown = Some(k);
                if self.transport.is_playing() && !self.transport.in_preroll() {
                    self.meter.frame_shown(now);
                }
                None
            }
            None if !self.transport.is_playing() => self.frames.error(&key),
            _ => None,
        }
    }
}
