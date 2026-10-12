//! Play, stop and shuttle: the clock that moves the playhead while playing.
//!
//! Playing starts with a preroll: the clock waits until the first frames are ready, so they are
//! not late the moment it starts. Then the audio device drives the clock: the playhead is where
//! the sound is. Without sound (no device, reverse or very fast shuttle) the wall clock drives it.
//! A device that fails, or stops taking samples, hands the clock to the wall clock from where the
//! sound got to, and playback goes on silent.
//!
//! Pure logic over times the caller passes in, so it is tested without a window or a sound card.
//! Ported from the playback part of FilmCraft's `ui-egui/src/lib.rs` (MIT OR Apache-2.0).

use resolution_engine_api::Rate;

use crate::frames::PREROLL_TIMEOUT_S;

/// How long (s) the audio clock may stand still before the wall clock takes over. A stream can
/// open and then never call back (a busy or misconfigured device); playback must not freeze on
/// it. Devices that are slow to start (Bluetooth) stay well under this.
pub const AUDIO_STALL_S: f64 = 1.0;

/// What to show after [`Transport::advance`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Still waiting for the first frames: show this frame.
    Preroll(i64),
    /// Show this frame.
    Play(i64),
    /// Playback reached the end (or the start, in reverse) and stopped on this frame.
    End(i64),
}

/// Why sound stopped driving the clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioLost {
    /// The stream reported an error.
    Failed,
    /// The device took no samples for [`AUDIO_STALL_S`].
    Stalled,
}

#[derive(Debug)]
struct Run {
    speed: f64,
    /// Timeline seconds when the clock (re)started, and the caller's time then. `started` is
    /// `None` during the preroll.
    from: f64,
    started: Option<f64>,
    preroll_since: Option<f64>,
    /// The audio device drives the clock.
    audio: bool,
    /// The audio clock's last reading and when it last moved.
    audio_seen: (u64, f64),
    /// The frame last shown.
    shown: i64,
}

#[derive(Debug, Default)]
pub struct Transport {
    run: Option<Run>,
}

impl Transport {
    /// Play from `frame` at `speed` (1 normal, 2 double, -1 reverse…), starting with the preroll.
    pub fn play(&mut self, frame: i64, speed: f64, rate: Rate) {
        self.run = Some(Run {
            speed,
            from: rate.seconds_of(frame),
            started: None,
            preroll_since: None,
            audio: false,
            audio_seen: (0, 0.0),
            shown: frame,
        });
    }

    /// Stop; returns the frame playback stopped on.
    pub fn stop(&mut self) -> Option<i64> {
        self.run.take().map(|r| r.shown)
    }

    pub fn is_playing(&self) -> bool {
        self.run.is_some()
    }

    pub fn speed(&self) -> Option<f64> {
        self.run.as_ref().map(|r| r.speed)
    }

    /// The frame on screen while playing.
    pub fn frame(&self) -> Option<i64> {
        self.run.as_ref().map(|r| r.shown)
    }

    pub fn in_preroll(&self) -> bool {
        self.run.as_ref().is_some_and(|r| r.started.is_none())
    }

    /// Whether the preroll is over at `now`: the first frames are `ready`, or the wait timed out.
    pub fn preroll_done(&mut self, now: f64, ready: bool) -> bool {
        let Some(run) = &mut self.run else { return false };
        let since = *run.preroll_since.get_or_insert(now);
        ready || now - since >= PREROLL_TIMEOUT_S
    }

    /// Start the clock at `now`. `audio`: sound started, and the device drives the clock.
    pub fn start_clock(&mut self, now: f64, audio: bool) {
        if let Some(run) = &mut self.run {
            run.started = Some(now);
            run.audio = audio;
            run.audio_seen = (0, now);
        }
    }

    /// Whether the audio device drives the clock.
    pub fn audio_clock(&self) -> bool {
        self.run.as_ref().is_some_and(|r| r.audio)
    }

