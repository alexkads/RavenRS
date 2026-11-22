use bytemuck::{Pod, Zeroable};
use ravenrs_core::cable::{Cable, cable_plugs, cable_primitives, cable_rest};
use ravenrs_core::{Brush, Color, ImageQuad, Point, Primitive, Quad, Scene};

use crate::cables::{CableDrawer, CableLookGpu, CablePictures, cables_drawable};

use crate::atlas::{GlyphAtlas, GlyphKey, Lookup, SUBPIXEL_BINS};
use crate::images::ImageCache;
use crate::paths::PathEngine;
use crate::timing::GpuTimer;

const INITIAL_ATLAS_SIZE: u32 = 1024;
/// Instances one cable takes: one per piece of the longest cable, drawn along a curve.
const CABLE_SEGMENTS: u32 =
    (ravenrs_core::cable::CABLE_MAX_POINTS as u32 - 1) * ravenrs_core::cable::CABLE_SMOOTHING as u32;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 2],
    scale: f32,
    srgb_target: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct QuadInstance {
    rect: [f32; 4],
    radii: [f32; 4],
    color0: [f32; 4],
    color1: [f32; 4],
    gradient: [f32; 4],
    border_color: [f32; 4],
    params: [f32; 4],
    /// Cosine and sine of the turn around the rect's center.
    turn: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlyphInstance {
    rect: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
    /// Cosine and sine of the turn, and the origin it turns around, in device pixels.
    turn: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ImageInstance {
    rect: [f32; 4],
    uv: [f32; 4],
    radii: [f32; 4],
    params: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BatchKind {
    Quads,
    Glyphs,
    /// Images of one texture, identified by image id.
    Image(u64),
    /// Shapes made of curves, on this page of the coverage atlas.
    Paths(u32),
    /// Cables drawn from the GPU world's buffer.
    Cables,
}

#[derive(Clone, Copy, Debug)]
struct Batch {
    kind: BatchKind,
    start: u32,
    end: u32,
    /// Scissor rectangle in device pixels: x, y, width, height.
    scissor: [u32; 4],
}

/// Counters for profiling and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    /// Quad instances drawn, shadows included.
    pub quads: usize,
    /// Glyphs drawn.
    pub glyphs: usize,
    /// Draw calls issued for the frame.
    pub draw_calls: usize,
    /// Glyphs resident in the atlas.
    pub atlas_glyphs: usize,
    /// Image quads drawn.
    pub images: usize,
    /// Image textures resident on the GPU.
    pub cached_images: usize,
    /// Shapes made of curves drawn.
    pub paths: usize,
    /// Straight pieces those curves were cut into.
    pub path_edges: usize,
    /// Draw calls split because the kind of primitive changed, as text after a quad.
    pub breaks_kind: usize,
    /// Draw calls split because the image changed.
    pub breaks_image: usize,
    /// Draw calls split because the clip changed.
    pub breaks_clip: usize,
    /// Bytes of instances sent to the GPU this frame.
    pub uploaded_bytes: usize,
    /// Glyphs rasterized since the renderer was created.
    pub glyphs_rasterized: u64,
    /// Times the glyph atlas lost everything: it grew or it was full.
    pub atlas_resets: u64,
    /// How full the glyph atlas is, in thousandths of its height.
    pub atlas_used: u32,
    /// Side of the glyph atlas in pixels.
    pub atlas_size: u32,
    /// How long the GPU took on a recent frame, when timing is on and it can tell.
    /// It arrives some frames late, so it is not this frame's.
    pub gpu_time: Option<std::time::Duration>,
}

/// A growable instance buffer that is reused across frames.
struct InstanceBuffer {
    buffer: wgpu::Buffer,
    capacity: usize,
    stride: usize,
    label: &'static str,
}

impl RenderStats {
    /// The part of these stats the diagnostics keep.
    pub fn work(&self) -> ravenrs_core::RenderWork {
        ravenrs_core::RenderWork {
            breaks_kind: self.breaks_kind as u32,
            breaks_image: self.breaks_image as u32,
            breaks_clip: self.breaks_clip as u32,
            uploaded_bytes: self.uploaded_bytes as u64,
            glyphs_rasterized: self.glyphs_rasterized,
            atlas_resets: self.atlas_resets,
            atlas_used: self.atlas_used,
            atlas_size: self.atlas_size,
            cached_images: self.cached_images as u32,
            gpu: self.gpu_time,
        }
    }
}

impl InstanceBuffer {
    fn new(device: &wgpu::Device, label: &'static str, stride: usize) -> Self {
        let capacity = 256;
        Self { buffer: Self::create(device, label, stride * capacity), capacity, stride, label }
    }

    fn create(device: &wgpu::Device, label: &'static str, size: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: size as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8]) {
        let needed = bytes.len() / self.stride;
        if needed > self.capacity {
            self.capacity = needed.next_power_of_two();
            self.buffer = Self::create(device, self.label, self.capacity * self.stride);
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytes);
        }
    }
}

/// Draws [`Scene`]s with wgpu. Create one per device; it is reused every frame.
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    srgb_target: bool,
    quad_pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    paths: PathEngine,
    image_sampler: wgpu::Sampler,
    image_cache: ImageCache,
    image_buffer: InstanceBuffer,
    image_instances: Vec<ImageInstance>,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    atlas_layout: wgpu::BindGroupLayout,
    atlas_bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    atlas: GlyphAtlas,
    quad_buffer: InstanceBuffer,
    glyph_buffer: InstanceBuffer,
    quads: Vec<QuadInstance>,
    glyphs: Vec<GlyphInstance>,
    batches: Vec<Batch>,
    clips: Vec<[u32; 4]>,
    target_size: (u32, u32),
    breaks: BatchBreaks,
    atlas_resets: u64,
    /// Glyphs rasterized by the atlases that are gone.
    glyphs_rasterized: u64,
    timer: Option<GpuTimer>,
    /// `None` where storage buffers cannot be read while drawing (WebGL2).
    cable_pipeline: Option<(wgpu::RenderPipeline, wgpu::BindGroupLayout)>,
    cables: Option<CableDrawer>,
    cable_looks: Vec<CableLookGpu>,
    cable_look_buffer: wgpu::Buffer,
    /// The bind group of the cables, and the shape generation and look buffer size it
    /// was made for.
    cable_bind: Option<(wgpu::BindGroup, u64, u64)>,
    /// The pictures on cables the GPU draws.
    cable_pictures: CablePictures,
    /// The plugs of the cables drawn so far in a run of cables, drawn once the run ends:
    /// the bodies go in one draw call and the plugs in another, instead of two per cable.
    cable_plugs_pending: Vec<Primitive>,
}

