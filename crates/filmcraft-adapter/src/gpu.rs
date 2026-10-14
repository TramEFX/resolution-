//! The viewers' frames, and Deliver's, on Resolution's GPU renderer (LOG D-035 to D-045).
//!
//! Each playback snapshot keeps a copy of its timeline in which every graded clip has its
//! grade's Lumetri effects taken off and points at a stand-in media item of its own: a copy of its
//! media item under an id of a range the project never reaches. The stand-in's source opens the
//! real media and either
//!
//! - hands over the decoded pictures as they are, noting which clip each is for, when the clip has
//!   nothing else on it: the GPU renderer grades them as it composites ("in the shader"); or
//! - hands over pictures already graded by the GPU renderer (a [`GradedSource`]), when the clip has
//!   other effects or opacity masks, or for frames FilmCraft draws itself: whatever FilmCraft then
//!   does with them, the grade was applied first, by its own math (LOG D-044).
//!
//! A frame is planned by FilmCraft (`plan_frame`, which also decodes the pictures) from that copy.
//! A layer drawing a picture decoded for a clip graded in the shader is known to be that clip's,
//! wherever the planner put it: inside a transition, over or under other tracks, in a flattened
//! nested timeline.
//!
//! Resolution's renderer then draws the plan: graded clips with their grade, by the grade's own
//! math; everything else as FilmCraft planned it (solids for dips, captions, titles and clips
//! FilmCraft rendered on the CPU, pictures with their opacity, blend mode and colour management;
//! Push transitions between their two sides). A layer carrying FilmCraft's standard effects has
//! them run on the CPU for that layer alone (their GPU port waits for an Effects panel, D-041).
//!
//! The whole frame is drawn by FilmCraft's CPU renderer from the copy, every graded clip's
//! pictures graded at their source ([`Graded::render_cpu`]), when:
//!
//! - FilmCraft would draw it on the CPU anyway (an adjustment layer, a transition other than
//!   dissolves, dips and Push, an HDR timeline);
//! - a layer's colour management has no GPU form (log media);
//! - a picture decoded for a clip graded in the shader is not drawn as decoded (FilmCraft
//!   converted it on the CPU first, or it has colour management to run before its grade);
//! - two such clips show the very same picture (the same media at the same moment), so the
//!   picture does not say which clip it is.
//!
//! Only without a GPU, or with the GPU renderer turned off, does FilmCraft draw from the real
//! project, the grades compiled to Lumetri (LOG D-028), which the project keeps so it still opens
//! sensibly in FilmCraft.
//!
//! **Deliver** (G3, LOG D-042): FilmCraft's export takes a registered frame renderer for the frames
//! it would otherwise render on the CPU. Ours ([`ExportFrames`]) draws them the same way, from a
//! graded copy of the export's project, and hands back the composite in linear light, before it is
//! laid over black, as FilmCraft's CPU renderer would. So a file renders as the viewer shows it.

use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use filmcraft_color::{GpuCurve, InputTransform, REFERENCE_WHITE_NITS};
use filmcraft_engine::MediaPool;
use filmcraft_engine::export::FrameRenderer;
use filmcraft_engine::project::{ClipId, EffectInstance, ItemId, ItemKind, ParamValue, Project};
use filmcraft_engine::render::plan::{FramePlan, PlanLayer, PlanStep, effect_image_with, is_cpu_frame, plan_frame};
use filmcraft_engine::render::{Image, RenderOptions, SourceProvider, render_sequence};
use filmcraft_engine::time::Tick;
use filmcraft_frame::{AudioBuffer, Chroma, PixelData, VideoFrame};
use filmcraft_media::{FrameRequest, MediaInfo, MediaSource, SharedSource, cancel};
use resolution_engine_api::{Frame, GpuPicture};
use resolution_grade::Grade;
use resolution_renderer::{Blend, Coding, Curve, Layer, Managed, Picture, Pixels, Renderer, Siting, Step, Transfer};

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

static RENDERER: OnceLock<Option<Mutex<Renderer>>> = OnceLock::new();
/// The renderer is on the front end's device ([`use_gpu_device`]).
static SHARED: AtomicBool = AtomicBool::new(false);

/// The renderer, one for the program, made when first wanted; `None` without a GPU.
fn renderer() -> Option<&'static Mutex<Renderer>> {
    RENDERER.get_or_init(|| Renderer::new().map(Mutex::new)).as_ref()
}

