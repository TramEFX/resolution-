//! The viewers' frames on Resolution's GPU renderer (LOG D-035, D-036, D-037).
//!
//! Each playback snapshot keeps a copy of its timeline in which every graded clip FilmCraft would
//! hand over as a plain decoded picture has its grade's Lumetri effects taken off and points at a
//! stand-in media item of its own: a copy of its media item under a new id. A frame is planned by
//! FilmCraft (`plan_frame`, which also decodes the pictures) from that copy, through a source
//! provider that opens each stand-in's real media and notes which clip every picture it decodes
//! belongs to. So a layer drawing one of those pictures is known to be that clip's, wherever the
//! planner put it: inside a transition, over or under other tracks, in a flattened nested
//! timeline.
//!
//! Resolution's renderer then draws the plan's layers: graded clips with their grade, by the
//! grade's own math; everything else as FilmCraft planned it (solids for dips, captions, titles
//! and clips FilmCraft rendered on the CPU, pictures with their opacity). A layer carrying
//! FilmCraft's standard effects has them run on the CPU for that layer alone (their GPU port is a
//! later step of G2).
//!
//! The whole frame is drawn by FilmCraft's CPU renderer, from the real project, compiled grades
//! and all, when:
//!
//! - FilmCraft would draw it on the CPU anyway (an adjustment layer, a transition other than
//!   dissolves, dips and Push, an HDR timeline);
//! - it is a Push transition (`FramePlan::Composite`), has a blend mode other than Normal, or has
//!   colour management FilmCraft runs on its GPU path (not ported yet);
//! - a graded clip's picture is not drawn as decoded (FilmCraft converted it on the CPU first), so
//!   its grade cannot be put back;
//! - two graded clips show the very same picture (the same media at the same moment), so the
//!   picture does not say which clip it is.
//!
//! A graded clip with other standard effects or opacity masks keeps its Lumetri effects in the
//! copy: the grade comes first in its effect stack, so FilmCraft renders that clip on the CPU,
//! graded, and the GPU draws it like any pre-rendered layer.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use filmcraft_engine::project::{ClipId, ItemId, Project};
use filmcraft_engine::render::plan::{FramePlan, PlanLayer, effect_image, is_cpu_frame, plan_frame};
use filmcraft_engine::render::{Blend, RenderOptions, SourceProvider};
use filmcraft_engine::time::Tick;
use filmcraft_frame::{AudioBuffer, Chroma, PixelData, VideoFrame};
use filmcraft_media::{FrameRequest, MediaInfo, MediaSource, SharedSource, cancel};
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

/// Pictures decoded for stand-ins, each with its clip.
type Seen = Arc<Mutex<Vec<(Arc<VideoFrame>, ClipId)>>>;

/// A timeline as the GPU renderer draws it.
pub(crate) struct Graded {
    /// The project with each graded clip we draw having its Lumetri effects taken off and its
    /// media item swapped for a stand-in.
    project: Arc<Project>,
    /// Stand-in media item → (the real media item, the clip).
    stand_ins: HashMap<ItemId, (ItemId, ClipId)>,
    grades: HashMap<ClipId, Grade>,
}

impl Graded {
    pub(crate) fn of(project: &Arc<Project>, seq: ItemId) -> Self {
        let plain = || Self { project: project.clone(), stand_ins: HashMap::new(), grades: HashMap::new() };
        let Some(s) = project.sequence(seq) else { return plain() };
        // the clips whose Lumetri effects are exactly their grade's (not foreign ones), with no
        // other standard effects or opacity masks after it
        let mut grades = HashMap::new();
        let mut media = Vec::new();
        for it in s.video_tracks.iter().flat_map(|t| &t.items) {
            let size = project.source_size(it.item).unwrap_or((s.settings.width, s.settings.height));
            let Some((g, false)) = crate::color::grade_in(project, seq, it.id, size) else { continue };
            let n = it.effects.iter().take_while(|e| e.effect == "lumetri").count();
            let others = it.effects[n..].iter().any(|e| e.enabled && e.def().is_some_and(|d| !d.intrinsic));
            if g.is_neutral() || others || it.has_opacity_masks() {
                continue;
            }
            grades.insert(it.id, g);
            media.push((it.id, it.item));
        }
        if grades.is_empty() {
            return plain();
        }
        let mut p = (**project).clone();
        let mut stand_ins = HashMap::new();
        let mut swap = HashMap::new();
        for (clip, item) in media {
            let Some(mut copy) = p.item(item).cloned() else { continue };
            let id = ItemId(p.alloc_id());
            copy.id = id;
            p.items.insert(id, copy);
            stand_ins.insert(id, (item, clip));
            swap.insert(clip, id);
        }
        if let Some(s) = p.sequence_mut(seq) {
            for it in s.video_tracks.iter_mut().flat_map(|t| &mut t.items) {
                if let Some(id) = swap.get(&it.id) {
                    let n = it.effects.iter().take_while(|e| e.effect == "lumetri").count();
                    it.effects.drain(..n);
                    it.item = *id;
                }
            }
        }
        grades.retain(|c, _| swap.contains_key(c));
        Self { project: Arc::new(p), stand_ins, grades }
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
        // a cheap check first, so a frame the CPU will draw is not decoded twice
        if is_cpu_frame(&self.project, seq, t) {
            return None;
        }
        let seen = Seen::default();
        let tagging = Tagging { inner: sources, stand_ins: &self.stand_ins, seen: seen.clone() };
        // Planned without draft playback's shortcut, which hands over shrunk copies of the decoded
        // pictures: a graded clip's layer is known by its picture.
        let plan = cancel::with_draft(false, || plan_frame(&self.project, seq, t, opts, &tagging));
        let FramePlan::Layers { width, height, layers } = plan else { return None };
        let seen = std::mem::take(&mut *seen.lock().ok()?);
        let grades = self.layer_grades(&layers, &seen)?;
        if layers.iter().any(|l| l.blend != Blend::Normal || l.input.is_some()) {
            return None;
        }
        // standard effects the GPU does not run yet: on the CPU, layer by layer
        let images: Vec<_> = layers.iter().map(|l| l.fx.as_ref().map(|fx| effect_image(&l.frame, fx))).collect();
        let mut out = Vec::with_capacity(layers.len());
        for ((l, grade), image) in layers.iter().zip(&grades).zip(&images) {
            let m = l.matrix;
            let matrix = [m.a, m.b, m.c, m.d, m.e, m.f];
            match image {
                Some(img) => out.push(Layer {
                    width: img.w as u32,
                    height: img.h as u32,
                    pixels: Pixels::RgbaF32(&img.px),
                    coding: Coding { transfer: Transfer::Linear, ..Coding::REC709 },
                    matrix,
                    opacity: l.opacity,
                    grade: None,
                }),
                None => out.push(layer(&l.frame, matrix, l.opacity, *grade)?),
            }
        }
        let rgba = gpu.lock().ok()?.render((width as u32, height as u32), &out)?;
        GPU_FRAMES.fetch_add(1, Ordering::Relaxed);
        Some(Frame { width: width as u32, height: height as u32, rgba })
    }

