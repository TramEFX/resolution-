//! The frame server against a fake timeline that logs what it renders and can be slow, fail or
//! panic on chosen frames.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use resolution_engine_api::{EngineError, Frame, Mixer, Playback, Rate, RenderHints, Result};
use resolution_playback::frames::{FrameKey, FrameServer, PREROLL_FRAMES};

#[derive(Default)]
struct Fake {
    delay: Duration,
    log: Mutex<Vec<i64>>,
    fail: Vec<i64>,
    panic: Vec<i64>,
}

impl Playback for Fake {
    fn revision(&self) -> u64 {
        1
    }
    fn rate(&self) -> Rate {
        Rate::new(24, 1)
    }
    fn frames(&self) -> i64 {
        1000
    }
    fn size(&self) -> (u32, u32) {
        (16, 9)
    }
    fn render(&self, frame: i64, max_side: u32, hints: &RenderHints) -> Result<Frame> {
        self.log.lock().expect("log").push(frame);
        let until = Instant::now() + self.delay;
        while Instant::now() < until {
            if hints.cancel.load(Ordering::Relaxed) {
                return Err(EngineError::Engine("cancelled".into()));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        if self.panic.contains(&frame) {
            panic!("decoder bug (deliberate, for this test)");
        }
        if self.fail.contains(&frame) {
            return Err(EngineError::Engine("unreadable".into()));
        }
        // square frames of the size asked for
        let side = max_side as usize;
        Ok(Frame { width: max_side, height: max_side, rgba: vec![frame as u8; side * side * 4] })
    }
    fn mixer(&self, _: u32) -> Mixer {
        Box::new(|_, buf, _| buf.fill(0.0))
    }
}

fn key(frame: i64) -> FrameKey {
    FrameKey { revision: 1, frame, side: 16, draft: false }
}

fn wait(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_requested_frame_renders_once_and_is_cached() {
    let fake = Arc::new(Fake::default());
    let source: Arc<dyn Playback> = fake.clone();
    let server = FrameServer::new(2);
    server.request(&source, key(7));
    wait("frame 7", || server.is_ready(&key(7)));
    assert_eq!(server.get(&key(7)).map(|f| f.rgba[0]), Some(7));
    server.request(&source, key(7));
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(*fake.log.lock().expect("log"), vec![7], "the second request is a cache hit");
    assert_eq!(server.stats().request_hits, 1);
}

#[test]
fn playback_renders_ahead_in_order_and_prerolls() {
    let fake = Arc::new(Fake { delay: Duration::from_millis(2), ..Default::default() });
    let source: Arc<dyn Playback> = fake.clone();
    let server = FrameServer::new(1);
    server.schedule_playback(&source, key(100), 1.0, true);
    wait("the preroll frames", || server.preroll_ready(key(100), 1.0, 999));
    let log = fake.log.lock().expect("log").clone();
    assert_eq!(
        log.get(..PREROLL_FRAMES as usize),
        Some(&[100, 101, 102, 103, 104, 105][..]),
        "due frame first, then in order"
    );
    // reverse play renders backwards
    let server = FrameServer::new(1);
    server.schedule_playback(&source, key(50), -1.0, true);
    wait("reverse preroll", || server.preroll_ready(key(50), -1.0, 999));
    assert!(server.is_ready(&key(45)) && !server.is_ready(&key(51)));
}

#[test]
fn frames_playback_has_passed_are_dropped_or_cancelled() {
    let fake = Arc::new(Fake { delay: Duration::from_millis(60), ..Default::default() });
    let source: Arc<dyn Playback> = fake.clone();
    let server = FrameServer::new(1);
    server.schedule_playback(&source, key(0), 1.0, false);
    std::thread::sleep(Duration::from_millis(10));
    // the playhead jumped ahead: frame 0 (running) and 1..14 (queued) are stale
    server.schedule_playback(&source, key(500), 1.0, false);
    wait("frame 500", || server.is_ready(&key(500)));
    let log = fake.log.lock().expect("log").clone();
    assert_eq!(log.first(), Some(&0));
    assert!(log.iter().all(|f| *f == 0 || *f >= 500), "stale frames rendered: {log:?}");
    assert!(!server.is_ready(&key(0)), "the cancelled frame is not cached");
    wait("the cancelled job to finish", || server.stats().cancelled >= 1);
    server.stop_prefetch();
}

#[test]
fn a_failing_or_panicking_frame_is_reported_once_and_the_workers_carry_on() {
    let fake = Arc::new(Fake { fail: vec![3], panic: vec![4], ..Default::default() });
    let source: Arc<dyn Playback> = fake.clone();
    let server = FrameServer::new(1);
    server.request(&source, key(3));
    wait("frame 3 to fail", || server.error(&key(3)).is_some());
    server.request(&source, key(4));
    wait("frame 4 to panic", || server.error(&key(4)).is_some());
    assert_eq!(server.error(&key(4)).as_deref(), Some("the renderer panicked"));
    server.request(&source, key(5));
    wait("frame 5 after the panic", || server.is_ready(&key(5)));
    server.request(&source, key(3));
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(*fake.log.lock().expect("log"), vec![3, 4, 5], "failed frames are not retried");
    assert_eq!(server.stats().failed, 2);
}

#[test]
fn the_newest_ready_frame_stands_in_for_one_still_rendering() {
    let fake = Arc::new(Fake::default());
    let source: Arc<dyn Playback> = fake.clone();
    let server = FrameServer::new(1);
    server.request(&source, key(10));
    wait("frame 10", || server.is_ready(&key(10)));
    assert_eq!(server.nearest(key(12), 4, 1).map(|(k, _)| k.frame), Some(10));
    assert!(server.nearest(key(12), 1, 1).is_none(), "too far back");
    assert_eq!(server.nearest(key(8), 4, -1).map(|(k, _)| k.frame), Some(10), "reverse looks ahead");
}

#[test]
fn the_cache_stays_within_its_budget_dropping_the_least_recently_used() {
    let source: Arc<dyn Playback> = Arc::new(Fake::default());
    let server = FrameServer::new(1);
    server.set_cache_budget(0); // raised to its 16 MiB floor
    // 1024x1024 frames are 4 MiB: four fit
    let big = |frame| FrameKey { side: 1024, ..key(frame) };
    for f in 0..4 {
        server.request(&source, big(f));
        wait("frame", || server.is_ready(&big(f)));
    }
    assert!(server.get(&big(0)).is_some(), "frame 0 used again: frame 1 is now the oldest");
    server.request(&source, big(4));
    wait("frame 4", || server.is_ready(&big(4)));
    let (bytes, budget) = server.cache_usage();
    assert!(bytes <= budget, "{bytes} cached, budget {budget}");
    assert!(!server.is_ready(&big(1)), "the least recently used frame went");
    assert!([0, 2, 3, 4].iter().all(|f| server.is_ready(&big(*f))));
}
