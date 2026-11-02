#![doc = include_str!("../README.md")]

mod assistant;
mod catalog;
mod engine;
mod machine;
mod manager;
mod models;
mod tools;

pub use assistant::LocalAssistant;
pub use catalog::{CATALOG, model_by_id};
pub use engine::{EngineError, Generated, LocalEngine};
pub use machine::Machine;
pub use manager::{ModelManager, ModelManagerLabels};
pub use models::{LocalModel, ModelError, ModelFamily, ModelFile, ModelFolder, ModelState};
pub use tools::{Tool, ToolCall, read_tool_call, tool_rules};
