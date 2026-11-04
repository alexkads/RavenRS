//! Shapes made of curves. Each frame every shape gets a place in a coverage atlas,
//! where its edges add up the exact area they cover in each pixel, in one pass. The
//! main pass then paints the shapes from it, in the order of the scene. There are no
//! triangles to fit and no samples to count, so edges are right at any size.

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use ravenrs_core::{ColorStop, FillRule, ImageData, PathElement, PathFill, PathPaint, Point};

/// 16 bits are always there, and rounding piles up a little with many edges to a pixel.
const COVERAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;
/// 32 bits are exact for what this does, where the device can blend them.
const FINE_COVERAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
const INITIAL_ATLAS: u32 = 1024;
/// How far a straight piece may stray from its curve, in device pixels.
const FLATNESS: f32 = 0.08;
/// More straight pieces than this for one curve add nothing anyone can see.
const MOST_PIECES: u32 = 128;
const RAMP_WIDTH: u32 = 256;
const RAMP_ROWS: u32 = 256;
/// The side of the texture for the images that paths show, where the device has it.
const IMAGE_ATLAS: u32 = 4096;
/// What is left empty between images, so that none reads its neighbor.
const IMAGE_GAP: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct EdgeInstance {
    line: [f32; 4],
    region: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShapeInstance {
    rect: [f32; 4],
    atlas: [f32; 4],
    color: [f32; 4],
    gradient: [f32; 4],
    params: [f32; 4],
    /// Where the clip's coverage is, whether it is even-odd, and whether it is
    /// inverted; x below zero for no clip.
    clip: [f32; 4],
}

/// A vertex buffer that grows as needed and is reused across frames.
struct Instances {
    buffer: wgpu::Buffer,
    capacity: usize,
    label: &'static str,
}

impl Instances {
    fn new(device: &wgpu::Device, label: &'static str, bytes: usize) -> Self {
        Self { buffer: Self::create(device, label, bytes), capacity: bytes, label }
    }

    fn create(device: &wgpu::Device, label: &'static str, bytes: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: bytes as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8]) {
        if bytes.len() > self.capacity {
            self.capacity = bytes.len().next_power_of_two();
            self.buffer = Self::create(device, self.label, self.capacity);
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytes);
        }
    }
}

/// Where shapes go in the atlas this frame: shelves, filled from the top.
#[derive(Default)]
struct Shelves {
    size: u32,
    x: u32,
    y: u32,
    height: u32,
}

impl Shelves {
    /// A place of this size, growing the atlas up to `limit`. Places already given stay valid.
    fn place(&mut self, width: u32, height: u32, limit: u32) -> Option<(u32, u32)> {
        if width > limit || height > limit {
            return None;
        }
        while width > self.size {
            self.size = (self.size * 2).min(limit);
        }
        if self.x + width > self.size {
            self.y += self.height;
            self.x = 0;
            self.height = 0;
        }
        while self.y + height > self.size {
            if self.size >= limit {
                return None;
            }
            self.size = (self.size * 2).min(limit);
        }
        let place = (self.x, self.y);
        self.x += width;
        self.height = self.height.max(height);
        Some(place)
    }
}

pub(crate) struct PathEngine {
    coverage_format: wgpu::TextureFormat,
    edge_pipeline: wgpu::RenderPipeline,
    shape_pipeline: wgpu::RenderPipeline,
    target_buffer: wgpu::Buffer,
    target_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    texture_group: Option<wgpu::BindGroup>,
    coverage: Option<(wgpu::TextureView, u32)>,
    ramps: wgpu::Texture,
    ramps_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    edge_buffer: Instances,
    shape_buffer: Instances,
    edges: Vec<EdgeInstance>,
    shapes: Vec<ShapeInstance>,
    shelves: Shelves,
    ramp_rows: HashMap<Vec<u8>, u32>,
    ramp_pixels: Vec<(u32, Vec<u8>)>,
    images: wgpu::Texture,
    images_view: wgpu::TextureView,
    image_side: u32,
    image_shelves: Shelves,
    image_places: HashMap<u64, [u32; 2]>,
    image_uploads: Vec<ImageData>,
    points: Vec<Point>,
}

