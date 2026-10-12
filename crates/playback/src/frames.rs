//! Frames rendered ahead of the playhead on a small pool of worker threads.
//!
//! Jobs are prioritised: the frame on screen first, then the frames playback needs next, in
//! order. Results land in a byte-budgeted cache keyed by [`FrameKey`]. The viewer shows the exact
//! frame when it is ready and otherwise holds the one it has, so scrubbing never flashes black and
//! never blocks the UI. Jobs for frames playback has passed are dropped, and running ones
//! cancelled: their decoders give up instead of finishing a picture nobody will see.
//!
//! When frames take longer to render than playback allows (effects, compositing), frames that
//! could not be ready in time are not started, and the workers get evenly spaced frames they can
//! finish ([`playback_plan`]). Otherwise every frame would be late, cancelled when the next one
//! falls due, and almost nothing would reach the screen (LOG B-012).
//!
//! Ported from FilmCraft's `ui-egui/src/frames.rs` (MIT OR Apache-2.0), without what Resolution
//! has no use for yet: GPU frame plans, thumbnails, render previews and profiling. One deliberate
//! difference: FilmCraft leaves decoding out of the render cost, because skipping frames does not
//! save decoding. Measured here, that made playback worse: generated media (FilmCraft's demo)
//! counts as decoding, so its cost was underestimated and the frames were late again (4.5 fps
//! against 9.7 with the whole job counted), and footage too slow to decode in real time played
//! no better either way. Decoding time is still measured ([`FrameStats::decode_ms`]).

use std::collections::{HashMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Instant;

use resolution_engine_api::{Frame, Playback, RenderHints};

/// One rendered picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameKey {
    /// The project revision ([`Playback::revision`]).
    pub revision: u64,
    pub frame: i64,
    /// Longest side in pixels.
    pub side: u32,
    /// Decoded in draft mode ([`RenderHints::draft`]). Draft frames have keys of their own, so a
    /// paused viewer never shows one.
    pub draft: bool,
}

/// Frames queued ahead of the playhead while playing.
pub fn prefetch_depth(speed: f64) -> i64 {
    if speed.abs() > 1.5 { 24 } else { 14 }
}

/// Playback starts its clock once this many frames from the playhead are ready, or after
/// [`PREROLL_TIMEOUT_S`]: otherwise the first frames after Play are due before any decoder had a
/// chance to produce them (a seek decodes from the previous keyframe).
pub const PREROLL_FRAMES: i64 = 6;
pub const PREROLL_TIMEOUT_S: f64 = 0.5;

/// Seconds before a scrubbed-to frame whose pictures a seek still decodes in full; earlier
/// non-reference pictures are skipped on the way.
pub const SCRUB_KEEP_S: f64 = 2.0;

/// Default cache budget (bytes).
pub const CACHE_BYTES: usize = 768 << 20;

/// Which frames playback asks for, given the per-frame render cost (s), the displayed frame rate
/// and the worker count: `(lead, stride)`. While the workers keep up, every frame from the
/// playhead on (`(1, 1)`). When they cannot, `lead` is the first frame ahead of the playhead worth
/// starting (nearer ones would be late however soon they start) and `stride` the spacing the
/// workers can finish in real time: the frames in between are dropped on purpose and evenly,
/// instead of every frame arriving late.
pub fn playback_plan(render_cost: f64, fps: f64, workers: usize) -> (i64, i64) {
    let frames_per_job = render_cost * fps;
    let load = frames_per_job * 1.15 / workers.max(1) as f64;
    if load <= 1.0 {
        return (1, 1);
    }
    (frames_per_job.ceil() as i64, load.ceil() as i64)
}

/// Cumulative counters, cheap enough to keep on all the time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameStats {
    /// Jobs finished, cancelled ones included.
    pub jobs: u64,
    /// Jobs whose render failed or panicked (the worker survives; nothing is cached).
    pub failed: u64,
    pub cancelled: u64,
    /// Playback prefetch jobs.
    pub prefetch: u64,
    /// Wall time of all jobs (ms).
    pub job_ms: f64,
    /// The part of it spent getting source pictures: decoding, or waiting for a decoder (ms).
    pub decode_ms: f64,
    /// Requests answered by a cached frame or one already queued or running.
    pub request_hits: u64,
    /// Requests that queued a new job.
    pub request_misses: u64,
}

