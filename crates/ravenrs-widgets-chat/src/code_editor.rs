//! An editor of source code: a monospaced text area with line numbers, colors by
//! language, and lines marked where something is wrong.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Insets, Key, Modifiers};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint};
use crate::editors::{CodeLook, Highlighter, LineMark};
use crate::text_field::{Cursor, TextHistory};
use crate::{Button, ButtonSize, ButtonStyle, IconButton, IconName, TextArea, TextField, Tone, View, util};

/// The language a [`CodeEditor`] colors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum CodeLanguage {
    /// No colors.
    Plain,
    /// Rust: keywords, types, strings, numbers, comments, attributes and macros.
    #[default]
    Rust,
}

/// Something found at a place in the code: what [`CodeEditor::diagnostics`] marks and
/// what a `Playground` lists under its editor.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
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

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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

#[doc(hidden)]
pub fn tone_color(tone: Tone) -> Semantic {
    match tone {
        Tone::Danger => Semantic::Danger,
        Tone::Warning => Semantic::Warning,
        Tone::Success => Semantic::Success,
        Tone::Accent => Semantic::Accent,
        Tone::Neutral => Semantic::InkMuted,
        // The base may add more.
        _ => Semantic::InkMuted,
    }
}

/// A source code editor: monospaced, no wrapping, line numbers, colors for the
/// language, Tab and Shift+Tab to indent, and Enter keeping the indentation. Ctrl or Cmd
/// with Enter is left to shortcuts, so a surrounding `Playground` can run the
/// code. Diagnostics mark their lines and underline what they cover. Cmd/Ctrl+Z undoes and
/// Cmd+Shift+Z or Ctrl+Y redoes. Cmd/Ctrl+F opens a find bar over the code, and Ctrl+H the same
/// bar with replacing: Enter and Shift+Enter walk the matches, which are marked in the code,
/// and Escape closes it, back in the code at the match.
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct CodeEditor {
    value: Binding<String>,
    language: CodeLanguage,
    line_numbers: bool,
    diagnostics: Vec<CodeDiagnostic>,
    indent: usize,
    rows: usize,
    fill: bool,
    read_only: bool,
    caret: Option<State<Cursor>>,
    focus_request: Option<State<bool>>,
    history: Option<State<TextHistory>>,
    follow_end: bool,
    on_complete: Option<Rc<dyn Fn(usize)>>,
    completions: Vec<CodeCompletion>,
    on_hover: Option<Rc<dyn Fn(usize)>>,
    hover: Option<String>,
    on_go_to: Option<Rc<dyn Fn(usize)>>,
    label: Option<String>,
}

/// Something a [`CodeEditor`] offers to put at the caret, from
/// [`completions`](CodeEditor::completions).
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct CodeCompletion {
    /// What the list shows, and what the word being typed narrows by.
    pub label: String,
    /// A muted note beside it, such as a type.
    pub detail: Option<String>,
    /// What goes in, in place of the word being typed.
    pub insert: String,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl CodeCompletion {
    /// A suggestion that puts in its own label.
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        Self { insert: label.clone(), label, detail: None }
    }

    /// A muted note beside it.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// What goes in, when it is not the label.
    pub fn insert(mut self, text: impl Into<String>) -> Self {
        self.insert = text.into();
        self
    }
}

