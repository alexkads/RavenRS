//! A build log as it comes: [`LogBuffer`] keeps the lines, read once for their colors, and
//! [`LogView`] shows them in a virtualized list that follows the end, with search, sections that
//! fold, and a line that can be pointed at by its number.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{AccessAction, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, TextStyle};

use super::log_ansi::{Span, parse};
use super::virtual_list::{ScrollCommand, VirtualList};
use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::view::View;
use crate::{Button, ButtonSize, ButtonStyle, HStack, Icon, IconName, Spacer, Text, TextField, VStack, util};

/// The height of one line, in points.
const LINE: f32 = 20.0;

#[derive(Debug)]
struct Line {
    spans: Vec<Span>,
    plain: String,
    section: Option<usize>,
}

#[derive(Debug)]
struct Section {
    title: String,
    lines: usize,
}

#[derive(Debug, Default)]
struct Kept {
    lines: Vec<Line>,
    sections: Vec<Section>,
}

/// The lines of a log, kept as they come. Clones share them; a [`LogView`] over it shows each new
/// line at the next build.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct LogBuffer {
    kept: Rc<RefCell<Kept>>,
    changed: State<u64>,
}

impl fmt::Debug for LogBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kept = self.kept.borrow();
        f.debug_struct("LogBuffer")
            .field("lines", &kept.lines.len())
            .field("sections", &kept.sections.len())
            .finish()
    }
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl LogBuffer {
    /// No lines yet.
    pub fn new() -> Self {
        Self { kept: Rc::default(), changed: State::new(0) }
    }

    /// Adds `text`, one line for each line it holds. Colors written as ANSI codes are read now
    /// and every other escape is taken out.
    pub fn push(&self, text: &str) {
        {
            let mut kept = self.kept.borrow_mut();
            let section = kept.sections.len().checked_sub(1);
            for line in text.split('\n') {
                let (spans, plain) = parse(line.strip_suffix('\r').unwrap_or(line));
                kept.lines.push(Line { spans, plain, section });
                if let Some(index) = section {
                    kept.sections[index].lines += 1;
                }
            }
        }
        self.changed.update(|n| *n += 1);
    }

    /// Starts a section, as a stage of a build: the lines pushed after it belong to it, and it
    /// folds them away.
    pub fn section(&self, title: &str) {
        self.kept.borrow_mut().sections.push(Section { title: title.to_owned(), lines: 0 });
        self.changed.update(|n| *n += 1);
    }

    /// Forgets every line and section.
    pub fn clear(&self) {
        *self.kept.borrow_mut() = Kept::default();
        self.changed.update(|n| *n += 1);
    }

    /// How many lines it holds.
    pub fn len(&self) -> usize {
        self.kept.borrow().lines.len()
    }

    /// Whether it holds no line.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Line `number`, counted from 1, as text with the colors taken out.
    pub fn line(&self, number: usize) -> Option<String> {
        self.kept.borrow().lines.get(number.checked_sub(1)?).map(|line| line.plain.clone())
    }
}

/// What one row of the list shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    /// The title of a section, by its place.
    Section(usize),
    /// A line, by its place.
    Line(usize),
}

/// The rows in order: each section's title before its lines, and no lines of a folded section.
fn rows_of(kept: &Kept, folded: &BTreeSet<usize>) -> Vec<Row> {
    let mut rows = Vec::with_capacity(kept.lines.len() + kept.sections.len());
    let mut shown = None;
    for (index, line) in kept.lines.iter().enumerate() {
        if line.section != shown {
            if let Some(section) = line.section {
                rows.push(Row::Section(section));
            }
            shown = line.section;
        }
        if line.section.is_none_or(|section| !folded.contains(&section)) {
            rows.push(Row::Line(index));
        }
    }
    // A section with no lines yet still shows its title.
    for (index, section) in kept.sections.iter().enumerate() {
        if section.lines == 0 {
            rows.push(Row::Section(index));
        }
    }
    rows
}