struct Job {
    key: FrameKey,
    source: Arc<dyn Playback>,
    prio: u32,
    /// Queued by playback ahead of the playhead (dropped when playback stops).
    prefetch: bool,
    /// Scheduled by playback: its time feeds the render cost estimate.
    playback: bool,
    catch_up: Option<f64>,
    cancel: Arc<AtomicBool>,
}

/// Finished frames, bounded by bytes (least recently used evicted first).
struct Cache {
    map: HashMap<FrameKey, (Arc<Frame>, u64)>,
    bytes: usize,
    budget: usize,
    clock: u64,
}

impl Cache {
    fn get(&mut self, k: &FrameKey) -> Option<Arc<Frame>> {
        self.clock += 1;
        let clock = self.clock;
        self.map.get_mut(k).map(|(f, used)| {
            *used = clock;
            f.clone()
        })
    }

    fn insert(&mut self, k: FrameKey, f: Arc<Frame>) {
        self.clock += 1;
        self.bytes += f.rgba.len();
        if let Some((old, _)) = self.map.insert(k, (f, self.clock)) {
            self.bytes -= old.rgba.len();
        }
        while self.bytes > self.budget && self.map.len() > 1 {
            let Some(oldest) =
                self.map.iter().filter(|(key, _)| **key != k).min_by_key(|(_, (_, used))| *used).map(|(key, _)| *key)
            else {
                break;
            };
            if let Some((f, _)) = self.map.remove(&oldest) {
                self.bytes -= f.rgba.len();
            }
        }
    }
}

type Notify = Box<dyn Fn() + Send + Sync>;

struct Shared {
    queue: Mutex<VecDeque<Job>>,
    cv: Condvar,
    cache: Mutex<Cache>,
    /// Frames that failed to render, with why; not retried until the key changes.
    errors: Mutex<HashMap<FrameKey, String>>,
    /// Running jobs: key, cancel flag, prefetch.
    in_flight: Mutex<Vec<(FrameKey, Arc<AtomicBool>, bool)>>,
    /// Running estimate (s) of a playback frame's render time: frames cannot be ready sooner than
    /// this after they start, and the workers finish at most `workers / cost` a second.
    render_cost: Mutex<f64>,
    workers: usize,
    stats: Mutex<FrameStats>,
    notify: Mutex<Option<Notify>>,
    quit: AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct FrameServer {
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<()>>,
}

impl FrameServer {
    pub fn new(workers: usize) -> Self {
        let shared = Arc::new(Shared {
            queue: Mutex::new(VecDeque::new()),
            cv: Condvar::new(),
            cache: Mutex::new(Cache { map: HashMap::new(), bytes: 0, budget: CACHE_BYTES, clock: 0 }),
            errors: Mutex::new(HashMap::new()),
            in_flight: Mutex::new(Vec::new()),
            render_cost: Mutex::new(0.0),
            workers: workers.max(1),
            stats: Mutex::new(FrameStats::default()),
            notify: Mutex::new(None),
            quit: AtomicBool::new(false),
        });
        let threads = (0..workers.max(1))
            .filter_map(|i| {
                let sh = shared.clone();
                std::thread::Builder::new().name(format!("frames-{i}")).spawn(move || worker(&sh)).ok()
            })
            .collect();
        Self { shared, threads }
    }

    /// The number of frame workers: two. FilmCraft's renderer already spreads one frame over every
    /// core, so more workers only compete for them: on a 16-thread machine 1 to 3 workers played
    /// every measured clip as well as 6 (LOG D-013; FilmCraft uses 2 to 6).
    pub fn default_workers() -> usize {
        2
    }

