//! The core of Raven CI (#835, #836, #838, #839): the register of plugins, external plugins over
//! JSON-RPC, the model of jobs and builds, and the queue.
#![allow(clippy::expect_used, clippy::print_stderr, reason = "tests fail loudly and say what they skip")]

mod common;
mod external;
mod model;
mod queue;
mod registry;