fn matches(plain: &str, query: &str) -> bool {
    !query.is_empty() && plain.to_lowercase().contains(query)
}

/// A build log: a virtualized list of lines in a monospaced font, with the colors the program wrote,
/// that follows the end as lines come. Scrolling up stops following, and "Go to the end" resumes it.
/// A search marks the lines that hold it; a section folds its lines; the number of a line points
/// at it.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct LogView {
    buffer: LogBuffer,
    follow_end: bool,
    line_numbers: bool,
    searchable: bool,
    height: f32,
    label: Option<String>,
    on_line: Option<Rc<dyn Fn(usize)>>,
}

impl fmt::Debug for LogView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LogView")
            .field("buffer", &self.buffer)
            .field("follow_end", &self.follow_end)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl LogView {
    /// The lines of `buffer`, following the end, with numbers and a search field.
    pub fn new(buffer: LogBuffer) -> Self {
        Self {
            buffer,
            follow_end: true,
            line_numbers: true,
            searchable: true,
            height: 320.0,
            label: None,
            on_line: None,
        }
    }

    /// Whether it starts at the last line and stays there as lines come; on by default.
    pub fn follow_end(mut self, follow: bool) -> Self {
        self.follow_end = follow;
        self
    }

    /// Whether each line shows its number; on by default.
    pub fn line_numbers(mut self, shown: bool) -> Self {
        self.line_numbers = shown;
        self
    }

    /// Whether it has a search field above the lines; on by default.
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        self
    }

    /// How tall the list of lines is, in points.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// What screen readers call it; the labels' `log_name` by default.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Called with the number of a line (from 1) when someone points at it by its number, as to
    /// put it in an address.
    pub fn on_line(mut self, handler: impl Fn(usize) + 'static) -> Self {
        self.on_line = Some(Rc::new(handler));
        self
    }
}

/// One row: a section's title, or a line with its number.
struct LogRow {
    buffer: LogBuffer,
    row: Row,
    numbers: bool,
    query: String,
    folded: State<BTreeSet<usize>>,
    anchor: State<Option<usize>>,
    on_line: Option<Rc<dyn Fn(usize)>>,
    folded_text: String,
    point_text: String,
}

