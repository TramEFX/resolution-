//! Plays a media file through the playback pipeline in real time, headless: frames rendered ahead
//! on the frame workers, shown on a simulated 60 Hz display, and the sound mixed ahead for a
//! simulated 48 kHz device. Reports how often the display had the exact frame and whether the
//! sound kept up. The numbers behind LOG D-013 and D-014.
//!
//! ```text
//! cargo run -p resolution-playback --example play_bench -- <media file | project.fcproj | --demo>
//!     [--from 0] [--seconds 10] [--side 1440] [--speed 1] [--workers n] [--hardware]
//!     [--cpu-render] [--sequential] [--wav heard.wav]
//! ```
//!
//! A project plays its open timeline (graded, say). `--cpu-render` draws every frame on
//! FilmCraft's CPU renderer, not Resolution's GPU renderer (LOG D-036). `--wav` writes the sound
//! the simulated device received, to compare with another decoder's.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use resolution_engine_api::{Engine, Playback};
use resolution_filmcraft_adapter::{FilmCraftEngine, frame_counts, use_gpu_renderer, use_hardware_decoders};
use resolution_playback::ahead::{self, AudioStats};
use resolution_playback::frames::{FrameKey, FrameServer};
use resolution_playback::{Step, Transport};
use serde_json::{Value, json};

const DISPLAY_HZ: f64 = 60.0;
const DEVICE_RATE: u32 = 48_000;
const DEVICE_PERIOD: usize = 480;

fn first_clip(node: &Value) -> Option<u64> {
    node["children"].as_array().into_iter().flatten().find_map(|c| {
        if c.get("children").is_some() {
            first_clip(c)
        } else if c["type"].as_str() != Some("Sequence") {
            c["item"].as_u64()
        } else {
            None
        }
    })
}

/// Prints the engine's log at info and above, once per message (decoders say why they declined).
struct Log(std::sync::Mutex<std::collections::HashSet<String>>);

impl log::Log for Log {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.level() <= log::Level::Info
    }
    fn log(&self, r: &log::Record<'_>) {
        let line = format!("[{} {}] {}", r.level(), r.target(), r.args());
        if self.enabled(r.metadata()) && self.0.lock().is_ok_and(|mut seen| seen.insert(line.clone())) {
            println!("{line}");
        }
    }
    fn flush(&self) {}
}

