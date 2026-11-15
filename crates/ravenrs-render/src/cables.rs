//! The window's cables on the GPU: [`GpuCables`] runs every cable's steps in one
//! compute dispatch (`shaders/cable_sim.wgsl`) and the renderer draws them straight
//! from the same buffer (`shaders/cable.wgsl`). The CPU keeps the book (which cables
//! there are, their pins, how many steps are due) and reads back, without waiting,
//! four bytes of "still moving" and the points, a frame late.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use bytemuck::{Pod, Zeroable};
use ravenrs_core::cable::{CABLE_MAX_POINTS, Cable, CableBook, CableTexture, CableWorld};
use ravenrs_core::{Point, Rect};

/// Whether this device can draw cables from a storage buffer: not WebGL2.
pub(crate) fn cables_drawable(device: &wgpu::Device) -> bool {
    device.limits().max_storage_buffers_per_shader_stage >= 4
}

/// Pins one cable may have on the GPU.
const MAX_PINS: usize = 8;
const POINT_BYTES: u64 = 16;
/// Where the points start in a read-back buffer, after the four bytes of movement.
const POINTS_AT: u64 = 256;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct Params {
    n: u32,
    steps: u32,
    pins: u32,
    _pad0: u32,
    rest: f32,
    fall: f32,
    keep: f32,
    stiffness: f32,
    friction: f32,
    _pad1: [f32; 3],
    pin: [[f32; 4]; MAX_PINS],
}

/// How the renderer sees one cable it draws.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct CableLookGpu {
    slot: u32,
    n: u32,
    texture: u32,
    dashed: u32,
    color: [f32; 4],
    stripe: [f32; 4],
    params: [f32; 4],
    extra: [f32; 4],
}

impl CableLookGpu {
    /// The look of `cable` in `slot`, or `None` when the GPU cannot draw its surface
    /// (a picture), and the shapes must.
    pub(crate) fn of(cable: &Cable, slot: u32, n: u32) -> Option<Self> {
        let look = &cable.look;
        let (texture, stripe) = match &look.texture {
            CableTexture::Smooth => (0, look.color),
            CableTexture::Braided => (1, look.color),
            CableTexture::Coiled => (2, look.color),
            CableTexture::Striped(c) => (3, *c),
            CableTexture::Image { .. } => return None,
            #[allow(unreachable_patterns)]
            _ => return None,
        };
        let [dash, gap] = look.dash.unwrap_or([0.0, 0.0]);
        Some(Self {
            slot,
            n,
            texture,
            dashed: u32::from(look.dash.is_some()),
            color: look.color.to_array(),
            stripe: stripe.to_array(),
            params: [look.thickness * cable.view.scale, look.shine, dash, gap],
            extra: [look.opacity.clamp(0.0, 1.0), cable.view.scale, cable.view.offset.x, cable.view.offset.y],
        })
    }
}

struct Readback {
    buffer: wgpu::Buffer,
    /// 0 free, 1 on its way, 2 ready to read, 3 failed.
    state: Arc<AtomicU8>,
    /// Which cable was in which slot, with how many points, when it was copied.
    slots: Vec<(u64, u32, usize)>,
}

struct Inner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    capacity: u32,
    state: wgpu::Buffer,
    params: wgpu::Buffer,
    shape: wgpu::Buffer,
    moved: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Grows whenever the shape buffer is made again, so the renderer rebinds it.
    generation: u64,
    slots: HashMap<u64, u32>,
    free: Vec<u32>,
    next: u32,
    book: CableBook,
    readbacks: Vec<Readback>,
    dispatches: u64,
}

fn buffers(device: &wgpu::Device, capacity: u32) -> [wgpu::Buffer; 3] {
    let points = u64::from(capacity) * CABLE_MAX_POINTS as u64 * POINT_BYTES;
    let make = |label, size, usage| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        })
    };
    let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
    [
        make("raven cable state", points, storage),
        make("raven cable params", u64::from(capacity) * std::mem::size_of::<Params>() as u64, storage),
        make("raven cable shape", points, storage),
    ]
}

