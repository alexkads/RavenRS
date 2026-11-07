//! A spreadsheet component for RavenRS: opens, edits and saves `.xlsx` workbooks, with formulas
//! that recalculate, on the desktop and on the web.
//!
//! ```no_run
//! use ravenrs_spreadsheet::{Spreadsheet, SpreadsheetState};
//!
//! let sheet = SpreadsheetState::new();
//! sheet.set_cell("A1", "Total").unwrap();
//! sheet.set_cell("B1", "=SUM(B2:B10)").unwrap();
//! let view = Spreadsheet::new(&sheet);
//! ```
//!
//! [`SpreadsheetState`] holds the workbook and is shared with the application; [`Spreadsheet`]
//! shows it with a toolbar, a formula bar, the grid and the sheet tabs. The engine underneath
//! is `ravenrs-sheet`, where every change is a command with undo.

use ravenrs_app_i18n::{Language, Texts};
use ravenrs_core::{Insets, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{SPACING, Semantic, TextStyle};
use ravenrs_widgets::{
    AnyView, BuildCx, Button, ButtonSize, ButtonStyle, Divider, FileRequest, HStack, IconButton, IconName,
    NodeId, Paint, ScrollView, Semantics, Spacer, Text, TextField, View, ViewExt,
};
use serde_json::json;

mod geometry;
mod state;
mod surface;

pub use ravenrs_sheet as sheet;
pub use state::SpreadsheetState;

use state::EditPlace;

/// Files Open offers.
const OPEN_EXTENSIONS: [&str; 6] = ["xlsx", "xlsm", "xls", "ods", "csv", "tsv"];
/// Biggest file Open reads.
const OPEN_MOST_BYTES: u64 = 64 * 1024 * 1024;

/// The text a spreadsheet shows.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct SpreadsheetLabels {
    /// The grid's name, for screen readers.
    pub grid: String,
    /// The formula bar's hint.
    pub formula: String,
    /// Undoes the last change.
    pub undo: String,
    /// Redoes what was undone.
    pub redo: String,
    /// Bold.
    pub bold: String,
    /// Italic.
    pub italic: String,
    /// Underline.
    pub underline: String,
    /// Strikethrough.
    pub strikethrough: String,
    /// Aligns to the left.
    pub align_left: String,
    /// Centers.
    pub align_center: String,
    /// Aligns to the right.
    pub align_right: String,
    /// The currency format.
    pub currency: String,
    /// The percentage format.
    pub percent: String,
    /// Sums the cells above or to the left.
    pub auto_sum: String,
    /// Opens a file.
    pub open: String,
    /// Saves the workbook as `.xlsx`.
    pub save: String,
    /// Adds a sheet.
    pub add_sheet: String,
}

impl Default for SpreadsheetLabels {
    fn default() -> Self {
        Self::from_texts(&Texts::new(Language::English))
    }
}

impl SpreadsheetLabels {
    /// The labels for a language and region such as `pt-BR`.
    pub fn for_culture(culture: &str) -> Self {
        if culture.starts_with("pt") { Self::pt_br() } else { Self::default() }
    }

    /// The texts of a catalog, in its language. What it does not have comes from English.
    pub fn from_texts(texts: &Texts) -> Self {
        Self {
            grid: texts.get("spreadsheet.grid"),
            formula: texts.get("spreadsheet.formula"),
            undo: texts.get("spreadsheet.undo"),
            redo: texts.get("spreadsheet.redo"),
            bold: texts.get("spreadsheet.bold"),
            italic: texts.get("spreadsheet.italic"),
            underline: texts.get("spreadsheet.underline"),
            strikethrough: texts.get("spreadsheet.strikethrough"),
            align_left: texts.get("spreadsheet.align_left"),
            align_center: texts.get("spreadsheet.align_center"),
            align_right: texts.get("spreadsheet.align_right"),
            currency: texts.get("spreadsheet.currency"),
            percent: texts.get("spreadsheet.percent"),
            auto_sum: texts.get("spreadsheet.auto_sum"),
            open: texts.get("spreadsheet.open"),
            save: texts.get("spreadsheet.save"),
            add_sheet: texts.get("spreadsheet.add_sheet"),
        }
    }

    /// The labels in Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self::from_texts(&Texts::new(Language::BrazilianPortuguese))
    }
}

/// A spreadsheet: toolbar, formula bar, the grid of cells and the sheet tabs.
///
/// Click or drag to select, type to replace a cell, double-click or F2 to edit it, Enter and
/// Tab to move on. Cmd or Ctrl with C, X, V, Z, Y, B, I, U and A copy, cut, paste, undo, redo,
/// format and select all. Dragging a column header's edge sizes the column, and double-clicking
/// it fits the column to its content.
pub struct Spreadsheet {
    state: SpreadsheetState,
    labels: Option<SpreadsheetLabels>,
    toolbar: bool,
    file_buttons: bool,
    read_only: bool,
}