/// Why the last frame needed more than one draw call.
#[derive(Clone, Copy, Debug, Default)]
struct BatchBreaks {
    kind: usize,
    image: usize,
    clip: usize,
}

impl std::fmt::Debug for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer")
            .field("srgb_target", &self.srgb_target)
            .field("atlas_size", &self.atlas.size)
            .finish_non_exhaustive()
    }
}

fn premultiplied_target(format: wgpu::TextureFormat) -> [Option<wgpu::ColorTargetState>; 1] {
    [Some(wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    })]
}

pub(crate) fn pipeline(
    device: &wgpu::Device,
    label: &str,
    source: &str,
    layout: &wgpu::PipelineLayout,
    instance_layout: wgpu::VertexBufferLayout<'_>,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_layout)],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &premultiplied_target(format),
        }),
        multiview_mask: None,
        cache: None,
    })
}

impl Renderer {
    /// Creates the pipelines for rendering into textures of `target_format`.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target_format: wgpu::TextureFormat) -> Self {
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("raven globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("raven atlas"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("raven globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("raven globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buffer.as_entire_binding() }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("raven atlas sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let atlas = GlyphAtlas::new(device, INITIAL_ATLAS_SIZE);
        let atlas_bind_group = Self::atlas_bind_group(device, &atlas_layout, &atlas, &sampler);

        let quad_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("raven quads"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });
        let glyph_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("raven glyphs"),
            bind_group_layouts: &[Some(&globals_layout), Some(&atlas_layout)],
            immediate_size: 0,
        });
        let quad_attributes = wgpu::vertex_attr_array![
            0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4,
            4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4
        ];
        let glyph_attributes =
            wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
        let image_attributes =
            wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
        let image_pipeline = pipeline(
            device,
            "raven image pipeline",
            include_str!("../shaders/image.wgsl"),
            &glyph_layout,
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<ImageInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &image_attributes,
            },
            target_format,
        );
        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("raven image sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let quad_pipeline = pipeline(
            device,
            "raven quad pipeline",
            include_str!("../shaders/quad.wgsl"),
            &quad_layout,
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<QuadInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &quad_attributes,
            },
            target_format,
        );
        let glyph_pipeline = pipeline(
            device,
            "raven glyph pipeline",
            include_str!("../shaders/glyph.wgsl"),
            &glyph_layout,
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GlyphInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &glyph_attributes,
            },
            target_format,
        );

        let paths = PathEngine::new(device, &globals_layout, target_format);

        let cable_pipeline = cables_drawable(device).then(|| {
            let cable_storage = |binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            };
            let cable_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("raven cables"),
                entries: &[cable_storage(0), cable_storage(1), cable_storage(2)],
            });
            let cable_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("raven cables"),
                bind_group_layouts: &[Some(&globals_layout), Some(&cable_layout)],
                immediate_size: 0,
            });
            let cable_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("raven cable pipeline"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/cable.wgsl").into()),
            });
            // Storage buffers are read in the vertex stage, which WebGL2 cannot do; there
            // the cables are drawn as shapes, so the pipeline is made only where it works.
            let cable_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("raven cable pipeline"),
                layout: Some(&cable_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &cable_module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &cable_module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &premultiplied_target(target_format),
                }),
                multiview_mask: None,
                cache: None,
            });
            (cable_pipeline, cable_layout)
        });
        let cable_look_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("raven cable looks"),
            size: 64 * std::mem::size_of::<CableLookGpu>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device: device.clone(),
            queue: queue.clone(),
            paths,
            srgb_target: target_format.is_srgb(),
            quad_pipeline,
            glyph_pipeline,
            image_pipeline,
            image_sampler,
            image_cache: ImageCache::default(),
            image_buffer: InstanceBuffer::new(
                device,
                "raven image instances",
                std::mem::size_of::<ImageInstance>(),
            ),
            image_instances: Vec::new(),
            globals_buffer,
            globals_bind_group,
            atlas_layout,
            atlas_bind_group,
            sampler,
            atlas,
            quad_buffer: InstanceBuffer::new(
                device,
                "raven quad instances",
                std::mem::size_of::<QuadInstance>(),
            ),
            glyph_buffer: InstanceBuffer::new(
                device,
                "raven glyph instances",
                std::mem::size_of::<GlyphInstance>(),
            ),
            quads: Vec::new(),
            glyphs: Vec::new(),
            batches: Vec::new(),
            clips: Vec::new(),
            target_size: (1, 1),
            breaks: BatchBreaks::default(),
            atlas_resets: 0,
            glyphs_rasterized: 0,
            timer: None,
            cable_pipeline,
            cables: None,
            cable_looks: Vec::new(),
            cable_look_buffer,
            cable_bind: None,
            cable_pictures: CablePictures::new(device),
            cable_plugs_pending: Vec::new(),
        }
    }

    /// Draws the cables of a GPU world from its own buffer. Without one, cables are
    /// drawn as shapes from the points the scene carries.
    pub fn set_cables(&mut self, drawer: Option<CableDrawer>) {
        self.cables = drawer.filter(|_| self.cable_pipeline.is_some());
        self.cable_bind = None;
    }

    /// Sends this frame's cable looks and makes the bind group again if a buffer changed.
    fn prepare_cables(&mut self) {
        let (Some(drawer), Some((_, layout))) = (&self.cables, &self.cable_pipeline) else { return };
        if self.cable_looks.is_empty() {
            return;
        }
        let bytes: &[u8] = bytemuck::cast_slice(&self.cable_looks);
        if bytes.len() as u64 > self.cable_look_buffer.size() {
            self.cable_look_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("raven cable looks"),
                size: (bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.queue.write_buffer(&self.cable_look_buffer, 0, bytes);
        if self.cable_pictures.upload(&self.device, &self.queue) {
            self.cable_bind = None;
        }
        let (shape, generation) = drawer.shape();
        let size = self.cable_look_buffer.size();
        if self.cable_bind.as_ref().is_none_or(|(_, g, s)| *g != generation || *s != size) {
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("raven cables"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: shape.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.cable_look_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.cable_pictures.buffer_ref().as_entire_binding(),
                    },
                ],
            });
            self.cable_bind = Some((group, generation, size));
        }
    }

    /// Starts or stops timing the GPU, reported in [`RenderStats::gpu_time`]. It stays
    /// off unless asked for, and does nothing on a GPU that cannot tell the time.
    pub fn set_gpu_timing(&mut self, on: bool) {
        match (on, &self.timer) {
            (true, None) => self.timer = GpuTimer::new(&self.device, &self.queue),
            (false, Some(_)) => self.timer = None,
            _ => {}
        }
    }

    fn lose_atlas(&mut self, size: u32) {
        self.glyphs_rasterized += self.atlas.rasterized();
        self.atlas_resets += 1;
        self.atlas = GlyphAtlas::new(&self.device, size);
        self.atlas_bind_group =
            Self::atlas_bind_group(&self.device, &self.atlas_layout, &self.atlas, &self.sampler);
    }

    fn atlas_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        atlas: &GlyphAtlas,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("raven atlas"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas.view),
                },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        })
    }

    /// Renders `scene` into `target`, a texture of `size` device pixels.
    /// `scale` converts the scene's logical pixels to device pixels.
    pub fn render(
        &mut self,
        scene: &Scene,
        target: &wgpu::TextureView,
        size: (u32, u32),
        scale: f32,
    ) -> RenderStats {
        let _span = tracing::trace_span!("ravenrs::render", primitives = scene.len()).entered();
        if size.0 == 0 || size.1 == 0 || scale <= 0.0 {
            return RenderStats::default();
        }
        self.target_size = size;
        self.image_cache.begin_frame();
        self.prepare(scene, scale);

        let globals = Globals {
            viewport: [size.0 as f32 / scale, size.1 as f32 / scale],
            scale,
            srgb_target: u32::from(self.srgb_target),
        };
        self.queue.write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
        self.quad_buffer.upload(&self.device, &self.queue, bytemuck::cast_slice(&self.quads));
        self.glyph_buffer.upload(&self.device, &self.queue, bytemuck::cast_slice(&self.glyphs));
        self.image_buffer.upload(&self.device, &self.queue, bytemuck::cast_slice(&self.image_instances));
        self.prepare_cables();

        let clear = self.color_for_target(scene.clear_color());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("raven frame") });
        // A page of shapes has to be drawn before the shapes that read it, and the main pass
        // cannot stop for that, so a page after the first splits it in two.
        self.paths.prepare(&self.device, &self.queue);
        let coverage_timing = self.timer.as_ref().map(GpuTimer::coverage_pass);
        self.paths.draw_page(&mut encoder, 0, coverage_timing);
        let mut load = wgpu::LoadOp::Clear(wgpu::Color {
            r: f64::from(clear[0]),
            g: f64::from(clear[1]),
            b: f64::from(clear[2]),
            a: f64::from(clear[3]),
        });
        let (mut from, mut page) = (0, 0);
        loop {
            let next = self.batches[from..]
                .iter()
                .position(|batch| matches!(batch.kind, BatchKind::Paths(p) if p > page))
                .map_or(self.batches.len(), |at| from + at);
            let timing = if from == 0 { self.timer.as_ref().map(GpuTimer::main_pass) } else { None };
            self.draw_main_pass(&mut encoder, target, size, load, from..next, timing);
            let Some(BatchKind::Paths(after)) = self.batches.get(next).map(|batch| batch.kind) else { break };
            self.paths.draw_page(&mut encoder, after, None);
            load = wgpu::LoadOp::Load;
            (from, page) = (next, after);
        }
        let gpu_time = self.timer.as_mut().and_then(|timer| timer.finish_frame(&mut encoder));
        self.queue.submit(Some(encoder.finish()));
        if let Some(timer) = &mut self.timer {
            timer.frame_submitted();
        }

        RenderStats {
            quads: self.quads.len(),
            glyphs: self.glyphs.len(),
            draw_calls: self.batches.len() + self.paths.pages(),
            atlas_glyphs: self.atlas.len(),
            images: self.image_instances.len(),
            cached_images: self.image_cache.len(),
            paths: self.paths.shapes(),
            path_edges: self.paths.edges(),
            breaks_kind: self.breaks.kind,
            breaks_image: self.breaks.image,
            breaks_clip: self.breaks.clip,
            uploaded_bytes: std::mem::size_of_val(self.quads.as_slice())
                + std::mem::size_of_val(self.glyphs.as_slice())
                + std::mem::size_of_val(self.image_instances.as_slice())
                + self.paths.uploaded_bytes(),
            glyphs_rasterized: self.glyphs_rasterized + self.atlas.rasterized(),
            atlas_resets: self.atlas_resets,
            atlas_used: (self.atlas.used() * 1000.0) as u32,
            atlas_size: self.atlas.size,
            gpu_time,
        }
    }

    /// Draws these batches on `target`.
    fn draw_main_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        load: wgpu::LoadOp<wgpu::Color>,
        batches: std::ops::Range<usize>,
        timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("raven main pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.globals_bind_group, &[]);
        let mut current = None;
        let mut scissor = [0, 0, size.0, size.1];
        for batch in &self.batches[batches] {
            if batch.scissor != scissor {
                scissor = batch.scissor;
                pass.set_scissor_rect(scissor[0], scissor[1], scissor[2], scissor[3]);
            }
            if current != Some(batch.kind) {
                match batch.kind {
                    BatchKind::Quads => {
                        pass.set_pipeline(&self.quad_pipeline);
                        pass.set_vertex_buffer(0, self.quad_buffer.buffer.slice(..));
                    }
                    BatchKind::Glyphs => {
                        pass.set_pipeline(&self.glyph_pipeline);
                        pass.set_bind_group(1, &self.atlas_bind_group, &[]);
                        pass.set_vertex_buffer(0, self.glyph_buffer.buffer.slice(..));
                    }
                    BatchKind::Image(id) => {
                        let Some(image) = self.image_cache.get(id) else { continue };
                        pass.set_pipeline(&self.image_pipeline);
                        pass.set_bind_group(1, &image.bind_group, &[]);
                        pass.set_vertex_buffer(0, self.image_buffer.buffer.slice(..));
                    }
                    BatchKind::Paths(_) => self.paths.bind(&mut pass),
                    BatchKind::Cables => {
                        let (Some((group, _, _)), Some((pipeline, _))) =
                            (&self.cable_bind, &self.cable_pipeline)
                        else {
                            continue;
                        };
                        pass.set_pipeline(pipeline);
                        pass.set_bind_group(1, group, &[]);
                    }
                }
                current = Some(batch.kind);
            }
            pass.draw(0..4, batch.start..batch.end);
        }
    }

    fn color_for_target(&self, c: Color) -> [f32; 4] {
        let [r, g, b, a] = if self.srgb_target { c.to_linear() } else { c.to_array() };
        [r * a, g * a, b * a, a]
    }

    /// Builds instance lists and batches. Grows the atlas and starts over if it fills up.
    fn prepare(&mut self, scene: &Scene, scale: f32) {
        let max_atlas = self.device.limits().max_texture_dimension_2d.min(8192);
        loop {
            match self.try_prepare(scene, scale) {
                Ok(()) => return,
                Err(AtlasFull) if self.atlas.size < max_atlas => {
                    let size = (self.atlas.size * 2).min(max_atlas);
                    tracing::debug!(size, "growing glyph atlas");
                    self.lose_atlas(size);
                }
                Err(AtlasFull) => {
                    // Already at the device limit: start from an empty atlas. Text in
                    // this frame that still does not fit is skipped and logged.
                    tracing::warn!("glyph atlas is full at the device limit; clearing it");
                    self.lose_atlas(self.atlas.size);
                    let _ = self.try_prepare_lossy(scene, scale);
                    return;
                }
            }
        }
    }

    fn try_prepare(&mut self, scene: &Scene, scale: f32) -> Result<(), AtlasFull> {
        self.build(scene, scale, false)
    }

    fn try_prepare_lossy(&mut self, scene: &Scene, scale: f32) -> Result<(), AtlasFull> {
        self.build(scene, scale, true)
    }

    fn build(&mut self, scene: &Scene, scale: f32, skip_missing: bool) -> Result<(), AtlasFull> {
        self.quads.clear();
        self.glyphs.clear();
        self.image_instances.clear();
        self.cable_looks.clear();
        self.cable_pictures.begin_frame();
        self.batches.clear();
        self.clips.clear();
        self.breaks = BatchBreaks::default();
        self.paths.begin_frame();
        self.cable_plugs_pending.clear();
        for primitive in scene.primitives() {
            if !matches!(primitive, Primitive::Cable(_)) {
                self.flush_cable_plugs(scale, skip_missing)?;
            }
            self.push_primitive(primitive, scale, skip_missing)?;
        }
        self.flush_cable_plugs(scale, skip_missing)
    }

    /// Draws the plugs held back for the run of cables that just ended.
    fn flush_cable_plugs(&mut self, scale: f32, skip_missing: bool) -> Result<(), AtlasFull> {
        if self.cable_plugs_pending.is_empty() {
            return Ok(());
        }
        let plugs = std::mem::take(&mut self.cable_plugs_pending);
        for primitive in &plugs {
            self.push_primitive(primitive, scale, skip_missing)?;
        }
        // Keep the allocation for the next run.
        self.cable_plugs_pending = plugs;
        self.cable_plugs_pending.clear();
        Ok(())
    }

    fn push_primitive(
        &mut self,
        primitive: &Primitive,
        scale: f32,
        skip_missing: bool,
    ) -> Result<(), AtlasFull> {
        match primitive {
            Primitive::PushClip(rect) => {
                let (w, h) = self.target_size;
                let x0 = (rect.x() * scale).floor().clamp(0.0, w as f32) as u32;
                let y0 = (rect.y() * scale).floor().clamp(0.0, h as f32) as u32;
                let x1 = (rect.max_x() * scale).ceil().clamp(0.0, w as f32) as u32;
                let y1 = (rect.max_y() * scale).ceil().clamp(0.0, h as f32) as u32;
                let outer = self.current_scissor();
                let ix0 = x0.max(outer[0]);
                let iy0 = y0.max(outer[1]);
                let ix1 = x1.min(outer[0] + outer[2]).max(ix0);
                let iy1 = y1.min(outer[1] + outer[3]).max(iy0);
                self.clips.push([ix0, iy0, ix1 - ix0, iy1 - iy0]);
            }
            Primitive::PopClip => {
                self.clips.pop();
            }
            Primitive::Image(image) => self.push_image(image),
            Primitive::Path(path) => {
                let start = self.paths.shapes() as u32;
                let scissor = self.current_scissor();
                let limit = self.device.limits().max_texture_dimension_2d.min(8192);
                self.paths.push(path, scale, scissor, limit);
                self.extend_batch(BatchKind::Paths(self.paths.page()), start, self.paths.shapes() as u32);
            }
            Primitive::Quad(quad) => {
                let start = self.quads.len() as u32;
                push_quad(&mut self.quads, quad);
                self.extend_batch(BatchKind::Quads, start, self.quads.len() as u32);
            }
            Primitive::Glyphs(run) => {
                let start = self.glyphs.len() as u32;
                let size_px = run.size * scale;
                let size_q = (size_px * 4.0).round().max(1.0) as u32;
                let color = run.color.to_array();
                let inv_atlas = 1.0 / self.atlas.size as f32;
                let (sin, cos) = run.rotation.sin_cos();
                for g in &run.glyphs {
                    let device_x = g.x * scale;
                    let mut ix = device_x.floor();
                    let mut bin = ((device_x - ix) * SUBPIXEL_BINS).round();
                    if bin >= SUBPIXEL_BINS {
                        ix += 1.0;
                        bin = 0.0;
                    }
                    let key = GlyphKey {
                        font_id: run.font.id,
                        font_index: run.font.index,
                        glyph: g.id,
                        size_q,
                        subpixel: bin as u8,
                    };
                    let entry = match self.atlas.get_or_insert(&self.queue, key, run.font.bytes()) {
                        Lookup::Hit(entry) => entry,
                        Lookup::Full if skip_missing => None,
                        Lookup::Full => return Err(AtlasFull),
                    };
                    let Some(a) = entry else { continue };
                    let device_y = (g.y * scale).round();
                    self.glyphs.push(GlyphInstance {
                        turn: [cos, sin, ix, device_y],
                        rect: [ix + a.left as f32, device_y - a.top as f32, a.width as f32, a.height as f32],
                        uv: [
                            a.x as f32 * inv_atlas,
                            a.y as f32 * inv_atlas,
                            (a.x + a.width) as f32 * inv_atlas,
                            (a.y + a.height) as f32 * inv_atlas,
                        ],
                        color,
                    });
                }
                self.extend_batch(BatchKind::Glyphs, start, self.glyphs.len() as u32);
            }
            Primitive::Cable(cable) => self.push_cable(cable, scale, skip_missing)?,
            // Primitives from a newer ravenrs-core this renderer does not know yet are skipped.
            _ => {}
        }
        Ok(())
    }

    /// A cable the GPU world simulates is drawn from its buffer, a picture on it too;
    /// any other as shapes.
    fn push_cable(&mut self, cable: &Cable, scale: f32, skip_missing: bool) -> Result<(), AtlasFull> {
        let pictures = &mut self.cable_pictures;
        let gpu = self
            .cables
            .as_ref()
            .and_then(|d| d.slot(cable))
            .and_then(|(slot, n)| CableLookGpu::of(cable, slot, n, |image| pictures.place(image)));
        let points: Vec<Point> = match &cable.points {
            Some(points) => points.to_vec(),
            None => cable_rest(cable),
        };
        let extra = match gpu {
            Some(look) => {
                let item = self.cable_looks.len() as u32;
                self.cable_looks.push(look);
                self.extend_batch(BatchKind::Cables, item * CABLE_SEGMENTS, (item + 1) * CABLE_SEGMENTS);
                self.cable_plugs_pending.extend(cable_plugs(cable, &points));
                return Ok(());
            }
            None => {
                // Drawn as shapes: the plugs of the cables before it go first, in order.
                self.flush_cable_plugs(scale, skip_missing)?;
                cable_primitives(cable, &points)
            }
        };
        for primitive in &extra {
            self.push_primitive(primitive, scale, skip_missing)?;
        }
        Ok(())
    }

    fn push_image(&mut self, image: &ImageQuad) {
        let layout = &self.atlas_layout;
        self.image_cache.prepare(&self.device, &self.queue, layout, &self.image_sampler, &image.image);
        let start = self.image_instances.len() as u32;
        let r = image.rect;
        self.image_instances.push(ImageInstance {
            rect: [r.x(), r.y(), r.width(), r.height()],
            uv: image.uv,
            radii: image.corner_radii,
            params: {
                let (sin, cos) = image.rotation.sin_cos();
                [image.opacity.clamp(0.0, 1.0), cos, sin, 0.0]
            },
        });
        self.extend_batch(BatchKind::Image(image.image.id()), start, start + 1);
    }

    fn current_scissor(&self) -> [u32; 4] {
        self.clips.last().copied().unwrap_or([0, 0, self.target_size.0, self.target_size.1])
    }

    fn extend_batch(&mut self, kind: BatchKind, start: u32, end: u32) {
        if start == end {
            return;
        }
        let scissor = self.current_scissor();
        // A fully clipped batch draws nothing; scissor rects must not be empty anyway.
        if scissor[2] == 0 || scissor[3] == 0 {
            return;
        }
        match self.batches.last_mut() {
            Some(last) if last.kind == kind && last.end == start && last.scissor == scissor => last.end = end,
            Some(last) => {
                if last.scissor != scissor {
                    self.breaks.clip += 1;
                } else if matches!((last.kind, kind), (BatchKind::Image(_), BatchKind::Image(_))) {
                    self.breaks.image += 1;
                } else {
                    self.breaks.kind += 1;
                }
                self.batches.push(Batch { kind, start, end, scissor });
            }
            None => self.batches.push(Batch { kind, start, end, scissor }),
        }
    }
}

