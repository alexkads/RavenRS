//! Declarative views and the UI runtime.
//!
//! # Writing components
//!
//! There are three ways to make your own component, from simplest to most powerful:
//!
//! 1. **A function** returning `impl View`, composing existing views. Good for
//!    stateless pieces of UI.
//! 2. **A [`Component`]**: a struct holding its state ([`State`](raven_reactive::State)
//!    fields, bindings, callbacks) with a `body()` that composes views. This is the
//!    SwiftUI-style building block and covers most needs.
//! 3. **A [`View`]** implemented directly: full control over layout boxes,
//!    painting (including arbitrary [`Scene`](raven_core::Scene) drawing through
//!    [`Paint::Custom`]), hit-testing, focus and click handling via [`BuildCx`].
//!    The built-in widgets are written this way, with no private API.
//!
//! [`Canvas`] is the shortcut for custom drawing inside a normal layout.
//!
//! # Runtime
//!
//! [`Ui`] owns the root view and turns it into a [`Scene`](raven_core::Scene):
//! *build* (views to elements, tracked by a reactive observer), *layout* (Taffy),
//! *paint*. It only rebuilds when state the last build read has changed or the
//! interaction state (hover, press, focus) changed, and it never touches the GPU.

mod button;
mod canvas;
mod card;
mod context;
mod fill;
mod stack;
mod text;
mod ui;
mod view;

pub use button::{Button, ButtonStyle};
pub use canvas::Canvas;
pub use card::{Card, GlassCard};
pub use context::{BuildCx, DrawFn, NodeId, Paint, QuadPaint, WidgetId};
pub use fill::Fill;
pub use stack::{HStack, Spacer, VStack};
pub use text::Text;
pub use ui::Ui;
pub use view::{AnyView, Component, FnView, View, ViewSeq, view_fn};
