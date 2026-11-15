use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Key, Modifiers, Quad, Rect, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::Semantic;

use crate::context::{BuildCx, DragPhase, NodeId, Paint, QuadPaint, Semantics};
use crate::{AnyView, ButtonSize, ButtonStyle, IconButton, IconName, View, util};

/// A single-line text input bound to a `String`.
///
/// Supports typing, deleting, moving by character, word and line, selecting
/// with Shift, the mouse (drag, double-click) or Cmd/Ctrl+A, copy, cut and
/// paste, and Enter (submit). Passwords can be pasted but not copied.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{IconName, TextField, Ui};
///
/// let query = State::new(String::new());
/// let field = TextField::new(query.binding()).placeholder("Search").icon(IconName::Search).clearable();
/// let mut ui = Ui::new(field, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct TextField {
    value: Binding<String>,
    placeholder: String,
    icon: Option<IconName>,
    secure: bool,
    width: Option<f32>,
    size: ButtonSize,
    prefix: Option<AnyView>,
    suffix: Option<AnyView>,
    clearable: bool,
    flat: bool,
    end_aligned: bool,
    autofocus: bool,
    keeps_focus: bool,
    select_on_open: bool,
    on_submit: Option<Rc<dyn Fn(String)>>,
    on_leave: Option<Rc<dyn Fn()>>,
    on_cancel: Option<Rc<dyn Fn()>>,
    intercept: Option<crate::KeyHandler>,
    recall: Option<State<Vec<String>>>,
    state: util::EditorState,
    /// Shown instead of the value while the field does not have focus.
    #[doc(hidden)]
    pub idle_text: Option<String>,
    /// A caret at the end stays there when a change makes the text longer.
    #[doc(hidden)]
    pub caret_follows_end: bool,
}

impl fmt::Debug for TextField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextField").field("placeholder", &self.placeholder).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TextField {
    /// An empty-placeholder field bound to `value`.
    pub fn new(value: Binding<String>) -> Self {
        Self {
            value,
            placeholder: String::new(),
            icon: None,
            secure: false,
            width: None,
            size: ButtonSize::Medium,
            prefix: None,
            suffix: None,
            clearable: false,
            flat: false,
            end_aligned: false,
            autofocus: false,
            keeps_focus: false,
            select_on_open: false,
            on_submit: None,
            on_leave: None,
            on_cancel: None,
            intercept: None,
            recall: None,
            state: util::EditorState::default(),
            idle_text: None,
            caret_follows_end: false,
        }
    }

    /// Muted text shown while the value is empty.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// An icon inside the field, before the text.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Shows dots instead of the characters (passwords).
    pub fn secure(mut self) -> Self {
        self.secure = true;
        self
    }

    /// Fixed width in logical pixels; by default the field stretches.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// A shorter field with smaller text, for toolbars, filter rows and cells: the same
    /// as [`Self::size`] with [`ButtonSize::Small`].
    pub fn compact(mut self) -> Self {
        self.size = ButtonSize::Small;
        self
    }

    /// Sets the height and text size to line up with a [`Button`](crate::Button) of the
    /// same size (`Medium` by default). The smaller ones have no minimum width.
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Any view inside the field, before the text, such as a currency or a unit.
    pub fn prefix(mut self, view: impl View + 'static) -> Self {
        self.prefix = Some(Box::new(view));
        self
    }

    /// Any view inside the field, after the text, such as a unit or a button.
    pub fn suffix(mut self, view: impl View + 'static) -> Self {
        self.suffix = Some(Box::new(view));
        self
    }

    /// A button inside the field that empties it, shown while there is text to clear.
    /// The keyboard comes back to the field afterwards.
    pub fn clearable(mut self) -> Self {
        self.clearable = true;
        self
    }

    /// No box of its own until it is pointed at or focused, filling what it is in, for fields that
    /// line up with what is around them, as under the columns of a grid.
    pub fn flat(mut self) -> Self {
        self.flat = true;
        self
    }

    /// Puts the text at the end of the field, as under a column of numbers.
    pub fn end_aligned(mut self) -> Self {
        self.end_aligned = true;
        self
    }

    /// Takes keyboard focus when it first appears.
    pub fn autofocus(mut self) -> Self {
        self.autofocus = true;
        self
    }

    /// Takes keyboard focus whenever nothing has it, such as when a window over it closes. For the
    /// one field a screen is used through.
    pub fn keeps_focus(mut self) -> Self {
        self.keeps_focus = true;
        self
    }

    /// Starts with all of its text selected, so that typing replaces it.
    pub fn select_on_open(mut self) -> Self {
        self.select_on_open = true;
        self
    }

    /// Called when Escape is pressed while the field is focused.
    pub fn on_cancel(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }

    /// Sees keys before the field edits; return true to consume them (lists
    /// navigated from a search field use it for arrows and Enter).
    pub fn intercept_keys(mut self, handler: crate::KeyHandler) -> Self {
        self.intercept = Some(handler);
        self
    }

    /// Keeps what Enter sent in `sent`, last at the end, and brings it back with the arrows:
    /// Up for what was sent before, Down for what came after, and past the last an empty
    /// field again. For a field commands are typed in, as at a terminal. A line sent twice in
    /// a row is kept once.
    pub fn recall(mut self, sent: State<Vec<String>>) -> Self {
        self.recall = Some(sent);
        self
    }

    /// Called with the current text when Enter is pressed.
    pub fn on_submit(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(handler));
        self
    }

    /// Called when the keyboard goes elsewhere after being here.
    pub fn on_leave(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_leave = Some(Rc::new(handler));
        self
    }

    /// Turns the control off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the value without letting it change; it can still be focused, selected and copied.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

