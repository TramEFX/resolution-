//! `resolution-app` — the Resolution editor.
//!
//! ```text
//! resolution-app [--demo | --project <path.fcproj>] [--import <file>]… [--seek <seconds>]
//!                [--mute] [--software-decode] [--theme <name>] [--source <clip>] [--play <s>]
//!                [--screenshot <out.png>]
//! ```
//! `--import` loads media and builds a timeline from the first clip. `--mute` plays without
//! opening an audio device. `--software-decode` turns off the operating system's video decoders
//! (LOG D-014). `--theme` switches to a colour theme (midnight, ember, mint, rose, charcoal;
//! LOG D-018), which is then kept, as if chosen in Workspace > Theme. `--source` opens a Media
//! Pool clip, by name, in the source viewer. Self-tests: `--play` plays for that long once a frame
//! is on screen, prints what was shown and heard, and quits; `--screenshot` saves the window as a
//! PNG once a frame is on screen (after the play test, with both) and quits.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod chrome;
mod icons;
mod keys;
mod model;
mod monitor;
mod theme;
mod timeline_ui;
mod worker;

const USAGE: &str = "usage: resolution-app [--demo | --project <path>] [--import <file>]… [--seek <s>] [--mute] \
                     [--software-decode] [--theme <name>] [--source <clip>] [--play <s>] [--screenshot <out.png>]";

fn options() -> Result<(app::Options, bool), String> {
    let mut o = app::Options::default();
    let mut software_decode = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "--demo" => o.demo = true,
            "--project" => o.project = Some(value("--project")?),
            "--import" => o.import.push(value("--import")?),
            "--seek" => o.seek = Some(value("--seek")?.parse().map_err(|e| format!("--seek: {e}"))?),
            "--mute" => o.mute = true,
            "--play" => o.play_for = Some(value("--play")?.parse().map_err(|e| format!("--play: {e}"))?),
            "--software-decode" => software_decode = true,
            "--theme" => o.theme = Some(value("--theme")?),
            "--source" => o.source = Some(value("--source")?),
            "--screenshot" => o.screenshot = Some(value("--screenshot")?.into()),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok((o, software_decode))
}

fn main() -> eframe::Result {
    let (mut options, software_decode) = match options() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            std::process::exit(2);
        }
    };
    // Real-time 4K needs the GPU's video decoder: 5 frames a second in software, 30 with it on
    // a phone clip (LOG D-014).
    options.decoding = if software_decode {
        "software decoding".into()
    } else {
        match resolution_filmcraft_adapter::use_hardware_decoders() {
            Ok(name) => format!("hardware decoding ({name})"),
            Err(why) => format!("software decoding: {why}"),
        }
    };
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Resolution")
            .with_inner_size([1600.0, 920.0])
            .with_min_inner_size([1000.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native("Resolution", native, Box::new(move |cc| Ok(Box::new(app::ResolutionApp::new(cc, options)))))
}
