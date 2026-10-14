//! The viewers' frames on Resolution's GPU renderer (LOG D-035, D-036).
//!
//! Each playback snapshot keeps a copy of its timeline without the Lumetri effects Resolution's
//! grades compile to (D-028), and each graded clip's grade. A frame is planned by FilmCraft
//! (`plan_frame`, which decodes its layers) from that copy. When the plan is a plain stack of
//! layers, Resolution's renderer draws it on the GPU with each clip's grade applied by the grade's
//! own math. Anything else is drawn by FilmCraft's CPU renderer from the real project, compiled
//! grades and all:
//!
//! - a transition at that time, an adjustment layer, an HDR timeline, captions;
//! - a nested timeline or other non-media clip;
//! - a layer FilmCraft would draw with its own GPU effects, blend modes or colour management;
//! - a graded clip FilmCraft had to pre-render on the CPU (other effects on it).
//!
//! FilmCraft's plans do not say which clip a layer comes from. Its planner adds, bottom track
//! first, one layer for each enabled media clip on an enabled track at that time. So with the
//! cases above excluded, the layers are the visible clips in that order. Each pairing is checked
//! on the picture's shape, and any doubt falls back to the CPU.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use filmcraft_engine::project::{ClipId, ItemId, ItemKind, Project};
use filmcraft_engine::render::plan::{FramePlan, PlanLayer, is_cpu_frame, plan_frame};
use filmcraft_engine::render::{Blend, RenderOptions, SourceProvider};
use filmcraft_engine::time::Tick;
use filmcraft_frame::{Chroma, PixelData};
use resolution_engine_api::Frame;
use resolution_grade::Grade;
use resolution_renderer::{Coding, Layer, Pixels, Renderer, Transfer};

static GPU_FRAMES: AtomicU64 = AtomicU64::new(0);
static CPU_FRAMES: AtomicU64 = AtomicU64::new(0);
static GPU_ON: AtomicBool = AtomicBool::new(true);

/// Whether the viewers' frames may be drawn on the GPU renderer (on unless turned off): off, every
/// frame is FilmCraft's CPU renderer's, to compare the two or get round a GPU that draws wrong.
pub fn use_gpu_renderer(on: bool) {
    GPU_ON.store(on, Ordering::Relaxed);
}

/// How many timeline frames the viewers have had from the GPU renderer and from the CPU, since
/// the program started.
pub fn frame_counts() -> (u64, u64) {
    (GPU_FRAMES.load(Ordering::Relaxed), CPU_FRAMES.load(Ordering::Relaxed))
}

pub(crate) fn counted_cpu_frame() {
    CPU_FRAMES.fetch_add(1, Ordering::Relaxed);
}

/// The renderer, one for the program, made when first wanted; `None` without a GPU.
fn renderer() -> Option<&'static Mutex<Renderer>> {
    static R: OnceLock<Option<Mutex<Renderer>>> = OnceLock::new();
    R.get_or_init(|| Renderer::new().map(Mutex::new)).as_ref()
}

/// A timeline as the GPU renderer draws it.
pub(crate) struct Graded {
    /// The project with the grades' Lumetri effects taken off the graded clips.
    project: Arc<Project>,
    grades: HashMap<ClipId, Grade>,
}

impl Graded {
    pub(crate) fn of(project: &Arc<Project>, seq: ItemId) -> Self {
        let Some(s) = project.sequence(seq) else {
            return Self { project: project.clone(), grades: HashMap::new() };
        };
        // the clips whose Lumetri effects are exactly their grade's (not foreign ones)
        let mut grades = HashMap::new();
        for it in s.video_tracks.iter().flat_map(|t| &t.items) {
            let size = project.source_size(it.item).unwrap_or((s.settings.width, s.settings.height));
            if let Some((g, false)) = crate::color::grade_in(project, seq, it.id, size)
                && !g.is_neutral()
            {
                grades.insert(it.id, g);
            }
        }
        if grades.is_empty() {
            return Self { project: project.clone(), grades };
        }
        let mut p = (**project).clone();
        if let Some(s) = p.sequence_mut(seq) {
            for it in s.video_tracks.iter_mut().flat_map(|t| &mut t.items) {
                if grades.contains_key(&it.id) {
                    let n = it.effects.iter().take_while(|e| e.effect == "lumetri").count();
                    it.effects.drain(..n);
                }
            }
        }
        Self { project: Arc::new(p), grades }
    }