#[doc(hidden)]
pub fn byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices().nth(char_index).map_or(s.len(), |(i, _)| i)
}

/// Caret and selection of a text field, as char indices. The selection runs
/// from `anchor` to `caret`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[doc(hidden)]
pub struct Cursor {
    pub caret: usize,
    pub anchor: Option<usize>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Cursor {
    /// A caret at `at` with nothing selected.
    pub fn at(at: usize) -> Self {
        Self { caret: at, anchor: None }
    }

    /// The selected range, if it is not empty.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        (a != self.caret).then(|| (a.min(self.caret), a.max(self.caret)))
    }

    /// Moves the caret, extending the selection when `extend` is true.
    #[doc(hidden)]
    pub fn move_to(&mut self, to: usize, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.caret);
        } else {
            self.anchor = None;
        }
        self.caret = to;
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The start of the word before `at` (Alt/Ctrl+Left).
fn word_start(chars: &[char], at: usize) -> usize {
    let mut i = at.min(chars.len());
    while i > 0 && !is_word(chars[i - 1]) {
        i -= 1;
    }
    while i > 0 && is_word(chars[i - 1]) {
        i -= 1;
    }
    i
}

/// The end of the word after `at` (Alt/Ctrl+Right).
fn word_end(chars: &[char], at: usize) -> usize {
    let mut i = at.min(chars.len());
    while i < chars.len() && !is_word(chars[i]) {
        i += 1;
    }
    while i < chars.len() && is_word(chars[i]) {
        i += 1;
    }
    i
}

/// The word around `at`, for double-click selection.
#[doc(hidden)]
pub fn word_at(text: &str, at: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let at = at.min(chars.len());
    let on_word = |i: usize| chars.get(i).is_some_and(|c| is_word(*c));
    if !on_word(at) && !(at > 0 && on_word(at - 1)) {
        return (at, (at + 1).min(chars.len()));
    }
    let mut start = at;
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    let mut end = at;
    while end < chars.len() && is_word(chars[end]) {
        end += 1;
    }
    (start, end)
}

/// Removes the selection, leaving the caret where it was. Returns false when
/// nothing was selected.
fn delete_selection(text: &mut String, cursor: &mut Cursor) -> bool {
    let Some((a, b)) = cursor.selection() else { return false };
    text.replace_range(byte_index(text, a)..byte_index(text, b), "");
    *cursor = Cursor::at(a);
    true
}

#[doc(hidden)]
pub fn insert(text: &mut String, cursor: &mut Cursor, s: &str) {
    delete_selection(text, cursor);
    let at = byte_index(text, cursor.caret);
    text.insert_str(at, s);
    *cursor = Cursor::at(cursor.caret + s.chars().count());
}

/// How a field treats the clipboard.
#[allow(missing_debug_implementations)] // internal: shared with the components' crates, hidden
#[doc(hidden)]
pub struct ClipboardAccess<'a> {
    pub clipboard: &'a crate::Clipboard,
    /// Passwords can be pasted into but never copied out of.
    pub secure: bool,
    /// Pasted text keeps its line breaks.
    pub multiline: bool,
}

/// Applies a key to `text` and the cursor: typing, deleting, moving (Shift
/// extends the selection; Alt or Ctrl moves by word; Cmd moves to the ends),
/// select all, and copy, cut and paste with Cmd or Ctrl. Returns false for
/// keys the field does not handle, including a paste the clipboard cannot
/// read synchronously (the browser then delivers it as typed text).
#[doc(hidden)]
pub fn edit(
    text: &mut String,
    cursor: &mut Cursor,
    key: &Key,
    modifiers: Modifiers,
    clipboard: Option<ClipboardAccess<'_>>,
) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    cursor.caret = cursor.caret.min(len);
    cursor.anchor = cursor.anchor.map(|a| a.min(len));
    let shift = modifiers.shift;
    let word = modifiers.alt || modifiers.ctrl;
    let command = modifiers.ctrl || modifiers.meta;
    let multiline = clipboard.as_ref().is_some_and(|access| access.multiline);
    if command && let Key::Character(c) = key {
        match c.to_ascii_lowercase().as_str() {
            "a" => *cursor = Cursor { caret: len, anchor: Some(0) },
            "c" | "x" => {
                let Some(access) = clipboard else { return false };
                let Some((a, b)) = cursor.selection() else { return true };
                if access.secure {
                    return true;
                }
                access.clipboard.write(&chars[a..b].iter().collect::<String>());
                if c.eq_ignore_ascii_case("x") {
                    delete_selection(text, cursor);
                }
            }
            "v" => {
                let Some(pasted) = clipboard.and_then(|access| access.clipboard.read()) else { return false };
                if multiline {
                    insert(text, cursor, &pasted.replace("\r\n", "\n").replace('\r', "\n"));
                } else {
                    // A single-line field keeps the first line.
                    let line = pasted.lines().next().unwrap_or_default();
                    insert(text, cursor, line);
                }
            }
            _ => return false,
        }
        return true;
    }
    match key {
        // Line breaks (from a browser paste, say) become spaces in one line.
        Key::Character(s) if !command => {
            insert(text, cursor, &s.replace("\r\n", " ").replace(['\n', '\r'], " "))
        }
        Key::Space if !command => insert(text, cursor, " "),
        Key::Backspace | Key::Delete if delete_selection(text, cursor) => {}
        Key::Backspace if cursor.caret > 0 => {
            let from = if modifiers.meta {
                0
            } else if word {
                word_start(&chars, cursor.caret)
            } else {
                cursor.caret - 1
            };
            text.replace_range(byte_index(text, from)..byte_index(text, cursor.caret), "");
            *cursor = Cursor::at(from);
        }
        Key::Delete if cursor.caret < len => {
            let to = if word { word_end(&chars, cursor.caret) } else { cursor.caret + 1 };
            text.replace_range(byte_index(text, cursor.caret)..byte_index(text, to), "");
            *cursor = Cursor::at(cursor.caret);
        }
        Key::Backspace | Key::Delete => {}
        Key::ArrowLeft | Key::ArrowRight => {
            let left = *key == Key::ArrowLeft;
            let to = match (cursor.selection(), shift) {
                // Without Shift, an arrow collapses the selection to that side.
                (Some((a, b)), false) if !word && !modifiers.meta => {
                    if left {
                        a
                    } else {
                        b
                    }
                }
                _ if modifiers.meta => {
                    if left {
                        0
                    } else {
                        len
                    }
                }
                _ if word => {
                    if left {
                        word_start(&chars, cursor.caret)
                    } else {
                        word_end(&chars, cursor.caret)
                    }
                }
                _ if left => cursor.caret.saturating_sub(1),
                _ => (cursor.caret + 1).min(len),
            };
            cursor.move_to(to, shift);
        }
        Key::Home | Key::ArrowUp => cursor.move_to(0, shift),
        Key::End | Key::ArrowDown => cursor.move_to(len, shift),
        _ => return false,
    }
    true
}

