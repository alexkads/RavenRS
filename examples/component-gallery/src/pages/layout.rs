//! The layout and navigation page.

use ravenrs::prelude::*;

use crate::{Gallery, columns, demo, page_header, sample_demos, samples};

impl Gallery {
    pub(crate) fn layout_nav(&self, width: f32) -> impl View + use<> {
        let slides: Vec<AnyView> = (0..4)
            .map(|i| -> AnyView { Box::new(Image::new(self.images[i].clone()).stretch().height(240.0)) })
            .collect();
        VStack::new((
            page_header(
                "Layout & navigation",
                "Resizable panels, carousels, menus, toggles, pagination and loaders.",
            ),
            sample_demos(&[
                ("VirtualList", "A list of any length with rows of any height, which can jump to a row and stay at the end."),
                ("SortableList", "Rows put in order by dragging or with Alt and the arrows, with a line at the target."),
                ("ZStack", "Views on top of each other, in a box as large as the largest."),
                ("FloatingToolbar", "A translucent row of commands over its content, with a handle that moves it."),
                ("ScrollEdgeArrow", "A strip that scrolls, with an arrow at each end that has more to show."),
                ("TitleBar", "A title bar of the app's own: it draws and calls back, and does not move the window."),
                ("PropertyRow", "A label and its value at the two ends of a line."),
                ("PageHeader", "Title, description, badges and actions."),
            ]),
            demo(
                "Tiles",
                "A dashboard to rearrange: drag by the grip, resize by the corner, or use the arrows on the grip.",
                samples::sample("TileLayout"),
            ),
            demo(
                "Cards",
                "Ten thousand records as cards, in as many columns as fit. Only those in sight are built.",
                samples::sample("CardView"),
            ),
            demo(
                "Footer",
                "Left, center and right areas under a hairline. Below 640 pixels of window width they stack, as the second one always does.",
                VStack::new((
                    samples::sample("Footer"),
                    Footer::new()
                        .divider(false)
                        .stacked()
                        .left((Footer::text("© 2026 Raven Finance"),))
                        .center((Footer::link("Privacy"), Footer::link("Terms")))
                        .right((Footer::text("v27.1"),)),
                ))
                .align(Align::Stretch),
            ),
            demo(
                "StatusBar",
                "One fixed line at the foot of a window. The sides keep their width, the center gives way, and a notice can tint only the center.",
                VStack::new((
                    samples::sample("StatusBar"),
                    StatusBar::new()
                        .left((StatusBar::action("Version 27.1"),))
                        .center((
                            StatusBar::text("Version 27.2 is available").truncate().grow(1.0),
                            StatusBar::action("What's new"),
                            StatusBar::action("Update").style(ButtonStyle::Primary),
                        ))
                        .center_tone(Tone::Warning)
                        .right((StatusBar::text("12 items"),)),
                ))
                .align(Align::Stretch),
            ),
            demo(
                "Menubar",
                "Desktop-style menus.",
                Menubar::new()
                    .menu("File")
                    .item(MenuItem::new("New").icon(IconName::FilePlus).shortcut("⌘N"))
                    .item(MenuItem::new("Open…").icon(IconName::FolderOpen).shortcut("⌘O"))
                    .separator()
                    .item(MenuItem::new("Print").icon(IconName::Printer).shortcut("⌘P"))
                    .menu("Edit")
                    .item(MenuItem::new("Undo").icon(IconName::Undo2).shortcut("⌘Z"))
                    .item(MenuItem::new("Redo").icon(IconName::Redo2).shortcut("⇧⌘Z"))
                    .menu("View")
                    .item(MenuItem::new("Zoom in").icon(IconName::ZoomIn))
                    .item(MenuItem::new("Zoom out").icon(IconName::ZoomOut)),
            ),
            demo(
                "Splitter",
                "Drag the handle between the panes.",
                Splitter::horizontal(
                    VStack::new((
                        Text::new("Explorer").style(TextStyle::Headline),
                        Text::new("src/main.rs").foreground(Semantic::InkMuted),
                    ))
                    .padding(16.0),
                    VStack::new((
                        Text::new("Editor").style(TextStyle::Headline),
                        Text::new("fn main() { … }").foreground(Semantic::InkMuted),
                    ))
                    .padding(16.0),
                )
                .ratio(0.32)
                .height(200.0),
            ),
            sample_demos(&[
                ("SplitPanes", "Any number of panes, with limits, by mouse or keyboard."),
                ("SplitHandle", "The bar by itself, for a pane you lay out."),
                ("DockArea", "Panels in a tree of splits and tabs. Drag a tab to another group or a border, or use its menu; bars resize, docks collapse, and the arrangement is saved as text."),
            ]),
            demo(
                "Carousel",
                "Arrows, dots and autoplay every 5 seconds.",
                Carousel::new(slides).height(240.0).autoplay(5.0),
            ),
            Grid::new(
                columns(width, 2),
                (
                    demo(
                        "Toggle buttons and groups",
                        "Single choice for alignment, several for text styles.",
                        VStack::new((
                            ToggleGroup::single(
                                self.align.binding(),
                                [
                                    (Some(IconName::AlignLeft), ""),
                                    (Some(IconName::AlignCenter), ""),
                                    (Some(IconName::AlignRight), ""),
                                    (Some(IconName::AlignJustify), ""),
                                ],
                            ),
                            ToggleGroup::multiple(
                                self.text_styles.binding(),
                                [
                                    (Some(IconName::Bold), ""),
                                    (Some(IconName::Italic), ""),
                                    (Some(IconName::Underline), ""),
                                    (Some(IconName::Strikethrough), ""),
                                ],
                            ),
                            ToggleButton::new(self.pinned.binding(), "Pin to top").icon(IconName::Pin),
                        ))
                        .spacing(12.0),
                    ),
                    demo(
                        "Pagination, spinner and aspect ratio",
                        "Page through results; loaders respect reduced motion.",
                        VStack::new((
                            Pagination::new(self.page_number.binding(), 24),
                            HStack::new((
                                Spinner::new(),
                                Spinner::new().size(28.0),
                                Text::new("Loading reports…").foreground(Semantic::InkMuted),
                            ))
                            .spacing(16.0)
                            .align(Align::Center),
                            AspectRatio::new(
                                16.0 / 9.0,
                                Image::new(self.images[2].clone()).stretch().corner_radius(10.0),
                            ),
                        ))
                        .spacing(16.0)
                        .align(Align::Stretch),
                    ),
                ),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }
}
