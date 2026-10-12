//! `resolution-app` — the Resolution editor.
//!
//! ```text
//! resolution-app [--demo | --project <path.fcproj>] [--import <file>]… [--seek <seconds>] [--screenshot <out.png>]
//! ```
//! `--import` loads media and builds a timeline from the first clip. `--screenshot` is a self-test:
//! once a frame is on screen, the window is saved as a PNG and the app quits.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod keys;
mod model;
mod timeline_ui;
mod worker;

fn options() -> Result<app::Options, String> {
    let mut o = app::Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "--demo" => o.demo = true,
            "--project" => o.project = Some(value("--project")?),
            "--import" => o.import.push(value("--import")?),
            "--seek" => o.seek = Some(value("--seek")?.parse().map_err(|e| format!("--seek: {e}"))?),
            "--screenshot" => o.screenshot = Some(value("--screenshot")?.into()),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(o)
}

fn main() -> eframe::Result {
    let options = match options() {
        Ok(o) => o,
        Err(e) => {
            eprintln!(
                "{e}\nusage: resolution-app [--demo | --project <path>] [--import <file>]… [--seek <s>] [--screenshot <out.png>]"
            );
            std::process::exit(2);
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
