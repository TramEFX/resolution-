//! Resolution's GPU renderer (LOG D-035): draws a frame's layers on the GPU, each with its
//! Resolution grade applied by the grade's own math, and reads the frame back as 8-bit sRGB.
//!
//! A layer is a decoded picture (8-bit sRGB RGBA, linear float RGBA, or planar Y'CbCr of 8 to
//! 16 bits), how it is placed (an affine map from its pixels to the frame's), its opacity and its
//! blend mode. Layers are drawn bottom to top into a premultiplied linear accumulator, which is
//! resolved over black. The compositing follows FilmCraft's compositor (`filmcraft-gpu`; MIT or
//! Apache-2.0), whose shader code was ported (LOG D-036, D-038):
//!
//! - bilinear loads, Y'CbCr decoded per pixel, supersampled when shrinking;
//! - colour management of HDR and wide-gamut Y'CbCr ([`Managed`]);
//! - the 27 blend modes of [`Blend`]: Normal and Dissolve by fixed-function "over"; the others read
//!   the accumulator under the layer from a copy and composite with the W3C formulas on
//!   sRGB-encoded colour;
//! - the Push transition between two layer stacks drawn apart ([`Step::Push`]).
//!
//! **Grades** run per pixel as a layer is composited, as [`Grade::apply`] does: each node's tone
//! functions and hue curves (as [`TONE_N`]-point tables), its saturation step, its qualifier's key
//! taken on the node's input, and its windows. The key is a real per-pixel mask over the whole
//! node, not a baked LUT (D-032's goal). A layer whose qualifier cleans its key over the picture
//! (Denoise, Blur Radius) is graded first in a stage of its own, at its working size, as
//! [`Grade::apply_image`] does (LOG D-046).
//!
//! A frame is read back as 8-bit sRGB ([`Renderer::render_steps`]) or left on the GPU as a
//! [`Picture`] for a front end drawing on the same device ([`Renderer::draw_steps`], LOG D-039).
//!
//! This crate knows no engine: callers hand it pictures as plain slices.

use std::sync::{Arc, OnceLock};

/// The GPU library, for callers sharing a device with the renderer ([`Renderer::on`]).
pub use wgpu;

use resolution_grade::{Grade, Shape};

/// Samples of each tone function in the shader.
pub const TONE_N: usize = 4096;
/// Numbers per node in the shader's node table (`render.wgsl`).
const NODE_FLOATS: usize = 104;
/// Tables per node, of [`TONE_N`] samples each: the three tone functions, then the six hue
/// curves in `resolution_grade::HSL_CURVES`' order.
const TABLES: usize = 9;
/// Windows per node the shader takes.
pub const MAX_WINDOWS: usize = 8;
/// The most samples a side the shader takes over a shrunk pixel's footprint: up to 32× smaller,
/// every source pixel counts.
const MAX_TAPS: f32 = 32.0;
/// Numbers in the shader's per-layer uniform (`render.wgsl`'s `U`).
const UNIFORM_FLOATS: usize = 56;

/// How a layer's pixels are stored.
#[derive(Clone, Copy)]
pub enum Pixels<'a> {
    /// Straight-alpha, sRGB-encoded RGBA, 8 bits per channel.
    Rgba8(&'a [u8]),
    /// Premultiplied, linear-light RGBA.
    RgbaF32(&'a [f32]),
    /// Planar Y'CbCr, 8-bit; `chroma` is the planes' (horizontal, vertical) subsampling shifts.
    Yuv8 { planes: [&'a [u8]; 3], chroma: (u32, u32), alpha: Option<&'a [u8]> },
    /// Planar Y'CbCr in 16-bit words, `bits` of them significant.
    Yuv16 { planes: [&'a [u16]; 3], chroma: (u32, u32), bits: u32, alpha: Option<&'a [u16]> },
}

/// The transfer function of a Y'CbCr picture's code values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    /// The sRGB curve (also used for Rec. 709 video, as FilmCraft decodes it).
    Srgb,
    Linear,
    /// SMPTE ST 2084, to linear light where 1 is 100 cd/m².
    Pq,
    /// ARIB STD-B67 (HLG), to scene light.
    Hlg,
}

/// Where a subsampled picture's chroma samples sit across its luma samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Siting {
    /// Halfway between them (MPEG-1, JPEG).
    Center,
    /// On the left one of each pair (MPEG-2, H.264, HEVC and ProRes; vertically halfway).
    Left,
}

/// How a Y'CbCr picture's codes become RGB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coding {
    /// The matrix's red and blue luma weights (Rec. 709: 0.2126, 0.0722).
    pub kr: f32,
    pub kb: f32,
    /// Full range (0 to 2^bits − 1) instead of video range (16 to 235 in 8 bits).
    pub full_range: bool,
    pub transfer: Transfer,
    pub siting: Siting,
}

impl Coding {
    /// Rec. 709, video range, the sRGB curve, chroma sited left.
    pub const REC709: Coding =
        Coding { kr: 0.2126, kb: 0.0722, full_range: false, transfer: Transfer::Srgb, siting: Siting::Left };
}