    /// Called from a worker thread whenever a job finishes (to wake the UI).
    pub fn on_ready(&self, f: impl Fn() + Send + Sync + 'static) {
        *lock(&self.shared.notify) = Some(Box::new(f));
    }

    /// Cache budget (bytes, at least 16 MiB); evicts down to it on the next insert.
    pub fn set_cache_budget(&self, bytes: usize) {
        lock(&self.shared.cache).budget = bytes.max(16 << 20);
    }

    /// (bytes cached, budget).
    pub fn cache_usage(&self) -> (usize, usize) {
        let c = lock(&self.shared.cache);
        (c.bytes, c.budget)
    }

    pub fn get(&self, key: &FrameKey) -> Option<Arc<Frame>> {
        lock(&self.shared.cache).get(key)
    }

    pub fn is_ready(&self, key: &FrameKey) -> bool {
        lock(&self.shared.cache).map.contains_key(key)
    }

    /// Why `key` failed to render, if it did.
    pub fn error(&self, key: &FrameKey) -> Option<String> {
        lock(&self.shared.errors).get(key).cloned()
    }

    /// The newest ready frame at or before `key.frame` in the direction of play (`dir` 1 forward,
    /// -1 reverse), at most `max_back` frames away.
    pub fn nearest(&self, key: FrameKey, max_back: i64, dir: i64) -> Option<(FrameKey, Arc<Frame>)> {
        let mut cache = lock(&self.shared.cache);
        (0..=max_back).find_map(|d| {
            let k = FrameKey { frame: key.frame - d * dir.signum(), ..key };
            cache.get(&k).map(|f| (k, f))
        })
    }

    /// The frame on screen while paused or scrubbing: rendered before anything else, replacing
    /// the one asked for before.
    pub fn request(&self, source: &Arc<dyn Playback>, key: FrameKey) {
        self.request_job(source, key, 0, (false, false), Some(SCRUB_KEEP_S));
    }

    /// `(prefetch, playback)`: see [`Job`].
    fn request_job(
        &self,
        source: &Arc<dyn Playback>,
        key: FrameKey,
        prio: u32,
        (prefetch, playback): (bool, bool),
        catch_up: Option<f64>,
    ) {
        let sh = &self.shared;
        let hit = || lock(&sh.stats).request_hits += 1;
        if lock(&sh.cache).map.contains_key(&key) || lock(&sh.errors).contains_key(&key) {
            hit();
            return;
        }
        if lock(&sh.in_flight).iter().any(|(k, c, _)| *k == key && !c.load(Ordering::Relaxed)) {
            hit();
            return;
        }
        let mut q = lock(&sh.queue);
        if let Some(j) = q.iter_mut().find(|j| j.key == key) {
            j.prio = j.prio.min(prio);
            j.prefetch &= prefetch;
            j.playback |= playback;
            // the playhead only moves on: the newest (smallest) margin
            j.catch_up = match (j.catch_up, catch_up) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            drop(q);
            hit();
            return;
        }
        lock(&sh.stats).request_misses += 1;
        if prio == 0 && !prefetch {
            // A new frame on screen replaces the one asked for before: scrubbing asks for one per
            // repaint, and without this the oldest position would decode first while the newest
            // waits. A running job for the same frame at an older revision is left to finish: it
            // needs the same source pictures.
            q.retain(|j| !(j.prio == 0 && !j.prefetch && j.key.side == key.side));
            for (k, c, prefetch) in lock(&sh.in_flight).iter() {
                if !prefetch && k.side == key.side && k.frame != key.frame {
                    c.store(true, Ordering::Relaxed);
                }
            }
        }
        q.push_back(Job { key, source: source.clone(), prio, prefetch, playback, catch_up, cancel: Arc::default() });
        drop(q);
        sh.cv.notify_one();
    }