impl fmt::Debug for CodeEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodeEditor")
            .field("language", &self.language)
            .field("diagnostics", &self.diagnostics.len())
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
            caret: None,
            focus_request: None,
            history: None,
            follow_end: false,
            on_complete: None,
            completions: Vec::new(),
            on_hover: None,
            hover: None,
            on_go_to: None,
            label: None,
        }
    }

    /// What screen readers call the code, such as `Code` or the file's name. A [`FormField`]
    /// around the editor names it the same way.
    ///
    /// [`FormField`]: crate::FormField
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Keeps the last line in sight as the text grows, unless it was scrolled away from: for
    /// what a program prints, read-only.
    pub fn follow_end(mut self, follow: bool) -> Self {
        self.follow_end = follow;
        self
    }

    /// Asked for suggestions at a character index of the text: on Ctrl+Space, and when `.` or
    /// `::` is typed. The app answers, now or later, with [`completions`](Self::completions);
    /// the list shows under the caret, narrowed by the word being typed. The arrows choose,
    /// Enter or Tab put the choice in place of the word, and Escape closes it.
    pub fn on_complete(mut self, ask: impl Fn(usize) + 'static) -> Self {
        self.on_complete = Some(Rc::new(ask));
        self
    }

    /// What the list of suggestions offers, once [`on_complete`](Self::on_complete) asked.
    pub fn completions(mut self, completions: Vec<CodeCompletion>) -> Self {
        self.completions = completions;
        self
    }

    /// Asked for a tip about what is at a character index, once the pointer rests on it for
    /// half a second. The app answers with [`hover`](Self::hover).
    pub fn on_hover(mut self, ask: impl Fn(usize) + 'static) -> Self {
        self.on_hover = Some(Rc::new(ask));
        self
    }

    /// The tip shown by the pointer, about what it rests on; `None` shows nothing.
    pub fn hover(mut self, tip: Option<String>) -> Self {
        self.hover = tip;
        self
    }

    /// Asked to go to where what is at a character index is defined: F12 at the caret, or a
    /// Cmd/Ctrl click.
    pub fn on_go_to(mut self, go: impl Fn(usize) + 'static) -> Self {
        self.on_go_to = Some(Rc::new(go));
        self
    }

    /// Keeps what Cmd/Ctrl+Z can undo in `history`, so it lasts while the editor comes and
    /// goes (a document in a tab, say) and the app can undo and redo from its own buttons.
    pub fn history(mut self, history: State<TextHistory>) -> Self {
        self.history = Some(history);
        self
    }

    /// Takes the keyboard focus whenever `request` turns on, and turns it back off: set the
    /// [`caret`](Self::caret) and turn it on to take people to a place in the code.
    pub fn focus_request(mut self, request: State<bool>) -> Self {
        self.focus_request = Some(request);
        self
    }

    /// Keeps where the caret and the selection are in `cursor`, for a status line that says
    /// "Ln 7, Col 31" with [`line_and_column`], or a help panel about the name under it.
    pub fn caret(mut self, cursor: State<Cursor>) -> Self {
        self.caret = Some(cursor);
        self
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

/// What the find bar of a [`CodeEditor`] holds.
#[derive(Clone)]
struct Finder {
    open: State<bool>,
    replacing: State<bool>,
    query: State<String>,
    replacement: State<String>,
    case: State<bool>,
    word: State<bool>,
}

/// The match after (or before, `back`) the caret, going round at the ends.
fn next_match(found: &[(usize, usize)], cursor: Cursor, back: bool) -> Option<(usize, usize)> {
    let (from, to) = cursor.selection().unwrap_or((cursor.caret, cursor.caret));
    if back {
        found.iter().rev().find(|m| m.0 < from).or(found.last()).copied()
    } else {
        found.iter().find(|m| m.0 > from || (m.0 == from && to == from)).or(found.first()).copied()
    }
}

/// Selects a match, with the caret at its end.
fn select(cursor: &State<Cursor>, found: (usize, usize)) {
    cursor.set(Cursor { caret: found.1, anchor: Some(found.0) });
}

impl View for CodeEditor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let own_cursor: State<Cursor> = cx.use_state(|| Cursor::at(usize::MAX));
        let cursor = self.caret.clone().unwrap_or(own_cursor);
        let own_request: State<bool> = cx.use_state(|| false);
        let focus = self.focus_request.clone().unwrap_or(own_request);
        let own_history: State<TextHistory> = cx.use_state(TextHistory::new);
        let history = self.history.clone().unwrap_or(own_history);
        let finder = Finder {
            open: cx.use_state(|| false),
            replacing: cx.use_state(|| false),
            query: cx.use_state(String::new),
            replacement: cx.use_state(String::new),
            case: cx.use_state(|| false),
            word: cx.use_state(|| false),
        };
        let writable = !self.read_only;
        if cx.is_focus_within() {
            for (letter, replacing) in [("f", false), ("h", true)] {
                let (finder, cursor, value) = (finder.clone(), cursor.clone(), self.value.clone());
                cx.shortcut(
                    Key::Character(letter.into()),
                    true,
                    Rc::new(move || {
                        // A selection on one line is what is looked for.
                        let picked = cursor
                            .get()
                            .selection()
                            .map(|(a, b)| value.get().chars().skip(a).take(b - a).collect::<String>());
                        if let Some(picked) = picked.filter(|p| !p.contains('\n')) {
                            finder.query.set(picked);
                        }
                        finder.replacing.set(replacing && writable);
                        finder.open.set(true);
                    }),
                );
            }
        }

        let text = self.value.get();
        let open = finder.open.get();
        let query = finder.query.get();
        let found =
            if open { find_all(&text, &query, finder.case.get(), finder.word.get()) } else { Vec::new() };

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
            found: Rc::new(found.clone()),
            follow_caret: open,
            follow_end: self.follow_end,
            on_complete: self.on_complete.clone(),
            suggestions: Rc::new(
                self.completions
                    .iter()
                    .map(|c| (c.label.clone(), c.detail.clone(), c.insert.clone()))
                    .collect(),
            ),
            on_hover: self.on_hover.clone(),
            hover: self.hover.clone(),
            on_go_to: self.on_go_to.clone(),
        };
        let area = TextArea::new(self.value.clone())
            .rows(self.rows)
            .read_only(self.read_only)
            .code(look)
            .cursor(cursor.clone())
            .focus_request(focus.clone())
            .history(history.clone());
        let area = cx.keyed_child(1, &area);
        if let Some(label) = &self.label {
            cx.label_control(area, label, None);
        }
        let mut children = Vec::new();
        if open {
            let bar = FindBar {
                finder: finder.clone(),
                found: Rc::new(found),
                value: self.value.clone(),
                cursor: cursor.clone(),
                focus: focus.clone(),
                history: history.clone(),
                writable,
            };
            children.push(cx.keyed_child(2, &bar));
        }
        children.push(area);
        cx.container(
            LayoutStyle {
                grow: if self.fill { 1.0 } else { 0.0 },
                min_height: Some(0.0),
                align: Align::Stretch,
                spacing: 4.0,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        )
    }
}