impl PathEngine {
    pub(crate) fn new(
        device: &wgpu::Device,
        globals_layout: &wgpu::BindGroupLayout,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let coverage_format = if device.features().contains(wgpu::Features::FLOAT32_BLENDABLE) {
            FINE_COVERAGE_FORMAT
        } else {
            COVERAGE_FORMAT
        };
        let target_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("raven path target"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let target_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("raven path target"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let target_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("raven path target"),
            layout: &target_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: target_buffer.as_entire_binding() }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("raven path textures"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let edge_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("raven path edges"),
            bind_group_layouts: &[Some(&target_layout)],
            immediate_size: 0,
        });
        let edge_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("raven path edges"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/path_coverage.wgsl").into()),
        });
        let edge_attributes = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4];
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let edge_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("raven path edges"),
            layout: Some(&edge_layout),
            vertex: wgpu::VertexState {
                module: &edge_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<EdgeInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &edge_attributes,
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &edge_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: coverage_format,
                    blend: Some(wgpu::BlendState { color: add, alpha: add }),
                    write_mask: wgpu::ColorWrites::RED,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let shape_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("raven path shapes"),
            bind_group_layouts: &[Some(globals_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let shape_attributes = wgpu::vertex_attr_array![
            0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4
        ];
        let shape_pipeline = crate::renderer::pipeline(
            device,
            "raven path shapes",
            include_str!("../shaders/path.wgsl"),
            &shape_layout,
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<ShapeInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &shape_attributes,
            },
            target_format,
        );
        let ramps = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("raven path ramps"),
            size: wgpu::Extent3d { width: RAMP_WIDTH, height: RAMP_ROWS, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let ramps_view = ramps.create_view(&wgpu::TextureViewDescriptor::default());
        let image_side = device.limits().max_texture_dimension_2d.min(IMAGE_ATLAS);
        let images = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("raven path images"),
            size: wgpu::Extent3d { width: image_side, height: image_side, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let images_view = images.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("raven path ramps"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            coverage_format,
            edge_pipeline,
            shape_pipeline,
            target_buffer,
            target_group,
            texture_layout,
            texture_group: None,
            coverage: None,
            ramps,
            ramps_view,
            sampler,
            edge_buffer: Instances::new(device, "raven path edges", 1 << 14),
            shape_buffer: Instances::new(device, "raven path shapes", 1 << 12),
            edges: Vec::new(),
            shapes: Vec::new(),
            shelves: Shelves { size: INITIAL_ATLAS, ..Shelves::default() },
            ramp_rows: HashMap::new(),
            ramp_pixels: Vec::new(),
            images,
            images_view,
            image_side,
            image_shelves: Shelves { size: image_side, ..Shelves::default() },
            image_places: HashMap::new(),
            image_uploads: Vec::new(),
            points: Vec::new(),
        }
    }

    pub(crate) fn begin_frame(&mut self) {
        self.edges.clear();
        self.shapes.clear();
        let size = self.shelves.size.max(INITIAL_ATLAS);
        self.shelves = Shelves { size, ..Shelves::default() };
    }

    pub(crate) fn shapes(&self) -> usize {
        self.shapes.len()
    }

    pub(crate) fn edges(&self) -> usize {
        self.edges.len()
    }

    /// Adds a shape, clipped to `scissor` in device pixels: x, y, width, height.
    pub(crate) fn push(&mut self, path: &PathFill, scale: f32, scissor: [u32; 4], limit: u32) {
        let [a, b, c, d, e, f] = path.transform;
        let device = |p: Point| Point::new((a * p.x + c * p.y + e) * scale, (b * p.x + d * p.y + f) * scale);
        self.points.clear();
        let contours = collect_contours(&path.elements, &device, &mut self.points);
        let Some(bounds) = bounds_of(&self.points) else { return };
        // Only what the clip lets through takes up the atlas.
        let left = bounds[0].floor().max(scissor[0] as f32);
        let top = bounds[1].floor().max(scissor[1] as f32);
        let right = bounds[2].ceil().min((scissor[0] + scissor[2]) as f32);
        let bottom = bounds[3].ceil().min((scissor[1] + scissor[3]) as f32);
        if right <= left || bottom <= top {
            return;
        }
        let (width, height) = ((right - left) as u32, (bottom - top) as u32);
        let Some((ax, ay)) = self.shelves.place(width, height, limit) else {
            tracing::warn!(width, height, "a shape does not fit in the coverage atlas");
            return;
        };
        let (shift_x, shift_y) = (ax as f32 - left, ay as f32 - top);
        let region = [ax as f32, ay as f32, (ax + width) as f32, (ay + height) as f32];
        add_edges(&self.points, &contours, (shift_x, shift_y), region, &mut self.edges);
        let clip = match &path.clip {
            None => [-1.0; 4],
            Some(clip) => {
                let mut points = Vec::new();
                let contours = collect_contours(&clip.elements, &device, &mut points);
                let Some((cx, cy)) = self.shelves.place(width, height, limit) else {
                    tracing::warn!(width, height, "a clip does not fit in the coverage atlas");
                    return;
                };
                let shift = (cx as f32 - left, cy as f32 - top);
                let region = [cx as f32, cy as f32, (cx + width) as f32, (cy + height) as f32];
                add_edges(&points, &contours, shift, region, &mut self.edges);
                let even_odd = if clip.rule == FillRule::EvenOdd { 1.0 } else { 0.0 };
                [cx as f32, cy as f32, even_odd, if clip.inverted { 1.0 } else { 0.0 }]
            }
        };
        let opacity = path.opacity.clamp(0.0, 1.0);
        let at = |p: Point| {
            let q = device(p);
            [q.x, q.y]
        };
        let mut image_row = [0.0; 2];
        let (kind, color, gradient, row) = match &path.paint {
            PathPaint::Solid(color) => (0.0, color.to_array(), [0.0; 4], 0.0),
            PathPaint::Linear { start, end, stops } => {
                let (s, t) = (at(*start), at(*end));
                (1.0, [0.0; 4], [s[0], s[1], t[0], t[1]], self.ramp(stops))
            }
            PathPaint::Radial { center, radius, stops } => {
                let s = at(*center);
                // A transform that does not keep circles is drawn with its mean scale.
                let k = ((a * d - b * c).abs().sqrt() * radius * scale).max(0.0);
                (2.0, [0.0; 4], [s[0], s[1], k, 0.0], self.ramp(stops))
            }
            PathPaint::Image { image, placement } => {
                let Some(place) = self.image_place(image) else { return };
                let [pa, pb, pc, pd, pe, pf] = *placement;
                // From the image's square to the device, and back for the shader.
                let (ma, mb) = (scale * (a * pa + c * pb), scale * (b * pa + d * pb));
                let (mc, md) = (scale * (a * pc + c * pd), scale * (b * pc + d * pd));
                let (me, mf) = (scale * (a * pe + c * pf + e), scale * (b * pe + d * pf + f));
                let det = ma * md - mb * mc;
                if det == 0.0 {
                    return;
                }
                let u = [md / det, -mc / det, (mc * mf - md * me) / det];
                let v = [-mb / det, ma / det, (mb * me - ma * mf) / det];
                image_row = [v[1], v[2]];
                (3.0, place, [u[0], u[1], u[2], v[0]], 0.0)
            }
            _ => return,
        };
        let even_odd = if path.rule == FillRule::EvenOdd { 1.0 } else { 0.0 };
        self.shapes.push(ShapeInstance {
            rect: [left, top, width as f32, height as f32],
            atlas: [ax as f32, ay as f32, even_odd, kind],
            color,
            gradient,
            params: [row, opacity, image_row[0], image_row[1]],
            clip,
        });
    }

    /// Where this image is in the atlas, as a texture coordinate and size that stay
    /// half a pixel inside it, placing it on first use.
    fn image_place(&mut self, image: &ImageData) -> Option<[f32; 4]> {
        let (width, height) = (image.width(), image.height());
        let at = match self.image_places.get(&image.id()) {
            Some(at) => *at,
            None => {
                let side = self.image_side;
                let fits = |shelves: &mut Shelves| shelves.place(width + IMAGE_GAP, height + IMAGE_GAP, side);
                let place = fits(&mut self.image_shelves).or_else(|| {
                    // Full: the oldest images go, and come back as they are used.
                    self.image_places.clear();
                    self.image_shelves = Shelves { size: side, ..Shelves::default() };
                    fits(&mut self.image_shelves)
                });
                let Some((x, y)) = place else {
                    tracing::warn!(width, height, "an image does not fit in the path atlas");
                    return None;
                };
                self.image_places.insert(image.id(), [x, y]);
                self.image_uploads.push(image.clone());
                [x, y]
            }
        };
        let side = self.image_side as f32;
        Some([
            (at[0] as f32 + 0.5) / side,
            (at[1] as f32 + 0.5) / side,
            (width as f32 - 1.0).max(0.0) / side,
            (height as f32 - 1.0).max(0.0) / side,
        ])
    }

    /// The row of the ramp texture holding these stops, as a texture coordinate.
    fn ramp(&mut self, stops: &[ColorStop]) -> f32 {
        let mut pixels = Vec::with_capacity(RAMP_WIDTH as usize * 4);
        for x in 0..RAMP_WIDTH {
            let color = color_at(stops, x as f32 / (RAMP_WIDTH - 1) as f32);
            pixels.extend(color.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
        }
        let next = self.ramp_rows.len() as u32;
        let row = match self.ramp_rows.get(&pixels) {
            Some(row) => *row,
            None => {
                // Full: the oldest colors go, and this frame's come back as they are used.
                let row = if next >= RAMP_ROWS {
                    self.ramp_rows.clear();
                    0
                } else {
                    next
                };
                self.ramp_rows.insert(pixels.clone(), row);
                self.ramp_pixels.push((row, pixels));
                row
            }
        };
        (row as f32 + 0.5) / RAMP_ROWS as f32
    }

    /// Uploads this frame's shapes and adds up their coverage.
    pub(crate) fn draw_coverage(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.shapes.is_empty() {
            return;
        }
        for (row, pixels) in self.ramp_pixels.drain(..) {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.ramps,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: row, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(RAMP_WIDTH * 4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d { width: RAMP_WIDTH, height: 1, depth_or_array_layers: 1 },
            );
        }
        for image in self.image_uploads.drain(..) {
            let Some(at) = self.image_places.get(&image.id()) else { continue };
            // Premultiplied, so that blending neighbors does not bring in the color
            // of what is fully transparent.
            let mut pixels = image.pixels().to_vec();
            for pixel in pixels.as_chunks_mut::<4>().0 {
                let alpha = u32::from(pixel[3]);
                for channel in &mut pixel[..3] {
                    *channel = ((u32::from(*channel) * alpha + 127) / 255) as u8;
                }
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.images,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: at[0], y: at[1], z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(image.width() * 4),
                    rows_per_image: Some(image.height()),
                },
                wgpu::Extent3d { width: image.width(), height: image.height(), depth_or_array_layers: 1 },
            );
        }
        let size = self.shelves.size;
        if self.coverage.as_ref().is_none_or(|(_, current)| *current != size) {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("raven path coverage"),
                size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.coverage_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.texture_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("raven path textures"),
                layout: &self.texture_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.ramps_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.images_view),
                    },
                ],
            }));
            self.coverage = Some((view, size));
        }
        queue.write_buffer(
            &self.target_buffer,
            0,
            bytemuck::cast_slice(&[size as f32, size as f32, 0.0, 0.0]),
        );
        self.edge_buffer.upload(device, queue, bytemuck::cast_slice(&self.edges));
        self.shape_buffer.upload(device, queue, bytemuck::cast_slice(&self.shapes));
        let Some((view, _)) = &self.coverage else { return };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("raven path coverage"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.edge_pipeline);
        pass.set_bind_group(0, &self.target_group, &[]);
        pass.set_vertex_buffer(0, self.edge_buffer.buffer.slice(..));
        pass.draw(0..4, 0..self.edges.len() as u32);
    }

    /// Readies the main pass to paint shapes.
    pub(crate) fn bind(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.shape_pipeline);
        if let Some(group) = &self.texture_group {
            pass.set_bind_group(1, group, &[]);
        }
        pass.set_vertex_buffer(0, self.shape_buffer.buffer.slice(..));
    }
}

