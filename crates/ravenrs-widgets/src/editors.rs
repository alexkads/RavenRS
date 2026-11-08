//! Editors for values a plain text field cannot hold well: long text, numbers,
//! masked text and times.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, CursorIcon, Insets, Key, Modifiers, Point, Quad, Rect, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, DragPhase, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::date::Date;
use crate::overlay::Options;
use crate::text_field::{ClipboardAccess, Cursor, TextField, byte_index, edit, index_at, insert, word_at};
use crate::{ButtonSize, DatePicker, IconButton, IconName, VStack, View, util};

// ---------------------------------------------------------------- TextArea

/// Lines shown by default in a [`TextArea`].
const DEFAULT_ROWS: usize = 4;
/// Width assumed until the first layout says how wide a [`TextArea`] is.
const FIRST_GUESS_WIDTH: f32 = 320.0;
const AREA_PADDING: f32 = 12.0;

/// Breaks `text` into lines that fit `width`, as ranges of char indices. Line
/// breaks in the text always break, the break itself belongs to no line, and
/// long lines break after a space, or anywhere in a word wider than `width`.
pub(crate) fn wrap_lines(
    text: &str,
    width: f32,
    mut measure: impl FnMut(&str) -> f32,
) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let mut lines = Vec::new();
    let mut paragraph = 0;
    loop {
        let end = chars[paragraph..].iter().position(|c| *c == '\n').map_or(chars.len(), |n| paragraph + n);
        wrap_paragraph(&chars, paragraph, end, width, &mut measure, &mut lines);
        if end == chars.len() {
            return lines;
        }
        paragraph = end + 1;
    }
}

fn wrap_paragraph(
    chars: &[char],
    start: usize,
    end: usize,
    width: f32,
    measure: &mut impl FnMut(&str) -> f32,
    lines: &mut Vec<(usize, usize)>,
) {
    let piece = |a: usize, b: usize| chars[a..b].iter().collect::<String>();
    let mut line = start;
    let mut used = 0.0;
    let mut at = start;
    while at < end {
        // A word with the spaces after it.
        let mut next = at;
        while next < end && !chars[next].is_whitespace() {
            next += 1;
        }
        while next < end && chars[next].is_whitespace() {
            next += 1;
        }
        let w = measure(&piece(at, next));
        if used + w > width && line < at {
            lines.push((line, at));
            line = at;
            used = 0.0;
        }
        if w > width {
            // Wider than a whole line: cut the word where it stops fitting.
            let mut from = at;
            loop {
                let mut to = from + 1;
                while to < next && measure(&piece(from, to + 1)) <= width {
                    to += 1;
                }
                if to == next {
                    line = from;
                    used = measure(&piece(from, to));
                    break;
                }
                lines.push((from, to));
                from = to;
            }
        } else {
            used += w;
        }
        at = next;
    }
    lines.push((line, end));
}

/// The line holding char index `caret`. A caret at a soft break belongs to the
/// next line, where typing would continue.
fn line_of(lines: &[(usize, usize)], caret: usize) -> usize {
    lines.iter().rposition(|(start, _)| *start <= caret).unwrap_or(0)
}

/// Colored stretches of a text, as `(from, to, color)` in char indices, sorted and apart.
pub(crate) type Highlighter = Rc<dyn Fn(&str) -> Vec<(usize, usize, Semantic)>>;

/// A line a code area marks: its index from 0, the color, and the char columns to underline.
pub(crate) type LineMark = (usize, Semantic, Option<(usize, usize)>);

/// How a text area shows code, set by [`crate::CodeEditor`]: a monospaced font, no
/// wrapping, line numbers, colors and marks, and keys for indenting.
#[derive(Clone)]
pub(crate) struct CodeLook {
    pub(crate) highlight: Option<Highlighter>,
    pub(crate) line_numbers: bool,
    pub(crate) marks: Rc<Vec<LineMark>>,
    /// Spaces Tab inserts and Enter carries on; Ctrl or Cmd with Enter is left to shortcuts.
    pub(crate) indent: usize,
    /// Takes the height its parent gives instead of a number of rows.
    pub(crate) fill: bool,
    pub(crate) background: Semantic,
}

/// A marked line as drawn: its top, color, and the span to underline in pixels.
type DrawnMark = (f32, ravenrs_core::Color, Option<(f32, f32)>);

/// Room between the line numbers and the code.
const GUTTER_GAP: f32 = 16.0;

/// A multi-line text input bound to a `String`. Enter starts a new line, the
/// arrows move between lines, and it scrolls past [`Self::rows`] lines.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{TextArea, Ui};
///
/// let notes = State::new(String::new());
/// let field = TextArea::new(notes.binding()).placeholder("Notes").rows(6);
/// let mut ui = Ui::new(field, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct TextArea {
    value: Binding<String>,
    placeholder: String,
    rows: usize,
    grows: Option<(usize, usize)>,
    width: Option<f32>,
    state: util::EditorState,
    /// Caret and selection kept by whoever built the area, so it can insert at the caret.
    cursor: Option<State<Cursor>>,
    code: Option<CodeLook>,
}

impl fmt::Debug for TextArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextArea").field("rows", &self.rows).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TextArea {
    /// An empty-placeholder area bound to `value`.
    pub fn new(value: Binding<String>) -> Self {
        Self {
            value,
            placeholder: String::new(),
            rows: DEFAULT_ROWS,
            grows: None,
            width: None,
            state: util::EditorState::default(),
            cursor: None,
            code: None,
        }
    }

    /// Shows the text as code.
    pub(crate) fn code(mut self, look: CodeLook) -> Self {
        self.code = Some(look);
        self
    }

    /// Keeps the caret and selection in `cursor`, for a component that inserts text where
    /// the caret is.
    pub(crate) fn cursor(mut self, cursor: State<Cursor>) -> Self {
        self.cursor = Some(cursor);
        self
    }

    /// Muted text shown while the value is empty.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// How many lines are visible; more than that scroll. 4 by default.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// Grows with the text from `min` to `max` lines, and scrolls past that, as a message
    /// box does. It replaces the fixed height of [`Self::rows`].
    pub fn grows(mut self, min: usize, max: usize) -> Self {
        let min = min.max(1);
        self.grows = Some((min, max.max(min)));
        self
    }

    /// Fixed width in logical pixels; by default the area stretches.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Turns the area off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the text without letting it change; it can still be focused, selected and copied.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

