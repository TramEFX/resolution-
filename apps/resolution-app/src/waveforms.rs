//! Waveforms for the timeline's audio clips. Reading one means decoding a clip's whole sound
//! (`Playback::peaks`), so one background thread works through the clips the timeline holds and
//! keeps their peaks here; the timeline draws whatever has arrived.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use resolution_engine_api::Playback;

/// A media item's waveform (see `Playback::peaks`), and its loudest sample, for showing quiet
/// recordings larger.
pub struct Peaks {
    pub buckets: Vec<(f32, f32)>,
    pub loudest: f32,
}

impl Peaks {
    fn new(buckets: Vec<(f32, f32)>) -> Self {
        let loudest = buckets.iter().fold(0f32, |m, (lo, hi)| m.max(lo.abs()).max(hi.abs()));
        Self { buckets, loudest }
    }
}

pub type Ready = HashMap<u64, Arc<Peaks>>;

pub struct Waveforms {
    ready: Arc<Mutex<Ready>>,
    asked: HashSet<u64>,
    jobs: Option<Sender<(u64, Arc<dyn Playback>)>>,
}

impl Waveforms {
    /// Start the reading thread; `repaint` wakes the UI when a waveform arrives.
    pub fn new(repaint: impl Fn() + Send + 'static) -> Self {
        let ready: Arc<Mutex<Ready>> = Arc::default();
        let (tx, rx) = mpsc::channel::<(u64, Arc<dyn Playback>)>();
        let results = ready.clone();
        let thread = std::thread::Builder::new().name("waveforms".into()).spawn(move || {
            for (item, playback) in rx {
                let peaks = Peaks::new(playback.peaks(item).unwrap_or_default());
                results.lock().unwrap_or_else(|e| e.into_inner()).insert(item, Arc::new(peaks));
                repaint();
            }
        });
        Self { ready, asked: HashSet::new(), jobs: thread.ok().map(|_| tx) }
    }

    /// Read the waveforms of `items` from `playback`, each item once.
    pub fn want(&mut self, items: impl IntoIterator<Item = u64>, playback: &Arc<dyn Playback>) {
        let Some(jobs) = &self.jobs else { return };
        for item in items {
            if self.asked.insert(item) {
                let _ = jobs.send((item, playback.clone()));
            }
        }
    }

    /// The waveforms read so far.
    pub fn ready(&self) -> Ready {
        self.ready.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// How tall a waveform column of loudness `m` (0..1) draws, as a share of its half height: a
/// decibel scale from -48 dB, with quiet recordings raised by up to 12 dB, as FilmCraft's
/// timeline shows them (`waveform_display_gain`, `ui-egui/src/panels/timeline.rs`).
pub fn column_height(m: f32, loudest: f32) -> f32 {
    const MAX_BOOST: f32 = 3.981_072; // +12 dB
    let gain = (1.0 / loudest.max(1e-9)).min(MAX_BOOST);
    let db = 20.0 * (m * gain).max(1e-5).log10();
    ((db + 48.0) / 48.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_follow_a_decibel_scale() {
        // full scale reaches the top; -48 dB and quieter draw nothing
        assert_eq!(column_height(1.0, 1.0), 1.0);
        assert_eq!(column_height(0.003, 1.0), 0.0);
        // -24 dB, halfway
        assert!((column_height(0.063_095_73, 1.0) - 0.5).abs() < 1e-3);
        // a quiet recording peaking at -12 dB is raised to full scale, but no further
        assert!((column_height(0.251_188_6, 0.251_188_6) - 1.0).abs() < 1e-3);
        assert!(column_height(0.1, 0.1) < 1.0);
    }
}
