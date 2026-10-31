//! # RavenRS
//!
//! Beautiful native interfaces, powered by Rust. Write the UI once with
//! declarative views and run it in a native window or in the browser
//! (WebAssembly + WebGPU), drawn by the same GPU renderer.
//!
//! ```no_run
//! use raven::prelude::*;
//!
//! fn app() -> impl View {
//!     let count = State::new(0);
//!     let c = count.clone();
//!     VStack::new((
//!         Text::new("Welcome to RavenRS").style(TextStyle::Display),
//!         Card::new(Text::bound(&count, |n| format!("Clicks: {n}")).style(TextStyle::Metric)),
//!         Button::new("Get Started").on_click(move || c.update(|n| *n += 1)),
//!     ))
//!     .spacing(24.0)
//!     .padding(32.0)
//! }
//!
//! # #[cfg(not(target_arch = "wasm32"))]
//! fn main() -> Result<(), raven::native::PlatformError> {
//!     raven::native::run(app(), raven::native::WindowOptions::new("Hello"))
//! }
//! # #[cfg(target_arch = "wasm32")]
//! # fn main() {}
//! ```
//!
//! The crates behind this facade can also be used on their own:
//! [`core`], [`reactive`], [`layout`], [`text`], [`style`], [`render`], [`widgets`].

pub use raven_core as core;
pub use raven_layout as layout;
pub use raven_reactive as reactive;
pub use raven_render as render;
pub use raven_style as style;
pub use raven_text as text;
pub use raven_widgets as widgets;

/// Desktop windows (Windows, macOS, Linux).
#[cfg(not(target_arch = "wasm32"))]
pub mod native {
    pub use raven_platform_native::{PlatformError, WindowOptions, run};
}

/// The browser: a `<canvas>` driven by WebGPU.
#[cfg(target_arch = "wasm32")]
pub mod web {
    pub use raven_platform_web::{WebApp, WebError, WebOptions, start};
}

/// Everything an app usually needs, in one import.
pub mod prelude {
    pub use raven_core::{Color, CursorIcon, Insets, LinearGradient, Point, Quad, Rect, Scene, Shadow, Size};
    pub use raven_layout::{Align, Justify};
    pub use raven_reactive::{Binding, Derived, State};
    pub use raven_style::{Semantic, TextStyle, Theme, Weight};
    pub use raven_widgets::{
        AnyView, BuildCx, Button, ButtonStyle, Canvas, Card, Component, Fill, GlassCard, HStack, NodeId,
        Paint, QuadPaint, Spacer, Text, Ui, VStack, View, ViewSeq, view_fn,
    };
}