/// How many steps a [`TextHistory`] keeps; older ones are let go.
const HISTORY_LIMIT: usize = 200;

/// One change to a text: at byte `at`, `removed` gave way to `inserted`, and the caret went
/// from `before` to `after`. Only what changed is kept, so a long text costs little per step.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Step {
    at: usize,
    removed: String,
    inserted: String,
    before: Cursor,
    after: Cursor,
}

impl Step {
    /// What changed from `old` to `new`: what lies between their common start and end.
    fn between(old: &str, new: &str, before: Cursor, after: Cursor) -> Self {
        let mut start = old.bytes().zip(new.bytes()).take_while(|(a, b)| a == b).count();
        while !old.is_char_boundary(start) || !new.is_char_boundary(start) {
            start -= 1;
        }
        let room = old.len().min(new.len()) - start;
        let mut end = old.bytes().rev().zip(new.bytes().rev()).take(room).take_while(|(a, b)| a == b).count();
        while !old.is_char_boundary(old.len() - end) || !new.is_char_boundary(new.len() - end) {
            end -= 1;
        }
        Self {
            at: start,
            removed: old[start..old.len() - end].to_owned(),
            inserted: new[start..new.len() - end].to_owned(),
            before,
            after,
        }
    }

    /// Takes in a step that comes right after this one in the same run: more letters of a
    /// word, or more characters taken out by Backspace or Delete.
    fn join(&mut self, next: Step) -> bool {
        if self.removed.is_empty() && next.removed.is_empty() && next.at == self.at + self.inserted.len() {
            self.inserted.push_str(&next.inserted);
        } else if self.inserted.is_empty() && next.inserted.is_empty() && next.at + next.removed.len() == self.at {
            // Backspace: each character comes off before the last.
            self.at = next.at;
            self.removed.insert_str(0, &next.removed);
        } else if self.inserted.is_empty() && next.inserted.is_empty() && next.at == self.at {
            // Delete: each character comes off after the caret, which stays.
            self.removed.push_str(&next.removed);
        } else {
            return false;
        }
        self.after = next.after;
        true
    }
}

/// What the step still open is made of, so the next key of the same kind joins it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Run {
    /// Letters, digits and `_` typed one after another: a word.
    Word,
    /// Characters taken out one by one with Backspace or Delete.
    Deleting,
}

