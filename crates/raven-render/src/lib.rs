//! The RavenRS render engine.
//!
//! [`Renderer`] turns a [`raven_core::Scene`] into one wgpu render pass:
//!
//! * **Quads** (rounded rectangles, borders, linear gradients, soft shadows) are
//!   instanced and shaded with a signed-distance function (`shaders/quad.wgsl`),
//!   so edges are anti-aliased analytically at any scale factor.
//! * **Text** is rasterized once per glyph/size/subpixel offset with `swash` into
//!   an R8 atlas and drawn as textured quads (`shaders/glyph.wgsl`).
//!
//! Consecutive primitives of the same kind are batched; painter's order is kept.
//! Instance buffers are reused and only grow. Nothing is read back from the GPU.
//!
//! The crate does not own windows or surfaces: platform crates create the
//! device and call [`Renderer::render`] with the frame's texture view.

mod atlas;
mod gpu;
mod renderer;

pub use gpu::{GpuContext, GpuError, preferred_surface_format};
pub use renderer::{RenderStats, Renderer};
pub use wgpu;