/// How a layer combines with what is under it: the blend modes of Premiere (and FilmCraft), in
/// FilmCraft's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    Normal,
    Dissolve,
    Darken,
    Multiply,
    ColorBurn,
    LinearBurn,
    DarkerColor,
    Lighten,
    Screen,
    ColorDodge,
    LinearDodge,
    LighterColor,
    Overlay,
    SoftLight,
    HardLight,
    VividLight,
    LinearLight,
    PinLight,
    HardMix,
    Difference,
    Exclusion,
    Subtract,
    Divide,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl Blend {
    pub const ALL: [Blend; 27] = [
        Blend::Normal,
        Blend::Dissolve,
        Blend::Darken,
        Blend::Multiply,
        Blend::ColorBurn,
        Blend::LinearBurn,
        Blend::DarkerColor,
        Blend::Lighten,
        Blend::Screen,
        Blend::ColorDodge,
        Blend::LinearDodge,
        Blend::LighterColor,
        Blend::Overlay,
        Blend::SoftLight,
        Blend::HardLight,
        Blend::VividLight,
        Blend::LinearLight,
        Blend::PinLight,
        Blend::HardMix,
        Blend::Difference,
        Blend::Exclusion,
        Blend::Subtract,
        Blend::Divide,
        Blend::Hue,
        Blend::Saturation,
        Blend::Color,
        Blend::Luminosity,
    ];

    /// The mode's number in the shader (its place in [`Blend::ALL`]).
    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|b| *b == self).unwrap_or(0) as u32
    }

    /// Whether the mode needs the colour under the layer (all but Normal and Dissolve).
    pub fn reads_destination(self) -> bool {
        !matches!(self, Blend::Normal | Blend::Dissolve)
    }
}

/// The signal curve of colour-managed media.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    Sdr,
    Linear,
    /// To working units: cd/m² over the reference white.
    Pq,
    Hlg,
}

/// Colour management of an HDR or wide-gamut Y'CbCr picture into the working space, stage by stage
/// as FilmCraft's `InputTransform` runs them: the curve, HLG's OOTF, the gamut matrix, BT.2390
/// tone mapping, and desaturation into the working gamut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Managed {
    pub curve: Curve,
    /// HLG OOTF: luma weights and the scale to working units.
    pub hlg: Option<([f32; 3], f32)>,
    /// Into the working primaries, row-major.
    pub matrix: Option<[[f32; 3]; 3]>,
    /// Tone mapping (source peak, target peak), cd/m².
    pub tone: Option<(f32, f32)>,
    /// Luma weights to desaturate into the working gamut with.
    pub desaturate: Option<[f32; 3]>,
    /// The working space's reference white, cd/m².
    pub reference_white: f32,
}

/// One layer of a frame.
#[derive(Clone, Copy)]
pub struct Layer<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: Pixels<'a>,
    pub coding: Coding,
    /// The layer's pixels to the frame's: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`, as
    /// `[a, b, c, d, e, f]`.
    pub matrix: [f64; 6],
    pub opacity: f32,
    pub blend: Blend,
    /// Colour management of a Y'CbCr picture beyond its [`Coding`].
    pub managed: Option<Managed>,
    /// The layer's grade, if it has one.
    pub grade: Option<&'a Grade>,
    /// Shown in the Highlight view of one of its grade's nodes instead (LOG D-048).
    pub highlight: Option<Highlight>,
}

/// The Highlight view of a layer (LOG D-048), as `resolution_grade::Grade::highlight_image`
/// shows it: node `node` (its index in the grade's nodes) keys the graded picture against flat
/// grey, or with `black_white` shows its matte in black and white.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Highlight {
    pub node: usize,
    pub black_white: bool,
}

/// A part of a frame, drawn onto what the parts before it drew.
#[derive(Clone)]
pub enum Step<'a> {
    /// Layers, bottom to top.
    Layers(Vec<Layer<'a>>),
    /// The Push transition: each side's layers drawn on their own, pushed together by `progress`
    /// (0 to 1) towards `direction` (0 down, 1 left, 2 up, 3 right), with motion blur `blur` (0 to
    /// 1), the result laid over the frame so far.
    Push { inputs: [Vec<Layer<'a>>; 2], progress: f32, direction: u32, blur: f32 },
}

const ACCUM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// sRGB-encoded colour in a plain 8-bit texture (encoded by the resolve shader): what egui's
/// renderer takes as a native texture.
const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A frame left on the GPU: 8-bit sRGB-encoded RGBA, opaque.
pub struct Picture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
    device: wgpu::Device,
    queue: wgpu::Queue,
    pixels: OnceLock<Option<Vec<u8>>>,
}

impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Picture({}x{})", self.size.0, self.size.1)
    }
}

impl Picture {
    /// The texture (format `Rgba8Unorm`, sRGB-encoded colour), to draw.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The pixels, straight 8-bit sRGB RGBA, read back from the GPU the first time they are
    /// asked for (waiting for it). `None` when the GPU could not be read.
    pub fn read(&self) -> Option<&[u8]> {
        self.pixels.get_or_init(|| self.read_back()).as_deref()
    }

    fn read_back(&self) -> Option<Vec<u8>> {
        let (w, h) = self.size;
        let row = (w * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(row * h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        // a healthy map takes milliseconds; a stuck driver must not hang playback
        self.device
            .poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(8)) })
            .ok()?;
        rx.recv().ok()?.ok()?;
        let data = slice.get_mapped_range().ok()?;
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            out.extend_from_slice(&data[(y * row) as usize..(y * row + w * 4) as usize]);
        }
        drop(data);
        buf.unmap();
        Some(out)
    }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layer_pipeline: wgpu::RenderPipeline,
    blend_pipeline: wgpu::RenderPipeline,
    resolve_pipeline: wgpu::RenderPipeline,
    push_pipeline: wgpu::RenderPipeline,
    layer_bgl: wgpu::BindGroupLayout,
    resolve_bgl: wgpu::BindGroupLayout,
    push_bgl: wgpu::BindGroupLayout,
    stage: StagePipelines,
    dummy: wgpu::TextureView,
}

