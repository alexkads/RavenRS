//! What changed in each version of an app, and the command block it may show.

use ravenrs_layout::Align;
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId};
use crate::{
    Badge, Button, ButtonSize, ButtonStyle, HStack, IconName, List, ListItem, ScrollView, Spacer, Text, Tone,
    VStack, View, ViewExt,
};

/// A [`ReleaseNotes`] drops its list of versions below this window width.
const LIST_BREAKPOINT: f32 = 640.0;
const LIST_WIDTH: f32 = 220.0;

/// A line of text to copy, such as a command: shown apart from the prose, with a
/// button that puts it on the clipboard.
#[derive(Clone, Debug)]
pub struct CodeBlock {
    code: String,
}

impl CodeBlock {
    /// A block showing `code`.
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }
}

impl View for CodeBlock {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let copied = cx.use_state(|| false);
        let clipboard = cx.clipboard();
        let code = self.code.clone();
        let done = copied.clone();
        let (icon, label) = if copied.get() {
            (IconName::Check, labels.copied.clone())
        } else {
            (IconName::Copy, labels.copy.clone())
        };
        let radius = cx.theme().radii.md;
        let padding = cx.theme().spacing.s2;
        cx.child(
            &HStack::new((
                Text::new(self.code.clone()).style(TextStyle::Caption).grow(1.0),
                Button::new(label).icon(icon).style(ButtonStyle::Ghost).size(ButtonSize::Mini).on_click(
                    move || {
                        clipboard.write(&code);
                        done.set(true);
                    },
                ),
            ))
            .spacing(padding)
            .align(Align::Center)
            .padding(padding)
            .background(Semantic::Control)
            .corner_radius(radius),
        )
    }
}

#[derive(Clone, Debug)]
enum Block {
    Section(String, String),
    Code(String),
}

/// What one version brought, for a [`ReleaseNotes`].
#[derive(Clone, Debug)]
pub struct ReleaseNote {
    version: String,
    date: Option<String>,
    title: String,
    changes: Vec<String>,
    blocks: Vec<Block>,
    badge: Option<(String, Tone)>,
}

impl ReleaseNote {
    /// A note on `version`, as the app words it, with the title of the release.
    pub fn new(version: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            date: None,
            title: title.into(),
            changes: Vec::new(),
            blocks: Vec::new(),
            badge: None,
        }
    }

    /// The release date, already formatted.
    pub fn date(mut self, date: impl Into<String>) -> Self {
        self.date = Some(date.into());
        self
    }

    /// The list of what changed.
    pub fn changes(mut self, changes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.changes = changes.into_iter().map(Into::into).collect();
        self
    }

    /// Adds a block of text with a heading, after what is already there.
    pub fn section(mut self, heading: impl Into<String>, text: impl Into<String>) -> Self {
        self.blocks.push(Block::Section(heading.into(), text.into()));
        self
    }

    /// Adds a [`CodeBlock`], after what is already there.
    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.blocks.push(Block::Code(code.into()));
        self
    }

    /// Marks the version, such as "Installed" or "New".
    pub fn badge(mut self, label: impl Into<String>, tone: Tone) -> Self {
        self.badge = Some((label.into(), tone));
        self
    }

    /// The details on the right of the list.
    fn details(&self) -> impl View + use<> {
        let date = self.date.clone().map(|date| format!("{} · {date}", self.version));
        let changes: Vec<Text> = self
            .changes
            .iter()
            .map(|change| Text::new(format!("• {change}")).style(TextStyle::Callout))
            .collect();
        let blocks: Vec<crate::AnyView> = self
            .blocks
            .iter()
            .map(|block| match block {
                Block::Section(heading, text) => Box::new(
                    VStack::new((
                        Text::new(heading.clone()).style(TextStyle::Callout).font_weight(Weight::SEMIBOLD),
                        Text::new(text.clone()).style(TextStyle::Callout).foreground(Semantic::InkMuted),
                    ))
                    .spacing(4.0),
                ) as crate::AnyView,
                Block::Code(code) => Box::new(CodeBlock::new(code.clone())) as crate::AnyView,
            })
            .collect();
        VStack::new((
            HStack::new((
                Text::new(date.unwrap_or_else(|| self.version.clone()))
                    .style(TextStyle::Caption)
                    .foreground(Semantic::InkMuted),
                self.badge.clone().map(|(label, tone)| Badge::new(label).tone(tone)),
            ))
            .spacing(8.0)
            .align(Align::Center),
            Text::new(self.title.clone()).style(TextStyle::Headline),
            VStack::new(changes).spacing(6.0),
            VStack::new(blocks).spacing(14.0).align(Align::Stretch),
        ))
        .spacing(14.0)
        .align(Align::Stretch)
    }
}