/// The outline as closed lists of device points, appended to `points`.
fn collect_contours(
    elements: &[PathElement],
    device: &impl Fn(Point) -> Point,
    points: &mut Vec<Point>,
) -> Vec<std::ops::Range<usize>> {
    let mut contours = Vec::new();
    let mut start = points.len();
    let mut pen = Point::new(0.0, 0.0);
    for element in elements {
        match *element {
            PathElement::MoveTo(p) => {
                if points.len() > start {
                    contours.push(start..points.len());
                }
                start = points.len();
                pen = device(p);
                points.push(pen);
            }
            PathElement::LineTo(p) => {
                pen = device(p);
                points.push(pen);
            }
            PathElement::CubicTo(p1, p2, p3) => {
                let (p1, p2, p3) = (device(p1), device(p2), device(p3));
                flatten(pen, p1, p2, p3, points);
                pen = p3;
            }
            PathElement::Close => {
                if points.len() > start {
                    contours.push(start..points.len());
                    pen = points[start];
                }
                start = points.len();
            }
        }
    }
    if points.len() > start {
        contours.push(start..points.len());
    }
    contours
}

/// Adds a shape's edges for its place in the atlas. Edges left of the place still
/// count for what is to their right, and those above or below it do not.
fn add_edges(
    points: &[Point],
    contours: &[std::ops::Range<usize>],
    shift: (f32, f32),
    region: [f32; 4],
    edges: &mut Vec<EdgeInstance>,
) {
    for contour in contours {
        let points = &points[contour.clone()];
        for (i, from) in points.iter().enumerate() {
            let to = points[(i + 1) % points.len()];
            if from.y == to.y {
                continue;
            }
            let (y0, y1) = (from.y + shift.1, to.y + shift.1);
            if y0.max(y1) <= region[1] || y0.min(y1) >= region[3] {
                continue;
            }
            edges.push(EdgeInstance { line: [from.x + shift.0, y0, to.x + shift.0, y1], region });
        }
    }
}