impl Inner {
    fn bind(&self) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("raven cable simulation"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.state.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.shape.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.moved.as_entire_binding() },
            ],
        })
    }

    /// Makes room for `slots` cables. The points start over from the book's.
    fn grow(&mut self, slots: u32) {
        if slots <= self.capacity {
            return;
        }
        self.capacity = slots.next_power_of_two().max(64);
        [self.state, self.params, self.shape] = buffers(&self.device, self.capacity);
        self.bind_group = self.bind();
        self.generation += 1;
        for (id, slot) in self.slots.clone() {
            if let Some(points) = self.book.shape(id) {
                self.upload(slot, &points);
            }
        }
    }

    fn upload(&self, slot: u32, points: &[Point]) {
        let data: Vec<[f32; 4]> = points.iter().map(|p| [p.x, p.y, p.x, p.y]).collect();
        let at = u64::from(slot) * CABLE_MAX_POINTS as u64 * POINT_BYTES;
        self.queue.write_buffer(&self.state, at, bytemuck::cast_slice(&data));
    }

    /// Reads what the GPU sent back since the last frame.
    fn collect(&mut self) {
        let _ = self.device.poll(wgpu::PollType::Poll);
        for rb in &mut self.readbacks {
            match rb.state.load(Ordering::Acquire) {
                2 => {
                    if let Ok(bytes) = rb.buffer.slice(..).get_mapped_range() {
                        let moved = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                        self.book.set_moved(moved as f32 / 1000.0);
                        for &(id, slot, n) in &rb.slots {
                            let at = (POINTS_AT + u64::from(slot) * CABLE_MAX_POINTS as u64 * POINT_BYTES)
                                as usize;
                            let Some(raw) = bytes.get(at..at + n * POINT_BYTES as usize) else { continue };
                            let points: Vec<Point> = bytemuck::cast_slice::<u8, [f32; 4]>(raw)
                                .iter()
                                .map(|p| Point::new(p[0], p[1]))
                                .collect();
                            if self.slots.get(&id) == Some(&slot) {
                                self.book.set_points(id, &points);
                            }
                        }
                    }
                    rb.buffer.unmap();
                    rb.state.store(0, Ordering::Release);
                }
                3 => rb.state.store(0, Ordering::Release),
                _ => {}
            }
        }
    }

    /// Gives every cable of the book a slot, and frees the slots of those gone.
    fn assign(&mut self) {
        let all = self.book.all();
        let keep: std::collections::HashSet<u64> = all.iter().copied().collect();
        let gone: Vec<u64> = self.slots.keys().filter(|id| !keep.contains(id)).copied().collect();
        for id in gone {
            if let Some(slot) = self.slots.remove(&id) {
                self.free.push(slot);
            }
        }
        for id in all {
            if !self.slots.contains_key(&id) {
                let slot = self.free.pop().unwrap_or_else(|| {
                    self.next += 1;
                    self.next - 1
                });
                self.slots.insert(id, slot);
            }
        }
        self.grow(self.next);
    }

    /// A read-back buffer that is free and big enough, made if need be; `None` while
    /// all three are on their way.
    fn readback_for(&mut self, size: u64) -> Option<usize> {
        let free = |rb: &Readback| rb.state.load(Ordering::Acquire) == 0;
        if let Some(i) = self.readbacks.iter().position(|rb| free(rb) && rb.buffer.size() >= size) {
            return Some(i);
        }
        let fresh = Readback {
            buffer: self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("raven cable readback"),
                size: size.next_power_of_two(),
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            state: Arc::new(AtomicU8::new(0)),
            slots: Vec::new(),
        };
        if let Some(i) = self.readbacks.iter().position(free) {
            self.readbacks[i] = fresh;
            Some(i)
        } else if self.readbacks.len() < 3 {
            self.readbacks.push(fresh);
            Some(self.readbacks.len() - 1)
        } else {
            None
        }
    }

    fn run(&mut self, steps: u64) {
        self.assign();
        for (id, points) in self.book.take_placed() {
            if let Some(&slot) = self.slots.get(&id) {
                self.upload(slot, &points);
            }
        }
        if self.slots.is_empty() {
            return;
        }
        let mut params = vec![Params::zeroed(); self.next as usize];
        for (&id, &slot) in &self.slots {
            if let Some(points) = self
                .book
                .shape(id)
                .or_else(|| self.book.leaving().into_iter().find(|c| c.id == id).and_then(|c| c.points))
            {
                params[slot as usize].n = points.len() as u32;
            }
        }
        for plan in self.book.plans() {
            let Some(&slot) = self.slots.get(&plan.id) else { continue };
            let p = &mut params[slot as usize];
            p.n = plan.points as u32;
            p.steps = steps as u32;
            p.rest = plan.step.rest;
            p.fall = plan.step.fall;
            p.keep = plan.step.keep;
            p.stiffness = plan.step.stiffness;
            p.friction = plan.step.friction;
            p.pins = plan.pins.len().min(MAX_PINS) as u32;
            for (k, (index, at)) in plan.pins.iter().take(MAX_PINS).enumerate() {
                p.pin[k] = [at.x, at.y, *index as f32, 0.0];
            }
        }
        self.queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&params));
        self.queue.write_buffer(&self.moved, 0, &[0; 4]);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("raven cables") });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("raven cable steps"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups(self.next, 1, 1);
        }
        self.dispatches += 1;
        // Read back, without waiting, into a free buffer: none free, the next frame does.
        let size = POINTS_AT + u64::from(self.next) * CABLE_MAX_POINTS as u64 * POINT_BYTES;
        let index = self.readback_for(size);
        if let Some(i) = index {
            encoder.copy_buffer_to_buffer(&self.moved, 0, &self.readbacks[i].buffer, 0, 4);
            encoder.copy_buffer_to_buffer(
                &self.shape,
                0,
                &self.readbacks[i].buffer,
                POINTS_AT,
                size - POINTS_AT,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        if let Some(i) = index {
            let counts: HashMap<u64, usize> =
                self.book.plans().into_iter().map(|p| (p.id, p.points)).collect();
            let rb = &mut self.readbacks[i];
            rb.slots = self
                .slots
                .iter()
                .filter_map(|(&id, &slot)| counts.get(&id).map(|&n| (id, slot, n)))
                .collect();
            rb.state.store(1, Ordering::Release);
            let state = rb.state.clone();
            rb.buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
                state.store(if result.is_ok() { 2 } else { 3 }, Ordering::Release);
            });
        }
    }
}