/// An app's versions, newest first: the list on the left, what the chosen one
/// brought on the right, and buttons that walk from one to the next.
///
/// It shows whichever version `selected` points at, so the app opens it on the
/// one it wants to talk about. It goes inside a [`Dialog`](crate::Dialog), or
/// anywhere else with room for its height. In a narrow window the list goes away
/// and the buttons remain.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{ReleaseNote, ReleaseNotes, Tone};
///
/// let selected = State::new(0);
/// let notes = ReleaseNotes::new(
///     vec![
///         ReleaseNote::new("Version 1.3", "Faster start")
///             .date("Nov 1, 2026")
///             .badge("New", Tone::Success)
///             .changes(["Opens in half the time"])
///             .section("Why update", "Large files no longer stall the window."),
///         ReleaseNote::new("Version 1.2", "Dark theme").badge("Installed", Tone::Neutral),
///     ],
///     selected.binding(),
/// );
/// ```
#[derive(Debug)]
pub struct ReleaseNotes {
    notes: Vec<ReleaseNote>,
    selected: Binding<usize>,
    height: f32,
}

impl ReleaseNotes {
    /// The versions in `notes`, newest first, on the one at `selected`.
    pub fn new(notes: Vec<ReleaseNote>, selected: Binding<usize>) -> Self {
        Self { notes, selected, height: 460.0 }
    }

    /// The height, in logical pixels (460 by default). Longer notes scroll.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
}

impl View for ReleaseNotes {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let last = self.notes.len().saturating_sub(1);
        let at = self.selected.get().min(last);
        let Some(note) = self.notes.get(at) else {
            return cx.empty();
        };
        let labels = cx.labels();
        let list = (cx.viewport().width >= LIST_BREAKPOINT).then(|| {
            let rows: Vec<ListItem> = self
                .notes
                .iter()
                .enumerate()
                .map(|(i, note)| {
                    let line = [note.badge.as_ref().map(|(label, _)| label.clone()), note.date.clone()];
                    let line = line.into_iter().flatten().collect::<Vec<_>>().join(" · ");
                    let selected = self.selected.clone();
                    let row = ListItem::new(note.version.clone())
                        .selected(i == at)
                        .on_click(move || selected.set(i));
                    if line.is_empty() { row } else { row.subtitle(line) }
                })
                .collect();
            ScrollView::new((List::new(rows),)).frame(Some(LIST_WIDTH), None)
        });
        let walk = |to: Option<usize>| {
            let selected = self.selected.clone();
            move || {
                if let Some(to) = to {
                    selected.set(to);
                }
            }
        };
        let (newer, older) = (at.checked_sub(1), (at < last).then_some(at + 1));
        let body = VStack::new((
            ScrollView::new((note.details(),)),
            HStack::new((
                Button::new(labels.release_newer.clone())
                    .icon(IconName::ChevronLeft)
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .disabled(newer.is_none())
                    .on_click(walk(newer)),
                Button::new(labels.release_older.clone())
                    .icon(IconName::ChevronRight)
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .disabled(older.is_none())
                    .on_click(walk(older)),
                Spacer::new(),
                Text::new(format!("{} {} {}", at + 1, labels.release_of, self.notes.len()))
                    .style(TextStyle::Caption)
                    .foreground(Semantic::InkMuted),
            ))
            .spacing(4.0)
            .align(Align::Center),
        ))
        .spacing(10.0)
        .align(Align::Stretch)
        .grow(1.0);
        cx.child(&HStack::new((list, body)).spacing(16.0).align(Align::Stretch).height(self.height))
    }
}
