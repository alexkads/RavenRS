//! Motion primitives: pure functions of time, with no timers or frame loop.
//!
//! [`Animated`] is the center of the crate: a value that heads for a target
//! under a [`Transition`] and can be sent somewhere else at any moment without
//! a jump in position or velocity.
//!
//! ```
//! use ravenrs_animation::{Animated, Transition};
//!
//! let mut x = Animated::new(0.0_f32);
//! x.set_target(100.0, Transition::snappy(), 0.0);
//! assert!(x.value(0.05) > 0.0);
//! // Interrupted halfway: it turns around smoothly, from where it is.
//! let before = x.value(0.1);
//! x.set_target(0.0, Transition::snappy(), 0.1);
//! assert!((x.value(0.1) - before).abs() < 1e-3);
//! ```

mod animated;
mod easing;
mod oklab;
mod spring;

pub use animated::{Animatable, Animated, Transition};
pub use easing::{CubicBezier, Easing};
pub use oklab::{from_oklab, mix_oklab, to_oklab};
pub use spring::Spring;