/// Puts the GPU renderer on the front end's own device, so frames asked for with
/// `RenderHints::gpu` can be left there for it to draw (LOG D-039). Only before the renderer is
/// first wanted: `false` when it already exists elsewhere.
pub fn use_gpu_device(device: resolution_renderer::wgpu::Device, queue: resolution_renderer::wgpu::Queue) -> bool {
    let mut ours = false;
    RENDERER.get_or_init(|| {
        ours = true;
        Some(Mutex::new(Renderer::on(device, queue)))
    });
    if ours {
        SHARED.store(true, Ordering::Relaxed);
    }
    ours
}

/// A frame left on the GPU, as the engine API hands it over.
#[derive(Debug)]
struct OnGpu(Picture);

impl GpuPicture for OnGpu {
    fn texture(&self) -> &(dyn Any + Send + Sync) {
        self.0.view()
    }
    fn read(&self) -> Option<Vec<u8>> {
        self.0.read().map(<[u8]>::to_vec)
    }
}

/// Pictures decoded for stand-ins graded in the shader, each with its clip.
type Seen = Arc<Mutex<Vec<(Arc<VideoFrame>, ClipId)>>>;

/// Stand-in media items' ids start here: far beyond any id a project hands out, so a stand-in
/// never meets a real item.
const STAND_IN_IDS: u64 = 1 << 60;

/// A graded clip's stand-in media item in a [`Graded`] copy.
struct StandIn {
    /// The clip's real media item.
    real: ItemId,
    clip: ClipId,
    grade: Arc<Grade>,
    /// The clip has nothing else on it: on the GPU path its pictures are graded in the shader;
    /// otherwise (other effects, opacity masks) they come graded from a [`GradedSource`].
    in_shader: bool,
}

/// A timeline as Resolution renders it.
pub(crate) struct Graded {
    /// The project with each graded clip's Lumetri effects taken off and its media item swapped
    /// for a stand-in.
    project: Arc<Project>,
    stand_ins: HashMap<ItemId, StandIn>,
}

