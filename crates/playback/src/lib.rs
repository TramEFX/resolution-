//! Real-time playback for Resolution's front ends (LOG D-013). Engine-neutral: it plays any
//! [`resolution_engine_api::Playback`].
//!
//! - [`frames`]: pictures rendered ahead of the playhead on worker threads, and cached.
//! - [`ahead`]: sound mixed ahead of the audio device on its own thread.
//! - [`output`]: the audio device, whose played samples are the playback clock.
//! - [`transport`]: play, stop and shuttle; the clock that moves the playhead.
//!
//! Ported from FilmCraft (MIT OR Apache-2.0), which keeps this in its UI crate and desktop app,
//! code Resolution does not use (ARCHITECTURE invariant 2). Each module names its source.

pub mod ahead;
pub mod frames;
pub mod output;
pub mod transport;

pub use ahead::{AudioStats, shuttle_mix_rate};
pub use frames::{FrameKey, FrameServer, FrameStats};
pub use output::AudioOut;
pub use transport::{AudioLost, Step, Transport};
