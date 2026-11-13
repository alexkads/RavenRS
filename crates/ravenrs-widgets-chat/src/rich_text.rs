//! Bold, italic, code and links in plain text, with the marks of chat apps.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Color, CursorIcon, Insets, Quad, Rect, Role, Scene};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle, Weight};
use ravenrs_text::TextAttrs;

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{Fill, Text, View};

/// How a piece of a line is marked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Marks {
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Marks {
    fn with(self, mark: char) -> Self {
        match mark {
            '*' => Self { bold: true, ..self },
            '_' => Self { italic: true, ..self },
            '~' => Self { strike: true, ..self },
            _ => Self { code: true, ..self },
        }
    }
}

/// A run of text with one look.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Span {
    text: String,
    marks: Marks,
    link: bool,
}

/// What a line starts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineKind {
    Plain,
    Item,
    Quote,
}

/// What a text is made of, from top to bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Block {
    Line(LineKind, Vec<Span>),
    Code(String),
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Block {
    fn is_plain(&self) -> bool {
        match self {
            Self::Line(LineKind::Plain, spans) => {
                spans.iter().all(|span| span.marks == Marks::default() && !span.link)
            }
            _ => false,
        }
    }
}

const FENCE: &str = "```";

fn parse(text: &str, marks: bool) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut rest = text;
    if marks {
        // Fences come first: they cross lines, and nothing inside them is a mark.
        while let Some(open) = rest.find(FENCE) {
            let inside = &rest[open + FENCE.len()..];
            let Some(close) = inside.find(FENCE) else { break };
            let code = inside[..close].trim_matches('\n');
            if code.trim().is_empty() {
                // Six backticks around nothing are six backticks.
                let end = open + FENCE.len() * 2 + close;
                parse_lines(&rest[..end], marks, &mut blocks);
                rest = &rest[end..];
                continue;
            }
            parse_lines(rest[..open].strip_suffix('\n').unwrap_or(&rest[..open]), marks, &mut blocks);
            blocks.push(Block::Code(code.to_owned()));
            let after = &inside[close + FENCE.len()..];
            rest = after.strip_prefix('\n').unwrap_or(after);
            if rest.is_empty() {
                return blocks;
            }
        }
    }
    parse_lines(rest, marks, &mut blocks);
    blocks
}

fn parse_lines(text: &str, marks: bool, blocks: &mut Vec<Block>) {
    if text.is_empty() {
        return;
    }
    for line in text.split('\n') {
        let filled = |body: &&str| !body.trim().is_empty();
        let (kind, body) = if !marks {
            (LineKind::Plain, line)
        } else if let Some(body) = line.strip_prefix("* ").or_else(|| line.strip_prefix("- ")).filter(filled)
        {
            (LineKind::Item, body)
        } else if let Some(body) = line.strip_prefix("> ").filter(filled) {
            (LineKind::Quote, body)
        } else {
            (LineKind::Plain, line)
        };
        let mut spans = Vec::new();
        parse_inline(body, Marks::default(), marks, &mut spans);
        blocks.push(Block::Line(kind, spans));
    }
}

/// The length of the link starting `text`, if one does. Punctuation that ends a sentence is
/// left out, or "see https://x/y." would lead to a page that does not exist. Only web
/// addresses count, so that a message can never hand the app another kind of address.
fn link_length(text: &str) -> Option<usize> {
    let scheme = ["https://", "http://"].into_iter().find(|scheme| text.starts_with(scheme))?;
    let end =
        text.find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'')).unwrap_or(text.len());
    let link = text[..end].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}']);
    (link.len() > scheme.len()).then_some(link.len())
}

/// A mark opens after the start, a space or punctuation, right against what it marks.
fn opens(text: &str, at: usize, mark: char) -> bool {
    let before = text[..at].chars().next_back();
    let after = text[at + mark.len_utf8()..].chars().next();
    before.is_none_or(|c| !c.is_alphanumeric() && c != mark)
        && after.is_some_and(|c| !c.is_whitespace() && c != mark)
}

/// Where the mark opened at `at` closes: against the text, before the end, a space or
/// punctuation. A code mark closes at the next backtick, whatever is around it.
fn closes(text: &str, at: usize, mark: char) -> Option<usize> {
    let start = at + mark.len_utf8();
    text[start..].match_indices(mark).map(|(i, _)| start + i).find(|&i| {
        if i == start {
            return false;
        }
        if mark == '`' {
            return true;
        }
        let before = text[..i].chars().next_back();
        let after = text[i + mark.len_utf8()..].chars().next();
        before.is_some_and(|c| !c.is_whitespace()) && after.is_none_or(|c| !c.is_alphanumeric())
    })
}