impl Graded {
    pub(crate) fn of(project: &Arc<Project>, seq: ItemId) -> Self {
        let plain = || Self { project: project.clone(), stand_ins: HashMap::new() };
        let Some(s) = project.sequence(seq) else { return plain() };
        // the clips whose Lumetri effects are exactly their grade's (not foreign ones)
        let mut graded = Vec::new();
        for it in s.video_tracks.iter().flat_map(|t| &t.items) {
            let size = project.source_size(it.item).unwrap_or((s.settings.width, s.settings.height));
            let Some((g, false)) = crate::color::grade_in(project, seq, it.id, size) else { continue };
            if g.is_neutral() {
                continue;
            }
            let n = it.effects.iter().take_while(|e| e.effect == "lumetri").count();
            let others = it.effects[n..].iter().any(|e| e.enabled && e.def().is_some_and(|d| !d.intrinsic));
            graded.push((it.id, it.item, g, !others && !it.has_opacity_masks()));
        }
        if graded.is_empty() {
            return plain();
        }
        let mut p = (**project).clone();
        let mut stand_ins = HashMap::new();
        let mut swap = HashMap::new();
        for (k, (clip, item, grade, in_shader)) in graded.into_iter().enumerate() {
            let Some(mut copy) = p.item(item).cloned() else { continue };
            let id = ItemId(STAND_IN_IDS + k as u64);
            copy.id = id;
            p.items.insert(id, copy);
            stand_ins.insert(id, StandIn { real: item, clip, grade: Arc::new(grade), in_shader });
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
        Self { project: Arc::new(p), stand_ins }
    }

    /// The copy.
    pub(crate) fn project(&self) -> &Arc<Project> {
        &self.project
    }

    /// Registers every stand-in in `pool` as a graded source of its real media (opened through
    /// `sources`), so FilmCraft's own renders of the copy, an export's among them, get graded
    /// pictures (LOG D-045). `false`, registering nothing, when the copy needs nothing of ours or
    /// cannot have it: no stand-ins, no GPU, the GPU renderer off, or a graded subclip (the pool
    /// resolves a subclip to its media item before its own id, so a stand-in for one is never
    /// asked for).
    pub(crate) fn register_sources(&self, pool: &MediaPool, sources: &dyn SourceProvider) -> bool {
        if self.stand_ins.is_empty() || !GPU_ON.load(Ordering::Relaxed) || renderer().is_none() {
            return false;
        }
        let media = |s: &StandIn| matches!(self.project.item(s.real).map(|i| &i.kind), Some(ItemKind::Media(_)));
        if !self.stand_ins.values().all(media) {
            return false;
        }
        let mut opened = Vec::with_capacity(self.stand_ins.len());
        for (id, s) in &self.stand_ins {
            let Some(inner) = sources.source(s.real) else { return false };
            opened.push((*id, Arc::new(GradedSource { inner, grade: s.grade.clone() }) as SharedSource));
        }
        for (id, src) in opened {
            pool.insert(id, src);
        }
        true
    }

    /// The frame at `t` of timeline `seq` as FilmCraft's CPU renderer draws it from the copy, every
    /// graded clip's pictures graded at their source (LOG D-044). `None` when there is nothing to
    /// grade, no GPU, or the GPU renderer is off: then FilmCraft draws from the real project.
    pub(crate) fn render_cpu(
        &self,
        seq: ItemId,
        t: Tick,
        opts: RenderOptions,
        sources: &dyn SourceProvider,
    ) -> Option<Image> {
        if self.stand_ins.is_empty() || !GPU_ON.load(Ordering::Relaxed) {
            return None;
        }
        renderer()?;
        let graded = Grading { inner: sources, stand_ins: &self.stand_ins, seen: None };
        Some(render_sequence(&self.project, seq, t, opts, &graded))
    }

    /// The frame at `t` of timeline `seq`, drawn on the GPU, or `None` when the GPU renderer does
    /// not cover it (see the module notes) or there is no GPU. With `keep`, the frame is left on
    /// the GPU when the renderer shares the front end's device.
    pub(crate) fn render(
        &self,
        seq: ItemId,
        t: Tick,
        opts: RenderOptions,
        sources: &dyn SourceProvider,
        keep: bool,
    ) -> Option<Frame> {
        let out = if keep && SHARED.load(Ordering::Relaxed) { Out::Keep } else { Out::Pixels };
        let Drawn::Frame(frame) = self.drawn(seq, t, opts, sources, out)? else { return None };
        GPU_FRAMES.fetch_add(1, Ordering::Relaxed);
        Some(frame)
    }

    /// The frame at `t` for an agent (LOG D-044): as the viewers draw it, on the GPU or from the
    /// copy with graded sources, read back; `None` only where FilmCraft must draw from the real
    /// project (no GPU, or the GPU renderer turned off).
    pub(crate) fn snapshot(
        &self,
        seq: ItemId,
        t: Tick,
        opts: RenderOptions,
        sources: &dyn SourceProvider,
    ) -> Option<Frame> {
        if let Some(Drawn::Frame(f)) = self.drawn(seq, t, opts, sources, Out::Pixels) {
            return Some(f);
        }
        let img = self.render_cpu(seq, t, opts, sources)?;
        Some(Frame::new(img.w as u32, img.h as u32, img.over_black_rgba8()))
    }

    /// The frame at `t`, drawn on the GPU, as FilmCraft's CPU renderer hands frames to an export:
    /// linear light, premultiplied, not laid over black.
    fn render_linear(&self, seq: ItemId, t: Tick, opts: RenderOptions, sources: &dyn SourceProvider) -> Option<Image> {
        let Drawn::Linear(w, h, px) = self.drawn(seq, t, opts, sources, Out::Linear)? else { return None };
        Some(Image { w: w as usize, h: h as usize, px })
    }

    fn drawn(
        &self,
        seq: ItemId,
        t: Tick,
        opts: RenderOptions,
        sources: &dyn SourceProvider,
        out: Out,
    ) -> Option<Drawn> {
        if !GPU_ON.load(Ordering::Relaxed) {
            return None;
        }
        let gpu = renderer()?;
        // a cheap check first, so a frame the CPU will draw is not decoded twice
        if is_cpu_frame(&self.project, seq, t) {
            return None;
        }
        let seen = Seen::default();
        let tagging = Grading { inner: sources, stand_ins: &self.stand_ins, seen: Some(seen.clone()) };
        // Planned without draft playback's shortcut, which hands over shrunk copies of the decoded
        // pictures: a graded clip's layer is known by its picture.
        let plan = cancel::with_draft(false, || plan_frame(&self.project, seq, t, opts, &tagging));
        let seen = std::mem::take(&mut *seen.lock().ok()?);
        let (_, _, parts) = parts(&plan)?;
        let layers: Vec<&PlanLayer> = parts.iter().flat_map(Part::layers).collect();
        let grades = self.layer_grades(&layers, &seen)?;
        draw(gpu, &plan, &grades, out)
    }

    /// Each layer's grade: the grade of the clip whose decoded picture it draws, if that clip is
    /// graded. `None` when a graded clip's picture went somewhere a grade cannot follow (see the
    /// module notes).
    fn layer_grades<'a>(
        &'a self,
        layers: &[&PlanLayer],
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
                    Some(&*self.stand_ins.values().find(|s| s.clip == *clip)?.grade)
                }
                None => None,
            };
            out.push(grade);
        }
        (drawn.len() == clip_of.len()).then_some(out)
    }
}