fn main() {
    if log::set_boxed_logger(Box::new(Log(Default::default()))).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opt = |name: &str, default: f64| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    let Some(file) = args.first().filter(|a| *a == "--demo" || !a.starts_with("--")) else {
        eprintln!(
            "usage: play_bench <media file | project.fcproj | --demo> [--from 0] [--seconds 10] [--side 1440] [--speed 1] [--workers n] [--hardware] [--cpu-render] [--sequential] [--wav heard.wav]"
        );
        std::process::exit(2);
    };
    let (seconds, side, speed) = (opt("--seconds", 10.0), opt("--side", 1440.0) as u32, opt("--speed", 1.0));
    let from_s = opt("--from", 0.0);
    let decoders = if args.iter().any(|a| a == "--hardware") {
        format!("hardware first ({:?})", use_hardware_decoders())
    } else {
        "software".into()
    };
    use_gpu_renderer(!args.iter().any(|a| a == "--cpu-render"));

    let mut engine = FilmCraftEngine::new();
    if file == "--demo" {
        engine.execute("raw.filmcraft/file.openDemoProject", json!({})).expect("the demo project");
    } else if file.ends_with(".fcproj") {
        engine.execute("project.open", json!({ "path": file })).expect("the project");
    } else {
        engine.execute("media.import", json!({ "paths": [file] })).expect("import");
        let project = engine.execute("project.inspect", json!({})).expect("inspect").result;
        let clip = first_clip(&project["root"]).expect("the imported clip");
        engine.execute("timeline.createFromClips", json!({ "items": [clip] })).expect("a timeline");
    }
    let p: Arc<dyn Playback> = engine.playback().expect("the timeline");
    let (rate, last) = (p.rate(), p.frames() - 1);
    let workers = opt("--workers", FrameServer::default_workers() as f64) as usize;
    // one frame at a time first: the cost of a frame with no other work running
    let one = Instant::now();
    let from = rate.frame_at(from_s);
    for f in [from, from + 1, from + 2] {
        p.render(f, side, &Default::default()).expect("frame");
    }
    println!("one frame alone: {:.0} ms", one.elapsed().as_secs_f64() * 1e3 / 3.0);
    if args.iter().any(|a| a == "--sequential") {
        // the raw rate: frames in order on one thread, every frame and every other frame
        for stride in [1, 2, 3] {
            let t = Instant::now();
            let n = 48;
            for i in 0..n {
                p.render(from + 3 + i * stride, side, &Default::default()).expect("frame");
            }
            println!("in order, every {stride}: {:.1} ms a frame", t.elapsed().as_secs_f64() * 1e3 / n as f64);
        }
        let (gpu, cpu) = frame_counts();
        println!("frames drawn on the GPU {gpu}, on the CPU {cpu}");
        return;
    }
    println!(
        "{file}: {}x{} at {:.3} fps, {} frames; viewer side {side}, speed {speed}, {workers} frame workers, decoding {decoders}",
        p.size().0,
        p.size().1,
        rate.fps(),
        p.frames()
    );

    let server = FrameServer::new(workers);
    let key = |frame| FrameKey { revision: p.revision(), frame, side, draft: false, highlight: None };
    let t0 = Instant::now();
    let now = || t0.elapsed().as_secs_f64();
    let mut transport = Transport::default();
    transport.play(from, speed, rate);
    while transport.in_preroll() {
        server.schedule_playback(&p, key(from), speed, true);
        if transport.preroll_done(now(), server.preroll_ready(key(from), speed, last)) {
            transport.start_clock(now(), false);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let preroll = now();

    // the sound, mixed ahead and taken by a simulated device on its own thread
    let stats = Arc::new(AudioStats::default());
    let done = Arc::new(AtomicBool::new(false));
    let wav = args.iter().position(|a| a == "--wav").and_then(|i| args.get(i + 1)).cloned();
    let device = {
        let mut fill =
            ahead::spawn(p.mixer(DEVICE_RATE), rate.sample_of(from, DEVICE_RATE), DEVICE_RATE, 2, stats.clone());
        let (done, keep) = (done.clone(), wav.is_some());
        std::thread::spawn(move || {
            let period = Duration::from_secs_f64(DEVICE_PERIOD as f64 / f64::from(DEVICE_RATE));
            let start = Instant::now();
            let mut buf = vec![0.0; DEVICE_PERIOD * 2];
            let mut heard = Vec::new();
            let mut k = 1;
            while !done.load(Ordering::Relaxed) {
                fill(&mut buf, 2);
                if keep {
                    heard.extend_from_slice(&buf);
                }
                if let Some(wait) = (period * k).checked_sub(start.elapsed()) {
                    std::thread::sleep(wait);
                }
                k += 1;
            }
            heard
        })
    };

    // the display: each refresh shows the frame due, or holds the newest one it has
    let (mut ticks, mut exact, mut shown_frames, mut worst_hold) = (0u64, 0u64, 0u64, 0.0f64);
    let mut on_screen: Option<i64> = None;
    let mut since = preroll;
    let refresh = Duration::from_secs_f64(1.0 / DISPLAY_HZ);
    let end_at = preroll + seconds;
    let mut ended = false;
    while now() < end_at {
        let t = now();
        let Some((step, _)) = transport.advance(t, None, rate, last) else { break };
        let due = match step {
            Step::Play(f) | Step::Preroll(f) => f,
            Step::End(f) => {
                ended = true;
                f
            }
        };
        server.schedule_playback(&p, key(due), speed, false);
        let dir = if speed < 0.0 { -1 } else { 1 };
        let show = server.nearest(key(due), 48, dir).map(|(k, _)| k.frame);
        ticks += 1;
        exact += u64::from(show == Some(due));
        if show != on_screen && show.is_some() {
            shown_frames += 1;
            on_screen = show;
            worst_hold = worst_hold.max(t - since);
            since = t;
        }
        if ended {
            break;
        }
        std::thread::sleep(refresh.saturating_sub(Duration::from_secs_f64(now() - t)));
    }
    let played = now() - preroll;
    // a frame still on screen at the end has been held too, however long (LOG B-016)
    worst_hold = worst_hold.max(now() - since);
    done.store(true, Ordering::Relaxed);
    let heard = device.join().unwrap_or_default();
    let st = server.stats();
    let due_frames = (played * rate.fps() * speed.abs()).round();
    println!("preroll {:.0} ms; played {played:.1} s{}", preroll * 1e3, if ended { " (reached the end)" } else { "" });
    println!(
        "display: exact frame on {:.1}% of {ticks} refreshes; {shown_frames} distinct frames for {due_frames} due ({:.1} fps shown); longest hold {:.0} ms",
        100.0 * exact as f64 / ticks.max(1) as f64,
        shown_frames as f64 / played,
        worst_hold * 1e3
    );
    println!(
        "frame jobs: {} done, {} cancelled, {} failed, {:.1} ms average ({:.1} ms getting source pictures)",
        st.jobs,
        st.cancelled,
        st.failed,
        st.job_ms / st.jobs.max(1) as f64,
        st.decode_ms / st.jobs.max(1) as f64
    );
    let (gpu, cpu) = frame_counts();
    println!("frames drawn on the GPU {gpu}, on the CPU {cpu}");
    let min_lead = stats.min_lead_frames.load(Ordering::Relaxed);
    println!(
        "sound: {} device callbacks, {} underruns, {} frames of silence; least lead {}",
        stats.callbacks.load(Ordering::Relaxed),
        stats.underruns.load(Ordering::Relaxed),
        stats.missing_frames.load(Ordering::Relaxed),
        if min_lead == u64::MAX {
            "n/a".into()
        } else {
            format!("{:.0} ms", min_lead as f64 * 1e3 / f64::from(DEVICE_RATE))
        }
    );
    if let Some(path) = wav {
        match write_wav(&path, &heard, DEVICE_RATE, 2) {
            Ok(()) => println!(
                "sound written to {path}: {:.2} s from timeline sample {} at {DEVICE_RATE} Hz",
                heard.len() as f64 / 2.0 / f64::from(DEVICE_RATE),
                rate.sample_of(from, DEVICE_RATE)
            ),
            Err(e) => eprintln!("could not write {path}: {e}"),
        }
    }
}

/// What the simulated device received, as a 32-bit float WAV file.
fn write_wav(path: &str, samples: &[f32], rate: u32, channels: u16) -> std::io::Result<()> {
    let data = (samples.len() * 4) as u32;
    let mut out = Vec::with_capacity(44 + samples.len() * 4);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * u32::from(channels) * 4).to_le_bytes());
    out.extend_from_slice(&(channels * 4).to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, out)
}