impl std::fmt::Debug for Spreadsheet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spreadsheet")
            .field("state", &self.state)
            .field("read_only", &self.read_only)
            .finish_non_exhaustive()
    }
}

impl Spreadsheet {
    /// Shows `state`'s workbook.
    pub fn new(state: &SpreadsheetState) -> Self {
        Self { state: state.clone(), labels: None, toolbar: true, file_buttons: true, read_only: false }
    }

    /// Replaces the labels, which otherwise follow the application's language.
    pub fn labels(mut self, labels: SpreadsheetLabels) -> Self {
        self.labels = Some(labels);
        self
    }

    /// Shows or hides the toolbar (shown by default).
    pub fn toolbar(mut self, show: bool) -> Self {
        self.toolbar = show;
        self
    }

    /// Shows or hides Open and Save on the toolbar (shown by default).
    pub fn file_buttons(mut self, show: bool) -> Self {
        self.file_buttons = show;
        self
    }

    /// Lets the workbook be read, selected and copied, but not changed.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    fn command_button(
        &self,
        icon: IconName,
        label: &str,
        command: &'static str,
        params: serde_json::Value,
    ) -> AnyView {
        let state = self.state.clone();
        Box::new(IconButton::new(icon).label(label.to_owned()).size(ButtonSize::Small).on_click(move || {
            let _ = state.execute(command, params.clone());
        }))
    }

    fn text_button(
        &self,
        text: &str,
        label: &str,
        command: &'static str,
        params: serde_json::Value,
    ) -> AnyView {
        let state = self.state.clone();
        Box::new(
            Button::new(text.to_owned())
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Small)
                .accessible_name(label.to_owned())
                .on_click(move || {
                    let _ = state.execute(command, params.clone());
                }),
        )
    }

    fn toolbar_view(&self, labels: &SpreadsheetLabels, cx: &mut BuildCx<'_>) -> impl View + use<> {
        let mut items: Vec<AnyView> = Vec::new();
        if self.file_buttons {
            let (open, picker, waker) = (self.state.clone(), cx.file_picker(), cx.ui_waker());
            items.push(Box::new(
                IconButton::new(IconName::FolderOpen)
                    .label(labels.open.clone())
                    .size(ButtonSize::Small)
                    .on_click(move || {
                        let (open, waker) = (open.clone(), waker.clone());
                        let mut request = FileRequest::default();
                        request.extensions = OPEN_EXTENSIONS.iter().map(|e| (*e).to_owned()).collect();
                        request.most_bytes = Some(OPEN_MOST_BYTES);
                        picker.pick(&request, move |files| {
                            if let Some(file) = files.first() {
                                if let Err(error) = open.replace(&file.name, &file.bytes) {
                                    open.show_error(error);
                                }
                                waker.wake();
                            }
                        });
                    }),
            ));
            let (save, picker) = (self.state.clone(), cx.file_picker());
            items.push(Box::new(
                IconButton::new(IconName::Save).label(labels.save.clone()).size(ButtonSize::Small).on_click(
                    move || {
                        if let Ok(bytes) = save.to_xlsx() {
                            picker.save(&save.file_name(), &bytes);
                        }
                    },
                ),
            ));
            items.push(Box::new(Divider::vertical()));
        }
        if !self.read_only {
            items.push(self.command_button(IconName::Undo2, &labels.undo, "edit.undo", json!({})));
            items.push(self.command_button(IconName::Redo2, &labels.redo, "edit.redo", json!({})));
            items.push(Box::new(Divider::vertical()));
            items.push(self.command_button(IconName::Bold, &labels.bold, "home.bold", json!({})));
            items.push(self.command_button(IconName::Italic, &labels.italic, "home.italic", json!({})));
            items.push(self.command_button(
                IconName::Underline,
                &labels.underline,
                "home.underline",
                json!({}),
            ));
            items.push(self.command_button(
                IconName::Strikethrough,
                &labels.strikethrough,
                "home.strikethrough",
                json!({}),
            ));
            items.push(Box::new(Divider::vertical()));
            items.push(self.command_button(
                IconName::AlignLeft,
                &labels.align_left,
                "home.alignLeft",
                json!({}),
            ));
            items.push(self.command_button(
                IconName::AlignCenter,
                &labels.align_center,
                "home.alignCenter",
                json!({}),
            ));
            items.push(self.command_button(
                IconName::AlignRight,
                &labels.align_right,
                "home.alignRight",
                json!({}),
            ));
            items.push(Box::new(Divider::vertical()));
            items.push(self.command_button(
                IconName::DollarSign,
                &labels.currency,
                "home.numberFormat",
                json!({ "format": "Currency" }),
            ));
            items.push(self.text_button("%", &labels.percent, "home.numberFormat", json!({ "code": "0%" })));
            items.push(self.command_button(IconName::Sigma, &labels.auto_sum, "formulas.autoSum", json!({})));
        }
        HStack::new(items)
            .spacing(SPACING.s1)
            .align(Align::Center)
            .wrap()
            .padding(Insets::symmetric(SPACING.s1, SPACING.s3))
            .background(Semantic::Surface)
    }

    fn formula_bar(&self, labels: &SpreadsheetLabels) -> impl View + use<> {
        let active = self.state.active_cell();
        let (read, write) = (self.state.clone(), self.state.clone());
        let (submit, cancel) = (self.state.clone(), self.state.clone());
        let input = TextField::new(Binding::new(
            move || match read.edit().get() {
                Some(edit) => edit.text,
                None => read.cell_input(&read.active_cell()),
            },
            move |text: String| match write.edit().get() {
                Some(_) => write.edit().update(|e| {
                    if let Some(e) = e {
                        e.text = text;
                    }
                }),
                None => write.begin_edit(Some(text), EditPlace::Bar),
            },
        ))
        .placeholder(labels.formula.clone())
        .compact()
        .flat()
        .read_only(self.read_only)
        .on_submit(move |_| submit.commit_edit(1, 0))
        .on_cancel(move || cancel.cancel_edit());
        let mut row: Vec<AnyView> = vec![
            Box::new(Text::new(active).style(TextStyle::Callout).foreground(Semantic::InkMuted).width(72.0)),
            Box::new(Divider::vertical()),
            Box::new(Text::new("fx").style(TextStyle::Callout).foreground(Semantic::InkMuted)),
            Box::new(input.grow(1.0)),
        ];
        if let Some(error) = self.state.error() {
            row.push(Box::new(
                Text::new(error).style(TextStyle::Caption).foreground(Semantic::Danger).truncate(),
            ));
        }
        HStack::new(row)
            .spacing(SPACING.s2)
            .align(Align::Center)
            .padding(Insets::symmetric(SPACING.s1, SPACING.s3))
    }

    fn sheet_tabs(&self, labels: &SpreadsheetLabels) -> impl View + use<> {
        let active = self.state.active_sheet();
        let mut tabs: Vec<AnyView> = self
            .state
            .sheet_names()
            .into_iter()
            .enumerate()
            .map(|(i, name)| {
                let state = self.state.clone();
                Box::new(
                    Button::new(name)
                        .style(if i == active { ButtonStyle::Secondary } else { ButtonStyle::Ghost })
                        .size(ButtonSize::Small)
                        .on_click(move || {
                            let _ = state.execute("sheet.activate", json!({ "sheet": i }));
                        }),
                ) as AnyView
            })
            .collect();
        if !self.read_only {
            tabs.push(self.command_button(
                IconName::Plus,
                &labels.add_sheet,
                "home.insertSheet",
                json!({ "before": usize::MAX }),
            ));
        }
        tabs.push(Box::new(Spacer::new()));
        ScrollView::new((HStack::new(tabs)
            .spacing(SPACING.s1)
            .align(Align::Center)
            .padding(Insets::symmetric(SPACING.s1, SPACING.s3)),))
        .horizontal()
        .background(Semantic::Surface)
    }
}

impl View for Spreadsheet {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let _ = self.state.version();
        let labels =
            self.labels.clone().unwrap_or_else(|| SpreadsheetLabels::for_culture(&cx.labels().culture));
        let mut children = Vec::new();
        if self.toolbar {
            let bar = self.toolbar_view(&labels, cx);
            children.push(cx.child(&bar));
            children.push(cx.child(&Divider::horizontal()));
        }
        children.push(cx.child(&self.formula_bar(&labels)));
        children.push(cx.child(&Divider::horizontal()));
        let grid = surface::Surface {
            state: self.state.clone(),
            name: labels.grid.clone(),
            read_only: self.read_only,
        };
        children.push(cx.child(&grid));
        children.push(cx.child(&Divider::horizontal()));
        children.push(cx.child(&self.sheet_tabs(&labels)));
        let root = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                overflow_hidden: true,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        );
        cx.semantics(root, Semantics::new(Role::Group).name(labels.grid));
        root
    }
}