/// Moves the caret to the same column of another line.
fn move_lines(lines: &[(usize, usize)], c: &mut Cursor, down: bool, extend: bool) {
    let here = line_of(lines, c.caret);
    let column = c.caret - lines[here].0;
    let there = if down { here + 1 } else { here.wrapping_sub(1) };
    let to = match lines.get(there) {
        Some((start, end)) => (start + column).min(*end),
        None if down => lines[here].1,
        None => lines[here].0,
    };
    c.move_to(to, extend);
}

/// Applies a key to a text area. Returns false for keys it does not handle.
fn edit_area(
    text: &mut String,
    c: &mut Cursor,
    lines: &[(usize, usize)],
    key: &Key,
    modifiers: Modifiers,
    access: ClipboardAccess<'_>,
    indent: usize,
) -> bool {
    let len = text.chars().count();
    c.caret = c.caret.min(len);
    let plain = !modifiers.ctrl && !modifiers.meta;
    match key {
        Key::Enter if indent > 0 && !plain => return false,
        Key::Enter if indent > 0 => {
            let chars: Vec<char> = text.chars().collect();
            let start = lines[line_of(lines, c.caret)].0;
            let before = &chars[start..c.caret.max(start)];
            let mut spaces = before.iter().take_while(|ch| **ch == ' ').count();
            if before
                .iter()
                .rev()
                .find(|ch| !ch.is_whitespace())
                .is_some_and(|ch| matches!(ch, '{' | '(' | '['))
            {
                spaces += indent;
            }
            insert(text, c, &format!("\n{}", " ".repeat(spaces)));
        }
        Key::Tab if indent > 0 && modifiers.shift => {
            let start = lines[line_of(lines, c.caret)].0;
            let leading = text.chars().skip(start).take(indent).take_while(|ch| *ch == ' ').count();
            if leading > 0 {
                let from = byte_index(text, start);
                text.replace_range(from..byte_index(text, start + leading), "");
                c.caret = c.caret.saturating_sub(leading).max(start);
                c.anchor = None;
            }
        }
        Key::Tab if indent > 0 => {
            let column = c.caret - lines[line_of(lines, c.caret)].0;
            insert(text, c, &" ".repeat(indent - column % indent));
        }
        Key::Enter => insert(text, c, "\n"),
        Key::ArrowUp | Key::ArrowDown if plain => {
            move_lines(lines, c, *key == Key::ArrowDown, modifiers.shift)
        }
        Key::Home if plain => c.move_to(lines[line_of(lines, c.caret)].0, modifiers.shift),
        Key::End if plain => c.move_to(lines[line_of(lines, c.caret)].1, modifiers.shift),
        Key::Character(s) if plain && s.contains(['\n', '\r']) => {
            insert(text, c, &s.replace("\r\n", "\n").replace('\r', "\n"));
        }
        _ => return edit(text, c, key, modifiers, Some(access)),
    }
    true
}

impl View for TextArea {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let state = self.state;
        let own: State<Cursor> = cx.use_state(|| Cursor::at(usize::MAX));
        let cursor = self.cursor.clone().unwrap_or(own);
        let followed: State<usize> = cx.use_state(|| usize::MAX);
        let focused = cx.is_focused() && !state.disabled;
        let hovered = cx.is_hovered() && !state.disabled;
        let text = self.value.get();
        let chars: Vec<char> = text.chars().collect();
        let code = self.code.clone();
        let mut attrs = util::attrs(&theme, TextStyle::Body, None);
        if code.is_some() {
            attrs.family = Some("monospace".to_owned());
            attrs.size -= 1.0;
        }
        let attrs = attrs;
        let line_height = attrs.line_height;

        // The width of the last layout, known from the second build on.
        let viewport = cx.scroll_viewport();
        let outer = self.width.or(viewport.map(|v| v.width)).unwrap_or(FIRST_GUESS_WIDTH);
        let inner = (outer - 2.0 * AREA_PADDING).max(1.0);
        let lines = if code.is_some() {
            // Code does not wrap: a line is what lies between line breaks.
            wrap_lines(&text, f32::INFINITY, |_| 0.0)
        } else {
            let attrs = attrs.clone();
            wrap_lines(&text, inner, |s| cx.measure_text(s, &attrs).width)
        };
        let lines = Rc::new(lines);
        let fill = code.as_ref().is_some_and(|c| c.fill);
        let rows_shown = if fill {
            viewport.map_or(self.rows, |v| ((v.height - AREA_PADDING) / line_height).ceil().max(1.0) as usize)
        } else {
            self.grows.map_or(self.rows, |(min, max)| lines.len().clamp(min, max))
        };
        let height = rows_shown as f32 * line_height + AREA_PADDING;
        let cur = cursor.get();
        let caret = cur.caret.min(chars.len());
        let caret_line = line_of(&lines, caret);

        // Keep the caret in view after it moves, and leave the wheel alone otherwise.
        let mut offset = cx.scroll_offset().y;
        // Everything fits again once text is deleted, and nothing is left to scroll.
        if self.grows.is_some() && lines.len() <= rows_shown && offset > 0.0 {
            offset = 0.0;
            cx.set_scroll_offset(Point::new(0.0, 0.0));
        }
        if focused && followed.get() != caret {
            followed.set_if_changed(caret);
            let top = caret_line as f32 * line_height;
            // While it grows, the last layout's height is already out of date.
            let last_height = viewport.filter(|_| self.grows.is_none()).map_or(height, |v| v.height);
            let visible = last_height - AREA_PADDING;
            let next = if top < offset {
                top
            } else if top + line_height > offset + visible {
                top + line_height - visible
            } else {
                offset
            };
            if next != offset {
                offset = next;
                cx.set_scroll_offset(Point::new(0.0, next));
            }
        }