/// A part of a frame plan the renderer draws as one [`Step`].
enum Part<'p> {
    Layers(&'p [PlanLayer]),
    Push { inputs: [&'p [PlanLayer]; 2], effect: &'p EffectInstance, progress: f32 },
}

impl<'p> Part<'p> {
    fn layers(&self) -> impl Iterator<Item = &'p PlanLayer> + use<'p> {
        let (a, b): (&'p [PlanLayer], &'p [PlanLayer]) = match *self {
            Part::Layers(ls) => (ls, &[]),
            Part::Push { inputs, .. } => (inputs[0], inputs[1]),
        };
        a.iter().chain(b)
    }
}

/// A plan's size and parts, or `None` for a plan the renderer does not draw: a CPU image, or a
/// transition other than Push.
fn parts(plan: &FramePlan) -> Option<(usize, usize, Vec<Part<'_>>)> {
    match plan {
        FramePlan::Layers { width, height, layers } => Some((*width, *height, vec![Part::Layers(layers)])),
        FramePlan::Composite { width, height, steps } => {
            let parts = steps
                .iter()
                .map(|s| match s {
                    PlanStep::Layers(ls) => Some(Part::Layers(ls)),
                    PlanStep::Transition { inputs, effect, progress, .. } => (effect.effect == "push")
                        .then_some(Part::Push { inputs: [&inputs[0], &inputs[1]], effect, progress: *progress }),
                })
                .collect::<Option<Vec<_>>>()?;
            Some((*width, *height, parts))
        }
        FramePlan::Image(_) => None,
    }
}

/// What a drawing is wanted as.
enum Out {
    /// 8-bit sRGB pixels.
    Pixels,
    /// Left on the GPU.
    Keep,
    /// Linear light, premultiplied, not laid over black.
    Linear,
}

enum Drawn {
    Frame(Frame),
    Linear(u32, u32, Vec<f32>),
}

/// Draws `plan` on the GPU, its layers (in plan order, Push sides included) with `grades`, as
/// `out` asks. `None` when a layer cannot be drawn there or the GPU failed.
fn draw(gpu: &Mutex<Renderer>, plan: &FramePlan, grades: &[Option<&Grade>], out: Out) -> Option<Drawn> {
    let (width, height, parts) = parts(plan)?;
    let all: Vec<&PlanLayer> = parts.iter().flat_map(Part::layers).collect();
    // standard effects the GPU does not run yet: on the CPU, layer by layer (with the layer's
    // colour management, which comes first)
    let images: Vec<_> =
        all.iter().map(|l| l.fx.as_ref().map(|fx| effect_image_with(&l.frame, fx, l.input.as_deref()))).collect();
    let mut layers = Vec::with_capacity(all.len());
    for ((l, grade), image) in all.iter().zip(grades).zip(&images) {
        let m = l.matrix;
        let matrix = [m.a, m.b, m.c, m.d, m.e, m.f];
        let blend = Blend::ALL.get(l.blend.index() as usize).copied()?;
        layers.push(match image {
            Some(img) => Layer {
                width: img.w as u32,
                height: img.h as u32,
                pixels: Pixels::RgbaF32(&img.px),
                coding: Coding { transfer: Transfer::Linear, ..Coding::REC709 },
                matrix,
                opacity: l.opacity,
                blend,
                managed: None,
                grade: None,
            },
            None => {
                let managed = match &l.input {
                    Some(t) => Some(managed(t)?),
                    None => None,
                };
                Layer { blend, managed, ..layer(&l.frame, matrix, l.opacity, *grade)? }
            }
        });
    }
    let mut next = layers.into_iter();
    let mut take = |n: usize| next.by_ref().take(n).collect::<Vec<_>>();
    let steps: Vec<Step<'_>> = parts
        .iter()
        .map(|p| match p {
            Part::Layers(ls) => Step::Layers(take(ls.len())),
            Part::Push { inputs, effect, progress } => {
                let (direction, blur) = push(effect);
                Step::Push {
                    inputs: [take(inputs[0].len()), take(inputs[1].len())],
                    progress: *progress,
                    direction,
                    blur,
                }
            }
        })
        .collect();
    let size = (width as u32, height as u32);
    let r = gpu.lock().ok()?;
    Some(match out {
        Out::Keep => {
            let picture = r.draw_steps(size, &steps);
            Drawn::Frame(Frame { width: size.0, height: size.1, rgba: Vec::new(), gpu: Some(Arc::new(OnGpu(picture))) })
        }
        Out::Pixels => Drawn::Frame(Frame::new(size.0, size.1, r.render_steps(size, &steps)?)),
        Out::Linear => Drawn::Linear(size.0, size.1, r.render_linear(size, &steps)?),
    })
}

/// An export's frame renderer (LOG D-042): one for each of the export's renderer slots. An export
/// of a graded copy (`deliver.render`, D-045) finds the copy among those [`exporting`]; any other
/// export makes a graded copy of its project when first asked.
struct ExportFrames {
    /// The project it was made for (by address) and its graded copy.
    graded: Option<(usize, Arc<Graded>)>,
}

/// Graded copies being exported, by their project's address.
static EXPORTING: Mutex<Vec<(usize, Weak<Graded>)>> = Mutex::new(Vec::new());

/// Notes that `graded`'s copy is being exported, for the export's [`ExportFrames`] to draw it with
/// its stand-ins rather than make a copy of the copy.
pub(crate) fn exporting(graded: &Arc<Graded>) {
    if let Ok(mut list) = EXPORTING.lock() {
        list.retain(|(_, g)| g.strong_count() > 0);
        list.push((Arc::as_ptr(&graded.project) as usize, Arc::downgrade(graded)));
    }
}

/// Registers [`ExportFrames`] as the frame renderer of FilmCraft's exports that ask for GPU
/// rendering (`gpuRendering: auto`, which `deliver.render` does unless told otherwise). Once is
/// enough; again is harmless.
pub(crate) fn register_export_renderer() {
    filmcraft_engine::export::register_frame_renderer(export_frames);
}

/// `deliver.render`'s parameters for FilmCraft's export: GPU rendering on unless they say
/// otherwise, or the GPU renderer is turned off ([`use_gpu_renderer`]).
pub(crate) fn deliver_params(mut params: serde_json::Value) -> serde_json::Value {
    if let Some(p) = params.as_object_mut()
        && !p.contains_key("gpuRendering")
    {
        let on = GPU_ON.load(Ordering::Relaxed);
        p.insert("gpuRendering".into(), serde_json::Value::from(if on { "auto" } else { "off" }));
    }
    params
}

fn export_frames() -> Option<Box<dyn FrameRenderer>> {
    renderer()?;
    Some(Box::new(ExportFrames { graded: None }))
}

impl FrameRenderer for ExportFrames {
    fn render(
        &mut self,
        project: &Project,
        seq: ItemId,
        t: Tick,
        opts: RenderOptions,
        sources: &dyn SourceProvider,
    ) -> Option<Image> {
        let id = project as *const Project as usize;
        if self.graded.as_ref().is_none_or(|(p, _)| *p != id) {
            let copy = EXPORTING
                .lock()
                .ok()
                .and_then(|list| list.iter().find(|(p, _)| *p == id).and_then(|(_, g)| g.upgrade()));
            let graded = copy.unwrap_or_else(|| Arc::new(Graded::of(&Arc::new(project.clone()), seq)));
            self.graded = Some((id, graded));
        }
        let (_, graded) = self.graded.as_ref()?;
        apart(|| graded.render_linear(seq, t, opts, sources))
    }
}

/// Runs `f`, an export frame's work, on a thread pool of its own, the export's worker waiting
/// for it without taking other work meanwhile (LOG B-038). FilmCraft's export draws its frames on
/// rayon's global pool, each frame taking one of a few frame renderers and waiting for one when
/// all are taken. A worker that holds a renderer and waits for parallel work of its own (decoding,
/// an effect) runs other queued work meanwhile, which can be another of the export's frames: that
/// frame waits for a renderer, among them the one held further up the same thread. Once every
/// renderer is held so, the export stops.
fn apart<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    static POOL: OnceLock<Option<rayon::ThreadPool>> = OnceLock::new();
    let pool =
        POOL.get_or_init(|| rayon::ThreadPoolBuilder::new().thread_name(|i| format!("export-frame-{i}")).build().ok());
    let Some(pool) = pool else { return f() };
    // a plain thread waiting on a plain join takes no other work
    std::thread::scope(|s| match s.spawn(|| pool.install(f)).join() {
        Ok(r) => r,
        // FilmCraft's export retires its renderers when one panics
        Err(panic) => std::panic::resume_unwind(panic),
    })
}