/// The bar a [`CodeEditor`] shows over its code to find, and to replace.
struct FindBar {
    finder: Finder,
    found: Rc<Vec<(usize, usize)>>,
    value: Binding<String>,
    cursor: State<Cursor>,
    focus: State<bool>,
    history: State<TextHistory>,
    writable: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FindBar {
    /// Selects the next match, or the one before.
    fn step(&self) -> impl Fn(bool) + 'static {
        let (found, cursor) = (self.found.clone(), self.cursor.clone());
        move |back| {
            if let Some(m) = next_match(&found, cursor.get(), back) {
                select(&cursor, m);
            }
        }
    }

    /// Replaces the selected match and goes to the next; with `all`, every match at once.
    fn replace(&self, all: bool) -> impl Fn() + 'static {
        let (found, cursor, value, history) =
            (self.found.clone(), self.cursor.clone(), self.value.clone(), self.history.clone());
        let replacement = self.finder.replacement.clone();
        move || {
            let before = value.get();
            let with = replacement.get();
            let chosen: Vec<(usize, usize)> = if all {
                found.to_vec()
            } else {
                match cursor.get().selection() {
                    Some(sel) if found.contains(&sel) => vec![sel],
                    // Nothing found is selected: go to the next one first.
                    _ => {
                        if let Some(m) = next_match(&found, cursor.get(), false) {
                            select(&cursor, m);
                        }
                        return;
                    }
                }
            };
            if chosen.is_empty() {
                return;
            }
            let mut after = before.clone();
            for &(a, b) in chosen.iter().rev() {
                let (from, to) =
                    (crate::text_field::byte_index(&after, a), crate::text_field::byte_index(&after, b));
                after.replace_range(from..to, &with);
            }
            history.update(|h| h.record_change(&before, &after, cursor.get()));
            let (a, b) = chosen[0];
            let shift = with.chars().count() as isize - (b - a) as isize;
            value.set(after);
            if all {
                cursor.set(Cursor::at(a + with.chars().count()));
            } else {
                // The next match, counted in the new text.
                let next = found
                    .iter()
                    .find(|m| m.0 >= b)
                    .map(|m| ((m.0 as isize + shift) as usize, (m.1 as isize + shift) as usize));
                match next {
                    Some(m) => select(&cursor, m),
                    None => cursor.set(Cursor::at(a + with.chars().count())),
                }
            }
        }
    }
}