/// The grade stage's passes (LOG D-046; `render.wgsl`).
struct StagePipelines {
    signal: wgpu::RenderPipeline,
    key: wgpu::RenderPipeline,
    median: wgpu::RenderPipeline,
    clean: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    node: wgpu::RenderPipeline,
    highlight: wgpu::RenderPipeline,
}

/// A texture drawn into.
struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

fn tex_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn buffer_entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

impl Renderer {
    /// A renderer on the system's GPU; `None` without one (a headless machine, no driver).
    pub fn new() -> Option<Self> {
        if wgpu::Instance::enabled_backend_features().is_empty() {
            return None;
        }
        let instance = wgpu::Instance::default();
        let options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        };
        let adapter = pollster::block_on(instance.request_adapter(&options)).ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        // wgpu's default handler panics on a validation or out-of-memory error: report it instead
        device.on_uncaptured_error(Arc::new(|e: wgpu::Error| eprintln!("GPU renderer: {e}")));
        Some(Self::on(device, queue))
    }

    /// A renderer on an existing device.
    pub fn on(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("resolution-render"),
            source: wgpu::ShaderSource::Wgsl(include_str!("render.wgsl").into()),
        });
        let push_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("resolution-push"),
            source: wgpu::ShaderSource::Wgsl(include_str!("push.wgsl").into()),
        });
        let read_only = wgpu::BufferBindingType::Storage { read_only: true };
        let layer_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("layer"),
            entries: &[
                buffer_entry(0, wgpu::BufferBindingType::Uniform),
                tex_entry(1),
                tex_entry(2),
                tex_entry(3),
                tex_entry(4),
                buffer_entry(5, read_only),
                buffer_entry(6, read_only),
                tex_entry(7),
            ],
        });
        let resolve_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("resolve"),
            entries: &[tex_entry(0)],
        });
        let push_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("push"),
            entries: &[tex_entry(0), tex_entry(1), buffer_entry(2, wgpu::BufferBindingType::Uniform)],
        });
        let pipeline = |label, module, bgl: &wgpu::BindGroupLayout, vs, fs, format, blend| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(bgl)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let over = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let layer_pipeline = pipeline("layer", &shader, &layer_bgl, "vs", "fs", ACCUM_FORMAT, over);
        let blend_pipeline = pipeline("blend", &shader, &layer_bgl, "vs", "fs_blend", ACCUM_FORMAT, None);
        let resolve_pipeline = pipeline("resolve", &shader, &resolve_bgl, "vs_full", "fs_full", OUTPUT_FORMAT, None);
        let push_pipeline = pipeline("push", &push_shader, &push_bgl, "vs", "fs", ACCUM_FORMAT, over);
        let full = |label, fs| pipeline(label, &shader, &layer_bgl, "vs_full", fs, ACCUM_FORMAT, None);
        let stage = StagePipelines {
            signal: pipeline("signal", &shader, &layer_bgl, "vs", "fs_signal", ACCUM_FORMAT, None),
            key: full("key", "fs_key"),
            median: full("median", "fs_median"),
            clean: full("clean", "fs_clean"),
            blur: full("blur", "fs_blur"),
            node: full("node", "fs_node"),
            highlight: full("highlight", "fs_highlight"),
        };
        let dummy = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("dummy"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        Self {
            device,
            queue,
            layer_pipeline,
            blend_pipeline,
            resolve_pipeline,
            push_pipeline,
            layer_bgl,
            resolve_bgl,
            push_bgl,
            stage,
            dummy,
        }
    }

    fn texture(&self, w: u32, h: u32, format: wgpu::TextureFormat, bytes: &[u8], bpp: u32) -> wgpu::TextureView {
        let t = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("plane"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &t,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * bpp), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        t.create_view(&Default::default())
    }

    fn target(&self, (w, h): (u32, u32), format: wgpu::TextureFormat, usage: wgpu::TextureUsages) -> Target {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("target"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Target { texture, view }
    }

    fn buffer(&self, usage: wgpu::BufferUsages, data: &[f32]) -> wgpu::Buffer {
        // storage buffers may not be empty
        let data = if data.is_empty() { &[0.0f32; 4][..] } else { data };
        let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("data"),
            size: bytes.len() as u64,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&buf, 0, &bytes);
        buf
    }

    /// The textures of a layer, and the shader's numbers for them: kind, code scale, alpha-plane
    /// scale, chroma plane size.
    fn upload(&self, l: &Layer<'_>) -> ([wgpu::TextureView; 4], f32, f32, f32, (u32, u32)) {
        let (w, h) = (l.width, l.height);
        let d = || self.dummy.clone();
        let half = |v: &[u16], scale: f32| -> Vec<u8> {
            v.iter().flat_map(|c| half::f16::from_f32(f32::from(*c) / scale).to_bits().to_le_bytes()).collect()
        };
        match &l.pixels {
            Pixels::Rgba8(px) => {
                ([self.texture(w, h, wgpu::TextureFormat::Rgba8Unorm, px, 4), d(), d(), d()], 0.0, 1.0, 0.0, (w, h))
            }
            Pixels::RgbaF32(px) => {
                let bytes: Vec<u8> = px.iter().flat_map(|v| v.to_le_bytes()).collect();
                (
                    [self.texture(w, h, wgpu::TextureFormat::Rgba32Float, &bytes, 16), d(), d(), d()],
                    1.0,
                    1.0,
                    0.0,
                    (w, h),
                )
            }
            Pixels::Yuv8 { planes, chroma, alpha } => {
                let (cw, ch) = (w.div_ceil(1 << chroma.0), h.div_ceil(1 << chroma.1));
                let f = wgpu::TextureFormat::R8Unorm;
                let a = alpha.map(|a| self.texture(w, h, f, a, 1));
                let alpha_scale = if a.is_some() { 1.0 } else { 0.0 };
                (
                    [
                        self.texture(w, h, f, planes[0], 1),
                        self.texture(cw, ch, f, planes[1], 1),
                        self.texture(cw, ch, f, planes[2], 1),
                        a.unwrap_or_else(d),
                    ],
                    2.0,
                    255.0,
                    alpha_scale,
                    (cw, ch),
                )
            }
            Pixels::Yuv16 { planes, chroma, bits, alpha } => {
                let (cw, ch) = (w.div_ceil(1 << chroma.0), h.div_ceil(1 << chroma.1));
                let scale = (1u32 << bits) as f32;
                let f = wgpu::TextureFormat::R16Float;
                let a = alpha.map(|a| self.texture(w, h, f, &half(a, scale), 2));
                // the alpha plane's codes span 0 to 2^bits − 1
                let alpha_scale = if a.is_some() { scale / (scale - 1.0) } else { 0.0 };
                (
                    [
                        self.texture(w, h, f, &half(planes[0], scale), 2),
                        self.texture(cw, ch, f, &half(planes[1], scale), 2),
                        self.texture(cw, ch, f, &half(planes[2], scale), 2),
                        a.unwrap_or_else(d),
                    ],
                    2.0,
                    scale,
                    alpha_scale,
                    (cw, ch),
                )
            }
        }
    }

    /// The frame of `size` pixels: `layers` drawn bottom to top over black, as straight 8-bit
    /// sRGB RGBA (opaque). `None` when the GPU could not finish it.
    pub fn render(&self, size: (u32, u32), layers: &[Layer<'_>]) -> Option<Vec<u8>> {
        self.render_steps(size, &[Step::Layers(layers.to_vec())])
    }

    /// The frame of `size` pixels: `steps` drawn in order over black, as straight 8-bit sRGB RGBA
    /// (opaque). `None` when the GPU could not finish it.
    pub fn render_steps(&self, size: (u32, u32), steps: &[Step<'_>]) -> Option<Vec<u8>> {
        let picture = self.draw_steps(size, steps);
        picture.read().map(<[u8]>::to_vec)
    }

    /// [`Renderer::render_steps`] left on the GPU: the commands are submitted, not waited for.
    /// Anything drawing the picture later on the same device sees it finished.
    pub fn draw_steps(&self, size: (u32, u32), steps: &[Step<'_>]) -> Picture {
        let (w, h) = (size.0.max(1), size.1.max(1));
        let mut enc = self.device.create_command_encoder(&Default::default());
        let (canvas, kept) = self.compose(&mut enc, (w, h), steps);
        let drawn = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let output = self.target((w, h), OUTPUT_FORMAT, drawn | wgpu::TextureUsages::COPY_SRC);
        let resolve = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("resolve"),
            layout: &self.resolve_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&canvas.view) }],
        });
        {
            let mut p = pass(&mut enc, &output.view, wgpu::LoadOp::Clear(wgpu::Color::BLACK));
            p.set_pipeline(&self.resolve_pipeline);
            p.set_bind_group(0, &resolve, &[]);
            p.draw(0..3, 0..1);
        }
        self.queue.submit([enc.finish()]);
        drop(kept);
        Picture {
            texture: output.texture,
            view: output.view,
            size: (w, h),
            device: self.device.clone(),
            queue: self.queue.clone(),
            pixels: OnceLock::new(),
        }
    }

    /// The frame of `size` pixels as `steps` composite it, before it is laid over black: linear
    /// light, premultiplied RGBA, four floats a pixel (as FilmCraft's CPU renderer hands an
    /// export its frames, LOG D-042). `None` when the GPU could not finish it.
    pub fn render_linear(&self, size: (u32, u32), steps: &[Step<'_>]) -> Option<Vec<f32>> {
        let (w, h) = (size.0.max(1), size.1.max(1));
        let mut enc = self.device.create_command_encoder(&Default::default());
        let (canvas, kept) = self.compose(&mut enc, (w, h), steps);
        // half floats, 8 bytes a pixel
        let row = (w * 8).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(row * h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &canvas.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        drop(kept);
        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(8)) })
            .ok()?;
        rx.recv().ok()?.ok()?;
        let data = slice.get_mapped_range().ok()?;
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let line = &data[(y * row) as usize..(y * row + w * 8) as usize];
            out.extend(line.as_chunks::<2>().0.iter().map(|b| half::f16::from_bits(u16::from_le_bytes(*b)).to_f32()));
        }
        drop(data);
        buf.unmap();
        Some(out)
    }

    /// Records `steps` drawn in order onto a new canvas (premultiplied linear light over
    /// transparent). The resources the commands use come back with it, to keep until they are
    /// submitted.
    fn compose(
        &self,
        enc: &mut wgpu::CommandEncoder,
        (w, h): (u32, u32),
        steps: &[Step<'_>],
    ) -> (Target, (Vec<wgpu::BindGroup>, Vec<Target>)) {
        let drawn = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let canvas = self.target((w, h), ACCUM_FORMAT, drawn | wgpu::TextureUsages::COPY_SRC);
        let layers = |s: &Step<'_>| -> Vec<Blend> {
            match s {
                Step::Layers(ls) => ls.iter().map(|l| l.blend).collect(),
                Step::Push { inputs, .. } => inputs.iter().flatten().map(|l| l.blend).collect(),
            }
        };
        let backdrop = steps.iter().flat_map(layers).any(Blend::reads_destination).then(|| {
            self.target((w, h), ACCUM_FORMAT, wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST)
        });
        // everything the passes use is kept until the commands are submitted
        let mut keep: Vec<wgpu::BindGroup> = Vec::new();
        let mut inputs_kept: Vec<Target> = Vec::new();
        clear(enc, &canvas.view);
        for step in steps {
            match step {
                Step::Layers(ls) => self.draw(enc, &canvas, backdrop.as_ref(), ls, (w, h), &mut keep),
                Step::Push { inputs, progress, direction, blur } => {
                    let sides = [0, 1].map(|i| {
                        let t = self.target((w, h), ACCUM_FORMAT, drawn | wgpu::TextureUsages::COPY_SRC);
                        clear(enc, &t.view);
                        self.draw(enc, &t, backdrop.as_ref(), &inputs[i], (w, h), &mut keep);
                        t
                    });
                    let p = if progress.is_finite() { progress.clamp(0.0, 1.0) } else { 0.0 };
                    let b = if blur.is_finite() { blur.clamp(0.0, 1.0) } else { 0.0 };
                    let ub = self.buffer(
                        wgpu::BufferUsages::UNIFORM,
                        &[w as f32, h as f32, p, b, *direction as f32, 0.0, 0.0, 0.0],
                    );
                    let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("push"),
                        layout: &self.push_bgl,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&sides[0].view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(&sides[1].view),
                            },
                            wgpu::BindGroupEntry { binding: 2, resource: ub.as_entire_binding() },
                        ],
                    });
                    {
                        let mut pass = pass(enc, &canvas.view, wgpu::LoadOp::Load);
                        pass.set_pipeline(&self.push_pipeline);
                        pass.set_bind_group(0, &group, &[]);
                        pass.draw(0..3, 0..1);
                    }
                    keep.push(group);
                    inputs_kept.extend(sides);
                }
            }
        }
        (canvas, (keep, inputs_kept))
    }

    /// Records `layers` drawn onto `target`, bottom to top. A layer whose blend mode reads the
    /// colour under it first has that region copied into `backdrop`.
    fn draw(
        &self,
        enc: &mut wgpu::CommandEncoder,
        target: &Target,
        backdrop: Option<&Target>,
        layers: &[Layer<'_>],
        out: (u32, u32),
        keep: &mut Vec<wgpu::BindGroup>,
    ) {
        for l in layers {
            let Some(bounds) = quad_bounds(l, out) else { continue };
            let reads = l.blend.reads_destination().then_some(backdrop).flatten();
            // a grade cleaning its keys over the picture is applied first, at a working size
            let staged = self.stage(enc, l, keep);
            let (graded, l) = match &staged {
                Some((_, size)) => {
                    let [a, b, c, d, e, f] = l.matrix;
                    let (kx, ky) = (f64::from(l.width) / f64::from(size.0), f64::from(l.height) / f64::from(size.1));
                    let sl = Layer {
                        width: size.0,
                        height: size.1,
                        matrix: [a * kx, b * kx, c * ky, d * ky, e, f],
                        grade: None,
                        highlight: None,
                        managed: None,
                        ..*l
                    };
                    (true, sl)
                }
                None => (false, *l),
            };
            let l = &l;
            let (views, kind, code_scale, alpha_scale, chroma) = match &staged {
                // straight, display-encoded colour: drawn as an 8-bit sRGB picture is
                Some((t, size)) => {
                    let d = || self.dummy.clone();
                    ([t.view.clone(), d(), d(), d()], 0.0, 1.0, 0.0, *size)
                }
                None => self.upload(l),
            };
            let (node_data, tone_data, count) =
                if graded { (Vec::new(), Vec::new(), 0) } else { grade_data(l.grade, (l.width, l.height), None) };
            let uniforms = uniforms(l, Decoding { kind, code_scale, alpha_scale, chroma }, out, count, [0.0; 4]);
            let ub = self.buffer(wgpu::BufferUsages::UNIFORM, &uniforms);
            let nb = self.buffer(wgpu::BufferUsages::STORAGE, &node_data);
            let tb = self.buffer(wgpu::BufferUsages::STORAGE, &tone_data);
            let under = reads.map_or(&self.dummy, |b| &b.view);
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("layer"),
                layout: &self.layer_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: ub.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&views[0]) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&views[1]) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&views[2]) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&views[3]) },
                    wgpu::BindGroupEntry { binding: 5, resource: nb.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 6, resource: tb.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(under) },
                ],
            });
            if let Some(b) = reads {
                let (x, y, cw, ch) = bounds;
                let origin = wgpu::Origin3d { x, y, z: 0 };
                enc.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &target.texture,
                        mip_level: 0,
                        origin,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: &b.texture,
                        mip_level: 0,
                        origin,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d { width: cw, height: ch, depth_or_array_layers: 1 },
                );
            }
            {
                let mut p = pass(enc, &target.view, wgpu::LoadOp::Load);
                p.set_pipeline(if reads.is_some() { &self.blend_pipeline } else { &self.layer_pipeline });
                p.set_bind_group(0, &group, &[]);
                p.draw(0..6, 0..1);
            }
            keep.push(group);
        }
    }

    /// Records the grade stage of `l` (LOG D-046), when its grade has a node whose qualifier cleans
    /// its key over the picture (Denoise, Blur Radius), or the layer is shown in the Highlight
    /// view (D-048): the layer drawn at its working size (as big as it shows, at most its own size)
    /// as straight, display-encoded colour, then graded node by node, each node's matte made in a
    /// mask first where it needs one (and the highlighted node's, kept for the last pass). The
    /// graded texture and its size; `None` for a layer graded as it is composited.
    fn stage(
        &self,
        enc: &mut wgpu::CommandEncoder,
        l: &Layer<'_>,
        keep: &mut Vec<wgpu::BindGroup>,
    ) -> Option<(Target, (u32, u32))> {
        let grade = l.grade?;
        let highlight = l.highlight.filter(|h| grade.nodes.get(h.node).is_some_and(|n| n.enabled));
        let active = active_nodes(grade, highlight.map(|h| h.node));
        // the highlighted node among those drawn
        let lit = highlight.and_then(|h| active.iter().position(|(i, _)| *i == h.node));
        if lit.is_none() && !active.iter().any(|(_, n)| n.keyed().is_some_and(resolution_grade::Qualifier::is_spatial))
        {
            return None;
        }
        let [a, b, c, d, ..] = l.matrix;
        let shown = ((a * a + b * b).sqrt().min((c * c + d * d).sqrt())).min(1.0);
        let ww = ((f64::from(l.width) * shown).ceil() as u32).clamp(1, l.width.max(1));
        let wh = ((f64::from(l.height) * f64::from(ww) / f64::from(l.width.max(1))).round() as u32).max(1);
        let size = (ww, wh);
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let mut image = self.target(size, ACCUM_FORMAT, usage);
        let mut next = self.target(size, ACCUM_FORMAT, usage);
        let mut mask = self.target(size, ACCUM_FORMAT, usage);
        let mut spare = self.target(size, ACCUM_FORMAT, usage);
        let (node_data, tone_data, count) = grade_data(Some(grade), (l.width, l.height), highlight.map(|h| h.node));
        let nb = self.buffer(wgpu::BufferUsages::STORAGE, &node_data);
        let tb = self.buffer(wgpu::BufferUsages::STORAGE, &tone_data);
        let (views, kind, code_scale, alpha_scale, chroma) = self.upload(l);
        // one pass: `pipeline` drawing into `into` with the stage's numbers `g0`, reading `t0`, `t1`
        let mut run = |enc: &mut wgpu::CommandEncoder,
                       pipeline: &wgpu::RenderPipeline,
                       into: &Target,
                       t: [&wgpu::TextureView; 4],
                       u: Vec<f32>,
                       vertices: u32| {
            let ub = self.buffer(wgpu::BufferUsages::UNIFORM, &u);
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("stage"),
                layout: &self.layer_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: ub.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(t[0]) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(t[1]) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(t[2]) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(t[3]) },
                    wgpu::BindGroupEntry { binding: 5, resource: nb.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 6, resource: tb.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&self.dummy) },
                ],
            });
            {
                let mut p = pass(enc, &into.view, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT));
                p.set_pipeline(pipeline);
                p.set_bind_group(0, &group, &[]);
                p.draw(0..vertices, 0..1);
            }
            keep.push(group);
        };
        // the layer at the working size
        let at_size = Layer {
            matrix: [f64::from(ww) / f64::from(l.width), 0.0, 0.0, f64::from(wh) / f64::from(l.height), 0.0, 0.0],
            opacity: 1.0,
            ..*l
        };
        let u = |g0: [f32; 4]| uniforms(l, Decoding { kind, code_scale, alpha_scale, chroma }, size, count, g0);
        run(
            enc,
            &self.stage.signal,
            &image,
            [&views[0], &views[1], &views[2], &views[3]],
            uniforms(&at_size, Decoding { kind, code_scale, alpha_scale, chroma }, size, 0, [0.0; 4]),
            6,
        );
        let dummy = &self.dummy;
        let mut held = None;
        for (i, (_, n)) in active.iter().enumerate() {
            let node = i as f32;
            // a matte made over the picture: one that needs neighbours, or the highlighted node's
            let spatial = n.keyed().filter(|q| q.is_spatial() || lit == Some(i));
            if let Some(q) = spatial {
                run(enc, &self.stage.key, &mask, [&image.view, dummy, dummy, dummy], u([node, 0.0, 0.0, 0.0]), 3);
                for _ in 0..q.denoise_passes(ww as usize) {
                    for axis in [0.0, 1.0] {
                        run(
                            enc,
                            &self.stage.median,
                            &spare,
                            [dummy, &mask.view, dummy, dummy],
                            u([node, axis, 0.0, 0.0]),
                            3,
                        );
                        std::mem::swap(&mut mask, &mut spare);
                    }
                }
                run(enc, &self.stage.clean, &spare, [dummy, &mask.view, dummy, dummy], u([node, 0.0, 0.0, 0.0]), 3);
                std::mem::swap(&mut mask, &mut spare);
                let sigma = q.blur_sigma(ww as usize);
                if sigma > 0.0 {
                    for axis in [0.0, 1.0] {
                        run(
                            enc,
                            &self.stage.blur,
                            &spare,
                            [dummy, &mask.view, dummy, dummy],
                            u([node, axis, sigma, 0.0]),
                            3,
                        );
                        std::mem::swap(&mut mask, &mut spare);
                    }
                }
            }
            let from_mask = if spatial.is_some() { 1.0 } else { 0.0 };
            let m = if spatial.is_some() { &mask.view } else { dummy };
            run(enc, &self.stage.node, &next, [&image.view, m, dummy, dummy], u([node, 0.0, 0.0, from_mask]), 3);
            std::mem::swap(&mut image, &mut next);
            if lit == Some(i) && spatial.is_some() {
                held = Some(std::mem::replace(&mut mask, self.target(size, ACCUM_FORMAT, usage)));
            }
        }
        if let (Some(at), Some(h)) = (lit, highlight) {
            let m = held.as_ref().map_or(dummy, |t: &Target| &t.view);
            let g0 = [
                at as f32,
                if h.black_white { 1.0 } else { 0.0 },
                if held.is_some() { 1.0 } else { 0.0 },
                resolution_grade::HIGHLIGHT_GRAY,
            ];
            run(enc, &self.stage.highlight, &next, [&image.view, m, dummy, dummy], u(g0), 3);
            std::mem::swap(&mut image, &mut next);
        }
        Some((image, size))
    }
}

