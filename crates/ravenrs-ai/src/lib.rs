#![doc = include_str!("../README.md")]

mod assistant;
mod catalog;
mod engine;
mod lfm3;
mod machine;
mod manager;
mod models;
mod speech;
mod tools;
mod voice;

pub use assistant::LocalAssistant;
pub use catalog::{CATALOG, VOICE_CATALOG, model_by_id};
pub use engine::{EngineError, Generated, LocalEngine};
pub use machine::Machine;
pub use manager::{ModelManager, ModelManagerLabels};
pub use models::{LocalModel, ModelError, ModelFamily, ModelFile, ModelFolder, ModelState};
pub use speech::{SPEECH_LANGUAGES, SPEECH_RATE, SpeechEngine, SpeechSettings, SpeechVoice};
pub use tools::{Tool, ToolCall, read_tool_call, tool_rules};
pub use voice::{LocalVoice, VoiceLabels, VoiceManager};