impl View for LogRow {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let mut attrs = util::attrs(&theme, TextStyle::Callout, None);
        attrs.family = Some("monospace".to_owned());
        let style = |grow: f32| LayoutStyle { grow, ..LayoutStyle::leaf() };
        let kept = self.buffer.kept.borrow();
        match self.row {
            Row::Section(index) => {
                let Some(section) = kept.sections.get(index) else { return cx.empty() };
                let folded = self.folded.get().contains(&index);
                let mut bold = attrs.clone();
                bold.weight = 600.0;
                // What it says is also its name: a screen reader hears how much a fold hides.
                let mut name = section.title.clone();
                if folded {
                    name.push_str(&format!(
                        " · {}",
                        self.folded_text.replace("{0}", &section.lines.to_string())
                    ));
                }
                // The mark is an icon: no embedded font draws ▸ and ▾, and they came out as an
                // empty box (#849).
                let chevron = if folded { IconName::ChevronRight } else { IconName::ChevronDown };
                let mark = cx.child(&Icon::new(chevron).size(14.0).color(theme.color(Semantic::InkMuted)));
                let text = cx.text(&name, &bold, theme.color(Semantic::Ink), style(1.0));
                // The section is one button to a screen reader: its title is not read twice.
                cx.semantics(text, Semantics { hidden: true, ..Semantics::new(Role::Label) });
                let node = cx.container(
                    LayoutStyle {
                        height: Some(LINE),
                        align: Align::Center,
                        padding: ravenrs_core::Insets::symmetric(0.0, 8.0),
                        spacing: 6.0,
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[mark, text],
                    Paint::Quad(QuadPaint::new(theme.color(Semantic::Control).with_alpha(0.6))),
                );
                let (toggle, access) = (self.folded.clone(), self.folded.clone());
                cx.on_click(
                    node,
                    Rc::new(move || {
                        toggle.update(|set| {
                            if !set.remove(&index) {
                                set.insert(index);
                            }
                        });
                    }),
                );
                let actions = [AccessAction::Click, AccessAction::Expand, AccessAction::Collapse];
                cx.on_access(node, &actions, move |action, _| {
                    access.update(|set| match action {
                        AccessAction::Expand => {
                            set.remove(&index);
                        }
                        AccessAction::Collapse => {
                            set.insert(index);
                        }
                        _ => {
                            if !set.remove(&index) {
                                set.insert(index);
                            }
                        }
                    });
                    true
                });
                cx.semantics(node, Semantics::new(Role::Button).name(name).expanded(!folded));
                cx.test_id(node, &format!("section:{}", index + 1));
                node
            }
            Row::Line(index) => {
                let Some(line) = kept.lines.get(index) else { return cx.empty() };
                let number = index + 1;
                let mut children = Vec::with_capacity(line.spans.len() + 1);
                if self.numbers {
                    let digits = cx.text(
                        &number.to_string(),
                        &attrs,
                        theme.color(Semantic::InkMuted),
                        LayoutStyle { min_width: Some(48.0), ..LayoutStyle::leaf() },
                    );
                    let (anchor, told) = (self.anchor.clone(), self.on_line.clone());
                    cx.on_click(
                        digits,
                        Rc::new(move || {
                            anchor.set(Some(number));
                            if let Some(told) = &told {
                                told(number);
                            }
                        }),
                    );
                    let name = self.point_text.replace("{0}", &number.to_string());
                    cx.semantics(digits, Semantics::new(Role::Button).name(name));
                    cx.test_id(digits, &format!("number:{number}"));
                    children.push(digits);
                }
                for span in &line.spans {
                    let mut look = attrs.clone();
                    if span.bold {
                        look.weight = 600.0;
                    }
                    let color = theme.color(span.color.unwrap_or(Semantic::Ink));
                    // A line never breaks, as in a terminal: a long one runs on, and the list
                    // scrolls across to it (#899).
                    let width = cx.measure_text(&span.text, &look).width;
                    children.push(cx.text(
                        &span.text,
                        &look,
                        color,
                        LayoutStyle { min_width: Some(width), ..LayoutStyle::leaf() },
                    ));
                }
                let fill = if self.anchor.get() == Some(number) {
                    Some(theme.color(Semantic::Accent).with_alpha(0.18))
                } else if matches(&line.plain, &self.query) {
                    Some(theme.color(Semantic::Warning).with_alpha(0.16))
                } else {
                    None
                };
                let paint = fill.map_or(Paint::None, |fill| Paint::Quad(QuadPaint::new(fill)));
                let node = cx.container(
                    LayoutStyle {
                        height: Some(LINE),
                        align: Align::Center,
                        padding: ravenrs_core::Insets::symmetric(0.0, 8.0),
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &children,
                    paint,
                );
                // The list already makes each row an item; its name comes from what the row says.
                cx.test_id(node, &format!("line:{number}"));
                node
            }
        }
    }
}

impl View for LogView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        // Reading it has this build run again when lines come.
        let _ = self.buffer.changed.get();
        let query = cx.use_state(String::new);
        let folded: State<BTreeSet<usize>> = cx.use_state(BTreeSet::new);
        let anchor: State<Option<usize>> = cx.use_state(|| None);
        let commands: State<Option<ScrollCommand>> = cx.use_state(|| None);
        let wanted = query.get().trim().to_lowercase();
        let (rows, found) = {
            let kept = self.buffer.kept.borrow();
            let rows = rows_of(&kept, &folded.get());
            let found = kept.lines.iter().filter(|line| matches(&line.plain, &wanted)).count();
            (Rc::new(rows), found)
        };
        let count = rows.len();
        // The widest line, with its number and the row's room, is how far the list scrolls across.
        let widest = {
            let theme = *cx.theme();
            let mut attrs = util::attrs(&theme, TextStyle::Callout, None);
            attrs.family = Some("monospace".to_owned());
            let kept = self.buffer.kept.borrow();
            let longest = kept.lines.iter().max_by_key(|line| line.plain.chars().count());
            let text = longest.map_or(0.0, |line| cx.measure_text(&line.plain, &attrs).width);
            text + if self.line_numbers { 48.0 } else { 0.0 } + 16.0
        };
        let item = {
            let (buffer, rows, folded, anchor) =
                (self.buffer.clone(), rows.clone(), folded.clone(), anchor.clone());
            let (numbers, query, on_line) = (self.line_numbers, wanted.clone(), self.on_line.clone());
            let (folded_text, point_text) = (labels.log_folded.clone(), labels.log_point_line.clone());
            move |at: usize| LogRow {
                buffer: buffer.clone(),
                row: rows.get(at).copied().unwrap_or(Row::Line(usize::MAX)),
                numbers,
                query: query.clone(),
                folded: folded.clone(),
                anchor: anchor.clone(),
                on_line: on_line.clone(),
                folded_text: folded_text.clone(),
                point_text: point_text.clone(),
            }
        };
        let list = VirtualList::new(count, item)
            .row_height(LINE)
            .follow_end(self.follow_end)
            .commands(commands.clone())
            .scroll_across(widest)
            .height(self.height)
            .label(self.label.clone().unwrap_or_else(|| labels.log_name.clone()));
        let to_end = Button::new(labels.log_go_to_end.clone())
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Small)
            .on_click(move || commands.set(Some(ScrollCommand::ToIndex(count.saturating_sub(1)))));
        let said = if wanted.is_empty() {
            String::new()
        } else {
            labels.log_matches.replace("{0}", &found.to_string())
        };
        let bar: Vec<crate::AnyView> = if self.searchable {
            vec![
                Box::new(TextField::new(query.binding()).placeholder(labels.log_search.clone()).width(260.0)),
                Box::new(Text::new(said).foreground(Semantic::InkMuted)),
                Box::new(Spacer::new()),
                Box::new(to_end),
            ]
        } else {
            vec![Box::new(Spacer::new()), Box::new(to_end)]
        };
        cx.child(&VStack::new((HStack::new(bar).spacing(8.0).align(Align::Center), list)).spacing(8.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer() -> LogBuffer {
        let log = LogBuffer::new();
        log.push("antes de tudo");
        log.section("Checkout");
        log.push("fetch\r\nHEAD\n");
        log.section("Testes");
        log
    }

    #[test]
    fn lines_split_and_carriage_returns_go() {
        let log = buffer();
        assert_eq!(log.len(), 4, "a trailing line end makes an empty line, as the program wrote it");
        assert_eq!(log.line(2).as_deref(), Some("fetch"));
        assert_eq!(log.line(3).as_deref(), Some("HEAD"));
        assert_eq!(log.line(0), None);
        assert_eq!(log.line(9), None);
    }

    #[test]
    fn rows_put_each_title_before_its_lines_and_folding_hides_them() {
        let log = buffer();
        let kept = log.kept.borrow();
        let open = rows_of(&kept, &BTreeSet::new());
        assert_eq!(
            open,
            vec![Row::Line(0), Row::Section(0), Row::Line(1), Row::Line(2), Row::Line(3), Row::Section(1)],
            "a section with no lines yet still shows its title"
        );
        let folded = rows_of(&kept, &BTreeSet::from([0]));
        assert_eq!(folded, vec![Row::Line(0), Row::Section(0), Row::Section(1)]);
    }

    #[test]
    fn search_ignores_case_and_an_empty_query_marks_nothing() {
        assert!(matches("docker FAIL cleans_up", "fail"));
        assert!(!matches("PASS", "fail"));
        assert!(!matches("qualquer coisa", ""));
    }

    #[test]
    fn clearing_forgets_lines_and_sections() {
        let log = buffer();
        log.clear();
        assert!(log.is_empty());
        assert!(rows_of(&log.kept.borrow(), &BTreeSet::new()).is_empty());
    }
}
