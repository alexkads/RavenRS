#![doc = include_str!("../README.md")]

mod atlas;
mod cables;
mod gpu;
mod images;
mod offscreen;
mod paths;
mod renderer;
mod timing;

pub use cables::{CableDrawer, GpuCables};
pub use gpu::{GpuContext, GpuError, gpu_report, preferred_surface_format, surface_report};
pub use offscreen::{render_to_rgba, render_with};
pub use renderer::{RenderStats, Renderer};
pub use wgpu;