    /// Playback scheduling for the frame due now (`key`): request it first, then the next frames
    /// in the direction of play, and drop jobs that are stale (another revision, behind the
    /// playhead or too far ahead).
    ///
    /// When frames cost more to render than the workers can keep up with, frames that could not
    /// be ready in time are not started and the workers get evenly spaced frames they can finish
    /// ([`playback_plan`]). While `preroll` (the clock has not started) every frame is wanted.
    pub fn schedule_playback(&self, source: &Arc<dyn Playback>, key: FrameKey, speed: f64, preroll: bool) {
        let dir = if speed < 0.0 { -1 } else { 1 };
        let step = speed.abs().max(1.0) as i64;
        let rate = source.rate();
        let last = source.frames() - 1;
        let forward = !preroll && dir > 0;
        let fps = rate.fps() * speed.abs().max(1.0) / step as f64;
        let (lead, stride) = if preroll { (1, 1) } else { playback_plan(self.render_cost(), fps, self.shared.workers) };
        if lead <= 1 {
            // the frame due now (replacing the one due before, which is late); once the clock
            // runs, the pictures before it are late too
            self.request_job(source, key, 0, (false, true), forward.then_some(0.0));
        }
        let ahead = prefetch_depth(speed);
        for i in 0..ahead {
            let f = key.frame + (lead + i * stride) * dir * step;
            if (0..=last).contains(&f) {
                let late = forward.then(|| rate.seconds_of(f) - rate.seconds_of(key.frame));
                self.request_job(source, FrameKey { frame: f, ..key }, i as u32 + 1, (true, true), late);
            }
        }
        let (rev, cur) = (key.revision, key.frame);
        let span = (lead + ahead * stride) * step + 26;
        let fresh = |k: &FrameKey| k.revision == rev && (k.frame - cur) * dir >= 0 && (k.frame - cur).abs() < span;
        // Queued frames nearer than the lead cannot be ready in time: dropped, or the workers
        // (taking the nearest first) would only ever start frames that are late before they
        // finish (LOG B-012). Running ones finish unless the playhead has passed them.
        let min_ahead = if lead > 1 { lead * step } else { 0 };
        lock(&self.shared.queue).retain(|j| fresh(&j.key) && (j.key.frame - cur) * dir >= min_ahead);
        for (k, c, _) in lock(&self.shared.in_flight).iter() {
            if !fresh(k) {
                c.store(true, Ordering::Relaxed);
            }
        }
    }

    /// The current estimate of a playback frame's render time (s); see [`playback_plan`].
    pub fn render_cost(&self) -> f64 {
        *lock(&self.shared.render_cost)
    }

    /// Whether playback can start at `key`: the next [`PREROLL_FRAMES`] frames (up to `last`)
    /// are ready, or failed (waiting longer would not help).
    pub fn preroll_ready(&self, key: FrameKey, speed: f64, last: i64) -> bool {
        let step = (speed.abs().max(1.0) as i64) * if speed < 0.0 { -1 } else { 1 };
        let errors = lock(&self.shared.errors);
        let cache = lock(&self.shared.cache);
        (0..PREROLL_FRAMES)
            .map(|i| FrameKey { frame: key.frame + i * step, ..key })
            .filter(|k| (0..=last).contains(&k.frame))
            .all(|k| cache.map.contains_key(&k) || errors.contains_key(&k))
    }

