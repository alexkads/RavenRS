//! Asks the GPU how long a frame took, without ever waiting for the answer.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

/// Main pass start and end, then coverage pass start and end.
const QUERIES: u32 = 4;
const QUERIES_SIZE: u64 = QUERIES as u64 * 8;
/// Answers in flight. When all are taken, frames go unmeasured rather than wait.
const SLOTS: usize = 4;

const FREE: u8 = 0;
const MAPPING: u8 = 1;
const READY: u8 = 2;

struct Slot {
    buffer: wgpu::Buffer,
    state: Arc<AtomicU8>,
}

/// Timestamps around the frame's passes. Answers arrive frames later through buffers
/// mapped in the background.
pub(crate) struct GpuTimer {
    device: wgpu::Device,
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    slots: Vec<Slot>,
    /// The slot this frame's answer was copied to, to map after the submit.
    copied: Option<usize>,
    /// Nanoseconds in one tick of the GPU's clock.
    period: f64,
    latest: Option<Duration>,
}

impl GpuTimer {
    /// A timer, unless this GPU cannot tell the time.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("raven frame timing"),
            ty: wgpu::QueryType::Timestamp,
            count: QUERIES,
        });
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("raven frame timing"),
            size: QUERIES_SIZE,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let slots = (0..SLOTS)
            .map(|_| Slot {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("raven frame timing answer"),
                    size: QUERIES_SIZE,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                state: Arc::new(AtomicU8::new(FREE)),
            })
            .collect();
        Some(Self {
            device: device.clone(),
            queries,
            resolve,
            slots,
            copied: None,
            period: f64::from(queue.get_timestamp_period()),
            latest: None,
        })
    }

    pub fn main_pass(&self) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        }
    }

    pub fn coverage_pass(&self) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(2),
            end_of_pass_write_index: Some(3),
        }
    }

    /// Collects answers that came in and records this frame's question. Returns the
    /// latest answer known.
    pub fn finish_frame(&mut self, encoder: &mut wgpu::CommandEncoder) -> Option<Duration> {
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.collect();
        self.copied = self.slots.iter().position(|slot| slot.state.load(Ordering::Acquire) == FREE);
        encoder.resolve_query_set(&self.queries, 0..QUERIES, &self.resolve, 0);
        if let Some(i) = self.copied {
            encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.slots[i].buffer, 0, QUERIES_SIZE);
        }
        self.latest
    }

    /// Asks for the answer of the frame just submitted.
    pub fn frame_submitted(&mut self) {
        let Some(i) = self.copied.take() else { return };
        let slot = &self.slots[i];
        slot.state.store(MAPPING, Ordering::Release);
        let state = slot.state.clone();
        slot.buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
            state.store(if result.is_ok() { READY } else { FREE }, Ordering::Release);
        });
    }

    fn collect(&mut self) {
        let period = self.period;
        for slot in &self.slots {
            if slot.state.load(Ordering::Acquire) != READY {
                continue;
            }
            let mut ticks = [0u64; QUERIES as usize];
            if let Ok(bytes) = slot.buffer.slice(..).get_mapped_range() {
                let (chunks, _) = bytes.as_chunks::<8>();
                for (tick, raw) in ticks.iter_mut().zip(chunks) {
                    *tick = u64::from_le_bytes(*raw);
                }
            }
            slot.buffer.unmap();
            slot.state.store(FREE, Ordering::Release);
            let total = ticks[1].saturating_sub(ticks[0]) + ticks[3].saturating_sub(ticks[2]);
            self.latest = Some(Duration::from_nanos((total as f64 * period) as u64));
        }
    }
}
