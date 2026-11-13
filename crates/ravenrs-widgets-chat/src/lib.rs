//! RavenRS conversation and text: `Chat` and `FloatingPanel`, `AttachmentStrip`,
//! `AudioPlayerButton` and `SelectionBar`, `RichText`, and `CodeEditor` and `Playground` (feature
//! `code`).
//!
//! Applications depend on `ravenrs-widgets` (or `ravenrs`), which re-exports all of this by the
//! same names.

// The base and the kit, by the names the components use: `crate::Button`, `crate::Tabs`,
// `crate::context::BuildCx`.
#[allow(unused_imports)]
use ravenrs_widgets_base::*;
#[allow(unused_imports)]
use ravenrs_widgets_kit::*;

#[doc(hidden)]
pub mod chat;
#[cfg(feature = "code")]
#[doc(hidden)]
pub mod code_editor;
#[doc(hidden)]
pub mod conversation;
#[cfg(feature = "code")]
#[doc(hidden)]
pub mod playground;
#[doc(hidden)]
pub mod rich_text;

pub use chat::{Chat, ChatAuthor, ChatMedia, ChatMessage, FloatingPanel};
#[cfg(feature = "code")]
pub use code_editor::{CodeDiagnostic, CodeEditor, CodeLanguage, highlight_rust};
pub use conversation::{Attachment, AttachmentStrip, AudioPlayerButton, SelectionBar};
#[cfg(feature = "code")]
pub use playground::{Playground, RunOutput, Runner};
pub use rich_text::RichText;
