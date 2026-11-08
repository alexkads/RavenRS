//! An editor of source code: a monospaced text area with line numbers, colors by
//! language, and lines marked where something is wrong.

use std::fmt;
use std::rc::Rc;

use ravenrs_reactive::Binding;
use ravenrs_style::Semantic;

use crate::context::{BuildCx, NodeId};
use crate::editors::{CodeLook, Highlighter, LineMark};
use crate::{TextArea, Tone, View};

/// The language a [`CodeEditor`] colors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum CodeLanguage {
    /// No colors.
    Plain,
    /// Rust: keywords, types, strings, numbers, comments, attributes and macros.
    #[default]
    Rust,
}

/// Something found at a place in the code: what [`CodeEditor::diagnostics`] marks and
/// what a [`crate::Playground`] lists under its editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeDiagnostic {
    /// The line, from 1.
    pub line: usize,
    /// The column of the first character, from 1; 0 marks the whole line.
    pub column: usize,
    /// How many characters it covers; 0 marks the whole line.
    pub length: usize,
    /// What happened.
    pub message: String,
    /// How to fix it, when there is a likely way.
    pub hint: Option<String>,
    /// [`Tone::Danger`] for errors, [`Tone::Warning`] for warnings; other tones read as notes.
    pub tone: Tone,
}

impl CodeDiagnostic {
    /// An error at `line` and `column` (both from 1), covering `length` characters.
    pub fn error(line: usize, column: usize, length: usize, message: impl Into<String>) -> Self {
        Self { line, column, length, message: message.into(), hint: None, tone: Tone::Danger }
    }

    /// A warning at `line` and `column` (both from 1), covering `length` characters.
    pub fn warning(line: usize, column: usize, length: usize, message: impl Into<String>) -> Self {
        Self { tone: Tone::Warning, ..Self::error(line, column, length, message) }
    }

    /// How to fix it.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

pub(crate) fn tone_color(tone: Tone) -> Semantic {
    match tone {
        Tone::Danger => Semantic::Danger,
        Tone::Warning => Semantic::Warning,
        Tone::Success => Semantic::Success,
        Tone::Accent => Semantic::Accent,
        Tone::Neutral => Semantic::InkMuted,
    }
}

/// A source code editor: monospaced, no wrapping, line numbers, colors for the
/// language, Tab and Shift+Tab to indent, and Enter keeping the indentation. Ctrl or Cmd
/// with Enter is left to shortcuts, so a surrounding [`crate::Playground`] can run the
/// code. Diagnostics mark their lines and underline what they cover.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{CodeDiagnostic, CodeEditor, Ui};
///
/// let source = State::new("fn main() {\n    println!(\"hi\");\n}".to_owned());
/// let editor = CodeEditor::new(source.binding())
///     .diagnostics(vec![CodeDiagnostic::error(2, 5, 8, "unknown macro")])
///     .rows(8);
/// let mut ui = Ui::new(editor, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct CodeEditor {
    value: Binding<String>,
    language: CodeLanguage,
    line_numbers: bool,
    diagnostics: Vec<CodeDiagnostic>,
    indent: usize,
    rows: usize,
    fill: bool,
    read_only: bool,
}

impl fmt::Debug for CodeEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodeEditor")
            .field("language", &self.language)
            .field("diagnostics", &self.diagnostics.len())
            .finish_non_exhaustive()
    }
}

impl CodeEditor {
    /// An editor of `value`, as Rust, with line numbers and an indent of 4 spaces.
    pub fn new(value: Binding<String>) -> Self {
        Self {
            value,
            language: CodeLanguage::Rust,
            line_numbers: true,
            diagnostics: Vec::new(),
            indent: 4,
            rows: 12,
            fill: false,
            read_only: false,
        }
    }

    /// The language it colors.
    pub fn language(mut self, language: CodeLanguage) -> Self {
        self.language = language;
        self
    }