fn push_span(spans: &mut Vec<Span>, text: &str, marks: Marks, link: bool) {
    if !text.is_empty() {
        spans.push(Span { text: text.to_owned(), marks, link });
    }
}

fn parse_inline(text: &str, base: Marks, marks: bool, spans: &mut Vec<Span>) {
    let (mut pending, mut at) = (0, 0);
    while let Some(c) = text[at..].chars().next() {
        // The link first and whole: a mark inside an address is not a mark.
        if c == 'h'
            && let Some(length) = link_length(&text[at..])
        {
            push_span(spans, &text[pending..at], base, false);
            push_span(spans, &text[at..at + length], base, true);
            at += length;
            pending = at;
            continue;
        }
        if marks
            && matches!(c, '*' | '_' | '~' | '`')
            && opens(text, at, c)
            && let Some(close) = closes(text, at, c)
        {
            push_span(spans, &text[pending..at], base, false);
            let inside = &text[at + 1..close];
            if c == '`' {
                push_span(spans, inside, base.with(c), false);
            } else {
                parse_inline(inside, base.with(c), true, spans);
            }
            at = close + 1;
            pending = at;
            continue;
        }
        at += c.len_utf8();
    }
    push_span(spans, &text[pending..], base, false);
}

/// The text a reader takes in, marks gone and one line per line.
fn read_aloud(blocks: &[Block]) -> String {
    let lines: Vec<String> = blocks
        .iter()
        .map(|block| match block {
            Block::Line(_, spans) => spans.iter().map(|span| span.text.as_str()).collect(),
            Block::Code(code) => code.clone(),
        })
        .collect();
    lines.join("\n")
}

/// Text with a little formatting, written the way chat apps write it: `*bold*`, `_italic_`,
/// `~struck~`, `` `code` ``, a fenced block of ```` ``` ````, lines that start with `* `, `- `
/// or `> `, and web addresses, which become links.
///
/// It wraps to the width it is offered, between words, and a marked piece stays with the
/// punctuation around it. Text with nothing to format is drawn as one [`Text`], for the same
/// cost. Links take the keyboard: Tab reaches them, and Enter or Space opens them. It only
/// recognizes `http` and `https`, and it hands the address to [`Self::on_link`] instead of
/// opening anything.
///
/// `ravenrs-text` shapes one style per text and embeds neither an italic nor a monospace
/// font, so italics are told apart by a muted color and code by a chip behind it.
///
/// ```
/// use ravenrs_widgets::{RichText, Ui};
///
/// let text = RichText::new("*Ready:* see `report.pdf` at https://example.com/r")
///     .on_link(|address| println!("open {address}"));
/// let mut ui = Ui::new(text, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct RichText {
    text: String,
    marks: bool,
    style: TextStyle,
    foreground: Fill,
    on_link: Option<Rc<dyn Fn(String)>>,
}