        // Only the lines in view are built; spacers hold the place of the rest.
        let first = ((offset / line_height).floor() as usize).min(lines.len().saturating_sub(1));
        let last = (first + rows_shown + 2).min(lines.len());
        let piece = |a: usize, b: usize| chars[a..b].iter().collect::<String>();
        let mut boundaries: Vec<(usize, Vec<f32>)> = Vec::with_capacity(last - first);
        let mut rows = Vec::with_capacity(last - first + 2);
        if first > 0 {
            rows.push(cx.container(
                LayoutStyle { height: Some(first as f32 * line_height), ..LayoutStyle::leaf() },
                &[],
                Paint::None,
            ));
        }
        if text.is_empty() {
            let color = state.ink(theme.color(Semantic::InkMuted));
            rows.push(cx.text(&self.placeholder, &attrs, color, LayoutStyle::leaf()));
            boundaries.push((0, vec![0.0]));
        } else {
            let spans =
                code.as_ref().and_then(|c| c.highlight.as_ref()).map(|h| h(&text)).unwrap_or_default();
            // In a monospaced font a character's place is its column times one advance, which
            // also counts the spaces a measured piece would leave out at its end.
            let advance = code.is_some().then(|| cx.measure_text("0000000000", &attrs).width / 10.0);
            for &(a, b) in &lines[first..last] {
                let shown = piece(a, b);
                let color = state.ink(theme.color(Semantic::Ink));
                let line = LayoutStyle { height: Some(line_height), ..LayoutStyle::leaf() };
                let xs: Vec<f32> = match advance {
                    Some(advance) => (a..=b).map(|i| (i - a) as f32 * advance).collect(),
                    None => (a..=b).map(|i| cx.measure_text(&piece(a, i), &attrs).width).collect(),
                };
                rows.push(if shown.is_empty() {
                    cx.container(line, &[], Paint::None)
                } else if spans.is_empty() {
                    cx.text(&shown, &attrs, color, line)
                } else {
                    // Each colored stretch sits where its first character falls.
                    let mut pieces = Vec::new();
                    let mut at = a;
                    let mut push = |cx: &mut BuildCx<'_>, from: usize, to: usize, semantic: Semantic| {
                        let words = piece(from, to);
                        if words.trim().is_empty() {
                            return;
                        }
                        let place =
                            LayoutStyle { absolute: Some((xs[from - a], 0.0)), ..LayoutStyle::leaf() };
                        pieces.push(cx.text(&words, &attrs, state.ink(theme.color(semantic)), place));
                    };
                    for &(from, to, semantic) in spans.iter().filter(|(from, to, _)| *to > a && *from < b) {
                        let (from, to) = (from.max(a), to.min(b));
                        if from > at {
                            push(cx, at, from, Semantic::Ink);
                        }
                        push(cx, from, to, semantic);
                        at = to;
                    }
                    if at < b {
                        push(cx, at, b, Semantic::Ink);
                    }
                    cx.container(line, &pieces, Paint::None)
                });
                boundaries.push((a, xs));
            }
        }
        if last < lines.len() {
            rows.push(cx.container(
                LayoutStyle {
                    height: Some((lines.len() - last) as f32 * line_height),
                    ..LayoutStyle::leaf()
                },
                &[],
                Paint::None,
            ));
        }
        let boundaries = Rc::new(boundaries);

        // The selection on each visible line, and the caret, under the text.
        let x_at = |i: usize, line: usize| -> f32 {
            let Some((start, xs)) = boundaries.get(line.wrapping_sub(first)) else { return 0.0 };
            xs.get(i.saturating_sub(*start)).copied().unwrap_or(0.0)
        };
        let selected: Vec<(f32, f32, f32)> = cur
            .selection()
            .map(|(a, b)| {
                (first..last)
                    .filter(|&n| lines[n].0 <= b && a <= lines[n].1)
                    .map(|n| {
                        let (from, to) = (a.max(lines[n].0), b.min(lines[n].1));
                        (n as f32 * line_height, x_at(from, n), x_at(to, n).max(x_at(from, n) + 4.0))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let caret_spot = (focused && cur.selection().is_none() && state.accepts_input())
            .then(|| (x_at(caret, caret_line), caret_line as f32 * line_height));
        let accent = theme.color(Semantic::Accent);
        let highlight = accent.with_alpha(if focused { 0.32 } else { 0.16 });
        // Marked lines: a band across the line, and a wavy-free underline under the columns.
        let marks: Vec<DrawnMark> = code
            .as_ref()
            .map(|c| {
                c.marks
                    .iter()
                    .filter(|(n, _, _)| (first..last).contains(n))
                    .map(|&(n, semantic, columns)| {
                        let under = columns.map(|(from, to)| {
                            let start = lines[n].0;
                            let (from, to) = ((start + from).min(lines[n].1), (start + to).min(lines[n].1));
                            (x_at(from, n), x_at(to, n).max(x_at(from, n) + 8.0))
                        });
                        (n as f32 * line_height, theme.color(semantic), under)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let gutter_marks: Vec<(f32, ravenrs_core::Color)> = marks.iter().map(|(y, c, _)| (*y, *c)).collect();
        let content = cx.container(
            LayoutStyle {
                align: Align::Start,
                grow: if code.is_some() { 1.0 } else { 0.0 },
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &rows,
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                for (y, color, under) in &marks {
                    let band = Rect::new(
                        f.x() - AREA_PADDING / 2.0,
                        f.y() + y,
                        f.width() + AREA_PADDING,
                        line_height,
                    );
                    scene.push_quad(Quad::new(band, color.with_alpha(0.14)));
                    if let Some((a, b)) = under {
                        let line = Rect::new(f.x() + a, f.y() + y + line_height - 3.0, b - a, 2.0);
                        scene.push_quad(Quad::new(line, *color).radius(1.0));
                    }
                }
                for (y, a, b) in &selected {
                    let band = Rect::new(f.x() + a, f.y() + y + 2.0, b - a, line_height - 4.0);
                    scene.push_quad(Quad::new(band, highlight).radius(2.0));
                }
                if let Some((x, y)) = caret_spot {
                    let bar = Rect::new(f.x() + x, f.y() + y + 2.0, 1.5, line_height - 4.0);
                    scene.push_quad(Quad::new(bar, accent).radius(0.75));
                }
            })),
        );
        cx.semantics(content, Semantics::hidden());

        // Line numbers, built for the same lines in view, with spacers for the rest.
        let gutter = code.as_ref().filter(|c| c.line_numbers).map(|_| {
            let digits = lines.len().max(1).to_string().len().max(2);
            let digit = cx.measure_text("0", &attrs).width;
            let width = digits as f32 * digit;
            let mut numbers = Vec::with_capacity(last - first + 2);
            if first > 0 {
                numbers.push(cx.container(
                    LayoutStyle { height: Some(first as f32 * line_height), ..LayoutStyle::leaf() },
                    &[],
                    Paint::None,
                ));
            }
            let marked: Vec<(usize, Semantic)> =
                code.as_ref().map(|c| c.marks.iter().map(|(n, s, _)| (*n, *s)).collect()).unwrap_or_default();
            for n in first..last.max(first + 1).min(lines.len().max(1)) {
                let label = (n + 1).to_string();
                let semantic = marked.iter().find(|(m, _)| *m == n).map_or(Semantic::InkMuted, |(_, s)| *s);
                let x = width - cx.measure_text(&label, &attrs).width;
                let place = LayoutStyle { absolute: Some((x, 0.0)), ..LayoutStyle::leaf() };
                let number = cx.text(&label, &attrs, state.ink(theme.color(semantic)), place);
                numbers.push(cx.container(
                    LayoutStyle { height: Some(line_height), width: Some(width), ..LayoutStyle::leaf() },
                    &[number],
                    Paint::None,
                ));
            }
            if last < lines.len() {
                numbers.push(cx.container(
                    LayoutStyle {
                        height: Some((lines.len() - last) as f32 * line_height),
                        ..LayoutStyle::leaf()
                    },
                    &[],
                    Paint::None,
                ));
            }
            let column = cx.container(
                LayoutStyle {
                    width: Some(width + GUTTER_GAP),
                    padding: Insets { right: GUTTER_GAP, ..Insets::default() },
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &numbers,
                Paint::Custom(Rc::new(move |f: Rect, scene| {
                    for (y, color) in &gutter_marks {
                        let bar = Rect::new(f.x() - AREA_PADDING, f.y() + y, 3.0, line_height);
                        scene.push_quad(Quad::new(bar, *color));
                    }
                })),
            );
            cx.semantics(column, Semantics::hidden());
            column
        });

        let border = if focused {
            Border { width: 1.5, color: accent }
        } else {
            let base = theme.color(Semantic::Border);
            Border {
                width: 1.0,
                color: state.ink(if hovered { base.lerp(theme.color(Semantic::Ink), 0.25) } else { base }),
            }
        };
        let children: Vec<NodeId> = gutter.into_iter().chain([content]).collect();
        let background = code.as_ref().map_or(Semantic::Control, |c| c.background);
        let node = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(AREA_PADDING / 2.0, AREA_PADDING),
                height: if fill { None } else { Some(height) },
                min_height: fill.then_some(line_height + AREA_PADDING),
                grow: if fill { 1.0 } else { 0.0 },
                width: self.width,
                min_width: Some(160.0),
                overflow_hidden: true,
                align_self: Some(if self.width.is_some() { Align::Start } else { Align::Stretch }),
                ..LayoutStyle::stack(if code.is_some() { Axis::Horizontal } else { Axis::Vertical })
            },
            &children,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(border),
                ..QuadPaint::new(state.fill(theme.color(background)))
            }),
        );
        cx.scrollable_virtual(node, false);
        crate::motion::control(cx, node);
        let mut semantics = Semantics::new(Role::TextInput).value(text.clone());
        if !self.placeholder.is_empty() {
            semantics = semantics.name(self.placeholder.clone());
            semantics.fallback_name = true;
        }
        cx.semantics(node, state.semantics(semantics));
        if state.disabled {
            return node;
        }

        // Press places the caret, dragging selects, Shift-click extends.
        let hit = {
            let (lines, boundaries) = (lines.clone(), boundaries.clone());
            move |p: Point, frame: Rect| -> usize {
                let n = (((p.y - frame.y()) / line_height).max(0.0) as usize).min(lines.len() - 1);
                let Some((start, xs)) = n.checked_sub(first).and_then(|i| boundaries.get(i)) else {
                    return lines[n].0;
                };
                start + index_at(xs, p.x - frame.x())
            }
        };
        let (drag_cursor, mods) = (cursor.clone(), cx.modifiers());
        cx.on_drag(
            content,
            Rc::new(move |d| {
                let at = hit(d.position, d.frame);
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
            content,
            Rc::new(move || {
                let caret = word_cursor.get().caret;
                let (a, b) = word_at(&word_value.get(), caret);
                word_cursor.set(Cursor { caret: b, anchor: Some(a) });
            }),
        );
        cx.cursor(content, CursorIcon::Text);

        let (value, clipboard) = (self.value.clone(), cx.clipboard());
        let indent = code.as_ref().map_or(0, |c| c.indent);
        cx.on_key(
            node,
            Rc::new(move |key, modifiers| {
                let mut text = value.get();
                let mut c = cursor.get();
                let access = ClipboardAccess { clipboard: &clipboard, secure: false, multiline: true };
                let handled = edit_area(&mut text, &mut c, &lines, key, modifiers, access, indent);
                if handled {
                    let changed = text != value.get();
                    if changed && !state.accepts_input() {
                        return true;
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

// ------------------------------------------------------------- NumberField

/// Reads a number typed with these separators. Besides the exact format it
/// takes the other common way to write a decimal, so "12.5" works where the
/// decimal separator is a comma: a lone group separator counts as decimal unless
/// exactly three digits follow it.
pub(crate) fn parse_number(text: &str, decimal: char, group: char) -> Option<f64> {
    let trimmed = text.trim();
    let normal: String = if trimmed.contains(decimal) {
        trimmed.chars().filter(|c| *c != group).map(|c| if c == decimal { '.' } else { c }).collect()
    } else if trimmed.matches(group).count() == 1 {
        let (_, after) = trimmed.split_once(group)?;
        if after.chars().count() == 3 {
            trimmed.chars().filter(|c| *c != group).collect()
        } else {
            trimmed.replace(group, ".")
        }
    } else {
        trimmed.chars().filter(|c| *c != group).collect()
    };
    let valid = !normal.is_empty()
        && normal.chars().enumerate().all(|(i, c)| c.is_ascii_digit() || c == '.' || (i == 0 && c == '-'));
    if !valid {
        return None;
    }
    normal.parse().ok().filter(|n: &f64| n.is_finite())
}

/// A typed number. While it has focus it shows what is being typed, and without
/// focus the formatted value, such as `R$ 1.234,50`. Text that is not a number in
/// range stays visible for fixing and leaves the value alone. Up and Down step.
///
/// The separators come from [`crate::Labels`], through [`crate::Localized`].
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{NumberField, Ui};
///
/// let price = State::new(Some(1234.5));
/// let field = NumberField::new(price.binding()).decimals(2).range(0.0, 1e9).prefix("R$ ");
/// let mut ui = Ui::new(field, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct NumberField {
    value: Binding<Option<f64>>,
    min: f64,
    max: f64,
    step: f64,
    decimals: usize,
    grouping: bool,
    prefix: String,
    suffix: String,
    placeholder: String,
    width: Option<f32>,
    spin_buttons: bool,
    label: Option<String>,
    on_leave: Option<Rc<dyn Fn()>>,
    state: util::EditorState,
}

impl std::fmt::Debug for NumberField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NumberField").field("label", &self.label).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl NumberField {
    /// Called when the keyboard goes elsewhere after being here.
    pub fn on_leave(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_leave = Some(Rc::new(handler));
        self
    }

    /// A field bound to `value`, unbounded, with no decimals; `None` is empty.
    pub fn new(value: Binding<Option<f64>>) -> Self {
        Self {
            value,
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
            step: 1.0,
            decimals: 0,
            grouping: true,
            prefix: String::new(),
            suffix: String::new(),
            placeholder: String::new(),
            width: None,
            spin_buttons: false,
            label: None,
            on_leave: None,
            state: util::EditorState::default(),
        }
    }

    /// A field for money held as a whole number of cents: exact, two decimals,
    /// showing `prefix`, such as `"R$ "`.
    pub fn money(cents: Binding<Option<i64>>, prefix: impl Into<String>) -> Self {
        let value = cents.map(
            |c| c.map(|c| c as f64 / 100.0),
            |c, v: Option<f64>| *c = v.map(|v| (v * 100.0).round() as i64),
        );
        Self::new(value).decimals(2).step(1.0).prefix(prefix)
    }

    /// Accepts only `min..=max`.
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = min;
        self.max = max.max(min);
        self
    }

    /// How much Up, Down and the buttons change the value. 1 by default.
    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Digits shown after the decimal separator without focus. 0 by default.
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = decimals;
        self
    }

    /// Whether thousands are separated without focus. On by default.
    pub fn grouping(mut self, grouping: bool) -> Self {
        self.grouping = grouping;
        self
    }

    /// Text before the number without focus, such as a currency.
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Text after the number without focus, such as `%`.
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }

    /// Muted text shown while the field is empty.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Fixed width in logical pixels; by default the field stretches.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// The name screen readers announce. Inside a [`crate::FormField`], its label is used instead.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Shows buttons that take a step away and add one.
    pub fn spin_buttons(mut self) -> Self {
        self.spin_buttons = true;
        self
    }

    /// Turns the field off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the value without letting it change; it can still be focused and read.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

impl View for NumberField {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let (decimal, group) = (labels.decimal_separator, labels.group_separator);
        // What was typed, with the value it was typed over; a value that changed
        // from outside since then wins over the old text.
        let draft: State<Option<(String, Option<f64>)>> = cx.use_state(|| None);
        let value = self.value.get();
        if draft.get().is_some_and(|(_, over)| over != value) {
            draft.set(None);
        }
        let plain = move |n: f64| {
            ravenrs_criteria::NumberStyle {
                decimals: fraction_digits(n),
                group: None,
                decimal,
                prefix: "",
                suffix: "",
            }
            .format(n)
        };
        let (min, max) = (self.min, self.max);
        let typed = draft.get();
        let formatted = value.map_or_else(String::new, |n| {
            ravenrs_criteria::NumberStyle {
                decimals: self.decimals,
                group: self.grouping.then_some(group),
                decimal,
                prefix: &self.prefix,
                suffix: &self.suffix,
            }
            .format(n)
        });
        let invalid = typed.as_ref().is_some_and(|(text, _)| {
            !text.trim().is_empty()
                && parse_number(text, decimal, group).is_none_or(|n| !(min..=max).contains(&n))
        });

        let (read, write, target) = (draft.clone(), draft.clone(), self.value.clone());
        let current = self.value.clone();
        let text = Binding::new(
            move || read.get().map(|(text, _)| text).or_else(|| current.get().map(plain)).unwrap_or_default(),
            move |text: String| {
                let parsed = parse_number(&text, decimal, group).filter(|n| (min..=max).contains(n));
                let next = if text.trim().is_empty() { None } else { parsed.or(target.get()) };
                write.set(Some((text, next)));
                if next != target.get() {
                    target.set(next);
                }
            },
        );
        let (step, stepped, cleared) = (self.step, self.value.clone(), draft.clone());
        let bump = Rc::new(move |up: bool| {
            let base = stepped.get().unwrap_or(if min.is_finite() { min } else { 0.0 });
            let next = if up { base + step } else { base - step };
            cleared.set(None);
            stepped.set(Some(next.clamp(min, max)));
        });
        let keys = bump.clone();
        let accepts = self.state.accepts_input();
        let mut field = TextField::new(text)
            .placeholder(self.placeholder.clone())
            .disabled(self.state.disabled)
            .read_only(self.state.read_only)
            .intercept_keys(Rc::new(move |key, _| match key {
                Key::ArrowUp | Key::ArrowDown if accepts => {
                    keys(*key == Key::ArrowUp);
                    true
                }
                _ => false,
            }));
        if let Some(left) = self.on_leave.clone() {
            field = field.on_leave(move || left());
        }
        // Bad text stays on show so it can be fixed; otherwise the formatted value.
        if !invalid {
            field.idle_text = Some(formatted);
        }
        if let Some(width) = self.width {
            field = field.width(width);
        }
        let node = cx.child(&field);
        if let Some(label) = &self.label {
            cx.update_semantics(node, |s| s.name = Some(label.clone()));
        }
        if !self.spin_buttons {
            return node;
        }
        let (down, up) = (bump.clone(), bump);
        let minus = cx.child(
            &IconButton::new(IconName::Minus)
                .label(labels.step_down.clone())
                .size(ButtonSize::Small)
                .disabled(!accepts)
                .on_click(move || down(false)),
        );
        let plus = cx.child(
            &IconButton::new(IconName::Plus)
                .label(labels.step_up.clone())
                .size(ButtonSize::Small)
                .disabled(!accepts)
                .on_click(move || up(true)),
        );
        cx.container(
            LayoutStyle {
                spacing: 4.0,
                align: Align::Center,
                align_self: Some(if self.width.is_some() { Align::Start } else { Align::Stretch }),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[node, minus, plus],
            Paint::None,
        )
    }
}

/// The fewest decimals that show `n` as it is, when editing it.
fn fraction_digits(n: f64) -> usize {
    let text = n.to_string();
    text.split_once('.').map_or(0, |(_, fraction)| fraction.len())
}

// ------------------------------------------------------------- MaskedField

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaskSlot {
    Digit,
    Letter,
    Any,
    Literal(char),
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl MaskSlot {
    fn accepts(self, c: char) -> bool {
        match self {
            Self::Digit => c.is_ascii_digit(),
            Self::Letter => c.is_alphabetic(),
            Self::Any => c.is_alphanumeric(),
            Self::Literal(_) => false,
        }
    }
}

/// `0` is a digit, `a` a letter and `*` a letter or digit. `\` makes the next
/// character plain, and any other character is shown as it is.
fn parse_mask(mask: &str) -> Vec<MaskSlot> {
    let mut chars = mask.chars();
    let mut slots = Vec::new();
    while let Some(c) = chars.next() {
        slots.push(match c {
            '0' => MaskSlot::Digit,
            'a' => MaskSlot::Letter,
            '*' => MaskSlot::Any,
            '\\' => MaskSlot::Literal(chars.next().unwrap_or('\\')),
            other => MaskSlot::Literal(other),
        });
    }
    slots
}

/// Pulls what the slots accept out of text in the mask's format, or any text.
fn mask_extract(slots: &[MaskSlot], text: &str) -> String {
    let mut chars = text.chars().peekable();
    let mut raw = String::new();
    for slot in slots {
        match slot {
            MaskSlot::Literal(l) => {
                if chars.peek() == Some(l) {
                    chars.next();
                }
            }
            slot => {
                for c in chars.by_ref() {
                    if slot.accepts(c) {
                        raw.push(c);
                        break;
                    }
                }
            }
        }
    }
    raw
}

/// Puts `raw` in the mask, stopping where it runs out.
fn mask_fill(slots: &[MaskSlot], raw: &str) -> String {
    let mut chars = raw.chars();
    let mut out = String::new();
    let mut pending = String::new();
    for slot in slots {
        match slot {
            MaskSlot::Literal(l) => pending.push(*l),
            _ => {
                let Some(c) = chars.next() else { break };
                out.push_str(&pending);
                pending.clear();
                out.push(c);
            }
        }
    }
    out
}

/// A text input that follows a pattern, such as `(00) 00000-0000`, in which `0`
/// is a digit, `a` a letter and `*` either; `\` makes the next character plain.
/// The binding holds only what was typed, without the pattern's own characters,
/// unless [`Self::keep_literals`] asks for them.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{MaskedField, Ui};
///
/// let phone = State::new(String::from("11987654321"));
/// let field = MaskedField::new(phone.binding(), "(00) 00000-0000");
/// let mut ui = Ui::new(field, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct MaskedField {
    value: Binding<String>,
    mask: String,
    keep_literals: bool,
    placeholder: Option<String>,
    width: Option<f32>,
    label: Option<String>,
    state: util::EditorState,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl MaskedField {
    /// A field bound to `value`, following `mask`.
    pub fn new(value: Binding<String>, mask: impl Into<String>) -> Self {
        Self {
            value,
            mask: mask.into(),
            keep_literals: false,
            placeholder: None,
            width: None,
            label: None,
            state: util::EditorState::default(),
        }
    }

    /// The name screen readers announce. Inside a [`crate::FormField`], its label is used instead.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The binding holds the text as shown, with the pattern's own characters.
    pub fn keep_literals(mut self) -> Self {
        self.keep_literals = true;
        self
    }

    /// Muted text shown while empty; by default the pattern with `_` in its slots.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Fixed width in logical pixels; by default the field stretches.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Turns the field off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the value without letting it change; it can still be focused and read.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

impl View for MaskedField {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let slots = Rc::new(parse_mask(&self.mask));
        let hint = self.placeholder.clone().unwrap_or_else(|| {
            slots.iter().map(|s| if let MaskSlot::Literal(l) = s { *l } else { '_' }).collect::<String>()
        });
        let (read, write) = (self.value.clone(), self.value.clone());
        let (from_raw, to_raw) = (slots.clone(), slots);
        let keep = self.keep_literals;
        let shown = Binding::new(
            move || {
                let value = read.get();
                let raw = if keep { mask_extract(&from_raw, &value) } else { value };
                mask_fill(&from_raw, &raw)
            },
            move |text: String| {
                let raw = mask_extract(&to_raw, &text);
                let next = if keep { mask_fill(&to_raw, &raw) } else { raw };
                if next != write.get() {
                    write.set(next);
                }
            },
        );
        let mut field = TextField::new(shown)
            .placeholder(hint)
            .disabled(self.state.disabled)
            .read_only(self.state.read_only);
        field.caret_follows_end = true;
        if let Some(width) = self.width {
            field = field.width(width);
        }
        let node = cx.child(&field);
        if let Some(label) = &self.label {
            cx.update_semantics(node, |s| s.name = Some(label.clone()));
        }
        node
    }
}

// ------------------------------------------------------------ Time editors

/// A time of day, with no date or time zone.
///
/// ```
/// use ravenrs_widgets::Time;
///
/// let t = Time::new(9, 30).unwrap();
/// assert_eq!(t.to_string(), "09:30");
/// assert!(Time::new(24, 0).is_none());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Time {
    hour: u8,
    minute: u8,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Time {
    /// Returns `None` for an impossible time.
    pub fn new(hour: u8, minute: u8) -> Option<Self> {
        (hour < 24 && minute < 60).then_some(Self { hour, minute })
    }

    /// The hour, from 0 to 23.
    pub fn hour(self) -> u8 {
        self.hour
    }

    /// The minute, from 0 to 59.
    pub fn minute(self) -> u8 {
        self.minute
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

/// A date and a time of day, with no time zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct DateTime {
    /// The day.
    pub date: Date,
    /// The time on that day.
    pub time: Time,
}

/// A 24-hour time field: type it, or choose from a list in steps of
/// [`Self::step_minutes`].
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Time, TimePicker, Ui};
///
/// let at = State::new(Time::new(9, 30));
/// let mut ui = Ui::new(TimePicker::new(at.binding()), ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct TimePicker {
    value: Binding<Option<Time>>,
    step: u16,
    width: f32,
    label: Option<String>,
    state: util::EditorState,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TimePicker {
    /// A picker bound to `value`; `None` is empty.
    pub fn new(value: Binding<Option<Time>>) -> Self {
        Self { value, step: 30, width: 120.0, label: None, state: util::EditorState::default() }
    }

    /// The minutes between the times in the list. 30 by default.
    pub fn step_minutes(mut self, minutes: u16) -> Self {
        self.step = minutes.clamp(1, 720);
        self
    }

    /// The name screen readers announce. Inside a [`crate::FormField`], its label is used instead.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Field width in logical pixels. 120 by default.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Turns the picker off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the time without letting it change; it can still be focused and read.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

/// Reads the four digits a time field holds, once they are all there.
fn time_from_digits(digits: &str) -> Option<Time> {
    if digits.len() != 4 {
        return None;
    }
    Time::new(digits[..2].parse().ok()?, digits[2..].parse().ok()?)
}

impl View for TimePicker {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let open: State<bool> = cx.use_state(|| false);
        // Unfinished digits live here until they make a time.
        let draft: State<Option<String>> = cx.use_state(|| None);
        let labels = cx.labels();
        let (read, write, target, value) =
            (draft.clone(), draft.clone(), self.value.clone(), self.value.clone());
        let digits = Binding::new(
            move || {
                read.get().unwrap_or_else(|| {
                    value.get().map_or_else(String::new, |t| format!("{:02}{:02}", t.hour, t.minute))
                })
            },
            move |text: String| {
                let time = time_from_digits(&text);
                write.set(if time.is_some() || text.is_empty() { None } else { Some(text.clone()) });
                if time.is_some() || text.is_empty() {
                    target.set(time);
                }
            },
        );
        let mut field = MaskedField::new(digits, "00:00")
            .width(self.width)
            .disabled(self.state.disabled)
            .read_only(self.state.read_only);
        if let Some(label) = &self.label {
            field = field.label(label.clone());
        }
        let field = cx.child(&field);
        let accepts = self.state.accepts_input();
        let toggle = open.clone();
        let button = cx.child(
            &IconButton::new(IconName::Clock)
                .label(labels.pick_time.clone())
                .disabled(!accepts)
                .on_click(move || toggle.update(|o| *o = !*o)),
        );
        let node = cx.container(
            LayoutStyle {
                spacing: 4.0,
                align: Align::Center,
                align_self: Some(Align::Fit),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[field, button],
            Paint::None,
        );
        if open.get() && accepts {
            let slots: Vec<Time> = (0..24 * 60)
                .step_by(self.step as usize)
                .filter_map(|m: u16| Time::new((m / 60) as u8, (m % 60) as u8))
                .collect();
            let options: Vec<String> = slots.iter().map(Time::to_string).collect();
            let chosen = self.value.clone();
            let near = self.value.get().map_or(0, |t| slots.iter().rposition(|s| *s <= t).unwrap_or(0));
            let (clear, closing) = (draft, open.clone());
            let selected = Binding::new(
                move || near,
                move |i: usize| {
                    clear.set(None);
                    chosen.set(slots.get(i).copied());
                },
            );
            let close: Rc<dyn Fn()> = Rc::new(move || closing.set(false));
            let list = Options { options: &options, selected, close: close.clone(), width: self.width };
            cx.overlay(node, Placement::Below, &list, Some(close));
        }
        node
    }
}

/// A [`DatePicker`] beside a [`TimePicker`], over one value. Picking a date first
/// sets the time to midnight, and clearing the time clears both.
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct DateTimePicker {
    value: Binding<Option<DateTime>>,
    month: Date,
    step: u16,
    label: Option<String>,
    state: util::EditorState,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DateTimePicker {
    /// `month` is shown when no date is selected yet.
    pub fn new(value: Binding<Option<DateTime>>, month: Date) -> Self {
        Self { value, month, step: 30, label: None, state: util::EditorState::default() }
    }

    /// The name screen readers announce. Both of its fields use it.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The minutes between the times in the list. 30 by default.
    pub fn step_minutes(mut self, minutes: u16) -> Self {
        self.step = minutes.clamp(1, 720);
        self
    }

    /// Turns the picker off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the value without letting it change; it can still be focused and read.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

impl View for DateTimePicker {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let date = self.value.map(
            |v| v.map(|v| v.date),
            |v, date: Option<Date>| {
                *v = date.map(|date| DateTime { date, time: v.map_or_else(Time::default, |old| old.time) });
            },
        );
        let time = self.value.map(
            |v| v.map(|v| v.time),
            |v, time: Option<Time>| {
                *v = match (*v, time) {
                    (Some(old), Some(time)) => Some(DateTime { date: old.date, time }),
                    _ => None,
                };
            },
        );
        let mut day = DatePicker::new(date, self.month)
            .fill()
            .disabled(self.state.disabled)
            .read_only(self.state.read_only);
        let mut hour = TimePicker::new(time)
            .step_minutes(self.step)
            .disabled(self.state.disabled)
            .read_only(self.state.read_only);
        if let Some(label) = &self.label {
            day = day.label(label.clone());
            hour = hour.label(label.clone());
        }
        let (day, hour) =
            (cx.child(&VStack::new((day,)).spacing(0.0).align(Align::Stretch).grow(1.0)), cx.child(&hour));
        cx.container(
            LayoutStyle {
                spacing: 8.0,
                align: Align::Center,
                align_self: Some(Align::Stretch),
                wrap: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[day, hour],
            Paint::None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_char(s: &str) -> f32 {
        s.chars().count() as f32 * 10.0
    }

    fn pieces(text: &str, width: f32) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        wrap_lines(text, width, by_char).iter().map(|&(a, b)| chars[a..b].iter().collect()).collect()
    }

    #[test]
    fn lines_break_at_line_breaks_and_after_spaces() {
        assert_eq!(pieces("", 100.0), [""]);
        assert_eq!(pieces("one\ntwo", 100.0), ["one", "two"]);
        assert_eq!(pieces("one\n", 100.0), ["one", ""]);
        assert_eq!(pieces("\n\n", 100.0), ["", "", ""]);
        assert_eq!(pieces("aaaa bbbb cccc", 100.0), ["aaaa bbbb ", "cccc"]);
    }

    #[test]
    fn a_word_wider_than_the_line_is_cut() {
        assert_eq!(pieces("abcdefghijkl", 50.0), ["abcde", "fghij", "kl"]);
        assert_eq!(pieces("hi abcdefghijkl yo", 50.0), ["hi ", "abcde", "fghij", "kl yo"]);
    }

    #[test]
    fn lines_cover_every_char_but_the_breaks() {
        let text = "the quick brown fox\njumps over\n\nthe lazy dog";
        let lines = wrap_lines(text, 70.0, by_char);
        let count: usize = lines.iter().map(|(a, b)| b - a).sum();
        assert_eq!(count + text.matches('\n').count(), text.chars().count());
        assert!(lines.windows(2).all(|w| w[0].1 <= w[1].0));
    }

    #[test]
    fn numbers_in_both_ways_of_writing_them() {
        let pt = |s| parse_number(s, ',', '.');
        assert_eq!(pt("1.234,50"), Some(1234.5));
        assert_eq!(pt("12,5"), Some(12.5));
        assert_eq!(pt("12.5"), Some(12.5), "a dot that cannot be thousands is the decimal");
        assert_eq!(pt("1.234"), Some(1234.0), "three digits after it are thousands");
        assert_eq!(pt("-3"), Some(-3.0));
        assert_eq!(pt(" 7 "), Some(7.0));
        for bad in ["", "abc", "1-2", "1,2,3", "--1", "1e5"] {
            assert_eq!(pt(bad), None, "{bad}");
        }
        let en = |s| parse_number(s, '.', ',');
        assert_eq!(en("1,234.50"), Some(1234.5));
        assert_eq!(en("12,5"), Some(12.5));
    }

    #[test]
    fn masks_fill_and_extract() {
        let phone = parse_mask("(00) 00000-0000");
        assert_eq!(mask_fill(&phone, "11987654321"), "(11) 98765-4321");
        assert_eq!(mask_fill(&phone, "119"), "(11) 9");
        assert_eq!(mask_fill(&phone, ""), "");
        assert_eq!(mask_extract(&phone, "(11) 98765-4321"), "11987654321");
        assert_eq!(mask_extract(&phone, "11x9"), "119", "what a slot refuses is skipped");
        assert_eq!(mask_extract(&phone, "(11) 98765-43219999"), "11987654321", "no more than it holds");
        let plate = parse_mask("aaa-0*00");
        assert_eq!(mask_extract(&plate, "ABC1D23"), "ABC1D23");
        assert_eq!(mask_fill(&plate, "ABC1D23"), "ABC-1D23");
        assert_eq!(parse_mask(r"\0-0"), [MaskSlot::Literal('0'), MaskSlot::Literal('-'), MaskSlot::Digit]);
    }

    #[test]
    fn four_digits_make_a_time() {
        assert_eq!(time_from_digits("0930"), Time::new(9, 30));
        assert_eq!(time_from_digits("093"), None);
        assert_eq!(time_from_digits("2460"), None);
    }
}