fn pass(
    enc: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'static> {
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
    .forget_lifetime()
}

fn clear(enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
    drop(pass(enc, view, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)));
}

/// The frame pixels a layer's quad can touch, as (x, y, width, height), with a pixel's margin;
/// `None` when it touches none. A matrix that is not finite covers the frame.
fn quad_bounds(l: &Layer<'_>, (w, h): (u32, u32)) -> Option<(u32, u32, u32, u32)> {
    let [a, b, c, d, e, f] = l.matrix;
    let (lw, lh) = (f64::from(l.width), f64::from(l.height));
    let pts = [(0.0, 0.0), (lw, 0.0), (0.0, lh), (lw, lh)].map(|(x, y)| (a * x + c * y + e, b * x + d * y + f));
    if pts.iter().any(|p| !p.0.is_finite() || !p.1.is_finite()) {
        return Some((0, 0, w, h));
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (x, y) in pts {
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    let px = |v: f64, hi: u32| v.clamp(0.0, f64::from(hi)) as u32;
    let (x0, y0) = (px(x0.floor() - 1.0, w), px(y0.floor() - 1.0, h));
    let (x1, y1) = (px(x1.ceil() + 1.0, w), px(y1.ceil() + 1.0, h));
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1 - x0, y1 - y0))
}