/// What can be undone and redone in a text field, a text area or a code editor: Cmd/Ctrl+Z
/// undoes, Cmd+Shift+Z or Ctrl+Y redoes.
///
/// A word typed letter by letter is one step, and so is each space or sign after it, each
/// paste, cut and line break. Moving the caret closes the step being typed. When the app
/// changes the text by itself, the next key starts a new step from what the app left, so an
/// undo never jumps back past it; [`record_change`](Self::record_change) makes such a change
/// a step of its own, to be undone too. It keeps the last 200 steps, and of each only what
/// changed, so a long text costs little.
///
/// Each field keeps its own; give a `CodeEditor` one through its `history` to keep it while
/// the editor comes and goes, or to undo from a button.
///
/// ```
/// use ravenrs_widgets::{Cursor, TextHistory};
///
/// let mut history = TextHistory::new();
/// let mut text = String::from("final");
/// let mut cursor = Cursor::at(5);
/// history.record_change("draft", &text, cursor);
/// assert!(history.undo(&mut text, &mut cursor));
/// assert_eq!(text, "draft");
/// assert!(history.redo(&mut text, &mut cursor));
/// assert_eq!(text, "final");
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextHistory {
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// The text as the last key left it, to notice a change made from outside.
    known: Option<String>,
    /// The kind of the step still open, which the next key of that kind joins.
    open: Option<Run>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TextHistory {
    /// Nothing to undo or redo.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether there is a step to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is a step to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Puts back the text and the caret as they were before the last step. False when there
    /// is nothing to undo, or when the text is no longer the one the step was made on.
    pub fn undo(&mut self, text: &mut String, cursor: &mut Cursor) -> bool {
        let Some(step) = self.undo.pop() else { return false };
        let end = step.at + step.inserted.len();
        if text.get(step.at..end) != Some(step.inserted.as_str()) {
            self.forget();
            return false;
        }
        text.replace_range(step.at..end, &step.removed);
        *cursor = step.before;
        self.redo.push(step);
        self.settle(text);
        true
    }

    /// Does again the last step undone. False when there is nothing to redo.
    pub fn redo(&mut self, text: &mut String, cursor: &mut Cursor) -> bool {
        let Some(step) = self.redo.pop() else { return false };
        let end = step.at + step.removed.len();
        if text.get(step.at..end) != Some(step.removed.as_str()) {
            self.forget();
            return false;
        }
        text.replace_range(step.at..end, &step.inserted);
        *cursor = step.after;
        self.undo.push(step);
        self.settle(text);
        true
    }

    /// Takes in a change the app made by itself (replacing all, reloading from disk) as one
    /// step: the text was `before` and is now `after`, and the caret was at `cursor`.
    pub fn record_change(&mut self, before: &str, after: &str, cursor: Cursor) {
        if before == after {
            return;
        }
        self.push(Step::between(before, after, cursor, cursor));
        self.redo.clear();
        self.known = Some(after.to_owned());
    }

    /// Lets go of every step, as when the text is replaced by another.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn forget(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.open = None;
    }

    fn settle(&mut self, text: &str) {
        self.known = Some(text.to_owned());
        self.open = None;
    }

    fn push(&mut self, step: Step) {
        self.undo.push(step);
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.open = None;
    }

    /// Applies a key with `apply`, keeping the step it makes; Cmd/Ctrl+Z, Cmd+Shift+Z and
    /// Ctrl+Y undo and redo instead. Returns whether the key was handled.
    #[doc(hidden)]
    pub fn apply(
        &mut self,
        text: &mut String,
        cursor: &mut Cursor,
        key: &Key,
        modifiers: Modifiers,
        apply: impl FnOnce(&mut String, &mut Cursor) -> bool,
    ) -> bool {
        let command = modifiers.ctrl || modifiers.meta;
        if command && let Key::Character(c) = key {
            let c = c.to_ascii_lowercase();
            if c == "z" && !modifiers.shift {
                self.undo(text, cursor);
                return true;
            }
            if (c == "z" && modifiers.shift) || (c == "y" && modifiers.ctrl && !modifiers.meta) {
                self.redo(text, cursor);
                return true;
            }
        }
        if self.known.as_deref().is_some_and(|known| known != text.as_str()) {
            // The app changed the text, and the steps kept are about another one.
            self.forget();
        }
        let (old, before) = (text.clone(), *cursor);
        if !apply(text, cursor) {
            return false;
        }
        if *text == old {
            if *cursor != before {
                self.open = None;
            }
            self.known = Some(old);
            return true;
        }
        let run = match key {
            Key::Character(s) if !command && before.selection().is_none() && s.chars().all(is_word) => {
                Some(Run::Word)
            }
            Key::Backspace | Key::Delete if !modifiers.alt && !command && before.selection().is_none() => {
                Some(Run::Deleting)
            }
            _ => None,
        };
        let step = Step::between(&old, text, before, *cursor);
        let joined = run.is_some()
            && self.open == run
            && self.undo.last_mut().is_some_and(|last| last.after.caret == before.caret && last.join(step.clone()));
        if !joined {
            self.push(step);
        }
        self.open = run;
        self.redo.clear();
        self.known = Some(text.clone());
        true
    }
}

/// The char index nearest to `x` (relative to the text's left edge), given
/// the x position of every char boundary.
#[doc(hidden)]
pub fn index_at(boundaries: &[f32], x: f32) -> usize {
    let after = boundaries.partition_point(|b| *b < x);
    match after {
        0 => 0,
        n if n >= boundaries.len() => boundaries.len().saturating_sub(1),
        n => {
            if x - boundaries[n - 1] < boundaries[n] - x {
                n - 1
            } else {
                n
            }
        }
    }
}

impl View for TextField {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let cursor: State<Cursor> = cx.use_state(|| {
            if self.select_on_open {
                Cursor { caret: self.value.get().chars().count(), anchor: Some(0) }
            } else {
                Cursor::at(usize::MAX)
            }
        });
        let focus_requested: State<bool> = cx.use_state(|| false);
        let history: State<TextHistory> = cx.use_state(TextHistory::new);
        // Which line sent before is shown, counted in the recall list; none while typing.
        let recalled: State<Option<usize>> = cx.use_state(|| None);
        let state = self.state;
        if self.autofocus && !state.disabled && !*focus_requested.peek() {
            focus_requested.set_if_changed(true);
            cx.request_focus();
        }
        if self.keeps_focus && !state.disabled && cx.nothing_focused() {
            cx.request_focus();
        }
        // Set by the clear button, which took the focus with its click.
        let refocus: State<bool> = cx.use_state(|| false);
        if *refocus.peek() {
            refocus.set(false);
            cx.request_focus();
        }
        let focused = cx.is_focused() && !state.disabled;
        let had_focus: State<bool> = cx.use_state(|| false);
        if *had_focus.peek() != focused {
            had_focus.set(focused);
            if !focused && let Some(left) = &self.on_leave {
                left();
            }
        }
        let hovered = cx.is_hovered() && !state.disabled;
        let text = self.value.get();
        let len = text.chars().count();
        let cur = cursor.get();
        let caret_at = cur.caret.min(len);
        let visible = self.idle_text.as_ref().filter(|_| !focused).unwrap_or(&text);
        let shown: String = if self.secure { "•".repeat(len) } else { visible.clone() };
        let dense = matches!(self.size, ButtonSize::Small | ButtonSize::Mini);
        let attrs = util::attrs(&theme, self.size.text_style(), None);

