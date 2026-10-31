use bytemuck::{Pod, Zeroable};
use raven_core::{Brush, Color, Primitive, Quad, Scene};

use crate::atlas::{GlyphAtlas, GlyphKey, Lookup, SUBPIXEL_BINS};

const INITIAL_ATLAS_SIZE: u32 = 1024;

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
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlyphInstance {
    rect: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BatchKind {
    Quads,
    Glyphs,
}

#[derive(Clone, Copy, Debug)]
struct Batch {
    kind: BatchKind,
    start: u32,
    end: u32,
}

/// Counters for profiling and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    /// Quad instances drawn, shadows included.
    pub quads: usize,
    pub glyphs: usize,
    pub draw_calls: usize,
    /// Glyphs resident in the atlas.
    pub atlas_glyphs: usize,
}

/// A growable instance buffer that is reused across frames.
struct InstanceBuffer {
    buffer: wgpu::Buffer,
    capacity: usize,
    stride: usize,
    label: &'static str,
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

fn pipeline(
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
            4 => Float32x4, 5 => Float32x4, 6 => Float32x4
        ];
        let glyph_attributes = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4];
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

        Self {
            device: device.clone(),
            queue: queue.clone(),
            srgb_target: target_format.is_srgb(),
            quad_pipeline,
            glyph_pipeline,
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
        }
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
        let _span = tracing::trace_span!("raven::render", primitives = scene.len()).entered();
        if size.0 == 0 || size.1 == 0 || scale <= 0.0 {
            return RenderStats::default();
        }
        self.prepare(scene, scale);

        let globals = Globals {
            viewport: [size.0 as f32 / scale, size.1 as f32 / scale],
            scale,
            srgb_target: u32::from(self.srgb_target),
        };
        self.queue.write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
        self.quad_buffer.upload(&self.device, &self.queue, bytemuck::cast_slice(&self.quads));
        self.glyph_buffer.upload(&self.device, &self.queue, bytemuck::cast_slice(&self.glyphs));

        let clear = self.color_for_target(scene.clear_color());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("raven frame") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("raven main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(clear[0]),
                            g: f64::from(clear[1]),
                            b: f64::from(clear[2]),
                            a: f64::from(clear[3]),
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            let mut current = None;
            for batch in &self.batches {
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
                    }
                    current = Some(batch.kind);
                }
                pass.draw(0..4, batch.start..batch.end);
            }
        }
        self.queue.submit(Some(encoder.finish()));

        RenderStats {
            quads: self.quads.len(),
            glyphs: self.glyphs.len(),
            draw_calls: self.batches.len(),
            atlas_glyphs: self.atlas.len(),
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
                    self.atlas = GlyphAtlas::new(&self.device, size);
                    self.atlas_bind_group =
                        Self::atlas_bind_group(&self.device, &self.atlas_layout, &self.atlas, &self.sampler);
                }
                Err(AtlasFull) => {
                    // Already at the device limit: start from an empty atlas. Text in
                    // this frame that still does not fit is skipped and logged.
                    tracing::warn!("glyph atlas is full at the device limit; clearing it");
                    self.atlas = GlyphAtlas::new(&self.device, self.atlas.size);
                    self.atlas_bind_group =
                        Self::atlas_bind_group(&self.device, &self.atlas_layout, &self.atlas, &self.sampler);
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
        self.batches.clear();
        for primitive in scene.primitives() {
            match primitive {
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
                            rect: [
                                ix + a.left as f32,
                                device_y - a.top as f32,
                                a.width as f32,
                                a.height as f32,
                            ],
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
            }
        }
        Ok(())
    }

    fn extend_batch(&mut self, kind: BatchKind, start: u32, end: u32) {
        if start == end {
            return;
        }
        match self.batches.last_mut() {
            Some(last) if last.kind == kind && last.end == start => last.end = end,
            _ => self.batches.push(Batch { kind, start, end }),
        }
    }
}

struct AtlasFull;

fn push_quad(out: &mut Vec<QuadInstance>, quad: &Quad) {
    let r = quad.rect;
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
        });
    }
    let (color0, color1, gradient, gradient_on) = match quad.fill {
        Brush::Solid(c) => (c.to_array(), c.to_array(), [0.0; 4], 0.0),
        Brush::LinearGradient(g) => {
            (g.from.to_array(), g.to.to_array(), [g.start.x, g.start.y, g.end.x, g.end.y], 1.0)
        }
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
    });
}
