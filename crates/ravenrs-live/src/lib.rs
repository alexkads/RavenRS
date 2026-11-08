//! An interpreter of the Rust that small RavenRS examples are written in, so they run in a
//! playground without a compiler: `let` and `let .. else`, closures, calls and chains of
//! builders, tuples, `vec!`, ranges and iterators, `format!`, `if`, `match`, `Option`, `Result`
//! and `?`, `State` and `Binding::map`, and the program's own functions, constants, structs and
//! `impl` blocks. The host program's types are reached through a [`Registry`], which
//! `#[ravenrs_live::expose]` fills from the host's own `impl` blocks, structs, enums and modules.
//!
//! What it does not read is an error with its line and column, never a guess.

#[doc(hidden)]
pub mod bridge;
mod eval;
mod registry;
mod value;

pub use eval::{Closure, LiveError, run};
pub use ravenrs_live_derive::expose;
pub use registry::{Fallback, HostFn, Registry};
pub use value::{FromLive, Host, IntoLive, Record, Value, late_problems, report};