        let mut children = Vec::new();
        if let Some(icon) = self.icon {
            let size = if self.size == ButtonSize::Mini { 12.0 } else { 16.0 };
            children.push(util::icon(cx, icon, size, state.ink(theme.color(Semantic::InkMuted))));
        }
        // Keyed, so that the clear button coming and going does not move their state.
        if let Some(prefix) = &self.prefix {
            children.push(cx.keyed_child(1, prefix));
        }
        // Where each char boundary falls, for the caret, the selection and clicks.
        let boundaries: Rc<Vec<f32>> = Rc::new(if shown.chars().count() <= 512 {
            let chars: Vec<char> = shown.chars().collect();
            (0..=chars.len())
                .map(|i| cx.measure_text(&chars[..i].iter().collect::<String>(), &attrs).width)
                .collect()
        } else {
            // Long text: one measurement, spread evenly.
            let w = cx.measure_text(&shown, &attrs).width;
            let n = shown.chars().count();
            (0..=n).map(|i| w * i as f32 / n as f32).collect()
        });
        let caret_x = boundaries.get(caret_at).copied().unwrap_or(0.0);
        let selection = cur.selection().map(|(a, b)| {
            let at = |i: usize| boundaries.get(i.min(len)).copied().unwrap_or(0.0);
            (at(a), at(b))
        });
        let highlight = theme.color(Semantic::Accent).with_alpha(if focused { 0.32 } else { 0.16 });
        let content = if shown.is_empty() {
            cx.text(
                &self.placeholder,
                &attrs,
                state.ink(theme.color(Semantic::InkMuted)),
                LayoutStyle::leaf(),
            )
        } else {
            cx.text(&shown, &attrs, state.ink(theme.color(Semantic::Ink)), LayoutStyle::leaf())
        };
        let accent = theme.color(Semantic::Accent);
        let line = attrs.line_height;
        let end_aligned = self.end_aligned;
        let text_width = boundaries.last().copied().unwrap_or(0.0);
        // How far the text starts into an area of this width.
        let inset = move |width: f32| if end_aligned { (width - text_width).max(0.0) } else { 0.0 };
        let text_area = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                align: Align::Center,
                justify: if end_aligned { Justify::End } else { Justify::Start },
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[content],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                let y = f.center().y - line / 2.0 + 2.0;
                let start = f.x() + inset(f.width());
                if let Some((a, b)) = selection {
                    scene.push_quad(
                        Quad::new(Rect::new(start + a, y, b - a, line - 4.0), highlight).radius(2.0),
                    );
                }
                if focused && selection.is_none() && state.accepts_input() {
                    let x = (start + caret_x).min(f.max_x() - 1.5);
                    scene.push_quad(Quad::new(Rect::new(x, y, 1.5, line - 4.0), accent).radius(0.75));
                }
            })),
        );
        cx.clip(text_area);
        // Press places the caret, dragging selects, Shift-click extends.
        if !state.disabled {
            let (drag_cursor, mods, b) = (cursor.clone(), cx.modifiers(), boundaries.clone());
            cx.on_drag(
                text_area,
                Rc::new(move |d| {
                    let at = index_at(&b, d.position.x - d.frame.x() - inset(d.frame.width()));
                    match d.phase {
                        DragPhase::Start => drag_cursor.update(|c| {
                            if mods.get().shift {
                                c.move_to(at, true);
                            } else {
                                *c = Cursor { caret: at, anchor: Some(at) };
                            }
                        }),
                        DragPhase::Move => drag_cursor.update(|c| c.move_to(at, true)),
                        _ => {}
                    }
                }),
            );
            let (word_cursor, word_value) = (cursor.clone(), self.value.clone());
            cx.on_double_click(
                text_area,
                Rc::new(move || {
                    let caret = word_cursor.get().caret;
                    let (a, b) = word_at(&word_value.get(), caret);
                    word_cursor.set(Cursor { caret: b, anchor: Some(a) });
                }),
            );
            cx.cursor(text_area, CursorIcon::Text);
        }
        children.push(text_area);
        if self.clearable && !text.is_empty() && state.accepts_input() {
            let (value, caret, refocus) = (self.value.clone(), cursor.clone(), refocus.clone());
            let clear = IconButton::new(IconName::X)
                .label(cx.labels().clear.clone())
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Mini)
                .on_click(move || {
                    value.set(String::new());
                    caret.set(Cursor::at(usize::MAX));
                    refocus.set(true);
                });
            children.push(cx.keyed_child(2, &clear));
        }
        if let Some(suffix) = &self.suffix {
            children.push(cx.keyed_child(3, suffix));
        }

        let border = if focused {
            Border { width: 1.5, color: accent }
        } else {
            let base = theme.color(Semantic::Border);
            Border {
                width: 1.0,
                color: state.ink(if hovered { base.lerp(theme.color(Semantic::Ink), 0.25) } else { base }),
            }
        };
        let style = LayoutStyle {
            padding: Insets::symmetric(0.0, if dense && !self.flat { 8.0 } else { 12.0 }),
            height: if self.flat { None } else { Some(self.size.height()) },
            grow: if self.flat { 1.0 } else { 0.0 },
            width: self.width,
            min_width: Some(if dense { 0.0 } else { 160.0 }),
            spacing: 8.0,
            align: Align::Center,
            align_self: Some(if self.width.is_some() { Align::Start } else { Align::Stretch }),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = if self.flat {
            let fill = if hovered && !focused {
                theme.color(Semantic::Ink).with_alpha(0.05)
            } else {
                Color::TRANSPARENT
            };
            QuadPaint {
                corner_radius: theme.radii.sm,
                border: focused.then_some(border),
                ..QuadPaint::new(fill)
            }
        } else {
            QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(border),
                ..QuadPaint::new(state.fill(theme.color(Semantic::Control)))
            }
        };
        let node = cx.container(style, &children, Paint::Quad(paint));
        crate::motion::control(cx, node);
        // The visible text is exposed as the value; the placeholder names the
        // field until a label (a FormField, say) gives it a real name.
        cx.semantics(text_area, Semantics::hidden());
        let mut semantics = Semantics::new(Role::TextInput).value(shown);
        if !self.placeholder.is_empty() {
            semantics = semantics.name(self.placeholder.clone());
            semantics.fallback_name = true;
        }
        cx.semantics(node, state.semantics(semantics));
        if state.disabled {
            return node;
        }

        let (value, submit, cancel) = (self.value.clone(), self.on_submit.clone(), self.on_cancel.clone());
        let (intercept, clipboard, secure) = (self.intercept.clone(), cx.clipboard(), self.secure);
        let follows_end = self.caret_follows_end;
        let recall = self.recall.clone();
        cx.on_key(
            node,
            Rc::new(move |key, modifiers| {
                if let Some(i) = &intercept
                    && i(key, modifiers)
                {
                    return true;
                }
                if *key == Key::Escape
                    && let Some(cancel) = &cancel
                {
                    cancel();
                    return true;
                }
                if *key == Key::Enter {
                    if let Some(sent) = recall.as_ref().filter(|_| state.accepts_input()) {
                        let line = value.get();
                        if !line.is_empty() && sent.peek().last() != Some(&line) {
                            sent.update(|sent| sent.push(line));
                        }
                        recalled.set(None);
                    }
                    if let (Some(submit), true) = (&submit, state.accepts_input()) {
                        submit(value.get());
                    }
                    return true;
                }
                if let Some(sent) = recall.as_ref().filter(|_| state.accepts_input())
                    && matches!(key, Key::ArrowUp | Key::ArrowDown)
                    && !modifiers.shift
                {
                    let count = sent.peek().len();
                    let now = recalled.get();
                    let next = match (key, now) {
                        (_, _) if count == 0 => return true,
                        (Key::ArrowUp, None) => Some(count - 1),
                        (Key::ArrowUp, Some(at)) => Some(at.saturating_sub(1)),
                        (_, Some(at)) if at + 1 < count => Some(at + 1),
                        _ => None,
                    };
                    let line = next.and_then(|at| sent.peek().get(at).cloned()).unwrap_or_default();
                    recalled.set(next);
                    cursor.set(Cursor::at(line.chars().count()));
                    value.set(line);
                    return true;
                }
                let mut text = value.get();
                let mut c = cursor.get();
                let access = ClipboardAccess { clipboard: &clipboard, secure, multiline: false };
                let handled = if state.accepts_input() {
                    let mut handled = false;
                    history.update(|steps| {
                        handled = steps.apply(&mut text, &mut c, key, modifiers, |t, c| {
                            edit(t, c, key, modifiers, Some(access))
                        });
                    });
                    handled
                } else {
                    edit(&mut text, &mut c, key, modifiers, Some(access))
                };
                if handled {
                    let changed = text != value.get();
                    if changed && !state.accepts_input() {
                        // Copying still works; the text stays as it was.
                        return true;
                    }
                    if follows_end && c.caret == text.chars().count() {
                        c.caret = usize::MAX;
                    }
                    cursor.set(c);
                    if changed {
                        value.set(text);
                    }
                }
                handled
            }),
        );
        cx.focusable(node, theme.radii.md);
        cx.cursor(node, CursorIcon::Text);
        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(text: &mut String, cursor: &mut Cursor, key: Key, modifiers: Modifiers) -> bool {
        edit(text, cursor, &key, modifiers, None)
    }

    const NONE: Modifiers = Modifiers { shift: false, ctrl: false, alt: false, meta: false, base_key: None };
    const SHIFT: Modifiers = Modifiers { shift: true, ..NONE };
    const ALT: Modifiers = Modifiers { alt: true, ..NONE };
    const CMD: Modifiers = Modifiers { meta: true, ..NONE };

    #[test]
    fn editing_keys() {
        let (mut text, mut c) = (String::from("héllo"), Cursor::at(usize::MAX));
        assert!(press(&mut text, &mut c, Key::Backspace, NONE));
        assert_eq!((text.as_str(), c.caret), ("héll", 4));
        press(&mut text, &mut c, Key::Home, NONE);
        press(&mut text, &mut c, Key::ArrowRight, NONE);
        press(&mut text, &mut c, Key::Delete, NONE);
        assert_eq!(text, "hll");
        press(&mut text, &mut c, Key::Character("é".into()), NONE);
        press(&mut text, &mut c, Key::Space, NONE);
        assert_eq!((text.as_str(), c.caret), ("hé ll", 3));
        assert!(!press(&mut text, &mut c, Key::Tab, NONE));
    }

    #[test]
    fn shift_selects_and_typing_replaces_the_selection() {
        let (mut text, mut c) = (String::from("hello world"), Cursor::at(0));
        for _ in 0..5 {
            press(&mut text, &mut c, Key::ArrowRight, SHIFT);
        }
        assert_eq!(c.selection(), Some((0, 5)));
        press(&mut text, &mut c, Key::Character("bye".into()), NONE);
        assert_eq!((text.as_str(), c), ("bye world", Cursor::at(3)));
        press(&mut text, &mut c, Key::End, SHIFT);
        press(&mut text, &mut c, Key::Backspace, NONE);
        assert_eq!(text, "bye");
    }

    #[test]
    fn arrows_collapse_a_selection_to_its_side() {
        let (mut text, mut c) = (String::from("abcdef"), Cursor { caret: 4, anchor: Some(1) });
        press(&mut text, &mut c, Key::ArrowLeft, NONE);
        assert_eq!(c, Cursor::at(1));
        c = Cursor { caret: 1, anchor: Some(4) };
        press(&mut text, &mut c, Key::ArrowRight, NONE);
        assert_eq!(c, Cursor::at(4));
    }

    #[test]
    fn words_and_line_ends() {
        let (mut text, mut c) = (String::from("one two  three"), Cursor::at(14));
        press(&mut text, &mut c, Key::ArrowLeft, ALT);
        assert_eq!(c.caret, 9);
        press(&mut text, &mut c, Key::ArrowLeft, ALT);
        assert_eq!(c.caret, 4);
        press(&mut text, &mut c, Key::ArrowRight, ALT);
        assert_eq!(c.caret, 7);
        press(&mut text, &mut c, Key::Backspace, ALT);
        assert_eq!((text.as_str(), c.caret), ("one   three", 4));
        press(&mut text, &mut c, Key::ArrowRight, CMD);
        assert_eq!(c.caret, 11);
        press(&mut text, &mut c, Key::Backspace, CMD);
        assert_eq!(text, "");
        assert_eq!(word_at("say hello there", 6), (4, 9));
        assert_eq!(word_at("a  b", 2), (2, 3), "between words a double-click selects one character");
    }

    #[test]
    fn select_all_copy_cut_and_paste() {
        let clipboard = crate::Clipboard::default();
        let access = || Some(ClipboardAccess { clipboard: &clipboard, secure: false, multiline: false });
        let (mut text, mut c) = (String::from("draft"), Cursor::at(0));
        assert!(edit(&mut text, &mut c, &Key::Character("a".into()), CMD, access()));
        assert_eq!(c.selection(), Some((0, 5)));
        edit(&mut text, &mut c, &Key::Character("c".into()), CMD, access());
        assert_eq!(clipboard.read().as_deref(), Some("draft"));
        edit(&mut text, &mut c, &Key::Character("x".into()), CMD, access());
        assert_eq!(text, "");
        clipboard.write("line one\nline two");
        edit(&mut text, &mut c, &Key::Character("v".into()), CMD, access());
        assert_eq!((text.as_str(), c.caret), ("line one", 8), "one line in a single-line field");
        let ctrl = Modifiers { ctrl: true, ..NONE };
        assert!(edit(&mut text, &mut c, &Key::Character("A".into()), ctrl, access()), "Ctrl works too");
    }

    #[test]
    fn passwords_are_never_copied() {
        let clipboard = crate::Clipboard::default();
        let (mut text, mut c) = (String::from("secret"), Cursor { caret: 6, anchor: Some(0) });
        let access = Some(ClipboardAccess { clipboard: &clipboard, secure: true, multiline: false });
        assert!(edit(&mut text, &mut c, &Key::Character("x".into()), CMD, access));
        assert_eq!((text.as_str(), clipboard.read()), ("secret", None));
    }

    #[test]
    fn unreadable_clipboard_leaves_paste_to_the_platform() {
        struct WriteOnly;
        impl crate::ClipboardProvider for WriteOnly {
            fn read_text(&self) -> Option<String> {
                None
            }
            fn write_text(&self, _: &str) {}
        }
        let clipboard = crate::Clipboard::new(WriteOnly);
        let (mut text, mut c) = (String::new(), Cursor::at(0));
        let access = Some(ClipboardAccess { clipboard: &clipboard, secure: false, multiline: false });
        assert!(!edit(&mut text, &mut c, &Key::Character("v".into()), CMD, access));
    }

    /// Types `keys` through a history, as a field does.
    fn type_through(history: &mut TextHistory, text: &mut String, c: &mut Cursor, key: Key, modifiers: Modifiers) {
        history.apply(text, c, &key, modifiers, |t, c| edit(t, c, &key, modifiers, None));
    }

    fn type_text(history: &mut TextHistory, text: &mut String, c: &mut Cursor, typed: &str) {
        for ch in typed.chars() {
            let key = if ch == ' ' { Key::Space } else { Key::Character(ch.to_string()) };
            type_through(history, text, c, key, NONE);
        }
    }

    fn z(shift: bool) -> (Key, Modifiers) {
        (Key::Character(if shift { "Z" } else { "z" }.into()), Modifiers { shift, ..CMD })
    }

    #[test]
    fn a_word_is_one_step_and_so_is_the_space_after_it() {
        let (mut h, mut text, mut c) = (TextHistory::new(), String::new(), Cursor::at(0));
        type_text(&mut h, &mut text, &mut c, "olá mundo");
        let (undo, none) = z(false);
        type_through(&mut h, &mut text, &mut c, undo.clone(), none);
        assert_eq!((text.as_str(), c.caret), ("olá ", 4));
        type_through(&mut h, &mut text, &mut c, undo.clone(), none);
        assert_eq!(text, "olá");
        type_through(&mut h, &mut text, &mut c, undo.clone(), none);
        assert_eq!(text, "");
        assert!(!h.can_undo());
        let (redo, shift) = z(true);
        type_through(&mut h, &mut text, &mut c, redo.clone(), shift);
        type_through(&mut h, &mut text, &mut c, redo, shift);
        assert_eq!(text, "olá ");
        let ctrl_y = Modifiers { ctrl: true, ..NONE };
        type_through(&mut h, &mut text, &mut c, Key::Character("y".into()), ctrl_y);
        assert_eq!((text.as_str(), c.caret), ("olá mundo", 9));
    }

    #[test]
    fn typing_after_an_undo_lets_the_redo_go() {
        let (mut h, mut text, mut c) = (TextHistory::new(), String::new(), Cursor::at(0));
        type_text(&mut h, &mut text, &mut c, "ab cd");
        let (undo, none) = z(false);
        type_through(&mut h, &mut text, &mut c, undo, none);
        assert!(h.can_redo());
        type_text(&mut h, &mut text, &mut c, "x");
        assert!(!h.can_redo());
        assert_eq!(text, "ab x");
    }

    #[test]
    fn backspaces_in_a_row_are_one_step_and_moving_closes_it() {
        let (mut h, mut text, mut c) = (TextHistory::new(), String::new(), Cursor::at(0));
        type_text(&mut h, &mut text, &mut c, "hello");
        for _ in 0..3 {
            type_through(&mut h, &mut text, &mut c, Key::Backspace, NONE);
        }
        assert_eq!(text, "he");
        let (undo, none) = z(false);
        type_through(&mut h, &mut text, &mut c, undo.clone(), none);
        assert_eq!((text.as_str(), c.caret), ("hello", 5));
        // Typing at another place is another step, even inside the same word run.
        type_through(&mut h, &mut text, &mut c, Key::Home, NONE);
        type_text(&mut h, &mut text, &mut c, "x");
        type_through(&mut h, &mut text, &mut c, Key::End, NONE);
        type_text(&mut h, &mut text, &mut c, "y");
        type_through(&mut h, &mut text, &mut c, undo, none);
        assert_eq!(text, "xhello");
    }

    #[test]
    fn a_paste_is_one_step_and_the_caret_comes_back() {
        let clipboard = crate::Clipboard::default();
        clipboard.write("pasted");
        let (mut h, mut text, mut c) = (TextHistory::new(), String::from("ab"), Cursor::at(1));
        let key = Key::Character("v".into());
        h.apply(&mut text, &mut c, &key, CMD, |t, c| {
            edit(t, c, &key, CMD, Some(ClipboardAccess { clipboard: &clipboard, secure: false, multiline: false }))
        });
        assert_eq!(text, "apastedb");
        let (undo, none) = z(false);
        type_through(&mut h, &mut text, &mut c, undo, none);
        assert_eq!((text.as_str(), c), ("ab", Cursor::at(1)));
    }

    #[test]
    fn a_change_from_outside_is_never_undone_past() {
        let (mut h, mut text, mut c) = (TextHistory::new(), String::new(), Cursor::at(0));
        type_text(&mut h, &mut text, &mut c, "one");
        text = "replaced".to_owned();
        c = Cursor::at(8);
        type_text(&mut h, &mut text, &mut c, "!");
        let (undo, none) = z(false);
        type_through(&mut h, &mut text, &mut c, undo.clone(), none);
        assert_eq!(text, "replaced");
        type_through(&mut h, &mut text, &mut c, undo, none);
        assert_eq!(text, "replaced", "what was typed before the change is gone");
        // record_change makes such a change a step of its own.
        let mut text = String::from("after");
        h.record_change("before", &text, Cursor::at(0));
        assert!(h.undo(&mut text, &mut c));
        assert_eq!(text, "before");
    }

    #[test]
    fn the_history_keeps_two_hundred_steps() {
        let (mut h, mut text, mut c) = (TextHistory::new(), String::new(), Cursor::at(0));
        for _ in 0..250 {
            type_text(&mut h, &mut text, &mut c, ".");
        }
        let mut undone = 0;
        while h.undo(&mut text, &mut c) {
            undone += 1;
        }
        assert_eq!((undone, text.len()), (200, 50));
    }

    #[test]
    fn steps_keep_only_what_changed() {
        let step = Step::between("let ação = 1;", "let reação = 1;", Cursor::at(4), Cursor::at(6));
        assert_eq!((step.removed.as_str(), step.inserted.as_str()), ("", "re"));
        let step = Step::between("aaa", "aa", Cursor::at(3), Cursor::at(2));
        assert_eq!((step.at, step.removed.as_str(), step.inserted.as_str()), (2, "a", ""));
        let step = Step::between("é", "è", Cursor::at(1), Cursor::at(1));
        assert_eq!((step.removed.as_str(), step.inserted.as_str()), ("é", "è"));
    }

    #[test]
    fn clicks_land_on_the_nearest_boundary() {
        let b = [0.0, 10.0, 20.0, 30.0];
        assert_eq!(index_at(&b, -5.0), 0);
        assert_eq!(index_at(&b, 4.0), 0);
        assert_eq!(index_at(&b, 6.0), 1);
        assert_eq!(index_at(&b, 26.0), 3);
        assert_eq!(index_at(&b, 99.0), 3);
    }
}
