//! Reactive state for RavenRS.
//!
//! The model is deliberately single-threaded (`Rc`/`Cell`, no locks): UI state
//! lives on the UI thread on every platform, including the browser main thread.
//!
//! * [`State<T>`] owns a value. Reading it inside [`Observer::track`] records a
//!   dependency; writing it invalidates only the observers that read it.
//! * [`Binding<T>`] is a read/write view of some state that a child component can
//!   receive without knowing where the value lives (SwiftUI's `@Binding`).
//! * [`Derived<T>`] is a cached computation over other states (a memo).
//! * [`Observer`] is what the UI runtime uses to know when to rebuild.
//!
//! Dependencies are re-collected on every tracked run. Instead of keeping back
//! references from observers to sources, each subscription carries the observer's
//! run *epoch*; a subscription from an older run is ignored and pruned on the next
//! notification. This keeps ownership one-directional (sources hold `Weak`
//! observers) and makes dropping either side safe at any time.

mod binding;
mod derived;
mod observer;
mod state;

pub use binding::Binding;
pub use derived::Derived;
pub use observer::Observer;
pub use state::State;
