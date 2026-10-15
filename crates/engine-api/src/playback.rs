//! Playback's view of the engine: the open timeline frozen at one revision, rendered and mixed on
//! any thread while the engine thread goes on editing (LOG D-013).
//!
//! Real-time playback needs many frames decoded ahead of the playhead and sound mixed ahead of
//! the audio device, in parallel, which a single engine thread cannot do. So the engine hands out
//! a [`Playback`]: a cheap, shareable snapshot that front ends render and mix from. It changes
//! nothing; edits still go through [`crate::Engine::execute`].

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};

use crate::{Frame, Result};

/// A frame rate as an exact fraction: 24000/1001 for 23.976.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    pub num: i64,
    pub den: i64,
}

impl Rate {
    pub fn new(num: i64, den: i64) -> Self {
        Self { num: num.max(1), den: den.max(1) }
    }

    pub fn fps(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    /// Seconds at the start of `frame`.
    pub fn seconds_of(self, frame: i64) -> f64 {
        frame as f64 * self.den as f64 / self.num as f64
    }

    /// The frame showing `seconds` after the start. Clock readings land exactly on frame starts
    /// often enough that rounding error must not move them back a frame, so a reading within a
    /// microsecond of the next frame counts as that frame.
    pub fn frame_at(self, seconds: f64) -> i64 {
        (seconds * self.num as f64 / self.den as f64 + 1e-6).floor() as i64
    }

    /// The frame showing at `ticks / ticks_per_second` seconds, exactly.
    pub fn frame_at_ticks(self, ticks: i64, ticks_per_second: i64) -> i64 {
        let n = i128::from(ticks) * i128::from(self.num);
        let d = i128::from(ticks_per_second) * i128::from(self.den);
        n.div_euclid(d.max(1)) as i64
    }

    /// The first audio sample of `frame` at `sample_rate`, exactly.
    pub fn sample_of(self, frame: i64, sample_rate: u32) -> i64 {
        let n = i128::from(frame) * i128::from(self.den) * i128::from(sample_rate);
        n.div_euclid(i128::from(self.num)) as i64
    }
}

/// How a frame is wanted, passed to the decoders.
#[derive(Clone, Debug, Default)]
pub struct RenderHints {
    /// Set when the frame is no longer wanted (playback moved on). Decoding gives up early and
    /// [`Playback::render`] returns an error instead of a partial frame.
    pub cancel: Arc<AtomicBool>,
    /// Pictures more than this many seconds before the frame are not needed: a decoder catching
    /// up may skip them. `Some(0.0)` while playing forward; `None` decodes everything.
    pub catch_up: Option<f64>,
    /// Approximate decoding of non-reference pictures is acceptable (playing at reduced size).
    pub draft: bool,
    /// Filled in by the renderer: nanoseconds spent getting source pictures (decoding, waiting
    /// for a decoder). Tells footage too slow to decode apart from effects too slow to render.
    pub decode_ns: Arc<AtomicU64>,
    /// The caller shows frames on the renderer's GPU device: a frame drawn there may be left
    /// there ([`crate::Frame::gpu`]) instead of read back (LOG D-039).
    pub gpu: bool,
    /// The Highlight view of one node of a clip's grade, instead of the program picture.
    pub highlight: Option<Highlight>,
    /// This clip of the timeline's video on its own, graded, without the clips over or under it
    /// or its transitions: its thumbnails' picture (LOG D-077). The clip, by its id in
    /// `timeline.inspect`.
    pub solo: Option<u64>,
}

/// The Highlight view (LOG D-048), as the Color page's viewer shows it: the picture where one
/// node of a clip's grade keys it, flat grey elsewhere; or, `black_white`, the node's matte in
/// black and white. Other clips are drawn as they are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Highlight {
    /// The clip, by its id in `color.inspect`.
    pub clip: u64,
    /// The node, its index in the grade from 0.
    pub node: usize,
    pub black_white: bool,
    /// Highlight Difference instead (LOG D-070): how far the node moves each pixel from its
    /// input, as a grey matte.
    pub difference: bool,
}

/// Sound from the timeline: `mix(start, buffer, channels)` writes interleaved frames into
/// `buffer`, beginning at timeline sample `start` at the mixer's sample rate.
pub type Mixer = Box<dyn FnMut(i64, &mut [f32], usize) + Send>;

/// Samples (at [`PEAK_RATE`]) summarised by one waveform peak.
pub const PEAK_SAMPLES: usize = 256;
/// The sample rate waveform peaks are measured at.
pub const PEAK_RATE: u32 = 48_000;

/// The open timeline, frozen at one project revision, for playback. Thread-safe: frames render
/// in parallel on worker threads.
pub trait Playback: Send + Sync {
    /// The project revision this snapshot shows.
    fn revision(&self) -> u64;
    fn rate(&self) -> Rate;
    /// The timeline's length in frames.
    fn frames(&self) -> i64;
    /// The timeline's frame size in pixels.
    fn size(&self) -> (u32, u32);
    /// The program picture at `frame`, opaque, its longest side at most `max_side` pixels.
    fn render(&self, frame: i64, max_side: u32, hints: &RenderHints) -> Result<Frame>;
    /// The program sound at `sample_rate`. Unlike pictures, sound follows the engine's newest
    /// project: edits made while playing are heard without restarting playback.
    fn mixer(&self, sample_rate: u32) -> Mixer;

    /// The waveform of media item `item`'s sound (its first audio stream), for drawing clips:
    /// the lowest and highest sample of every [`PEAK_SAMPLES`] samples at [`PEAK_RATE`], from
    /// the start of the media. Slow, since it decodes the whole stream: call it off the UI
    /// thread. `None` when the item has no sound or the engine cannot read it.
    fn peaks(&self, item: u64) -> Option<Vec<(f32, f32)>> {
        let _ = item;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICKS: i64 = 254_016_000_000;

    #[test]
    fn frame_arithmetic_is_exact_at_ntsc_rates() {
        let r = Rate::new(24_000, 1001);
        for f in [0, 1, 3, 1000, 86_399] {
            // the start of frame f, in ticks and in seconds, is frame f again
            let ticks = f * (TICKS / 24_000 * 1001);
            assert_eq!(r.frame_at_ticks(ticks, TICKS), f);
            assert_eq!(r.frame_at_ticks(ticks - 1, TICKS), f - 1);
            assert_eq!(r.frame_at(r.seconds_of(f)), f, "frame {f}");
        }
        // 24 frames of 23.976 last 1.001 s: 48 048 samples at 48 kHz
        assert_eq!(r.sample_of(24, 48_000), 48_048);
        assert_eq!(r.sample_of(1, 48_000), 2002);
        assert_eq!(Rate::new(30, 1).sample_of(-1, 48_000), -1600);
    }
}