/// The cables of a window, simulated on the GPU. Give it to the `Ui` with
/// `set_cable_world` and its [`drawer`](Self::drawer) to the [`Renderer`](crate::Renderer).
pub struct GpuCables(Rc<RefCell<Inner>>);

/// What the renderer needs to draw the cables of a [`GpuCables`].
#[derive(Clone)]
pub struct CableDrawer(Rc<RefCell<Inner>>);

impl std::fmt::Debug for GpuCables {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GpuCables")
    }
}

impl std::fmt::Debug for CableDrawer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CableDrawer")
    }
}

impl GpuCables {
    /// A world on this device. `None` where the device runs no compute shaders
    /// (WebGL2): there the CPU world does the work.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if device.limits().max_compute_workgroups_per_dimension == 0 || !cables_drawable(device) {
            return None;
        }
        let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("raven cable simulation"),
            entries: &[storage(0, false), storage(1, true), storage(2, false), storage(3, false)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("raven cable simulation"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("raven cable simulation"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/cable_sim.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("raven cable simulation"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("step"),
            compilation_options: Default::default(),
            cache: None,
        });
        let capacity = 64;
        let [state, params, shape] = buffers(device, capacity);
        let moved = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("raven cable movement"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("raven cable simulation"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: state.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: shape.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: moved.as_entire_binding() },
            ],
        });
        Some(Self(Rc::new(RefCell::new(Inner {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            layout,
            capacity,
            state,
            params,
            shape,
            moved,
            bind_group,
            generation: 0,
            slots: HashMap::new(),
            free: Vec::new(),
            next: 0,
            book: CableBook::new(),
            readbacks: Vec::new(),
            dispatches: 0,
        }))))
    }

    /// The handle the renderer draws the cables with.
    pub fn drawer(&self) -> CableDrawer {
        CableDrawer(self.0.clone())
    }

    /// Compute dispatches run so far.
    pub fn dispatches(&self) -> u64 {
        self.0.borrow().dispatches
    }

    /// Waits for the GPU and reads back what it has: for tests, which cannot wait for
    /// the next frame.
    pub fn finish(&self) {
        let inner = self.0.borrow();
        let _ = inner.device.poll(wgpu::PollType::wait_indefinitely());
        drop(inner);
        self.0.borrow_mut().collect();
    }
}

impl CableWorld for GpuCables {
    fn sync(&mut self, cables: &[Cable], view: Rect, reduced: bool) {
        self.0.borrow_mut().book.sync(cables, view, reduced);
    }

    fn step(&mut self, dt: f32) {
        let mut inner = self.0.borrow_mut();
        inner.collect();
        let steps = inner.book.due(dt);
        inner.run(steps);
        inner.book.advance(steps);
    }

    fn moving(&self) -> bool {
        self.0.borrow().book.moving()
    }

    fn shape(&self, id: u64) -> Option<Arc<[Point]>> {
        self.0.borrow().book.shape(id)
    }

    fn ids(&self) -> Vec<u64> {
        self.0.borrow().book.ids()
    }

    fn leaving(&self) -> Vec<Cable> {
        self.0.borrow().book.leaving()
    }

    fn on_gpu(&self) -> bool {
        true
    }
}

impl CableDrawer {
    /// The slot and point count of a cable, if the GPU has it.
    pub(crate) fn slot(&self, cable: &Cable) -> Option<(u32, u32)> {
        let inner = self.0.borrow();
        let slot = *inner.slots.get(&cable.id)?;
        let n = cable.points.as_ref().map_or(cable.physics.points as usize, |p| p.len());
        Some((slot, n.clamp(2, CABLE_MAX_POINTS) as u32))
    }

    /// The buffer with the points, and its generation.
    pub(crate) fn shape(&self) -> (wgpu::Buffer, u64) {
        let inner = self.0.borrow();
        (inner.shape.clone(), inner.generation)
    }
}