impl View for FindBar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels().clone();
        let finder = &self.finder;
        let replacing = finder.replacing.get() && self.writable;
        let close: Rc<dyn Fn()> = {
            let (open, focus) = (finder.open.clone(), self.focus.clone());
            Rc::new(move || {
                open.set(false);
                focus.set(true);
            })
        };
        let step = Rc::new(self.step());
        let keys: crate::KeyHandler = {
            let step = step.clone();
            Rc::new(move |key: &Key, modifiers: Modifiers| {
                if *key == Key::Enter {
                    step(modifiers.shift);
                    return true;
                }
                false
            })
        };
        let field = TextField::new(finder.query.binding())
            .placeholder(labels.code_find.clone())
            .compact()
            .width(220.0)
            .autofocus()
            .select_on_open()
            .intercept_keys(keys)
            .on_cancel({
                let close = close.clone();
                move || close()
            });
        let current = self.cursor.get().selection().and_then(|sel| self.found.iter().position(|m| *m == sel));
        let count = if self.found.is_empty() {
            labels.code_no_results.clone()
        } else {
            labels
                .code_find_count
                .replace("{current}", &current.map_or("?".to_owned(), |i| (i + 1).to_string()))
                .replace("{total}", &self.found.len().to_string())
        };
        let toggle = |cx: &mut BuildCx<'_>, key: u64, glyph: &str, name: &str, on: &State<bool>| {
            let flip = on.clone();
            let button = Button::new(glyph)
                .accessible_name(name)
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Mini)
                .selected(on.get())
                .on_click(move || flip.update(|v| *v = !*v));
            cx.keyed_child(key, &button)
        };
        let mut first_row = Vec::new();
        if self.writable {
            let flip = finder.replacing.clone();
            let chevron = if replacing { IconName::ChevronDown } else { IconName::ChevronRight };
            let show = IconButton::new(chevron)
                .label(labels.code_show_replace.clone())
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Mini)
                .selected(replacing)
                .on_click(move || flip.update(|v| *v = !*v));
            first_row.push(cx.keyed_child(10, &show));
        }
        first_row.push(cx.keyed_child(11, &field));
        first_row.push(toggle(cx, 12, "Aa", &labels.code_match_case, &finder.case));
        first_row.push(toggle(cx, 13, "ab", &labels.code_whole_word, &finder.word));
        let attrs = util::attrs(&theme, TextStyle::Caption, None);
        let muted = theme.color(Semantic::InkMuted);
        let counter =
            cx.text(&count, &attrs, muted, LayoutStyle { min_width: Some(72.0), ..LayoutStyle::leaf() });
        first_row.push(counter);
        for (key, icon, name, back) in [
            (14, IconName::ArrowUp, labels.code_find_previous.clone(), true),
            (15, IconName::ArrowDown, labels.code_find_next.clone(), false),
        ] {
            let step = step.clone();
            let button = IconButton::new(icon)
                .label(name)
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Mini)
                .disabled(self.found.is_empty())
                .on_click(move || step(back));
            first_row.push(cx.keyed_child(key, &button));
        }
        let shut = IconButton::new(IconName::X)
            .label(labels.code_close_find.clone())
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Mini)
            .on_click(move || close());
        first_row.push(cx.keyed_child(16, &shut));
        let row = |cx: &mut BuildCx<'_>, items: &[NodeId]| {
            cx.container(
                LayoutStyle { spacing: 4.0, align: Align::Center, ..LayoutStyle::stack(Axis::Horizontal) },
                items,
                Paint::None,
            )
        };
        let mut rows = vec![row(cx, &first_row)];
        if replacing {
            let swap = TextField::new(finder.replacement.binding())
                .placeholder(labels.code_replace.clone())
                .compact()
                .width(220.0);
            let one = Button::new(labels.code_replace.clone())
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Mini)
                .disabled(self.found.is_empty())
                .on_click(self.replace(false));
            let every = Button::new(labels.code_replace_all.clone())
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Mini)
                .disabled(self.found.is_empty())
                .on_click(self.replace(true));
            // Lined up under the find field, past the chevron.
            let gap =
                cx.container(LayoutStyle { width: Some(24.0), ..LayoutStyle::leaf() }, &[], Paint::None);
            let items =
                [gap, cx.keyed_child(20, &swap), cx.keyed_child(21, &one), cx.keyed_child(22, &every)];
            rows.push(row(cx, &items));
        }
        cx.container(
            LayoutStyle {
                padding: Insets::symmetric(4.0, 6.0),
                spacing: 4.0,
                align: Align::Start,
                align_self: Some(Align::End),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &rows,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(ravenrs_core::Border { width: 1.0, color: theme.color(Semantic::Border) }),
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        )
    }
}