/// The shader's numbers for a layer (`render.wgsl`'s `U`).
/// How the shader reads a layer's textures: its kind, the scale its codes come at, its alpha
/// plane's scale, and its chroma planes' size (what [`Renderer::upload`] says of them).
#[derive(Clone, Copy)]
struct Decoding {
    kind: f32,
    code_scale: f32,
    alpha_scale: f32,
    chroma: (u32, u32),
}

fn uniforms(l: &Layer<'_>, src: Decoding, out: (u32, u32), nodes: usize, stage: [f32; 4]) -> Vec<f32> {
    let Decoding { kind, code_scale, alpha_scale, chroma } = src;
    let [a, b, c, d, e, f] = l.matrix;
    // video range in code units of the picture's own bit depth: 2^bits levels, whatever scale the
    // texture holds the codes at (8-bit codes come back as code / 255; LOG B-035)
    let levels = match l.pixels {
        Pixels::Yuv16 { bits, .. } => (1u32 << bits) as f32,
        _ => 256.0,
    };
    let bits_scale = levels / 256.0;
    let (yo, ys, co, cs) = match (kind == 2.0, l.coding.full_range) {
        (true, false) => (16.0 * bits_scale, 219.0 * bits_scale, 128.0 * bits_scale, 224.0 * bits_scale),
        (true, true) => (0.0, levels - 1.0, levels / 2.0, levels - 1.0),
        _ => (0.0, 1.0, 0.0, 1.0),
    };
    // source pixels per frame pixel: supersample when shrinking, at least one sample per source
    // pixel across the footprint (a shrunk picture averages all its pixels, LOG D-040). The work
    // is about one sample per source pixel whatever the size, up to MAX_TAPS a side.
    let sx = (a * a + b * b).sqrt();
    let sy = (c * c + d * d).sqrt();
    let footprint = (1.0 / sx.min(sy).max(1e-6)) as f32;
    let taps = if footprint > 1.25 { footprint.ceil().min(MAX_TAPS) } else { 1.0 };
    let transfer = match l.coding.transfer {
        Transfer::Srgb => 0.0,
        Transfer::Linear => 1.0,
        Transfer::Pq => 2.0,
        Transfer::Hlg => 3.0,
    };
    let mut u = vec![
        a as f32,
        b as f32,
        c as f32,
        d as f32,
        e as f32,
        f as f32,
        out.0 as f32,
        out.1 as f32,
        l.width as f32,
        l.height as f32,
        chroma.0 as f32,
        chroma.1 as f32,
        l.opacity,
        kind,
        taps,
        transfer,
        yo,
        ys,
        co,
        cs,
        l.coding.kr,
        l.coding.kb,
        code_scale,
        footprint.max(1.0),
        alpha_scale,
        nodes as f32,
        l.blend.index() as f32,
        if l.coding.siting == Siting::Left { 1.0 } else { 0.0 },
    ];
    // colour management (q0 to q5), only for Y'CbCr
    let m = l.managed.filter(|_| kind == 2.0);
    u.extend(m.map_or([0.0; 24], |m| {
        let curve = match m.curve {
            Curve::Sdr => 0.0,
            Curve::Linear => 1.0,
            Curve::Pq => 2.0,
            Curve::Hlg => 3.0,
        };
        let on = |v: bool| if v { 1.0 } else { 0.0 };
        let (hl, hk) = m.hlg.unwrap_or(([0.0; 3], 0.0));
        let mx = m.matrix.unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        let (sp, dp) = m.tone.unwrap_or((0.0, 0.0));
        let dl = m.desaturate.unwrap_or([0.0; 3]);
        #[rustfmt::skip]
        let q = [
            1.0, curve, hk, m.reference_white,
            hl[0], hl[1], hl[2], on(m.matrix.is_some()),
            mx[0][0], mx[0][1], mx[0][2], on(m.tone.is_some()),
            mx[1][0], mx[1][1], mx[1][2], sp,
            mx[2][0], mx[2][1], mx[2][2], dp,
            dl[0], dl[1], dl[2], on(m.desaturate.is_some()),
        ];
        q
    }));
    u.extend(stage);
    debug_assert_eq!(u.len(), UNIFORM_FLOATS);
    u
}

