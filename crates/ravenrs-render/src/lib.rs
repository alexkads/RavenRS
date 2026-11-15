#![doc = include_str!("../README.md")]

mod atlas;
mod gpu;
mod images;
mod offscreen;
mod paths;
mod renderer;
mod timing;

pub use gpu::{GpuContext, GpuError, gpu_report, preferred_surface_format, surface_report};
pub use offscreen::render_to_rgba;
pub use renderer::{RenderStats, Renderer};
pub use wgpu;