    /// The frame to show at `now`. `played`: sample frames the device has played since sound
    /// started and its sample rate; `None` when the stream failed. `last`: the timeline's last
    /// frame. Returns `None` when not playing, and why the audio clock was lost if it just was.
    pub fn advance(
        &mut self,
        now: f64,
        played: Option<(u64, u32)>,
        rate: Rate,
        last: i64,
    ) -> Option<(Step, Option<AudioLost>)> {
        let run = self.run.as_mut()?;
        let Some(mut started) = run.started else { return Some((Step::Preroll(run.shown), None)) };
        let mut lost = None;
        if run.audio {
            match played {
                None => {
                    // the stream failed: go on from the frame on screen rather than jumping
                    run.from = rate.seconds_of(run.shown);
                    started = now;
                    run.audio = false;
                    lost = Some(AudioLost::Failed);
                }
                Some((f, _)) if f != run.audio_seen.0 => run.audio_seen = (f, now),
                Some((f, sr)) if now - run.audio_seen.1 >= AUDIO_STALL_S => {
                    // the device stopped taking samples: go on from where the sound got to
                    run.from += if sr > 0 { f as f64 / f64::from(sr) } else { 0.0 } * run.speed;
                    started = now;
                    run.audio = false;
                    lost = Some(AudioLost::Stalled);
                }
                Some(_) => {}
            }
            run.started = Some(started);
        }
        let elapsed = match played {
            Some((f, sr)) if run.audio && sr > 0 => f as f64 / f64::from(sr),
            _ => now - started,
        };
        let t = run.from + elapsed * run.speed;
        let frame = rate.frame_at(t);
        let step = if run.speed > 0.0 && frame > last {
            Step::End(last.max(0))
        } else if run.speed < 0.0 && t <= 0.0 {
            Step::End(0)
        } else {
            run.shown = frame;
            Step::Play(frame)
        };
        if let Step::End(f) = step {
            run.shown = f;
            self.run = None;
        }
        Some((step, lost))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rate = Rate { num: 24, den: 1 };

    fn started(from: i64, speed: f64, audio: bool) -> Transport {
        let mut t = Transport::default();
        t.play(from, speed, R);
        t.start_clock(10.0, audio);
        t
    }

    #[test]
    fn waits_for_the_first_frames_then_runs_on_the_wall_clock() {
        let mut t = Transport::default();
        t.play(48, 1.0, R);
        assert!(t.in_preroll());
        assert_eq!(t.advance(5.0, None, R, 1000), Some((Step::Preroll(48), None)));
        assert!(!t.preroll_done(5.0, false), "not ready, just started waiting");
        assert!(t.preroll_done(5.0 + PREROLL_TIMEOUT_S, false), "gives up waiting");
        assert!(t.preroll_done(5.1, true));
        t.start_clock(10.0, false);
        assert_eq!(t.advance(10.5, None, R, 1000), Some((Step::Play(60), None)));
    }

    #[test]
    fn the_audio_device_drives_the_clock() {
        let mut t = started(0, 1.0, true);
        // the device has played half a second, whatever the wall clock says
        assert_eq!(t.advance(99.0, Some((24_000, 48_000)), R, 1000), Some((Step::Play(12), None)));
        // shuttle at 2x: the device plays at its rate, the timeline moves twice as fast
        let mut t = started(0, 2.0, true);
        assert_eq!(t.advance(99.0, Some((24_000, 48_000)), R, 1000), Some((Step::Play(24), None)));
    }

    #[test]
    fn a_failed_stream_hands_over_without_a_jump() {
        let mut t = started(0, 1.0, true);
        assert_eq!(t.advance(10.5, Some((24_000, 48_000)), R, 1000), Some((Step::Play(12), None)));
        assert_eq!(t.advance(11.0, None, R, 1000), Some((Step::Play(12), Some(AudioLost::Failed))));
        assert!(!t.audio_clock());
        assert_eq!(t.advance(11.5, None, R, 1000), Some((Step::Play(24), None)));
    }

    #[test]
    fn a_stalled_device_hands_over_from_where_the_sound_got_to() {
        let mut t = started(0, 1.0, true);
        assert_eq!(t.advance(10.5, Some((24_000, 48_000)), R, 1000).map(|s| s.0), Some(Step::Play(12)));
        assert_eq!(t.advance(11.4, Some((24_000, 48_000)), R, 1000).map(|s| s.1), Some(None), "not stalled yet");
        assert_eq!(t.advance(11.5, Some((24_000, 48_000)), R, 1000), Some((Step::Play(12), Some(AudioLost::Stalled))));
        assert_eq!(t.advance(12.0, Some((24_000, 48_000)), R, 1000), Some((Step::Play(24), None)));
    }

    #[test]
    fn stops_at_the_last_frame_and_at_the_start() {
        let mut t = started(90, 1.0, false);
        assert_eq!(t.advance(10.5, None, R, 99), Some((Step::End(99), None)));
        assert!(!t.is_playing());
        let mut t = started(12, -1.0, false);
        assert_eq!(t.advance(10.25, None, R, 99), Some((Step::Play(6), None)));
        assert_eq!(t.advance(10.5, None, R, 99), Some((Step::End(0), None)));
        assert_eq!(t.stop(), None, "already stopped");
    }

    #[test]
    fn stop_returns_the_frame_on_screen() {
        let mut t = started(0, 1.0, false);
        t.advance(11.0, None, R, 1000);
        assert_eq!(t.stop(), Some(24));
    }
}