    /// Each layer's grade: the grade of the clip whose decoded picture it draws, if that clip is
    /// graded. `None` when a graded clip's picture went somewhere a grade cannot follow (see the
    /// module notes).
    fn layer_grades<'a>(
        &'a self,
        layers: &[PlanLayer],
        seen: &[(Arc<VideoFrame>, ClipId)],
    ) -> Option<Vec<Option<&'a Grade>>> {
        let mut clip_of: HashMap<*const VideoFrame, ClipId> = HashMap::new();
        for (f, clip) in seen {
            if clip_of.insert(Arc::as_ptr(f), *clip).is_some_and(|c| c != *clip) {
                return None;
            }
        }
        let mut drawn = HashSet::new();
        let mut out = Vec::with_capacity(layers.len());
        for l in layers {
            let ptr = Arc::as_ptr(&l.frame);
            let grade = match clip_of.get(&ptr) {
                Some(clip) => {
                    drawn.insert(ptr);
                    // effects or colour management to run on the picture first: its grade comes
                    // before them and cannot be put back here
                    if l.fx.is_some() || l.input.is_some() {
                        return None;
                    }
                    Some(self.grades.get(clip)?)
                }
                None => None,
            };
            out.push(grade);
        }
        (drawn.len() == clip_of.len()).then_some(out)
    }
}

/// A source provider that opens each stand-in's real media and notes, for every picture decoded
/// for a stand-in, whose clip it is. The pictures are kept, so their addresses stay theirs.
struct Tagging<'a> {
    inner: &'a dyn SourceProvider,
    stand_ins: &'a HashMap<ItemId, (ItemId, ClipId)>,
    seen: Seen,
}

impl SourceProvider for Tagging<'_> {
    fn source(&self, item: ItemId) -> Option<SharedSource> {
        match self.stand_ins.get(&item) {
            Some(&(real, clip)) => {
                let inner = self.inner.source(real)?;
                Some(Arc::new(Tagged { inner, clip, seen: self.seen.clone() }))
            }
            None => self.inner.source(item),
        }
    }
}

/// A stand-in's source: the real media's, noting each picture it decodes.
struct Tagged {
    inner: SharedSource,
    clip: ClipId,
    seen: Seen,
}

impl MediaSource for Tagged {
    fn info(&self) -> &MediaInfo {
        self.inner.info()
    }
    fn video_frame(&self, req: FrameRequest) -> filmcraft_media::Result<Arc<VideoFrame>> {
        let f = self.inner.video_frame(req)?;
        if let Ok(mut v) = self.seen.lock() {
            v.push((f.clone(), self.clip));
        }
        Ok(f)
    }
    fn audio(&self, start: i64, frames: usize, sample_rate: u32) -> filmcraft_media::Result<AudioBuffer> {
        self.inner.audio(start, frames, sample_rate)
    }
    fn audio_stream(
        &self,
        stream: usize,
        start: i64,
        frames: usize,
        sample_rate: u32,
    ) -> filmcraft_media::Result<AudioBuffer> {
        self.inner.audio_stream(stream, start, frames, sample_rate)
    }
}

/// A decoded picture as the renderer takes it, or `None` when the renderer does not draw it.
fn layer<'a>(f: &'a VideoFrame, matrix: [f64; 6], opacity: f32, grade: Option<&'a Grade>) -> Option<Layer<'a>> {
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
    Some(Layer { width: f.width, height: f.height, pixels, coding, matrix, opacity, grade })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
