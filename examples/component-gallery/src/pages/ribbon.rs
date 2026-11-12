//! The ribbon page.

use ravenrs::prelude::*;

use crate::{Gallery, page_header};

impl Gallery {
    pub(crate) fn ribbon_page(&self, _width: f32) -> impl View + use<> {
        let zoom = self.ribbon_zoom.clone();
        let (zin, zout) = (zoom.clone(), zoom.clone());
        let toasts = self.toasts.clone();
        let (t1, t2) = (toasts.clone(), toasts.clone());
        let align = self.align.clone();
        let align_toggle = |i: usize, label: &'static str, icon: IconName| {
            let (read, write) = (align.clone(), align.clone());
            RibbonItem::toggle(label, icon, Binding::new(move || read.get() == i, move |_| write.set(i)))
                .icon_only()
        };
        let ribbon = Ribbon::new()
            .app_menu(
                "File",
                vec![
                    MenuItem::new("New report").icon(IconName::FilePlus).shortcut("⌘N"),
                    MenuItem::new("Open…").icon(IconName::FolderOpen).shortcut("⌘O"),
                    MenuItem::new("Save").icon(IconName::Save).shortcut("⌘S"),
                    MenuItem::new("Export to PDF")
                        .icon(IconName::Download)
                        .on_select(move || t1.show("Exported", "invoice-1042.pdf", Tone::Success)),
                    MenuItem::new("Print").icon(IconName::Printer).shortcut("⌘P"),
                ],
            )
            .quick_access(RibbonItem::button("Save", IconName::Save))
            .quick_access(RibbonItem::button("Undo", IconName::Undo2))
            .quick_access(RibbonItem::button("Redo", IconName::Redo2))
            .search(self.ribbon_search.binding())
            .page(
                RibbonPage::new("Home")
                    .group(
                        RibbonGroup::new("Clipboard")
                            .item(
                                RibbonItem::button("Paste", IconName::ClipboardPaste)
                                    .menu_item(MenuItem::new("Paste").icon(IconName::ClipboardPaste))
                                    .menu_item(MenuItem::new("Paste values only").icon(IconName::Type)),
                            )
                            .item(RibbonItem::button("Cut", IconName::Scissors).small())
                            .item(RibbonItem::button("Copy", IconName::Copy).small())
                            .item(RibbonItem::button("Format painter", IconName::Paintbrush).small()),
                    )
                    .group(
                        RibbonGroup::new("Font")
                            .item(
                                RibbonItem::toggle("Bold", IconName::Bold, self.ribbon_bold.binding())
                                    .icon_only(),
                            )
                            .item(
                                RibbonItem::toggle("Italic", IconName::Italic, self.ribbon_italic.binding())
                                    .icon_only(),
                            )
                            .item(RibbonItem::button("Underline", IconName::Underline).icon_only())
                            .item(RibbonItem::button("Strikethrough", IconName::Strikethrough).icon_only())
                            .item(RibbonItem::button("Highlight", IconName::Highlighter).icon_only())
                            .item(RibbonItem::button("Font color", IconName::Baseline).icon_only())
                            .dialog_launcher(move || {
                                t2.show("Font", "The font dialog would open here.", Tone::Neutral)
                            }),
                    )
                    .group(
                        RibbonGroup::new("Alignment")
                            .item(align_toggle(0, "Align left", IconName::AlignLeft))
                            .item(align_toggle(1, "Center", IconName::AlignCenter))
                            .item(align_toggle(2, "Align right", IconName::AlignRight))
                            .item(align_toggle(3, "Justify", IconName::AlignJustify))
                            .item(RibbonItem::button("Bulleted list", IconName::List).icon_only())
                            .item(RibbonItem::button("Numbered list", IconName::ListOrdered).icon_only()),
                    )
                    .group(
                        RibbonGroup::new("Editing")
                            .item(RibbonItem::button("Find", IconName::Search).small())
                            .item(RibbonItem::button("Replace", IconName::ArrowUpDown).small())
                            .item(RibbonItem::button("Select all", IconName::TextCursor).small()),
                    ),
            )
            .page(
                RibbonPage::new("Insert")
                    .group(
                        RibbonGroup::new("Controls")
                            .item(RibbonItem::button("Label", IconName::Type))
                            .item(RibbonItem::button("Table", IconName::Table2))
                            .item(RibbonItem::button("Picture", IconName::ImagePlus))
                            .item(RibbonItem::button("Chart", IconName::ChartColumn)),
                    )
                    .group(
                        RibbonGroup::new("Elements")
                            .item(RibbonItem::button("Shapes", IconName::Shapes).small())
                            .item(RibbonItem::button("Page number", IconName::Pilcrow).small())
                            .item(RibbonItem::button("Link", IconName::Link2).small()),
                    ),
            )
            .page(
                RibbonPage::new("Data")
                    .group(
                        RibbonGroup::new("Data source")
                            .item(RibbonItem::button("Add data source", IconName::Database))
                            .item(RibbonItem::button("Query builder", IconName::Filter)),
                    )
                    .group(
                        RibbonGroup::new("Calculations")
                            .item(RibbonItem::button("Calculated field", IconName::Calculator).small())
                            .item(RibbonItem::button("Summary", IconName::Sigma).small())
                            .item(RibbonItem::button("Parameters", IconName::SlidersHorizontal).small()),
                    ),
            )
            .page(
                RibbonPage::new("View")
                    .group(
                        RibbonGroup::new("Show")
                            .item(
                                RibbonItem::toggle("Ruler", IconName::Ruler, self.ribbon_ruler.binding())
                                    .large(),
                            )
                            .item(
                                RibbonItem::toggle(
                                    "Gridlines",
                                    IconName::Grid2x2,
                                    self.ribbon_grid.binding(),
                                )
                                .large(),
                            )
                            .item(
                                RibbonItem::toggle(
                                    "Table selected",
                                    IconName::Table,
                                    self.ribbon_table.binding(),
                                )
                                .large(),
                            ),
                    )
                    .group(
                        RibbonGroup::new("Zoom")
                            .item(
                                RibbonItem::button("Zoom in", IconName::ZoomIn)
                                    .on_click(move || zin.update(|z| *z = (*z + 10).min(400))),
                            )
                            .item(
                                RibbonItem::button("Zoom out", IconName::ZoomOut)
                                    .on_click(move || zout.update(|z| *z = z.saturating_sub(10).max(10))),
                            ),
                    ),
            )
            .page(
                RibbonPage::new("Layout")
                    .contextual("Table tools", Tone::Warning, self.ribbon_table.get())
                    .group(
                        RibbonGroup::new("Rows & columns")
                            .item(RibbonItem::button("Insert row", IconName::Rows3))
                            .item(RibbonItem::button("Insert column", IconName::Columns3))
                            .item(RibbonItem::button("Merge cells", IconName::Merge).small())
                            .item(RibbonItem::button("Split cells", IconName::Split).small()),
                    ),
            );
        VStack::new((
            page_header(
                "Ribbon",
                "Office-style command surface: File menu, quick access toolbar, pages, groups with large and small commands, toggles, drop-downs, dialog launchers, a contextual \"Table tools\" tab and search. Double-click a tab to collapse it.",
            ),
            Card::new(
                VStack::new((
                    ribbon,
                    HStack::new((
                        VStack::new((
                            Text::new("FIELD LIST").style(TextStyle::Caption).foreground(Semantic::InkMuted),
                            List::new((
                                ListItem::new("InvoiceID").icon(IconName::Pilcrow),
                                ListItem::new("Customer").icon(IconName::User),
                                ListItem::new("Amount").icon(IconName::DollarSign),
                                ListItem::new("Issued").icon(IconName::CalendarDays),
                            )),
                        ))
                        .spacing(8.0)
                        .width(210.0)
                        .padding(16.0),
                        VStack::new((
                            Text::bound(&self.ribbon_zoom, |z| format!("Invoice report · {z}%"))
                                .style(TextStyle::Caption)
                                .foreground(Semantic::InkMuted),
                            VStack::new((
                                Text::new("REPORT HEADER").style(TextStyle::Caption).foreground(Semantic::InkMuted),
                                Text::new("Invoice #1042").style(TextStyle::Title),
                                Divider::horizontal(),
                                Text::new("DETAIL").style(TextStyle::Caption).foreground(Semantic::InkMuted),
                                HStack::new((Text::new("[Customer]").grow(1.0), Text::new("[Amount]"))).spacing(12.0),
                                Divider::horizontal(),
                                Text::new("PAGE FOOTER").style(TextStyle::Caption).foreground(Semantic::InkMuted),
                                Text::new("Page [PageNumber] of [PageCount]").foreground(Semantic::InkMuted),
                            ))
                            .spacing(12.0)
                            .padding(24.0)
                            .background(Semantic::Surface)
                            .border(1.0, Semantic::Border)
                            .corner_radius(4.0),
                        ))
                        .spacing(12.0)
                        .padding(16.0)
                        .grow(1.0),
                    ))
                    .align(Align::Stretch),
                ))
                .spacing(0.0)
                .align(Align::Stretch),
            )
            .padding(0.0)
            .corner_radius(14.0),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }
}
