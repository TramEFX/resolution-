//! Resolution's GPU renderer (LOG D-035): draws a frame's layers on the GPU, each with its
//! Resolution grade applied by the grade's own math, and reads the frame back as 8-bit sRGB.
//!
//! A layer is a decoded picture (8-bit sRGB RGBA, linear float RGBA, or planar Y'CbCr of 8 to
//! 16 bits), how it is placed (an affine map from its pixels to the frame's) and its opacity.
//! Layers are drawn bottom to top into a premultiplied linear accumulator ("over"), which is
//! resolved over black. The sampling follows FilmCraft's compositor (`filmcraft-gpu`; MIT or
//! Apache-2.0): bilinear loads, Y'CbCr decoded per pixel, supersampled when shrinking.
//!
//! **The grade stage** runs a layer's [`Grade`] per pixel, as [`Grade::apply`] does: each node's
//! tone functions (as [`TONE_N`]-point tables), its saturation step, its qualifier's key taken on
//! the node's input, and its windows. The key is a real per-pixel mask over the whole node, not
//! a baked LUT (D-032's goal).
//!
//! This crate knows no engine: callers hand it pictures as plain slices.

use std::sync::Arc;

use resolution_grade::{Grade, Shape};

/// Samples of each tone function in the shader.
pub const TONE_N: usize = 4096;
/// Numbers per node in the shader's node table (`render.wgsl`).
const NODE_FLOATS: usize = 104;
/// Windows per node the shader takes.
pub const MAX_WINDOWS: usize = 8;

/// How a layer's pixels are stored.
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

/// The transfer function of a picture's code values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    /// The sRGB curve (also used for Rec. 709 video, as FilmCraft decodes it).
    Srgb,
    Linear,
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
}

impl Coding {
    /// Rec. 709, video range, the sRGB curve.
    pub const REC709: Coding = Coding { kr: 0.2126, kb: 0.0722, full_range: false, transfer: Transfer::Srgb };
}

/// One layer of a frame.
pub struct Layer<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: Pixels<'a>,
    pub coding: Coding,
    /// The layer's pixels to the frame's: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`, as
    /// `[a, b, c, d, e, f]`.
    pub matrix: [f64; 6],
    pub opacity: f32,
    /// The layer's grade, if it has one.
    pub grade: Option<&'a Grade>,
}

const ACCUM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layer_pipeline: wgpu::RenderPipeline,
    resolve_pipeline: wgpu::RenderPipeline,
    layer_bgl: wgpu::BindGroupLayout,
    resolve_bgl: wgpu::BindGroupLayout,
    dummy: wgpu::TextureView,
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

fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
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
        let layer_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("layer"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex_entry(1),
                tex_entry(2),
                tex_entry(3),
                tex_entry(4),
                storage_entry(5),
                storage_entry(6),
            ],
        });
        let resolve_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("resolve"),
            entries: &[tex_entry(0)],
        });
        let pipeline = |label, bgl: &wgpu::BindGroupLayout, vs, fs, format, blend| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(bgl)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let layer_pipeline = pipeline(
            "layer",
            &layer_bgl,
            "vs",
            "fs",
            ACCUM_FORMAT,
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        );
        let resolve_pipeline = pipeline("resolve", &resolve_bgl, "vs_full", "fs_full", OUTPUT_FORMAT, None);
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
        Self { device, queue, layer_pipeline, resolve_pipeline, layer_bgl, resolve_bgl, dummy }
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
        let (w, h) = (size.0.max(1), size.1.max(1));
        let target = |format, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("target"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let accum = target(ACCUM_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
        let output = target(OUTPUT_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let accum_view = accum.create_view(&Default::default());
        let output_view = output.create_view(&Default::default());
        // everything the passes bind, made before they are recorded
        let groups: Vec<wgpu::BindGroup> = layers
            .iter()
            .map(|l| {
                let (views, kind, code_scale, alpha_scale, chroma) = self.upload(l);
                let (node_data, tone_data, count) = grade_data(l.grade, (l.width, l.height));
                let uniforms = uniforms(l, kind, code_scale, alpha_scale, chroma, (w, h), count);
                let ub = self.buffer(wgpu::BufferUsages::UNIFORM, &uniforms);
                let nb = self.buffer(wgpu::BufferUsages::STORAGE, &node_data);
                let tb = self.buffer(wgpu::BufferUsages::STORAGE, &tone_data);
                let entries = [
                    wgpu::BindGroupEntry { binding: 0, resource: ub.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&views[0]) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&views[1]) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&views[2]) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&views[3]) },
                    wgpu::BindGroupEntry { binding: 5, resource: nb.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 6, resource: tb.as_entire_binding() },
                ];
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("layer"),
                    layout: &self.layer_bgl,
                    entries: &entries,
                })
            })
            .collect();
        let resolve = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("resolve"),
            layout: &self.resolve_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&accum_view) }],
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        let pass = |enc: &mut wgpu::CommandEncoder, view, clear| {
            enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
            .forget_lifetime()
        };
        {
            let mut p = pass(&mut enc, &accum_view, wgpu::Color::TRANSPARENT);
            p.set_pipeline(&self.layer_pipeline);
            for g in &groups {
                p.set_bind_group(0, g, &[]);
                p.draw(0..6, 0..1);
            }
        }
        {
            let mut p = pass(&mut enc, &output_view, wgpu::Color::BLACK);
            p.set_pipeline(&self.resolve_pipeline);
            p.set_bind_group(0, &resolve, &[]);
            p.draw(0..3, 0..1);
        }
        let row = (w * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(row * h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &output,
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

/// The shader's numbers for a layer (`render.wgsl`'s `U`).
fn uniforms(
    l: &Layer<'_>,
    kind: f32,
    code_scale: f32,
    alpha_scale: f32,
    chroma: (u32, u32),
    out: (u32, u32),
    nodes: usize,
) -> Vec<f32> {
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
    // source pixels per frame pixel: supersample when shrinking
    let sx = (a * a + b * b).sqrt();
    let sy = (c * c + d * d).sqrt();
    let footprint = (1.0 / sx.min(sy).max(1e-6)) as f32;
    let taps = if footprint > 1.25 { footprint.ceil().min(4.0) } else { 1.0 };
    let transfer = match l.coding.transfer {
        Transfer::Srgb => 0.0,
        Transfer::Linear => 1.0,
    };
    vec![
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
        0.0,
        0.0,
    ]
}

/// A grade as the shader reads it: [`NODE_FLOATS`] numbers per enabled node (tone on, the
/// saturation step, the qualifier, the windows in the picture's pixels), and the nodes' tone
/// tables; and how many nodes. Windows beyond [`MAX_WINDOWS`] are left out.
pub fn grade_data(grade: Option<&Grade>, (w, h): (u32, u32)) -> (Vec<f32>, Vec<f32>, usize) {
    let Some(grade) = grade else { return (Vec::new(), Vec::new(), 0) };
    let (w, h) = (w as f32, h as f32);
    let active: Vec<_> = grade.nodes.iter().filter(|n| n.enabled && !n.is_neutral()).collect();
    let mut nodes = Vec::with_capacity(active.len() * NODE_FLOATS);
    let mut tones = Vec::with_capacity(active.len() * 3 * TONE_N);
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
        let windows: Vec<_> = n.windows.iter().filter(|x| x.enabled).take(MAX_WINDOWS).collect();
        b[20] = windows.len() as f32;
        for (i, win) in windows.iter().enumerate() {
            let o = 24 + i * 10;
            // as the engine's masks place them: sizes in fractions of the picture's width
            b[o..o + 9].copy_from_slice(&[
                if win.shape == Shape::Circle { 1.0 } else { 0.0 },
                win.center[0] * w,
                win.center[1] * h,
                win.size[0] * w / 2.0,
                win.size[1] * w / 2.0,
                win.rotation.to_radians(),
                win.softness * w,
                win.opacity / 100.0,
                if win.invert { 1.0 } else { 0.0 },
            ]);
        }
        nodes.extend_from_slice(&b);
        for c in 0..3 {
            tones.extend((0..TONE_N).map(|i| n.tone(c, i as f32 / (TONE_N - 1) as f32)));
        }
    }
    (nodes, tones, active.len())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
