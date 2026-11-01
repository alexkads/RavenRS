#![doc = include_str!("../README.md")]
//!
//! ## Crates behind this facade
//!
//! [`core`], [`animation`], [`reactive`], [`layout`], [`text`], [`style`], [`render`] and [`widgets`]
//! can also be used on their own.

/// Keeps the code in the skill for AI assistants compiling.
#[cfg(doctest)]
#[doc = include_str!("../../../plugins/ravenrs/skills/ravenrs/SKILL.md")]
struct SkillDoctests;

pub use ravenrs_animation as animation;
pub use ravenrs_core as core;
pub use ravenrs_layout as layout;
pub use ravenrs_reactive as reactive;
pub use ravenrs_render as render;
pub use ravenrs_style as style;
pub use ravenrs_text as text;
pub use ravenrs_widgets as widgets;

/// Desktop windows (Windows, macOS, Linux).
#[cfg(not(target_arch = "wasm32"))]
pub mod native {
    pub use ravenrs_platform_native::{
        AppIdentity, PlatformError, SaveDialog, WindowOptions, diagnose, open_with_system, run, run_as,
        run_with_icon,
    };
}

/// The browser: a `<canvas>` driven by WebGPU.
#[cfg(target_arch = "wasm32")]
pub mod web {
    pub use ravenrs_platform_web::{WebApp, WebError, WebOptions, start};
}

/// Everything an app usually needs, in one import.
pub mod prelude {
    pub use ravenrs_animation::{Animated, Easing, Spring, Transition};
    pub use ravenrs_core::{
        AccessAction, AccessTree, Color, CursorIcon, Effect, ImageData, ImageQuad, Insets, LinearGradient,
        Point, Quad, Rect, Role, Scene, Shadow, Size, Toggled,
    };
    pub use ravenrs_layout::{Align, Justify};
    pub use ravenrs_reactive::{Binding, Derived, State};
    pub use ravenrs_style::{Semantic, TextStyle, Theme, Weight};
    pub use ravenrs_widgets::*;
}
