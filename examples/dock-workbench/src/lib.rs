//! Three workbenches built on the `DockArea`, each a whole desktop app around it, to check that the
//! dock holds one together:
//!
//! * [`editor`]: a code editor. A rail of views at the left switches the side bar, the code is in
//!   the middle split in two, the terminal and the problems are below, and the outline is hidden at
//!   the right edge.
//! * [`ide`]: an IDE. A menu bar and a toolbar over the work, the toolbox hidden at the left edge,
//!   the project tree over the properties at the right, the list of errors and the output below, and
//!   a search window floating over the code.
//! * [`chat`]: a chat app. The conversations in a side bar that collapses, the conversation in the
//!   middle, and what the assistant made in a panel at the right.
//!
//! Every name, file and message in them is made up.
//!
//! `cargo run -p dock-workbench -- editor` (or `ide`, or `chat`) opens one in a window.

pub mod chat;
pub mod editor;
pub mod ide;

use ravenrs::prelude::*;

/// Which workbench to build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Workbench {
    /// The code editor.
    Editor,
    /// The IDE.
    Ide,
    /// The chat app.
    Chat,
}

impl Workbench {
    /// All three, in the order the docs show them.
    pub const ALL: [Self; 3] = [Self::Editor, Self::Ide, Self::Chat];

    /// The one a command-line word names: `editor`, `ide` or `chat`.
    pub fn named(word: &str) -> Option<Self> {
        match word {
            "editor" => Some(Self::Editor),
            "ide" => Some(Self::Ide),
            "chat" => Some(Self::Chat),
            _ => None,
        }
    }

    /// Its word, as [`Self::named`] takes it.
    pub fn word(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Ide => "ide",
            Self::Chat => "chat",
        }
    }

    /// The title of its window.
    pub fn title(self) -> &'static str {
        match self {
            Self::Editor => "Raven Code",
            Self::Ide => "Raven Studio",
            Self::Chat => "Raven Chat",
        }
    }

    /// The app, ready for a window.
    pub fn app(self) -> AnyView {
        match self {
            Self::Editor => Box::new(editor::editor()),
            Self::Ide => Box::new(ide::ide()),
            Self::Chat => Box::new(chat::chat()),
        }
    }
}

/// A column of lines in the monospaced style of a terminal or a log, read only.
pub(crate) fn console(text: &str) -> impl View + use<> {
    let text = State::new(text.to_owned());
    CodeEditor::new(text.binding()).language(CodeLanguage::Plain).line_numbers(false).read_only(true).fill()
}