    /// Shows or hides the line numbers.
    pub fn line_numbers(mut self, shown: bool) -> Self {
        self.line_numbers = shown;
        self
    }

    /// Marks these places.
    pub fn diagnostics(mut self, diagnostics: Vec<CodeDiagnostic>) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    /// How many spaces Tab inserts; at least 1.
    pub fn indent(mut self, spaces: usize) -> Self {
        self.indent = spaces.max(1);
        self
    }

    /// How many lines are visible; more scroll. 12 by default.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// Takes the height its parent gives, instead of [`Self::rows`].
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    /// Shows the code without letting it change; it can still be selected and copied.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }
}

impl View for CodeEditor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let highlight: Option<Highlighter> = match self.language {
            CodeLanguage::Plain => None,
            CodeLanguage::Rust => Some(Rc::new(highlight_rust)),
        };
        let marks: Vec<LineMark> = self
            .diagnostics
            .iter()
            .filter(|d| d.line > 0)
            .map(|d| {
                let columns = (d.column > 0 && d.length > 0).then(|| (d.column - 1, d.column - 1 + d.length));
                (d.line - 1, tone_color(d.tone), columns)
            })
            .collect();
        let look = CodeLook {
            highlight,
            line_numbers: self.line_numbers,
            marks: Rc::new(marks),
            indent: self.indent,
            fill: self.fill,
            background: Semantic::Surface,
        };
        let area = TextArea::new(self.value.clone()).rows(self.rows).read_only(self.read_only).code(look);
        area.build(cx)
    }
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false",
    "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while",
];

/// The colored stretches of Rust code, as char indices. It reads tokens, not syntax, so
/// it never fails: unfinished strings and comments color to the end.
pub fn highlight_rust(text: &str) -> Vec<(usize, usize, Semantic)> {
    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();
    let mut i = 0;
    let mut attribute_depth = 0usize;
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        let next = chars.get(i + 1).copied();
        let semantic = if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            Some(Semantic::InkMuted)
        } else if c == '/' && next == Some('*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            Some(Semantic::InkMuted)
        } else if c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += if chars[i] == '\\' { 2 } else { 1 };
            }
            i = (i + 1).min(chars.len());
            Some(Semantic::Success)
        } else if c == '\'' && chars.get(i + 2) == Some(&'\'') {
            i += 3;
            Some(Semantic::Success)
        } else if c == '#' && (next == Some('[') || (next == Some('!') && chars.get(i + 2) == Some(&'['))) {
            i += if next == Some('!') { 3 } else { 2 };
            attribute_depth = 1;
            Some(Semantic::Chart4)
        } else if attribute_depth > 0 && (c == '[' || c == ']') {
            attribute_depth = if c == '[' { attribute_depth + 1 } else { attribute_depth - 1 };
            i += 1;
            Some(Semantic::Chart4)
        } else if c.is_ascii_digit() {
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '.')
            {
                if chars[i] == '.' && !chars.get(i + 1).is_some_and(char::is_ascii_digit) {
                    break;
                }
                i += 1;
            }
            Some(Semantic::Chart3)
        } else if c.is_alphabetic() || c == '_' {
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if attribute_depth > 0 {
                Some(Semantic::Chart4)
            } else if chars.get(i) == Some(&'!') && chars.get(i + 1) != Some(&'=') {
                i += 1;
                Some(Semantic::Chart4)
            } else if RUST_KEYWORDS.contains(&word.as_str()) {
                Some(Semantic::FocusRing)
            } else if word.starts_with(char::is_uppercase) {
                Some(Semantic::Chart2)
            } else {
                None
            }
        } else {
            i += 1;
            (attribute_depth > 0 && !c.is_whitespace()).then_some(Semantic::Chart4)
        };
        if let Some(semantic) = semantic {
            match spans.last_mut() {
                Some((_, end, last)) if *end == start && *last == semantic => *end = i,
                _ => spans.push((start, i, semantic)),
            }
        }
    }
    spans
}
