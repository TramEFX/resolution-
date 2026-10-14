//! `deliver.render` (LOG D-042, D-045): a render of the open timeline, as FilmCraft's
//! `file.exportMedia` makes it, but of the timeline's graded copy ([`Graded`]), with every graded
//! clip's stand-in registered in the media pool as a graded source. So frames FilmCraft's export
//! draws itself on the CPU (an adjustment layer, transitions other than dissolves, dips and Push)
//! show the grades by their own math too, as the viewers do (D-044); the frames the GPU renderer
//! draws find the copy and grade in the shader.
//!
//! The export's settings, range and job are FilmCraft's own (`settings_from_params`,
//! `range_param`, `spawn_export`). Its output path follows FilmCraft's rules, which are private to
//! its export tools, so they are repeated here ([`output_path`]).

use std::path::Path;
use std::sync::Arc;

use filmcraft_engine::export::{ExportSettings, Format};
use filmcraft_engine::export_tools::{
    default_export_dir, expand_home, range_param, sequence_param, settings_from_params, spawn_export,
};
use filmcraft_engine::project::ItemId;
use filmcraft_engine::{EngineError, Session};
use serde_json::{Value, json};

use crate::gpu::{self, Graded};

/// FilmCraft's command `deliver.render` maps to, for its messages.
const CMD: &str = "file.exportMedia";

/// The render, as `file.exportMedia` answers it; `None` when the timeline needs nothing of ours (no
/// graded clip, no GPU, the GPU renderer off, a graded subclip): FilmCraft's own export then does it.
pub(crate) fn render(s: &mut Session, p: &Value) -> Option<Result<Value, EngineError>> {
    let seq = match sequence_param(s, p, CMD) {
        Ok(seq) => seq,
        Err(e) => return Some(Err(e)),
    };
    let graded = Arc::new(Graded::of(&s.project, seq));
    let provider = s.media.full_res_provider(s.project.clone(), s.services.clone());
    if !graded.register_sources(&s.media, &provider) {
        return None;
    }
    Some(start(s, p, seq, graded))
}

fn start(s: &mut Session, p: &Value, seq: ItemId, graded: Arc<Graded>) -> Result<Value, EngineError> {
    let (preset, mut settings) = settings_from_params(s, p, CMD)?;
    if p.get("path").and_then(Value::as_str).is_none() {
        return Err(EngineError::Other(format!("{CMD}: need `path`")));
    }
    settings.range = range_param(s, graded.project(), seq, p.get("range"), p, CMD)?;
    settings.path = output_path(s, p, seq, &settings);
    let wait = p.get("wait").and_then(Value::as_bool).unwrap_or(false);
    let path = settings.path.clone();
    let name = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string();
    // the export's GPU frame renderer finds the copy by its project
    gpu::exporting(&graded);
    let id = spawn_export(s, graded.project().clone(), seq, settings, format!("Export {name}"), wait)?;
    let mut out = json!({ "job": id, "path": path });
    if let Some(pr) = preset {
        out["preset"] = json!(pr.name);
    }
    if wait && let Some(j) = s.jobs.iter().find(|j| j.id == id) {
        let v = j.to_json();
        if let Some(e) = v["result"]["error"].as_str() {
            return Err(EngineError::Other(format!("export failed: {e}")));
        }
        out["result"] = v["result"].clone();
    }
    Ok(out)
}

/// The file the render writes, by FilmCraft's rules (`export_tools::output_path`): `path` as
/// given, or a folder (ending in a separator, or one that exists) to put the timeline's name in;
/// the settings' extension added when missing.
fn output_path(s: &Session, p: &Value, seq: ItemId, settings: &ExportSettings) -> String {
    let name = s.project.item(seq).map(|i| i.name.clone()).unwrap_or_else(|| "Sequence".into());
    let base = match p.get("path").and_then(Value::as_str).map(expand_home) {
        Some(x) if x.ends_with(['/', '\\']) || Path::new(&x).is_dir() => {
            Path::new(&x).join(file_safe(&name)).to_string_lossy().to_string()
        }
        Some(x) => x,
        None => default_export_dir(s).join(file_safe(&name)).to_string_lossy().to_string(),
    };
    with_extension(&base, settings)
}

fn file_safe(n: &str) -> String {
    let s: String = n.chars().map(|c| if c.is_alphanumeric() || " -_().".contains(c) { c } else { '_' }).collect();
    let s = s.trim().to_string();
    if s.is_empty() { "Export".into() } else { s }
}

fn with_extension(path: &str, settings: &ExportSettings) -> String {
    let ext = settings.extension();
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some(e)
            if e.eq_ignore_ascii_case(ext)
                || (settings.format == Format::TiffSequence && e.eq_ignore_ascii_case("tiff"))
                || (settings.format == Format::JpegSequence && e.eq_ignore_ascii_case("jpeg"))
                || (settings.format == Format::Aac && e.eq_ignore_ascii_case("aac")) =>
        {
            path.to_string()
        }
        Some(_) if settings.is_image_sequence() || matches!(settings.format, Format::Aiff) => {
            format!("{}.{ext}", path.rsplit_once('.').map_or(path, |x| x.0))
        }
        _ => format!("{path}.{ext}"),
    }
}