struct AtlasFull;

fn push_quad(out: &mut Vec<QuadInstance>, quad: &Quad) {
    let r = quad.rect;
    let (sin, cos) = quad.rotation.sin_cos();
    let turn = [cos, sin, 0.0, 0.0];
    let rect = [r.x(), r.y(), r.width(), r.height()];
    if let Some(shadow) = quad.shadow {
        out.push(QuadInstance {
            rect: [rect[0] + shadow.offset.x, rect[1] + shadow.offset.y, rect[2], rect[3]],
            radii: quad.corner_radii,
            color0: shadow.color.to_array(),
            color1: [0.0; 4],
            gradient: [0.0; 4],
            border_color: [0.0; 4],
            params: [0.0, shadow.blur * 0.5, 1.0, 0.0],
            turn,
        });
    }
    let (color0, color1, gradient, gradient_on) = match quad.fill {
        Brush::Solid(c) => (c.to_array(), c.to_array(), [0.0; 4], 0.0),
        Brush::LinearGradient(g) => {
            (g.from.to_array(), g.to.to_array(), [g.start.x, g.start.y, g.end.x, g.end.y], 1.0)
        }
        // Brushes from a newer ravenrs-core are drawn transparent rather than wrong.
        _ => ([0.0; 4], [0.0; 4], [0.0; 4], 0.0),
    };
    let (border_width, border_color) = quad.border.map_or((0.0, [0.0; 4]), |b| (b.width, b.color.to_array()));
    out.push(QuadInstance {
        rect,
        radii: quad.corner_radii,
        color0,
        color1,
        gradient,
        border_color,
        params: [border_width, 0.0, 0.0, gradient_on],
        turn,
    });
}