    /// Drop queued jobs that fail `keep`, and cancel running ones: a worker blocked behind a
    /// decoder would otherwise go on to decode a frame playback has passed.
    pub fn retain_queue(&self, keep: impl Fn(&FrameKey) -> bool) {
        lock(&self.shared.queue).retain(|j| keep(&j.key));
        for (k, c, _) in lock(&self.shared.in_flight).iter() {
            if !keep(k) {
                c.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Playback stopped: drop its queued jobs and cancel the running ones.
    pub fn stop_prefetch(&self) {
        lock(&self.shared.queue).retain(|j| !j.prefetch);
        for (_, c, prefetch) in lock(&self.shared.in_flight).iter() {
            if *prefetch {
                c.store(true, Ordering::Relaxed);
            }
        }
    }

    pub fn stats(&self) -> FrameStats {
        lock(&self.shared.stats).clone()
    }

    pub fn workers(&self) -> usize {
        self.threads.len()
    }
}

impl Drop for FrameServer {
    fn drop(&mut self) {
        self.shared.quit.store(true, Ordering::Release);
        lock(&self.shared.queue).clear();
        for (_, c, _) in lock(&self.shared.in_flight).iter() {
            c.store(true, Ordering::Relaxed);
        }
        self.shared.cv.notify_all();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

fn worker(sh: &Shared) {
    loop {
        let job = {
            let mut q = lock(&sh.queue);
            loop {
                if sh.quit.load(Ordering::Acquire) {
                    return;
                }
                // highest priority (lowest number) first; first in, first out among equals
                let best = q.iter().enumerate().min_by_key(|(i, j)| (j.prio, *i)).map(|(i, _)| i);
                if let Some(job) = best.and_then(|i| q.remove(i)) {
                    break job;
                }
                q = sh.cv.wait(q).unwrap_or_else(PoisonError::into_inner);
            }
        };
        lock(&sh.in_flight).push((job.key, job.cancel.clone(), job.prefetch));
        let started = Instant::now();
        let hints = RenderHints {
            cancel: job.cancel.clone(),
            catch_up: job.catch_up,
            draft: job.key.draft,
            ..Default::default()
        };
        // A panic while decoding one frame (bad media, a decoder bug) must not take the worker
        // down: a dead worker leaves its frame "in flight" forever and the viewer blank.
        let result = catch_unwind(AssertUnwindSafe(|| job.source.render(job.key.frame, job.key.side, &hints)));
        lock(&sh.in_flight).retain(|(k, c, _)| !(*k == job.key && Arc::ptr_eq(c, &job.cancel)));
        // a cancelled job may have given up halfway: neither a frame nor an error
        let cancelled = job.cancel.load(Ordering::Relaxed);
        let failed = !cancelled
            && match result {
                Ok(Ok(frame)) => {
                    lock(&sh.cache).insert(job.key, Arc::new(frame));
                    false
                }
                Ok(Err(e)) => remember_error(sh, job.key, e.to_string()),
                Err(_) => remember_error(sh, job.key, "the renderer panicked".into()),
            };
        let decode = hints.decode_ns.load(Ordering::Relaxed) as f64 / 1e9;
        if job.playback {
            // The whole job, decoding included (see the module docs). A cancelled job only tells
            // that the work takes at least this long.
            let cost = started.elapsed().as_secs_f64();
            let mut c = lock(&sh.render_cost);
            if !cancelled || cost > *c {
                *c = if *c == 0.0 { cost } else { *c * 0.8 + cost * 0.2 };
            }
        }
        {
            let mut st = lock(&sh.stats);
            st.jobs += 1;
            st.failed += u64::from(failed);
            st.cancelled += u64::from(cancelled);
            st.prefetch += u64::from(job.prefetch);
            st.job_ms += started.elapsed().as_secs_f64() * 1e3;
            st.decode_ms += decode * 1e3;
        }
        if let Some(f) = lock(&sh.notify).as_ref() {
            f();
        }
    }
}

fn remember_error(sh: &Shared, key: FrameKey, e: String) -> bool {
    let mut errors = lock(&sh.errors);
    if errors.len() >= 256 {
        errors.clear();
    }
    errors.insert(key, e);
    true
}

#[cfg(test)]
mod tests {
    use super::playback_plan;

    #[test]
    fn playback_plan_skips_frames_only_when_workers_cannot_keep_up() {
        // 10 ms a frame at 24 fps on 2 workers: every frame
        assert_eq!(playback_plan(0.010, 24.0, 2), (1, 1));
        assert_eq!(playback_plan(0.0, 24.0, 2), (1, 1), "no estimate yet");
        // 70 ms a frame, 1 worker: start 2 frames ahead, every 2nd frame
        assert_eq!(playback_plan(0.070, 24.0, 1), (2, 2));
        // 415 ms a frame on 6 workers: 10 frames ahead, every 2nd
        assert_eq!(playback_plan(0.415, 24.0, 6), (10, 2));
    }
}
