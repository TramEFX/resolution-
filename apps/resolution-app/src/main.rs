//! `resolution-app` — the Resolution editor.
//!
//! ```text
//! resolution-app [--demo | --project <path.fcproj>] [--import <file>]… [--seek <seconds>]
//!                [--mute] [--software-decode] [--cpu-render] [--theme <name>] [--source <clip>]
//!                [--page <name>] [--play <s>] [--screenshot <out.png>] [--window-at <x,y>]
//!                [--highlight grey|bw|diff] [--palette <name>] [--bars]
//!                [--mini-timeline] [--window-size <w,h>] [--journal <file.jsonl>]
//! ```
//! `--import` loads media and builds a timeline from the first clip. `--mute` plays without
//! opening an audio device. `--software-decode` turns off the operating system's video decoders
//! (LOG D-014). `--cpu-render` draws every timeline frame on FilmCraft's CPU renderer instead of
//! Resolution's GPU renderer (LOG D-036). `--theme` switches to a colour theme (midnight, ember, mint, rose, charcoal;
//! LOG D-018), which is then kept, as if chosen in Workspace > Theme. `--source` opens a Media
//! Pool clip, by name, in the source viewer; `--page` starts on a page (`deliver`). Self-tests:
//! `--play` plays for that long once a frame is on screen, prints what was shown and heard, and
//! quits; `--screenshot` saves the window as a PNG once a frame is on screen (after the play test,
//! with both) and quits. `--window-at` puts the window's top left corner at that position, in
//! the screens' own pixels as Windows lays them out (LOG D-049): self-tests go on a screen of
//! their own, away from the user's. `--window-size` makes the window that size, in points (at
//! least 1000 × 640), to look at a smaller window. `--highlight` starts with the Color page's Highlight view on
//! (LOG D-048), flat grey or black and white, or Highlight Difference (D-070), for self-tests of it. `--palette` opens a Color page
//! palette (`curves`, `qualifier`, `window`, or a hue curve as `curves:hue_sat` and so on) and
//! `--bars` the Primaries' Bars mode, and `--mini-timeline` the Mini-Timeline, for self-tests of them
//! (LOG D-069, D-071). `--journal` writes the action
//! journal to a file as it goes: a line of JSON as each command starts and ends (LOG D-054).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod chrome;
mod color_page;
mod deliver;
mod icons;
mod journal;
mod keys;
mod model;
mod monitor;
mod theme;
mod timeline_ui;
mod waveforms;
mod worker;

const USAGE: &str = "usage: resolution-app [--demo | --project <path>] [--import <file>]… [--seek <s>] [--mute] \
                     [--software-decode] [--cpu-render] [--theme <name>] [--source <clip>] [--page <name>] [--play <s>] [--screenshot <out.png>] [--window-at <x,y>] [--highlight grey|bw|diff] [--palette <name>] [--bars] [--mini-timeline] [--window-size <w,h>] [--journal <file>]";

fn options() -> Result<(app::Options, bool, bool), String> {
    let mut o = app::Options::default();
    let mut software_decode = false;
    let mut cpu_render = false;
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
            "--cpu-render" => cpu_render = true,
            "--theme" => o.theme = Some(value("--theme")?),
            "--source" => o.source = Some(value("--source")?),
            "--page" => o.page = Some(value("--page")?),
            "--screenshot" => o.screenshot = Some(value("--screenshot")?.into()),
            "--window-at" => o.window_at = Some(position(&value("--window-at")?)?),
            "--window-size" => {
                let text = value("--window-size")?;
                let [w, h] = position(&text).map_err(|_| format!("--window-size: `{text}` is not w,h"))?;
                o.window_size = Some([w.max(1000.0), h.max(640.0)]);
            }
            "--journal" => o.journal = Some(value("--journal")?.into()),
            "--highlight" => {
                o.highlight = Some(match value("--highlight")?.as_str() {
                    "grey" | "gray" => false,
                    "bw" => true,
                    // Highlight Difference (LOG D-070), for self-tests until it has a command
                    "diff" => {
                        o.highlight_difference = true;
                        false
                    }
                    other => return Err(format!("--highlight: `{other}` is not grey, bw or diff")),
                })
            }
            "--palette" => {
                let name = value("--palette")?;
                if color_page::palette_of(&name).is_none() {
                    return Err(format!("--palette: `{name}` is not curves, qualifier, window or curves:<hue curve>"));
                }
                o.palette = Some(name);
            }
            "--bars" => o.bars = true,
            "--mini-timeline" => o.mini_timeline = true,
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok((o, software_decode, cpu_render))
}

/// `x,y` as a screen position; either may be negative (a screen left of or above the main one).
fn position(text: &str) -> Result<[f32; 2], String> {
    let bad = || format!("--window-at: `{text}` is not x,y");
    let (x, y) = text.split_once(',').ok_or_else(bad)?;
    Ok([x.trim().parse().map_err(|_| bad())?, y.trim().parse().map_err(|_| bad())?])
}

fn main() -> eframe::Result {
    let (mut options, software_decode, cpu_render) = match options() {
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
    resolution_filmcraft_adapter::use_gpu_renderer(!cpu_render);
    // agents reach this editor through `resolution mcp --editor` (LOG D-026)
    options.agent_link = Some(resolution_agent_link::default_path());
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Resolution")
            .with_inner_size([1600.0, 920.0])
            .with_min_inner_size([1000.0, 640.0]),
        // a window put somewhere on purpose is not kept there for next time
        persist_window: options.window_at.is_none() && options.window_size.is_none(),
        ..Default::default()
    };
    eframe::run_native("Resolution", native, Box::new(move |cc| Ok(Box::new(app::ResolutionApp::new(cc, options)))))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_window_position_reads_as_x_comma_y() {
        assert_eq!(super::position("-1920,165"), Ok([-1920.0, 165.0]));
        assert_eq!(super::position(" 10 , -5 "), Ok([10.0, -5.0]));
        for bad in ["", "10", "10,", "a,b", "1;2"] {
            assert!(super::position(bad).is_err(), "{bad:?}");
        }
    }
}