    /// The frame at `t` of timeline `seq`, drawn on the GPU, or `None` when the GPU renderer does
    /// not cover it (see the module notes) or there is no GPU.
    pub(crate) fn render(
        &self,
        seq: ItemId,
        t: Tick,
        opts: RenderOptions,
        sources: &dyn SourceProvider,
    ) -> Option<Frame> {
        if !GPU_ON.load(Ordering::Relaxed) {
            return None;
        }
        let gpu = renderer()?;
        if opts.captions && has_captions(&self.project, seq, t) {
            return None;
        }
        // cheap checks first, so a frame the CPU will draw is not decoded twice
        if is_cpu_frame(&self.project, seq, t) {
            return None;
        }
        let clips = visible_clips(&self.project, seq, t)?;
        let FramePlan::Layers { width, height, layers } = plan_frame(&self.project, seq, t, opts, sources) else {
            return None;
        };
        if layers.len() != clips.len() {
            return None;
        }
        let mut out = Vec::with_capacity(layers.len());
        for (l, (clip, size)) in layers.iter().zip(&clips) {
            let grade = self.grades.get(clip);
            out.push(layer(l, *size, grade)?);
        }
        let rgba = gpu.lock().ok()?.render((width as u32, height as u32), &out)?;
        GPU_FRAMES.fetch_add(1, Ordering::Relaxed);
        Some(Frame { width: width as u32, height: height as u32, rgba })
    }
}

/// Whether the timeline shows captions at `t` (the planner draws them as extra layers).
fn has_captions(p: &Project, seq: ItemId, t: Tick) -> bool {
    p.sequence(seq).is_some_and(|s| !filmcraft_engine::render::caption_overlays(s, t, 2, 2).is_empty())
}

/// The clips the planner makes layers of at `t`, bottom first, with their media's picture size;
/// `None` when the frame has anything else (a transition, a clip that is not plain media).
fn visible_clips(p: &Project, seq: ItemId, t: Tick) -> Option<Vec<(ClipId, (u32, u32))>> {
    let s = p.sequence(seq)?;
    let mut out = Vec::new();
    for tr in s.video_tracks.iter().filter(|tr| tr.enabled) {
        if tr.transitions.iter().any(|x| x.range().contains(t)) {
            return None;
        }
        let Some(it) = tr.item_at(t) else { continue };
        if !it.enabled {
            continue;
        }
        match p.item(it.item).map(|i| &i.kind) {
            Some(ItemKind::Media(_) | ItemKind::Subclip { .. }) => {}
            _ => return None,
        }
        out.push((it.id, p.source_size(it.item)?));
    }
    Some(out)
}

/// A plan layer as the renderer takes it, or `None` when the renderer does not draw it.
fn layer<'a>(l: &'a PlanLayer, media: (u32, u32), grade: Option<&'a Grade>) -> Option<Layer<'a>> {
    if l.fx.is_some() || l.input.is_some() || l.blend != Blend::Normal {
        return None;
    }
    let f = &l.frame;
    // the layer is the clip's picture: same shape (a decoder may hand over fewer pixels)
    let shape = |(w, h): (u32, u32)| f64::from(w) / f64::from(h.max(1));
    if (shape((f.width, f.height)) / shape(media) - 1.0).abs() > 0.02 {
        return None;
    }
    // a graded clip pre-rendered on the CPU (other effects on it) is already placed: its grade
    // and windows would land wrong
    if grade.is_some() && matches!(f.data, PixelData::RgbaF32(_)) {
        return None;
    }
    let transfer = match f.color.transfer {
        filmcraft_color::Transfer::Linear => Transfer::Linear,
        filmcraft_color::Transfer::Bt709 | filmcraft_color::Transfer::Srgb => Transfer::Srgb,
        _ => return None,
    };
    let (kr, kb) = f.color.matrix.kr_kb();
    let coding = Coding { kr, kb, full_range: f.color.range == filmcraft_color::Range::Full, transfer };
    let shifts = |c: Chroma| c.shifts();
    let pixels = match &f.data {
        PixelData::Rgba8(d) => Pixels::Rgba8(d),
        PixelData::RgbaF32(d) => Pixels::RgbaF32(d),
        PixelData::Yuv8 { planes, chroma, alpha } => Pixels::Yuv8 {
            planes: [&planes[0], &planes[1], &planes[2]],
            chroma: shifts(*chroma),
            alpha: alpha.as_deref().map(Vec::as_slice),
        },
        PixelData::Yuv16 { planes, chroma, bits, alpha } => Pixels::Yuv16 {
            planes: [&planes[0], &planes[1], &planes[2]],
            chroma: shifts(*chroma),
            bits: *bits,
            alpha: alpha.as_deref().map(Vec::as_slice),
        },
    };
    let m = l.matrix;
    Some(Layer {
        width: f.width,
        height: f.height,
        pixels,
        coding,
        matrix: [m.a, m.b, m.c, m.d, m.e, m.f],
        opacity: l.opacity,
        grade,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