/// A Push transition's direction (0 down, 1 left, 2 up, 3 right) and motion blur (0 to 1), as
/// FilmCraft's compositor reads them.
fn push(effect: &EffectInstance) -> (u32, f32) {
    let value = effect
        .param("direction")
        .map(|p| p.value.clone())
        .or_else(|| effect.def().and_then(|d| d.param("direction")).map(|p| p.default.clone()));
    let direction = match value {
        Some(ParamValue::Choice(v)) => v,
        _ => 0,
    };
    let blur = effect.f64_at("motion_blur", Default::default()) as f32 / 100.0;
    (direction, if blur.is_finite() { blur.clamp(0.0, 1.0) } else { 0.0 })
}

/// A layer's colour management as the renderer runs it; `None` when it has no GPU form.
fn managed(t: &InputTransform) -> Option<Managed> {
    let p = t.gpu_params()?;
    Some(Managed {
        curve: match p.curve {
            GpuCurve::Sdr => Curve::Sdr,
            GpuCurve::Linear => Curve::Linear,
            GpuCurve::Pq => Curve::Pq,
            GpuCurve::Hlg => Curve::Hlg,
        },
        hlg: p.hlg,
        matrix: p.matrix,
        tone: p.tone,
        desaturate: p.desaturate,
        reference_white: REFERENCE_WHITE_NITS as f32,
    })
}