impl fmt::Debug for RichText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RichText")
            .field("text", &self.text)
            .field("style", &self.style)
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl RichText {
    /// Text that may have marks in it.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            marks: true,
            style: TextStyle::Body,
            foreground: Semantic::Ink.into(),
            on_link: None,
        }
    }

    /// Takes every mark literally, and only finds the links: for text from somewhere that
    /// does not use these marks.
    #[must_use]
    pub fn links_only(mut self) -> Self {
        self.marks = false;
        self
    }

    /// A step of the type scale (`Body` by default).
    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    /// Text color, as a theme role (`Ink` by default) or a literal color.
    #[must_use]
    pub fn foreground(mut self, fill: impl Into<Fill>) -> Self {
        self.foreground = fill.into();
        self
    }

    /// Called with the address of a link when it is clicked, or pressed with Enter or Space.
    /// Without it, links look like links but cannot be activated or focused.
    #[must_use]
    pub fn on_link(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_link = Some(Rc::new(handler));
        self
    }

    /// The text without its marks, on one line: for a preview, such as in a list of
    /// conversations.
    pub fn plain_text(text: &str) -> String {
        read_aloud(&parse(text, true)).split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

/// One piece of a word: words change look in the middle, as in `(*this*),`.
struct Piece {
    text: String,
    marks: Marks,
    link: bool,
    /// The strike goes on over the space after it.
    strike_on: bool,
}

/// Cuts a line at the places it can wrap. A chip of code or a link stays in one piece, unless
/// it is too wide for a whole line, when its own text wraps.
fn words(spans: &[Span]) -> Vec<Vec<Piece>> {
    let mut words: Vec<Vec<Piece>> = Vec::new();
    let mut glued = false;
    for span in spans {
        let piece = |text: &str, strike_on| Piece {
            text: text.to_owned(),
            marks: span.marks,
            link: span.link,
            strike_on,
        };
        if span.link || span.marks.code {
            if glued && let Some(word) = words.last_mut() {
                word.push(piece(&span.text, false));
            } else {
                words.push(vec![piece(&span.text, false)]);
            }
            glued = true;
            continue;
        }
        let parts: Vec<&str> = span.text.split_whitespace().collect();
        if parts.is_empty() {
            glued = false;
            continue;
        }
        let spaced_before = span.text.starts_with(char::is_whitespace);
        for (i, part) in parts.iter().enumerate() {
            let strike_on = span.marks.strike && i + 1 < parts.len();
            if i == 0
                && glued
                && !spaced_before
                && let Some(word) = words.last_mut()
            {
                word.push(piece(part, strike_on));
            } else {
                words.push(vec![piece(part, strike_on)]);
            }
        }
        glued = !span.text.ends_with(char::is_whitespace);
    }
    words
}

/// The line drawn over or under a word, behind its glyphs.
fn rule(frame: Rect, y: f32, color: Color, scene: &mut Scene) {
    scene.push_quad(Quad::new(Rect::new(frame.x(), frame.y() + y, frame.width(), 1.0), color));
}

/// A link inside a line.
struct LinkPiece {
    text: String,
    attrs: TextAttrs,
    on_link: Option<Rc<dyn Fn(String)>>,
}

impl View for LinkPiece {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let active = self.on_link.is_some();
        let role = if !active {
            Semantic::Accent
        } else if cx.is_pressed() {
            Semantic::AccentPressed
        } else if cx.is_hovered() {
            Semantic::AccentHover
        } else {
            Semantic::Accent
        };
        // The accent is made to fill things. Drawn thin on any surface it needs some ink,
        // which takes it the right way in either kind of theme, to stay readable.
        let color = theme.color(role).lerp(theme.color(Semantic::Ink), 0.3);
        let text = cx.text(&self.text, &self.attrs, color, LayoutStyle::leaf());
        cx.semantics(text, Semantics::hidden());
        let (words, attrs) = (self.text.clone(), self.attrs.clone());
        let node = cx.container(
            LayoutStyle { min_width: Some(0.0), ..LayoutStyle::stack(Axis::Horizontal) },
            &[text],
            // Under each row, as wide as the row: an address too long for a line wraps by itself.
            Paint::Rich(Rc::new(move |frame, paint| {
                let mut layout = paint.text.layout(&words, &attrs);
                layout.measure(Some(frame.width() + 0.5));
                for line in layout.lines() {
                    let row = Rect::new(frame.x(), frame.y() + line.top, line.width, line.height);
                    rule(row, line.height - 3.0, color, paint.scene);
                }
            })),
        );
        if let Some(on_link) = &self.on_link {
            let (on_link, address) = (on_link.clone(), self.text.clone());
            cx.on_click(node, Rc::new(move || on_link(address.clone())));
            cx.focusable(node, theme.radii.sm);
            cx.cursor(node, CursorIcon::Pointer);
            cx.semantics(node, Semantics::new(Role::Link).name(self.text.clone()));
        }
        node
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl RichText {
    /// One word, made of one or more pieces, with the space that follows it.
    fn word(
        &self,
        cx: &mut BuildCx<'_>,
        pieces: &[Piece],
        ink: Color,
        space: f32,
        next_link: &mut u64,
    ) -> NodeId {
        let theme = *cx.theme();
        let base = crate::util::attrs(&theme, self.style, None);
        let mut children = Vec::with_capacity(pieces.len());
        let mut trailing = space;
        for piece in pieces {
            let mut attrs = base.clone();
            if piece.marks.bold {
                attrs.weight = Weight::SEMIBOLD.0;
            }
            if piece.link {
                let key = *next_link;
                *next_link += 1;
                children.push(cx.keyed_child(
                    key,
                    &LinkPiece { text: piece.text.clone(), attrs, on_link: self.on_link.clone() },
                ));
                continue;
            }
            let color = if piece.marks.italic { theme.color(Semantic::InkMuted) } else { ink };
            let text = cx.text(&piece.text, &attrs, color, LayoutStyle::leaf());
            cx.semantics(text, Semantics::hidden());
            let node = if piece.marks.code {
                cx.container(
                    LayoutStyle {
                        padding: Insets::symmetric(0.0, theme.spacing.s1),
                        min_width: Some(0.0),
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[text],
                    Paint::Quad(QuadPaint {
                        corner_radius: theme.radii.sm,
                        ..QuadPaint::new(theme.color(Semantic::Border))
                    }),
                )
            } else if piece.marks.strike {
                let middle = base.line_height / 2.0;
                let reach = if piece.strike_on { space } else { 0.0 };
                if piece.strike_on {
                    trailing = 0.0;
                }
                cx.container(
                    LayoutStyle {
                        padding: Insets { right: reach, ..Insets::ZERO },
                        min_width: Some(0.0),
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[text],
                    Paint::Custom(Rc::new(move |frame, scene| rule(frame, middle, color, scene))),
                )
            } else {
                text
            };
            children.push(node);
        }
        cx.container(
            LayoutStyle {
                padding: Insets { right: trailing, ..Insets::ZERO },
                min_width: Some(0.0),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::None,
        )
    }

    /// The words of a line, wrapping in whatever width it gets.
    fn flow(&self, cx: &mut BuildCx<'_>, spans: &[Span], ink: Color, next_link: &mut u64) -> NodeId {
        let attrs = crate::util::attrs(cx.theme(), self.style, None);
        // Measured between letters, since a space at the end of a text may not count.
        let one = cx.measure_text("n", &attrs).width;
        let space = cx.measure_text("n n", &attrs).width - one * 2.0;
        let words = words(spans);
        let children: Vec<NodeId> =
            words.iter().map(|pieces| self.word(cx, pieces, ink, space, next_link)).collect();
        cx.container(
            LayoutStyle {
                wrap: true,
                grow: 1.0,
                min_width: Some(0.0),
                // An empty line still takes its height, as it does in a text.
                min_height: Some(attrs.line_height),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::None,
        )
    }
}

impl View for RichText {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let blocks = parse(&self.text, self.marks);
        if blocks.iter().all(Block::is_plain) {
            return cx.child(&Text::new(self.text.clone()).style(self.style).foreground(self.foreground));
        }
        let theme = *cx.theme();
        let ink = self.foreground.resolve(&theme);
        let attrs = crate::util::attrs(&theme, self.style, None);
        let mut next_link = 0;
        let mut children = Vec::with_capacity(blocks.len());
        for block in &blocks {
            let node = match block {
                Block::Line(LineKind::Plain, spans) => self.flow(cx, spans, ink, &mut next_link),
                Block::Line(LineKind::Item, spans) => {
                    let bullet = cx.text("•", &attrs, ink, LayoutStyle::leaf());
                    cx.semantics(bullet, Semantics::hidden());
                    let line = self.flow(cx, spans, ink, &mut next_link);
                    cx.container(
                        LayoutStyle {
                            spacing: theme.spacing.s2,
                            padding: Insets { left: theme.spacing.s1, ..Insets::ZERO },
                            ..LayoutStyle::stack(Axis::Horizontal)
                        },
                        &[bullet, line],
                        Paint::None,
                    )
                }
                Block::Line(LineKind::Quote, spans) => {
                    let bar = cx.container(
                        LayoutStyle {
                            width: Some(2.0),
                            align_self: Some(Align::Stretch),
                            ..LayoutStyle::leaf()
                        },
                        &[],
                        Paint::Quad(QuadPaint {
                            corner_radius: 1.0,
                            ..QuadPaint::new(theme.color(Semantic::Border))
                        }),
                    );
                    let line = self.flow(cx, spans, theme.color(Semantic::InkMuted), &mut next_link);
                    cx.container(
                        LayoutStyle { spacing: theme.spacing.s2, ..LayoutStyle::stack(Axis::Horizontal) },
                        &[bar, line],
                        Paint::None,
                    )
                }
                Block::Code(code) => {
                    let text = cx.text(code, &attrs, ink, LayoutStyle::leaf());
                    cx.semantics(text, Semantics::hidden());
                    cx.container(
                        LayoutStyle {
                            padding: Insets::symmetric(theme.spacing.s2, theme.spacing.s3),
                            align_self: Some(Align::Stretch),
                            ..LayoutStyle::stack(Axis::Vertical)
                        },
                        &[text],
                        Paint::Quad(QuadPaint {
                            corner_radius: theme.radii.md,
                            ..QuadPaint::new(theme.color(Semantic::Border))
                        }),
                    )
                }
            };
            children.push(node);
        }
        let node = cx.container(
            LayoutStyle { align: Align::Stretch, min_width: Some(0.0), ..LayoutStyle::stack(Axis::Vertical) },
            &children,
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::Label).name(read_aloud(&blocks)));
        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str) -> Vec<(String, Marks, bool)> {
        let blocks = parse(text, true);
        let Block::Line(_, spans) = &blocks[0] else { panic!("a line") };
        spans.iter().map(|s| (s.text.clone(), s.marks, s.link)).collect()
    }

    fn marked(text: &str, marks: Marks) -> (String, Marks, bool) {
        (text.to_owned(), marks, false)
    }

    const BOLD: Marks = Marks { bold: true, italic: false, strike: false, code: false };

    #[test]
    fn marks_nest_and_vanish_from_the_text() {
        let both = Marks { italic: true, ..BOLD };
        assert_eq!(line("*a _b_ c*"), [marked("a ", BOLD), marked("b", both), marked(" c", BOLD)]);
        assert_eq!(RichText::plain_text("*a* _b_\n~c~ `d`"), "a b c d");
    }

    #[test]
    fn a_mark_must_touch_what_it_marks_and_not_sit_inside_a_word() {
        for literal in ["2 * 3 * 4", "snake_case_name", "a*b*c", "*not closed", "`` no"] {
            let blocks = parse(literal, true);
            assert!(blocks.iter().all(Block::is_plain), "{literal}");
            assert_eq!(read_aloud(&blocks), literal);
        }
        assert_eq!(
            line("(*yes*),"),
            [marked("(", Marks::default()), marked("yes", BOLD), marked("),", Marks::default())]
        );
    }

    #[test]
    fn code_keeps_marks_literal_and_a_fence_crosses_lines() {
        let code = Marks { code: true, ..Marks::default() };
        assert_eq!(line("`*no*`"), [marked("*no*", code)]);
        let blocks = parse("before\n```\nlet *a* = 1;\nb\n```\nafter", true);
        assert_eq!(blocks[1], Block::Code("let *a* = 1;\nb".into()));
        assert_eq!(blocks.len(), 3);
        assert_eq!(read_aloud(&parse("``````", true)), "``````");
    }

    #[test]
    fn links_are_whole_web_addresses_without_the_end_of_the_sentence() {
        assert_eq!(
            line("see https://x.io/a_b_c."),
            [
                marked("see ", Marks::default()),
                ("https://x.io/a_b_c".to_owned(), Marks::default(), true),
                marked(".", Marks::default()),
            ]
        );
        for not_a_link in ["javascript:alert(1)", "file:///etc/passwd", "https://", "http"] {
            assert!(parse(not_a_link, true).iter().all(Block::is_plain), "{not_a_link}");
        }
    }

    #[test]
    fn links_only_leaves_the_marks_alone() {
        let blocks = parse("*a* https://x.io\n* b", false);
        let Block::Line(LineKind::Plain, spans) = &blocks[0] else { panic!("a plain line") };
        assert_eq!(spans[0].text, "*a* ");
        assert!(spans[1].link);
        assert!(matches!(&blocks[1], Block::Line(LineKind::Plain, _)));
    }

    #[test]
    fn a_word_keeps_its_punctuation_and_a_chip_its_spaces() {
        let blocks = parse("(*a*), `b c` d", true);
        let Block::Line(_, spans) = &blocks[0] else { panic!("a line") };
        let words = words(spans);
        let read: Vec<Vec<&str>> =
            words.iter().map(|word| word.iter().map(|p| p.text.as_str()).collect()).collect();
        assert_eq!(read, [vec!["(", "a", "),"], vec!["b c"], vec!["d"]]);
    }
}