/// The nodes the shader draws, with their indices in the grade: the enabled ones that change
/// something, and node `keep` if it is enabled (the highlighted node, whose matte shows even
/// before it corrects anything).
fn active_nodes(grade: &Grade, keep: Option<usize>) -> Vec<(usize, &resolution_grade::Node)> {
    grade.nodes.iter().enumerate().filter(|(i, n)| n.enabled && (!n.is_neutral() || keep == Some(*i))).collect()
}

/// A grade as the shader reads it: [`NODE_FLOATS`] numbers per enabled node (tone on, the
/// saturation step, the qualifier, which hue curves are on, the windows in the picture's pixels),
/// and the nodes' tables ([`TABLES`] each); and how many nodes. Windows beyond [`MAX_WINDOWS`] are
/// left out.
pub fn grade_data(grade: Option<&Grade>, (w, h): (u32, u32), keep: Option<usize>) -> (Vec<f32>, Vec<f32>, usize) {
    let Some(grade) = grade else { return (Vec::new(), Vec::new(), 0) };
    let (w, h) = (w as f32, h as f32);
    let active: Vec<_> = active_nodes(grade, keep).into_iter().map(|(_, n)| n).collect();
    let mut nodes = Vec::with_capacity(active.len() * NODE_FLOATS);
    let mut tones = Vec::with_capacity(active.len() * TABLES * TONE_N);
    // polygon windows' corners, in the picture's pixels, after every node's tables: each its
    // corner count, then x and y for each corner (LOG D-053)
    let after_tables = active.len() * TABLES * TONE_N;
    let mut corners: Vec<f32> = Vec::new();
    for n in &active {
        let mut b = [0f32; NODE_FLOATS];
        let s = n.saturation();
        b[0] = if n.tone_is_identity() { 0.0 } else { 1.0 };
        b[1] = s.amount;
        b[2] = s.boost;
        b[3] = s.hue_turns;
        if let Some(q) = n.keyed() {
            let on = |v: bool| if v { 1.0 } else { 0.0 };
            b[4] = 1.0;
            b[5] = on(q.invert);
            b[6..20].copy_from_slice(&[
                on(q.hue_on),
                q.hue_center,
                q.hue_width,
                q.hue_soft,
                on(q.sat_on),
                q.sat_low,
                q.sat_high,
                q.sat_low_soft,
                q.sat_high_soft,
                on(q.lum_on),
                q.lum_low,
                q.lum_high,
                q.lum_low_soft,
                q.lum_high_soft,
            ]);
        }
        if let Some(q) = n.keyed() {
            // Clean Black and Clean White, as black and white points (LOG D-046)
            b[22] = q.clean_black / 200.0;
            b[23] = q.clean_white / 200.0;
        }
        let windows: Vec<_> = n.windows.iter().filter(|x| x.enabled).take(MAX_WINDOWS).collect();
        b[20] = windows.len() as f32;
        // the hue curves that change something, one bit each
        b[21] = n
            .curves
            .hsl()
            .iter()
            .enumerate()
            .filter(|(_, (c, _))| !c.is_identity())
            .map(|(i, _)| (1 << i) as f32)
            .sum();
        for (i, win) in windows.iter().enumerate() {
            let o = 24 + i * 10;
            // as the engine's masks place them: sizes in fractions of the picture's width
            b[o..o + 9].copy_from_slice(&[
                match win.shape {
                    Shape::Linear => 0.0,
                    Shape::Circle => 1.0,
                    Shape::Gradient => 2.0,
                    Shape::Polygon => 3.0,
                },
                win.center[0] * w,
                win.center[1] * h,
                win.size[0] * w / 2.0,
                win.size[1] * w / 2.0,
                win.rotation.to_radians(),
                win.softness * w,
                win.opacity / 100.0,
                if win.invert { 1.0 } else { 0.0 },
            ]);
            if win.shape == Shape::Polygon {
                // turned and placed as the shader's other shapes are: x' = x·cos − y·sin
                b[o + 9] = (after_tables + corners.len()) as f32;
                let (rx, ry) = (win.size[0] * w / 2.0, win.size[1] * w / 2.0);
                let (sin, cos) = win.rotation.to_radians().sin_cos();
                let (cx, cy) = (win.center[0] * w, win.center[1] * h);
                corners.push(win.points.len() as f32);
                for p in &win.points {
                    let (x, y) = (p[0] * rx, p[1] * ry);
                    corners.extend([cx + x * cos - y * sin, cy + x * sin + y * cos]);
                }
            }
        }
        nodes.extend_from_slice(&b);
        for c in 0..3 {
            tones.extend((0..TONE_N).map(|i| n.tone(c, i as f32 / (TONE_N - 1) as f32)));
        }
        // a curve over hue sampled round the wheel (the shader wraps), the others end to end
        for (c, periodic) in n.curves.hsl() {
            let step = if periodic { TONE_N } else { TONE_N - 1 } as f32;
            tones.extend((0..TONE_N).map(|i| c.eval(i as f32 / step, periodic)));
        }
    }
    tones.extend(corners);
    (nodes, tones, active.len())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