/// The curve as straight pieces, the number of them from how much it bends.
fn flatten(p0: Point, p1: Point, p2: Point, p3: Point, out: &mut Vec<Point>) {
    let ddx = (p0.x - 2.0 * p1.x + p2.x).abs().max((p1.x - 2.0 * p2.x + p3.x).abs());
    let ddy = (p0.y - 2.0 * p1.y + p2.y).abs().max((p1.y - 2.0 * p2.y + p3.y).abs());
    let bend = (ddx * ddx + ddy * ddy).sqrt();
    let pieces = ((0.75 * bend / FLATNESS).sqrt().ceil() as u32).clamp(1, MOST_PIECES);
    for i in 1..=pieces {
        let t = i as f32 / pieces as f32;
        let u = 1.0 - t;
        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        out.push(Point::new(
            a * p0.x + b * p1.x + c * p2.x + d * p3.x,
            a * p0.y + b * p1.y + c * p2.y + d * p3.y,
        ));
    }
}

fn bounds_of(points: &[Point]) -> Option<[f32; 4]> {
    let first = points.first()?;
    let mut bounds = [first.x, first.y, first.x, first.y];
    for p in points {
        bounds[0] = bounds[0].min(p.x);
        bounds[1] = bounds[1].min(p.y);
        bounds[2] = bounds[2].max(p.x);
        bounds[3] = bounds[3].max(p.y);
    }
    Some(bounds)
}

