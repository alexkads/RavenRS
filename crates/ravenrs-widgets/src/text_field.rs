use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, CursorIcon, Insets, Key, Modifiers, Quad, Rect, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, DragPhase, NodeId, Paint, QuadPaint, Semantics};
use crate::{IconName, View, util};

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
/// let field = TextField::new(query.binding()).placeholder("Search").icon(IconName::Search);
/// let mut ui = Ui::new(field, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct TextField {
    value: Binding<String>,
    placeholder: String,
    icon: Option<IconName>,
    secure: bool,
    width: Option<f32>,
    compact: bool,
    autofocus: bool,
    on_submit: Option<Rc<dyn Fn(String)>>,
    on_cancel: Option<Rc<dyn Fn()>>,
    intercept: Option<crate::KeyHandler>,
}

impl fmt::Debug for TextField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextField").field("placeholder", &self.placeholder).finish_non_exhaustive()
    }
}

impl TextField {
    /// An empty-placeholder field bound to `value`.
    pub fn new(value: Binding<String>) -> Self {
        Self {
            value,
            placeholder: String::new(),
            icon: None,
            secure: false,
            width: None,
            compact: false,
            autofocus: false,
            on_submit: None,
            on_cancel: None,
            intercept: None,
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

    /// A shorter field with smaller text, for toolbars, filter rows and cells.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Takes keyboard focus when it first appears.
    pub fn autofocus(mut self) -> Self {
        self.autofocus = true;
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

    /// Called with the current text when Enter is pressed.
    pub fn on_submit(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(handler));
        self
    }
}

fn byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices().nth(char_index).map_or(s.len(), |(i, _)| i)
}

/// Caret and selection of a text field, as char indices. The selection runs
/// from `anchor` to `caret`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Cursor {
    pub caret: usize,
    pub anchor: Option<usize>,
}

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
    fn move_to(&mut self, to: usize, extend: bool) {
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
pub(crate) fn word_at(text: &str, at: usize) -> (usize, usize) {
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

fn insert(text: &mut String, cursor: &mut Cursor, s: &str) {
    delete_selection(text, cursor);
    let at = byte_index(text, cursor.caret);
    text.insert_str(at, s);
    *cursor = Cursor::at(cursor.caret + s.chars().count());
}

/// How a field treats the clipboard.
pub(crate) struct ClipboardAccess<'a> {
    pub clipboard: &'a crate::Clipboard,
    /// Passwords can be pasted into but never copied out of.
    pub secure: bool,
}

/// Applies a key to `text` and the cursor: typing, deleting, moving (Shift
/// extends the selection; Alt or Ctrl moves by word; Cmd moves to the ends),
/// select all, and copy, cut and paste with Cmd or Ctrl. Returns false for
/// keys the field does not handle, including a paste the clipboard cannot
/// read synchronously (the browser then delivers it as typed text).
pub(crate) fn edit(
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
                // A single-line field keeps the first line.
                let line = pasted.lines().next().unwrap_or_default();
                insert(text, cursor, line);
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

/// The char index nearest to `x` (relative to the text's left edge), given
/// the x position of every char boundary.
pub(crate) fn index_at(boundaries: &[f32], x: f32) -> usize {
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
        let cursor: State<Cursor> = cx.use_state(|| Cursor::at(usize::MAX));
        let focus_requested: State<bool> = cx.use_state(|| false);
        if self.autofocus && !*focus_requested.peek() {
            focus_requested.set_if_changed(true);
            cx.request_focus();
        }
        let focused = cx.is_focused();
        let hovered = cx.is_hovered();
        let text = self.value.get();
        let len = text.chars().count();
        let cur = cursor.get();
        let caret_at = cur.caret.min(len);
        let shown: String = if self.secure { "•".repeat(len) } else { text.clone() };
        let attrs =
            util::attrs(&theme, if self.compact { TextStyle::Callout } else { TextStyle::Body }, None);

        let mut children = Vec::new();
        if let Some(icon) = self.icon {
            children.push(util::icon(cx, icon, 16.0, theme.color(Semantic::InkMuted)));
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
            cx.text(&self.placeholder, &attrs, theme.color(Semantic::InkMuted), LayoutStyle::leaf())
        } else {
            cx.text(&shown, &attrs, theme.color(Semantic::Ink), LayoutStyle::leaf())
        };
        let accent = theme.color(Semantic::Accent);
        let line = attrs.line_height;
        let text_area = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                align: Align::Center,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[content],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                let y = f.center().y - line / 2.0 + 2.0;
                if let Some((a, b)) = selection {
                    scene.push_quad(
                        Quad::new(Rect::new(f.x() + a, y, b - a, line - 4.0), highlight).radius(2.0),
                    );
                }
                if focused && selection.is_none() {
                    let x = (f.x() + caret_x).min(f.max_x() - 1.5);
                    scene.push_quad(Quad::new(Rect::new(x, y, 1.5, line - 4.0), accent).radius(0.75));
                }
            })),
        );
        cx.clip(text_area);
        // Press places the caret, dragging selects, Shift-click extends.
        let (drag_cursor, mods, b) = (cursor.clone(), cx.modifiers(), boundaries.clone());
        cx.on_drag(
            text_area,
            Rc::new(move |d| {
                let at = index_at(&b, d.position.x - d.frame.x());
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
        children.push(text_area);

        let border = if focused {
            Border { width: 1.5, color: accent }
        } else {
            let base = theme.color(Semantic::Border);
            Border {
                width: 1.0,
                color: if hovered { base.lerp(theme.color(Semantic::Ink), 0.25) } else { base },
            }
        };
        let style = LayoutStyle {
            padding: Insets::symmetric(0.0, if self.compact { 8.0 } else { 12.0 }),
            height: Some(if self.compact { 32.0 } else { 40.0 }),
            width: self.width,
            min_width: Some(if self.compact { 0.0 } else { 160.0 }),
            spacing: 8.0,
            align: Align::Center,
            align_self: Some(if self.width.is_some() { Align::Start } else { Align::Stretch }),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = QuadPaint {
            corner_radius: theme.radii.md,
            border: Some(border),
            ..QuadPaint::new(theme.color(Semantic::Control))
        };
        let node = cx.container(style, &children, Paint::Quad(paint));
        // The visible text is exposed as the value; the placeholder names the
        // field until a label (a FormField, say) gives it a real name.
        cx.semantics(text_area, Semantics::hidden());
        let mut semantics = Semantics::new(Role::TextInput).value(shown);
        if !self.placeholder.is_empty() {
            semantics = semantics.name(self.placeholder.clone());
            semantics.fallback_name = true;
        }
        cx.semantics(node, semantics);

        let (value, submit, cancel) = (self.value.clone(), self.on_submit.clone(), self.on_cancel.clone());
        let (intercept, clipboard, secure) = (self.intercept.clone(), cx.clipboard(), self.secure);
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
                    if let Some(submit) = &submit {
                        submit(value.get());
                    }
                    return true;
                }
                let mut text = value.get();
                let mut c = cursor.get();
                let access = ClipboardAccess { clipboard: &clipboard, secure };
                let handled = edit(&mut text, &mut c, key, modifiers, Some(access));
                if handled {
                    cursor.set(c);
                    if text != value.get() {
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

    const NONE: Modifiers = Modifiers { shift: false, ctrl: false, alt: false, meta: false };
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
        let access = || Some(ClipboardAccess { clipboard: &clipboard, secure: false });
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
        let access = Some(ClipboardAccess { clipboard: &clipboard, secure: true });
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
        let access = Some(ClipboardAccess { clipboard: &clipboard, secure: false });
        assert!(!edit(&mut text, &mut c, &Key::Character("v".into()), CMD, access));
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