/// A source provider for a [`Graded`] copy: each stand-in opens its clip's real media. With
/// `seen` (the GPU path), a clip graded in the shader hands over its pictures as decoded, each
/// noted with its clip (kept, so their addresses stay theirs); every other stand-in hands over
/// its pictures graded ([`GradedSource`]).
struct Grading<'a> {
    inner: &'a dyn SourceProvider,
    stand_ins: &'a HashMap<ItemId, StandIn>,
    seen: Option<Seen>,
}

impl SourceProvider for Grading<'_> {
    fn source(&self, item: ItemId) -> Option<SharedSource> {
        let Some(s) = self.stand_ins.get(&item) else { return self.inner.source(item) };
        let inner = self.inner.source(s.real)?;
        Some(match &self.seen {
            Some(seen) if s.in_shader => Arc::new(Tagged { inner, clip: s.clip, seen: seen.clone() }),
            _ => Arc::new(GradedSource { inner, grade: s.grade.clone() }),
        })
    }
}

/// A graded clip's source: the real media's pictures, graded by the GPU renderer at the size they
/// are decoded at, as linear-light premultiplied pictures (LOG D-044). A picture the renderer
/// cannot grade (no GPU) comes as decoded.
struct GradedSource {
    inner: SharedSource,
    grade: Arc<Grade>,
}

impl MediaSource for GradedSource {
    fn info(&self) -> &MediaInfo {
        self.inner.info()
    }
    fn video_frame(&self, req: FrameRequest) -> filmcraft_media::Result<Arc<VideoFrame>> {
        let f = self.inner.video_frame(req)?;
        Ok(graded_picture(&f, &self.grade).map_or(f, Arc::new))
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

/// `f` graded by `grade` on the GPU, the same size, linear light and premultiplied.
fn graded_picture(f: &VideoFrame, grade: &Grade) -> Option<VideoFrame> {
    let gpu = renderer()?;
    let l = layer(f, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 1.0, Some(grade))?;
    let px = gpu.lock().ok()?.render_linear((f.width, f.height), &[Step::Layers(vec![l])])?;
    Some(VideoFrame { par: f.par, pts: f.pts, ..VideoFrame::rgba_f32(f.width, f.height, px) })
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
        filmcraft_color::Transfer::Pq => Transfer::Pq,
        filmcraft_color::Transfer::Hlg => Transfer::Hlg,
    };
    let (kr, kb) = f.color.matrix.kr_kb();
    // FilmCraft's frames do not say where their chroma sits: as H.264, HEVC and ProRes do by
    // default (LOG D-040)
    let coding =
        Coding { kr, kb, full_range: f.color.range == filmcraft_color::Range::Full, transfer, siting: Siting::Left };
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
    Some(Layer {
        width: f.width,
        height: f.height,
        pixels,
        coding,
        matrix,
        opacity,
        blend: Blend::Normal,
        managed: None,
        grade,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
