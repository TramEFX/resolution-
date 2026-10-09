//! `resolution commands` — every command the engine offers (vocabulary, then raw engine commands)
//! `resolution keys` — the Resolve keymap
//! `resolution run [--project <path>] <id> [json] [<id> [json]]…` — run commands headlessly
//! `resolution mcp [--project <path> | --demo]` — serve MCP over stdio for agents

use std::process::ExitCode;

use resolution_engine_api::Engine;
use resolution_filmcraft_adapter::FilmCraftEngine;
use resolution_keymap::Keymap;
use serde_json::{Value, json};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("commands") => {
            for c in FilmCraftEngine::new().commands() {
                println!(
                    "{:<48} {}{}",
                    c.id,
                    c.label,
                    if c.params.is_empty() { String::new() } else { format!("  {}", c.params) }
                );
            }
            ExitCode::SUCCESS
        }
        Some("keys") => {
            let k = Keymap::resolve();
            for b in &k.bindings {
                println!("{:<20} {}", b.keys, b.command);
            }
            ExitCode::SUCCESS
        }
        Some("run") => run(&args[1..]),
        Some("mcp") => mcp(&args[1..]),
        _ => {
            eprintln!(
                "usage: resolution commands | keys | run [--project <path>] <id> [json]… | mcp [--project <path> | --demo]"
            );
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> ExitCode {
    let mut engine = FilmCraftEngine::new();
    let mut rest = args;
    if let [flag, path, tail @ ..] = rest
        && flag == "--project"
    {
        if let Err(e) = engine.execute("project.open", json!({ "path": path })) {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
        rest = tail;
    }
    let mut i = 0;
    while i < rest.len() {
        let id = &rest[i];
        let params = match rest.get(i + 1).map(|s| serde_json::from_str::<Value>(s)) {
            Some(Ok(v)) if v.is_object() => {
                i += 1;
                v
            }
            _ => json!({}),
        };
        i += 1;
        match engine.execute(id, params) {
            Ok(out) => println!(
                "{}",
                json!({ "command": id, "result": out.result, "revision": out.revision, "undo": out.undo_label })
            ),
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

fn mcp(args: &[String]) -> ExitCode {
    let mut engine = FilmCraftEngine::new();
    let opened = match args {
        [] => Ok(()),
        [flag] if flag == "--demo" => engine.execute("raw.filmcraft/file.openDemoProject", json!({})).map(|_| ()),
        [flag, path] if flag == "--project" => engine.execute("project.open", json!({ "path": path })).map(|_| ()),
        _ => {
            eprintln!("usage: resolution mcp [--project <path> | --demo]");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = opened {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    // stdout belongs to the MCP protocol; diagnostics go to stderr.
    match resolution_mcp::serve_stdio(Box::new(engine)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