/// Where `query` is in `text`, as `(from, to)` char indices that do not overlap: with `case`,
/// only with the same capitals; with `word`, only where it is not part of a longer word.
pub fn find_all(text: &str, query: &str, case: bool, word: bool) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }
    let fold = |c: char| if case { c } else { c.to_lowercase().next().unwrap_or(c) };
    let hay: Vec<char> = text.chars().map(fold).collect();
    let needle: Vec<char> = query.chars().map(fold).collect();
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut found = Vec::new();
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if hay[i..i + needle.len()] == needle[..] {
            let end = i + needle.len();
            let whole = !word
                || (i.checked_sub(1).is_none_or(|b| !is_word(hay[b]))
                    && hay.get(end).is_none_or(|c| !is_word(*c)));
            if whole {
                found.push((i, end));
                i = end;
                continue;
            }
        }
        i += 1;
    }
    found
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false",
    "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while",
];

/// The line and the column of the character `at` of `text`, both from 1: what an editor's
/// status line shows. `at` counts characters, as the caret of a text field does.
pub fn line_and_column(text: &str, at: usize) -> (usize, usize) {
    let (mut line, mut column) = (1, 1);
    for c in text.chars().take(at) {
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

/// Where `line` and `column`, both from 1, fall in `text`, counted in characters as
/// [`line_and_column`] counts them: the other way around. Column 0 is the start of the line,
/// and a place past the end of a line or of the text stops there.
pub fn offset_of(text: &str, line: usize, column: usize) -> usize {
    let mut at = 0;
    for (n, content) in text.split('\n').enumerate() {
        let length = content.chars().count();
        if n + 1 == line.max(1) {
            return at + column.saturating_sub(1).min(length);
        }
        at += length + 1;
    }
    text.chars().count()
}

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