fn color_at(stops: &[ColorStop], t: f32) -> [f32; 4] {
    let Some(first) = stops.first() else { return [0.0; 4] };
    if t <= first.offset {
        return first.color.to_array();
    }
    for pair in stops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if t <= b.offset {
            let k = if b.offset > a.offset { (t - a.offset) / (b.offset - a.offset) } else { 1.0 };
            let (from, to) = (a.color.to_array(), b.color.to_array());
            return [0, 1, 2, 3].map(|i| from[i] + (to[i] - from[i]) * k);
        }
    }
    stops.last().map_or([0.0; 4], |stop| stop.color.to_array())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_in_the_atlas_do_not_overlap_and_it_grows_up_to_its_limit() {
        let mut shelves = Shelves { size: 64, ..Shelves::default() };
        assert_eq!(shelves.place(40, 10, 256), Some((0, 0)));
        assert_eq!(shelves.place(40, 20, 256), Some((0, 10)));
        assert_eq!(shelves.place(10, 5, 256), Some((40, 10)));
        assert_eq!(shelves.place(200, 10, 256), Some((50, 10)), "the atlas grew to take it");
        assert_eq!(shelves.size, 256);
        assert_eq!(shelves.place(300, 10, 256), None);
    }

    #[test]
    fn a_tighter_curve_takes_more_pieces_and_ends_exactly_where_it_should() {
        let mut gentle = Vec::new();
        let p = |x, y| Point::new(x, y);
        flatten(p(0.0, 0.0), p(1.0, 1.0), p(2.0, 1.0), p(3.0, 0.0), &mut gentle);
        let mut tight = Vec::new();
        flatten(p(0.0, 0.0), p(100.0, 300.0), p(200.0, 300.0), p(300.0, 0.0), &mut tight);
        assert!(gentle.len() < tight.len());
        assert_eq!(tight.last().copied(), Some(p(300.0, 0.0)));
    }

    #[test]
    fn a_ramp_runs_between_its_stops_and_holds_outside_them() {
        let stop = |offset, v| ColorStop { offset, color: ravenrs_core::Color::rgba(v, v, v, 1.0) };
        let stops = [stop(0.25, 0.0), stop(0.75, 1.0)];
        assert_eq!(color_at(&stops, 0.0)[0], 0.0);
        assert_eq!(color_at(&stops, 0.5)[0], 0.5);
        assert_eq!(color_at(&stops, 1.0)[0], 1.0);
    }
}
