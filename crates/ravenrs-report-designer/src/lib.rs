//! RavenRS Report Designer: an end-user report designer running on the
//! desktop and in the browser. Pick a template or run the wizard, drag fields
//! onto bands, edit properties, and preview the paginated result instantly.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::prelude::*;
use ravenrs_core::Key;
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_report::barcode::Symbology;
use ravenrs_report::chart::ChartKind;
use ravenrs_report::wizard::{self, FieldInfo, FieldKind, WizardOptions};
use ravenrs_report::{
    Control, DataSource, DataSourceKind, DateTime, Line, Report, Style, Table, TableRow, TextAlign, samples,
};
use ravenrs_report_viewer::{DocumentViewer, ParameterLabels, ParameterPanel, ViewerLabels, ViewerState};
use ravenrs_style::{RADII, SPACING};
use ravenrs_widgets::{DragPhase, Paint, Placement, QuadPaint};

/// What the modules of the designer use, as an application would.
pub(crate) mod prelude {
    pub use ravenrs_core::{Color, CursorIcon, Insets, Point, Quad, Rect, Scene};
    pub use ravenrs_layout::Align;
    pub use ravenrs_reactive::{Binding, State};
    pub use ravenrs_style::{Semantic, TextStyle, Theme, Weight};
    pub use ravenrs_widgets::*;
}

mod bands;
mod editor;
mod groups;
mod preview;
mod props;
mod surface;
mod thermal;

use bands::BandRef;
use editor::{Editor, FieldDrag, Selection};
use preview::RenderCache;

/// The designer app.
pub struct Designer {
    embedded: Option<Embedded>,
    /// Whether the status bar offers the themes.
    theme_menu: bool,
    editor: Editor,
    cache: Rc<RefCell<RenderCache>>,
    params: State<HashMap<String, serde_json::Value>>,
    left_tab: State<usize>,
    wizard: WizardState,
}

/// What the designer says and does inside an application, in place of its gallery and wizard.
#[derive(Clone)]
struct Embedded {
    save_label: String,
    close_label: String,
    on_save: Rc<dyn Fn(Report)>,
    on_close: Rc<dyn Fn()>,
}

#[derive(Clone)]
struct WizardState {
    source: State<usize>,
    title: State<String>,
    chosen: State<Vec<String>>,
    group: State<usize>,
    totals: State<bool>,
}

impl std::fmt::Debug for Designer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Designer").finish_non_exhaustive()
    }
}

impl Default for Designer {
    fn default() -> Self {
        Self::new()
    }
}

/// The keys [`Designer::sample`] knows, one per report of the template gallery.
pub const SAMPLE_KEYS: [&str; 5] = ["invoice", "sales", "sales-10k", "dashboard", "customers"];

impl Designer {
    /// A designer showing the sales report, with the template gallery open.
    pub fn new() -> Self {
        Self {
            embedded: None,
            theme_menu: true,
            editor: Editor::new(samples::sales_by_region(300)),
            cache: Rc::new(RefCell::new(RenderCache::default())),
            params: State::new(HashMap::new()),
            left_tab: State::new(0),
            wizard: WizardState {
                source: State::new(0),
                title: State::new("Relatório de vendas".into()),
                chosen: State::new(vec![
                    "Date".into(),
                    "Customer".into(),
                    "Product".into(),
                    "Quantity".into(),
                    "UnitPrice".into(),
                ]),
                group: State::new(4),
                totals: State::new(true),
            },
        }
    }
}

impl Designer {
    /// A designer of this report inside an application, with no gallery or wizard: the layout is
    /// what it is about, with the data embedded in it for the field list and the preview.
    /// `on_save` gets the report as it stands, and `on_close` is for leaving.
    pub fn embedded(
        report: Report,
        save_label: impl Into<String>,
        close_label: impl Into<String>,
        on_save: impl Fn(Report) + 'static,
        on_close: impl Fn() + 'static,
    ) -> Self {
        let mut designer = Self::new();
        designer.editor = Editor::new(report);
        designer.editor.gallery_open.set(false);
        designer.embedded = Some(Embedded {
            save_label: save_label.into(),
            close_label: close_label.into(),
            on_save: Rc::new(on_save),
            on_close: Rc::new(on_close),
        });
        designer
    }

    /// Whether the status bar offers the themes, on unless said otherwise. Inside an application
    /// the menu changes the application's theme, which is the same one; an application that keeps
    /// a look of its own says no here.
    pub fn theme_menu(mut self, offered: bool) -> Self {
        self.theme_menu = offered;
        self
    }

    /// Opens one of the sample reports by its key, in the preview, with the gallery closed: what
    /// the web page reads from `?report=` so each page of the home fan opens its own report.
    /// The keys are those of [`SAMPLE_KEYS`]; an unknown one keeps the gallery open.
    pub fn sample(self, key: &str) -> Self {
        let report = match key {
            "invoice" => samples::invoice(),
            "sales" => samples::sales_by_region(300),
            "sales-10k" => samples::sales_by_region(10_000),
            "dashboard" => samples::sales_dashboard(),
            "customers" => samples::customer_list(),
            _ => return self,
        };
        self.editor.load(report);
        self.editor.tab.set(1);
        self.editor.gallery_open.set(false);
        self
    }

    fn embedded_bar(&self) -> Option<AnyView> {
        let embedded = self.embedded.clone()?;
        let (editor, on_save, on_close) = (self.editor.clone(), embedded.on_save, embedded.on_close);
        Some(Box::new(
            HStack::new((
                Button::new(embedded.save_label).size(ButtonSize::Small).on_click(move || {
                    on_save(editor.report.get());
                }),
                Button::new(embedded.close_label)
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .on_click(move || on_close()),
            ))
            .spacing(SPACING.s2)
            .align(Align::Center)
            .padding(SPACING.s2),
        ))
    }
}

/// The data sets the wizard offers, with their row counts.
const WIZARD_SOURCES: [(&str, usize); 2] =
    [("Vendas (300 pedidos)", 300), ("Vendas (10.000 pedidos)", 10_000)];

/// Width of the toolbox: one column of icon buttons.
pub(crate) const TOOLBOX_WIDTH: f32 = 48.0;
/// Width of the docked panels: nine color swatches and their padding.
pub(crate) const DOCK_WIDTH: f32 = 320.0;
/// The least and the most zoom of the design surface.
const ZOOM_LEAST: f32 = 0.5;
const ZOOM_MOST: f32 = 3.0;

impl Component for Designer {
    fn body(&self) -> impl View {
        let e = self.editor.clone();
        // The ribbon and the side panels only serve the design surface.
        let design = e.tab.get() == 0;
        let on_its_own = self.embedded.is_none();
        VStack::new((
            self.embedded_bar(),
            design.then(|| self.ribbon()),
            FillRow((
                design.then(|| self.toolbox()),
                design.then(Divider::vertical),
                CenterPane { editor: e.clone(), cache: self.cache.clone(), params: self.params.clone() },
                design.then(Divider::vertical),
                design.then(|| self.dock()),
            )),
            Divider::horizontal(),
            StatusBar { editor: e.clone(), cache: self.cache.clone(), themes: self.theme_menu },
            thermal::ThermalDialog {
                editor: e.clone(),
                cache: self.cache.clone(),
                params: self.params.clone(),
            },
            on_its_own.then(|| Gallery { editor: e.clone() }),
            on_its_own.then(|| WizardDialog { editor: e.clone(), state: self.wizard.clone() }),
            ExpressionDialog(e.clone()),
            FieldGhost(e.clone()),
            Shortcuts { editor: e, cache: self.cache.clone(), params: self.params.clone() },
        ))
        .spacing(0.0)
        .align(Align::Stretch)
        .grow(1.0)
    }
}

/// The tabs under the document that switch between the design surface, the preview
/// and the JSON, as tabs of the document.
fn view_tabs(e: &Editor) -> impl View + use<> {
    HStack::new((Tabs::new(e.tab.binding())
        .item("Design")
        .item("Visualizar")
        .item("JSON")
        .size(ButtonSize::Small),))
    .padding(Insets::symmetric(0.0, SPACING.s3))
    .background(Semantic::Surface)
}

/// A row that takes the space left and never grows past it, so the panels
/// inside scroll instead of pushing the status bar off the window.
struct FillRow<C>(C);

impl<C: ViewSeq> View for FillRow<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let children = cx.children(&self.0);
        cx.container(
            LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                overflow_hidden: true,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::None,
        )
    }
}

/// A column of the window as tall as the row it is in, so what grows inside it takes the height
/// left: the toolbox and the docked panels.
fn window_column(content: impl View + 'static, width: f32) -> impl View {
    content.background(Semantic::Surface).width(width)
}

/// Renders the report as it stands and sends it to the printer.
fn print_report(
    editor: &Editor,
    cache: &Rc<RefCell<RenderCache>>,
    params: &State<HashMap<String, serde_json::Value>>,
) {
    let doc = preview::document(editor, cache, &params.get());
    preview::print(&doc);
}

/// Renders the report as it stands and exports it.
fn export_report(
    editor: &Editor,
    cache: &Rc<RefCell<RenderCache>>,
    params: &State<HashMap<String, serde_json::Value>>,
    format: preview::Export,
) {
    let doc = preview::document(editor, cache, &params.get());
    preview::export(&doc, format);
}

impl Designer {
    fn ribbon(&self) -> impl View + use<> {
        let e = self.editor.clone();
        let exporter = |item: RibbonItem, format: preview::Export| {
            let (ed, cache, params) = (e.clone(), self.cache.clone(), self.params.clone());
            item.on_click(move || export_report(&ed, &cache, &params, format))
        };
        let export_item = |label: &'static str, icon: IconName, format: preview::Export| {
            let (ed, cache, params) = (e.clone(), self.cache.clone(), self.params.clone());
            MenuItem::new(label).icon(icon).on_select(move || export_report(&ed, &cache, &params, format))
        };
        let printer = |item: RibbonItem| {
            let (ed, cache, params) = (e.clone(), self.cache.clone(), self.params.clone());
            item.on_click(move || print_report(&ed, &cache, &params))
        };
        let (pm_e, pm_c, pm_p) = (e.clone(), self.cache.clone(), self.params.clone());
        let (e1, e2, e3, e4, e5, e6) = (e.clone(), e.clone(), e.clone(), e.clone(), e.clone(), e.clone());
        let (z1, z2) = (e.clone(), e.clone());
        let selected = e.selected_control().is_some();
        let action = |label: &'static str, icon: IconName, run: fn(&Editor)| {
            let e = e.clone();
            RibbonItem::button(label, icon).on_click(move || run(&e))
        };
        // A toggle over the selected control's style, off and dimmed with nothing selected.
        let style_toggle =
            |label: &'static str, icon: IconName, is: fn(&Style) -> bool, set: fn(&mut Style, bool)| {
                let (read, write) = (e.clone(), e.clone());
                RibbonItem::toggle(
                    label,
                    icon,
                    Binding::new(
                        move || read.selected_control().is_some_and(|(_, _, c)| is(&c.style)),
                        move |on| write.edit_control(label, |c| set(&mut c.style, on)),
                    ),
                )
                .small()
                .enabled(selected)
            };
        let align = |label: &'static str, icon: IconName, to: TextAlign| {
            let (read, write) = (e.clone(), e.clone());
            RibbonItem::toggle(
                label,
                icon,
                Binding::new(
                    move || {
                        read.selected_control()
                            .is_some_and(|(_, _, c)| c.style.text_align.unwrap_or_default() == to)
                    },
                    move |_| write.edit_control("align", |c| c.style.text_align = Some(to)),
                ),
            )
            .small()
            .enabled(selected)
        };
        let font_step = |label: &'static str, icon: IconName, by: f32| {
            let e = e.clone();
            RibbonItem::button(label, icon).small().enabled(selected).on_click(move || {
                e.edit_control("font size", |c| {
                    c.style.font_size = Some((c.style.font_size.unwrap_or(10.0) + by).clamp(4.0, 72.0));
                })
            })
        };
        let on_its_own = self.embedded.is_none();
        let mut file_menu = vec![
            MenuItem::new("Imprimir…")
                .icon(IconName::Printer)
                .shortcut("⌘P")
                .on_select(move || print_report(&pm_e, &pm_c, &pm_p)),
            export_item("Exportar para PDF", IconName::FileText, preview::Export::Pdf),
            export_item("Exportar para Excel", IconName::Table, preview::Export::Xlsx),
            export_item("Exportar para CSV", IconName::Download, preview::Export::Csv),
            MenuItem::new("Ver o JSON do relatório").icon(IconName::Code).on_select(move || e3.tab.set(2)),
        ];
        if on_its_own {
            file_menu.insert(
                0,
                MenuItem::new("Assistente de relatório…")
                    .icon(IconName::Sparkles)
                    .on_select(move || e2.wizard_open.set(true)),
            );
            file_menu.insert(
                0,
                MenuItem::new("Novo a partir de modelo…")
                    .icon(IconName::FilePlus)
                    .on_select(move || e1.gallery_open.set(true)),
            );
        }
        let home = RibbonPage::new("Início");
        let home = if on_its_own {
            home.group(
                RibbonGroup::new("Relatório")
                    .item(
                        RibbonItem::button("Modelos", IconName::LayoutDashboard)
                            .large()
                            .on_click(move || e6.gallery_open.set(true)),
                    )
                    .item({
                        let e = e.clone();
                        RibbonItem::button("Assistente", IconName::Sparkles)
                            .large()
                            .on_click(move || e.wizard_open.set(true))
                    }),
            )
        } else {
            home
        };
        let has_copy = e.copied.with(Option::is_some);
        // The Home page: clipboard, font, alignment, layout, zoom.
        let home = home
            .group(
                RibbonGroup::new("Editar")
                    .item(action("Colar", IconName::ClipboardPaste, Editor::paste).large().enabled(has_copy))
                    .item(
                        action("Recortar", IconName::Scissors, Editor::cut_selected)
                            .small()
                            .enabled(selected),
                    )
                    .item(action("Copiar", IconName::Copy, Editor::copy_selected).small().enabled(selected))
                    .item(
                        action("Excluir", IconName::Trash2, Editor::delete_selected)
                            .small()
                            .enabled(selected),
                    ),
            )
            .group(RibbonGroup::new("Dados").item({
                let e = e.clone();
                RibbonItem::button("Expressão", IconName::Code)
                    .large()
                    .enabled(e.control_has_expression())
                    .on_click(move || e.open_expression(editor::ExpressionTarget::Control))
            }))
            .group(
                RibbonGroup::new("Fonte")
                    .item(style_toggle(
                        "Negrito",
                        IconName::Bold,
                        |s| s.bold == Some(true),
                        |s, on| s.bold = on.then_some(true),
                    ))
                    .item(font_step("Aumentar fonte", IconName::Plus, 1.0))
                    .item(font_step("Diminuir fonte", IconName::Minus, -1.0)),
            )
            .group(
                RibbonGroup::new("Alinhamento")
                    .item(align("À esquerda", IconName::AlignLeft, TextAlign::Left))
                    .item(align("Centralizar", IconName::AlignCenter, TextAlign::Center))
                    .item(align("À direita", IconName::AlignRight, TextAlign::Right)),
            )
            .group(
                RibbonGroup::new("Layout")
                    .item({
                        let e = e.clone();
                        RibbonItem::button("Trazer para a frente", IconName::ArrowUp)
                            .small()
                            .enabled(selected)
                            .on_click(move || e.reorder_selected(true))
                    })
                    .item({
                        let e = e.clone();
                        RibbonItem::button("Enviar para trás", IconName::ArrowDown)
                            .small()
                            .enabled(selected)
                            .on_click(move || e.reorder_selected(false))
                    }),
            )
            .group({
                // Shift- or Cmd-click picks more controls; they follow the first one picked (#46).
                let several = e.several_selected();
                let arrange = |name: &'static str, icon: IconName, how: editor::Arrange| {
                    let e = e.clone();
                    RibbonItem::button(name, icon)
                        .small()
                        .enabled(several)
                        .on_click(move || e.arrange_selected(how))
                };
                RibbonGroup::new("Organizar")
                    .item(arrange("Alinhar à esquerda", IconName::AlignLeft, editor::Arrange::Left))
                    .item(arrange("Alinhar ao centro", IconName::AlignCenter, editor::Arrange::Center))
                    .item(arrange("Alinhar à direita", IconName::AlignRight, editor::Arrange::Right))
                    .item(arrange("Alinhar em cima", IconName::ChevronUp, editor::Arrange::Top))
                    .item(arrange("Alinhar no meio", IconName::EllipsisVertical, editor::Arrange::Middle))
                    .item(arrange("Alinhar embaixo", IconName::ChevronDown, editor::Arrange::Bottom))
                    .item(arrange("Mesma largura", IconName::SlidersHorizontal, editor::Arrange::SameWidth))
                    .item(arrange("Mesma altura", IconName::ArrowUpDown, editor::Arrange::SameHeight))
            })
            .group(
                RibbonGroup::new("Zoom")
                    .item(
                        RibbonItem::button("Aproximar", IconName::ZoomIn)
                            .small()
                            .on_click(move || z1.zoom.update(|z| *z = (*z + 0.25).min(ZOOM_MOST))),
                    )
                    .item(
                        RibbonItem::button("Afastar", IconName::ZoomOut)
                            .small()
                            .on_click(move || z2.zoom.update(|z| *z = (*z - 0.25).max(ZOOM_LEAST))),
                    ),
            );
        let mut controls = RibbonGroup::new("Controles");
        for (i, (label, icon, make)) in TOOLS.into_iter().enumerate() {
            let e = e.clone();
            let item = RibbonItem::button(label, icon).on_click(move || insert_tool(&e, make));
            controls = controls.item(if i < 6 { item.large() } else { item.small() });
        }
        Ribbon::new()
            .app_menu("Arquivo", file_menu)
            .quick_access(RibbonItem::button("Desfazer", IconName::Undo2).on_click(move || e4.undo()))
            .quick_access(RibbonItem::button("Refazer", IconName::Redo2).on_click(move || e5.redo()))
            .quick_access(printer(RibbonItem::button("Imprimir", IconName::Printer)))
            .page(home)
            .page(RibbonPage::new("Inserir").group(controls))
            .page(
                RibbonPage::new("Saída").group(
                    RibbonGroup::new("Saída")
                        .item({
                            let e = e.clone();
                            RibbonItem::button("Impressora direta", IconName::Printer)
                                .large()
                                .on_click(move || e.thermal_open.set(true))
                        })
                        .item(printer(RibbonItem::button("Imprimir", IconName::Printer).large()))
                        .item(exporter(
                            RibbonItem::button("PDF", IconName::FileText).large(),
                            preview::Export::Pdf,
                        ))
                        .item(exporter(
                            RibbonItem::button("Excel", IconName::Table).large(),
                            preview::Export::Xlsx,
                        ))
                        .item(exporter(
                            RibbonItem::button("CSV", IconName::Download).small(),
                            preview::Export::Csv,
                        )),
                ),
            )
    }

    /// The toolbox down the left side: the pointer, then one button per
    /// control, which drops it into the band in use.
    fn toolbox(&self) -> impl View + use<> {
        let e = self.editor.clone();
        let pick = Binding::new(
            || 0,
            move |i: usize| {
                if let Some((_, _, make)) = i.checked_sub(1).and_then(|i| TOOLS.get(i)) {
                    insert_tool(&e, *make);
                }
            },
        );
        let dragging = self.editor.clone();
        let mut rail = ToolRail::new(pick, [(IconName::MousePointer, "Ponteiro")])
            .icons_only(true)
            .separator()
            // A control dragged onto a band falls where the pointer lets go (#44).
            .on_drag_tool(move |i, d| drag_tool(&dragging, i, d));
        for (name, icon, _) in TOOLS {
            rail = rail.tool(ToolRailItem::new(icon, name));
        }
        window_column(ScrollView::new((rail.width(TOOLBOX_WIDTH),)).grow(1.0), TOOLBOX_WIDTH)
    }

    /// The panels docked on the right: the field list and the report
    /// explorer as tabs on top, the properties below, with a bar to drag between them.
    fn dock(&self) -> impl View + use<> {
        let e = self.editor.clone();
        let fixed = State::new(0);
        let top = VStack::new((
            Tabs::new(self.left_tab.binding())
                .item("Lista de campos")
                .item("Explorador")
                .item("Agrupar e ordenar")
                .size(ButtonSize::Small),
            VStack::new((LeftContent { editor: e.clone(), tab: self.left_tab.clone() },))
                .padding(SPACING.s3)
                .align(Align::Stretch)
                .grow(1.0),
        ))
        .align(Align::Stretch)
        .grow(1.0);
        let bottom = VStack::new((
            Tabs::new(fixed.binding()).item("Propriedades").size(ButtonSize::Small),
            props::Properties(e),
        ))
        .align(Align::Stretch)
        .grow(1.0);
        window_column(
            SplitPanes::vertical()
                .pane(SplitPane::new(top).size(320.0).range(160.0, 640.0))
                .pane(SplitPane::new(bottom).range(220.0, f32::MAX)),
            DOCK_WIDTH,
        )
    }
}

/// A control of the toolbox: its name, its icon and how a new one starts.
type Tool = (&'static str, IconName, fn() -> Control);

/// The controls of the toolbox and of the Insert page, in order.
const TOOLS: [Tool; 16] =
    [
        ("Rótulo", IconName::Type, || Control::label("Texto").at(0.0, 0.0, 140.0, 18.0)),
        ("Campo", IconName::Database, || Control::field("[Customer]").at(0.0, 0.0, 140.0, 18.0)),
        ("Tabela", IconName::Table, || {
            Control::table(Table::new().row(
                TableRow::new(18.0).text(120.0, "Coluna 1").text(120.0, "Coluna 2").text(120.0, "Coluna 3"),
            ))
        }),
        ("Imagem", IconName::Image, || Control::picture("").at(0.0, 0.0, 100.0, 60.0)),
        ("Código de barras", IconName::Tag, || {
            Control::barcode(Symbology::Code128, "[Id]").at(0.0, 0.0, 160.0, 50.0)
        }),
        ("Gráfico", IconName::ChartColumn, || {
            Control::chart(ChartKind::Bar, "[Region]", "[Quantity] * [UnitPrice]").at(0.0, 0.0, 320.0, 180.0)
        }),
        ("Caixa de seleção", IconName::CircleCheck, || {
            Control::check_box("[Paid]", "Pago").at(0.0, 0.0, 100.0, 18.0)
        }),
        ("Linha", IconName::Minus, || Control {
            kind: ravenrs_report::ControlKind::Line(Line::default()),
            ..Control::line().at(0.0, 0.0, 200.0, 4.0)
        }),
        ("Retângulo", IconName::Square, || {
            Control::rect(4.0)
                .at(0.0, 0.0, 120.0, 40.0)
                .style(Style::new().background("#f4f5f8").border("all", 0.75, "#dadde5"))
        }),
        ("Total", IconName::Sigma, || {
            Control::field("Sum([Total])").format("C2").at(0.0, 0.0, 140.0, 18.0).style(Style::new().bold())
        }),
        ("Página N de M", IconName::FileText, || {
            Control::field("FormatString('Página {0} de {1}', PageNumber(), PageCount())")
                .at(0.0, 0.0, 160.0, 16.0)
        }),
        ("Link", IconName::Link, || Control::link("Abrir site", "https://").at(0.0, 0.0, 140.0, 18.0)),
        ("Corte", IconName::Scissors, || Control::printer_command(ravenrs_report::PrinterAction::Cut)),
        ("Gaveta", IconName::Inbox, || Control::printer_command(ravenrs_report::PrinterAction::OpenDrawer)),
        ("Bipe", IconName::Bell, || Control::printer_command(ravenrs_report::PrinterAction::Beep)),
        ("Marcador", IconName::Bookmark, || Control::bookmark("[Customer]", 0).at(0.0, 0.0, 120.0, 12.0)),
    ];

/// Drops a new control into the band in use, at a free spot, and selects it.
/// A control of the toolbox dragged onto a band: the ghost follows the pointer, and letting go
/// over a band puts the control there, on the grid. `i` counts the pointer first.
fn drag_tool(e: &Editor, i: usize, d: &ravenrs_widgets::Drag) {
    let Some(&(name, _, make)) = i.checked_sub(1).and_then(|i| TOOLS.get(i)) else { return };
    match d.phase {
        DragPhase::Start => {
            e.drag.set(Some(FieldDrag {
                path: name.to_owned(),
                tool: Some(i - 1),
                start: d.position,
                at: d.position,
            }));
        }
        DragPhase::Move => e.drag.update(|g| {
            if let Some(g) = g {
                g.at = d.position;
            }
        }),
        _ => {
            let Some(g) = e.drag.get() else { return };
            e.drag.set(None);
            if (g.at.x - g.start.x).hypot(g.at.y - g.start.y) <= 4.0 {
                return;
            }
            if let Some((band, at)) = e.band_at(d.position) {
                let mut c = make();
                c.x = editor::snap(at.x.max(0.0));
                c.y = editor::snap((at.y - c.height / 2.0).max(0.0));
                e.add_control(band, c);
                e.tab.set(0);
            }
        }
    }
}

fn insert_tool(e: &Editor, make: fn() -> Control) {
    let band = e.target_band();
    let mut c = make();
    let (x, y) = e.free_spot(band, c.height);
    c.x = x;
    c.y = y;
    e.add_control(band, c);
    e.tab.set(0);
}

struct LeftContent {
    editor: Editor,
    tab: State<usize>,
}

impl View for LeftContent {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.editor;
        let content: AnyView = match self.tab.get() {
            0 => Box::new(FieldList(e.clone())),
            1 => Box::new(BandList(e.clone())),
            _ => Box::new(groups::GroupPanel(e.clone())),
        };
        cx.child(&ScrollView::new((content,)).grow(1.0))
    }
}

/// The data source's fields; drag one onto a band, or click to add it to the selected band.
struct FieldList(Editor);

impl View for FieldList {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.0;
        let report = e.report.get();
        let source = report.data_source.clone().unwrap_or_else(|| "Dados".into());
        let data = report.embedded_data().cloned().unwrap_or_default();
        let mut sections: Vec<AnyView> = vec![Box::new(
            HStack::new((
                Icon::new(IconName::Database).size(14.0),
                Text::new(source).font_weight(Weight::SEMIBOLD),
            ))
            .spacing(SPACING.s2)
            .align(Align::Center),
        )];
        let fields = wizard::fields(&data, report.data_member.as_deref());
        let mut rows: Vec<AnyView> = Vec::new();
        for f in &fields {
            rows.push(Box::new(FieldItem { editor: e.clone(), field: f.clone(), collection: None }));
        }
        for f in report.calculated_fields.iter() {
            rows.push(Box::new(FieldItem {
                editor: e.clone(),
                field: FieldInfo { path: f.name.clone(), kind: FieldKind::Number },
                collection: None,
            }));
        }
        // Fields of nested collections (invoice items) bind inside their detail band.
        for f in fields.iter().filter(|f| f.kind == FieldKind::Collection) {
            let items = wizard::rows_of(&data, report.data_member.as_deref())
                .first()
                .and_then(|row| row.get(&f.path))
                .cloned()
                .unwrap_or_default();
            for sub in wizard::fields(&items, None) {
                rows.push(Box::new(FieldItem {
                    editor: e.clone(),
                    field: sub,
                    collection: Some(f.path.clone()),
                }));
            }
        }
        sections.push(Box::new(VStack::new(rows).spacing(SPACING.s1).align(Align::Stretch)));
        sections.push(Box::new(
            Text::new("Arraste um campo até uma banda, ou clique para inserir na banda selecionada.")
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
        ));
        cx.child(&VStack::new(sections).spacing(SPACING.s3).align(Align::Stretch))
    }
}

struct FieldItem {
    editor: Editor,
    field: FieldInfo,
    collection: Option<String>,
}

impl View for FieldItem {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let icon = match self.field.kind {
            FieldKind::Number => IconName::Sigma,
            FieldKind::Date => IconName::Calendar,
            FieldKind::Boolean => IconName::CircleCheck,
            FieldKind::Collection => IconName::List,
            _ => IconName::Type,
        };
        let label = match &self.collection {
            Some(c) => format!("{c} › {}", self.field.path),
            None => self.field.path.clone(),
        };
        let item = SidebarItem::new(icon, label).compact();
        if self.field.kind == FieldKind::Collection {
            return cx.child(&item);
        }
        let (kind, path, collection) = (self.field.kind, self.field.path.clone(), self.collection.clone());
        // A click, or Enter while focused: into the selected band, or the band that lists this collection.
        let dragged = Rc::new(std::cell::Cell::new(false));
        let (e, was_dragged) = (self.editor.clone(), dragged.clone());
        let add_path = path.clone();
        let item = item.on_click(move || {
            if was_dragged.replace(false) {
                return;
            }
            let band = match &collection {
                Some(c) => e
                    .report
                    .with(|r| r.detail_reports.iter().position(|d| &d.data_member == c))
                    .map_or(e.target_band(), BandRef::DetailReportDetail),
                None => e.target_band(),
            };
            let (x, y) = e.free_spot(band, 18.0);
            e.add_control(band, field_control(&add_path, kind).at(x, y, 140.0, 18.0));
            e.tab.set(0);
        });
        let row = cx.child(&item);
        let e = self.editor.clone();
        cx.on_drag(
            row,
            Rc::new(move |d| match d.phase {
                DragPhase::Start => e.drag.set(Some(FieldDrag {
                    path: path.clone(),
                    tool: None,
                    start: d.position,
                    at: d.position,
                })),
                DragPhase::Move => e.drag.update(|g| {
                    if let Some(g) = g {
                        g.at = d.position;
                    }
                }),
                _ => {
                    let Some(g) = e.drag.get() else { return };
                    e.drag.set(None);
                    if (g.at.x - g.start.x).hypot(g.at.y - g.start.y) <= 4.0 {
                        return;
                    }
                    // The pointer may come up on this same row: that is no click.
                    dragged.set(true);
                    if let Some((band, at)) = e.band_at(d.position) {
                        dragged.set(false);
                        let c = field_control(&path, kind).at(
                            editor::snap(at.x.max(0.0)),
                            editor::snap((at.y - 9.0).max(0.0)),
                            140.0,
                            18.0,
                        );
                        e.add_control(band, c);
                        e.tab.set(0);
                    }
                }
            }),
        );
        row
    }
}

/// A new field control with a sensible format for its kind.
fn field_control(path: &str, kind: FieldKind) -> Control {
    let c = Control::field(format!("[{path}]"));
    match kind {
        FieldKind::Date => c.format("dd/MM/yyyy"),
        FieldKind::Number
            if path.to_lowercase().contains("price") || path.to_lowercase().contains("total") =>
        {
            c.format("C2").style(Style::new().align(ravenrs_report::TextAlign::Right))
        }
        FieldKind::Number => c.style(Style::new().align(ravenrs_report::TextAlign::Right)),
        _ => c,
    }
}

/// The report explorer: bands in order, click to select.
struct BandList(Editor);

impl View for BandList {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.0;
        let report = e.report.get();
        let selection = e.selection.get();
        let items: Vec<AnyView> = BandRef::all(&report)
            .into_iter()
            .map(|b| {
                let count = b.get(&report).map_or(0, |band| band.controls.len());
                let selected = matches!(selection, Selection::Band(s) | Selection::Control(s, _) if s == b);
                let sel = e.selection.clone();
                Box::new(
                    SidebarItem::new(IconName::Rows3, b.title(&report))
                        .compact()
                        .count(count.to_string())
                        .selected(selected)
                        .on_click(move || sel.set(Selection::Band(b))),
                ) as AnyView
            })
            .collect();
        cx.child(&VStack::new(items).spacing(SPACING.s1).align(Align::Stretch))
    }
}

/// The floating chip that follows a dragged field.
struct FieldGhost(Editor);

impl View for FieldGhost {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let anchor = cx.empty();
        if let Some(g) = self.0.drag.get().filter(|g| (g.at.x - g.start.x).hypot(g.at.y - g.start.y) > 4.0) {
            let over = self.0.band_at(g.at).is_some();
            let label = if g.tool.is_some() { g.path.clone() } else { format!("[{}]", g.path) };
            let chip = Badge::new(label)
                .icon(if over { IconName::Plus } else { IconName::Database })
                .tone(if over { Tone::Accent } else { Tone::Neutral });
            cx.overlay(anchor, Placement::At(Point::new(g.at.x + 12.0, g.at.y + 8.0)), &chip, None);
        }
        anchor
    }
}

/// Undo, redo and delete from the keyboard.
struct Shortcuts {
    editor: Editor,
    cache: Rc<RefCell<RenderCache>>,
    params: State<HashMap<String, serde_json::Value>>,
}

impl View for Shortcuts {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.editor;
        let (u, r, d, b) = (e.clone(), e.clone(), e.clone(), e.clone());
        let (pe, pc, pp) = (e.clone(), self.cache.clone(), self.params.clone());
        cx.shortcut(Key::Character("p".into()), true, Rc::new(move || print_report(&pe, &pc, &pp)));
        cx.shortcut(Key::Character("z".into()), true, Rc::new(move || u.undo()));
        cx.shortcut(Key::Character("y".into()), true, Rc::new(move || r.redo()));
        cx.shortcut(Key::Delete, false, Rc::new(move || d.delete_selected()));
        cx.shortcut(Key::Backspace, false, Rc::new(move || b.delete_selected()));
        cx.empty()
    }
}

/// Design, preview and JSON views of the report.
struct CenterPane {
    editor: Editor,
    cache: Rc<RefCell<RenderCache>>,
    params: State<HashMap<String, serde_json::Value>>,
}

impl View for CenterPane {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.editor;
        let theme = *cx.theme();
        let view: AnyView = match e.tab.get() {
            1 => Box::new(PreviewPane {
                editor: e.clone(),
                cache: self.cache.clone(),
                params: self.params.clone(),
            }),
            2 => Box::new(JsonPane(e.clone())),
            _ => Box::new(ScrollView::new((surface::Surface(e.clone()),)).horizontal().grow(1.0)),
        };
        let content = cx.child(&view);
        let node = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                min_height: Some(0.0),
                overflow_hidden: true,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Canvas))),
        );
        // A click on the empty canvas selects the report itself.
        let sel = e.selection.clone();
        cx.on_click(node, Rc::new(move || sel.set(Selection::None)));
        let divider = cx.child(&Divider::horizontal());
        let tabs = cx.child(&view_tabs(e));
        cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                min_height: Some(0.0),
                overflow_hidden: true,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[node, divider, tabs],
            Paint::None,
        )
    }
}

struct PreviewPane {
    editor: Editor,
    cache: Rc<RefCell<RenderCache>>,
    params: State<HashMap<String, serde_json::Value>>,
}

impl View for PreviewPane {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.editor;
        let params = self.params.get();
        let doc = preview::document(e, &self.cache, &params);
        let state = cx.use_state(|| ViewerState::sharing(e.page.clone(), e.zoom.clone()));
        let report = e.report.get();
        let (pe, pc, pp) = (e.clone(), self.cache.clone(), self.params.clone());
        // The designer has its own print button, which renders the report as it stands.
        let viewer = DocumentViewer::new(doc, &state.get())
            .labels(ViewerLabels::pt_br())
            .print_button(false)
            .toolbar_trailing(move || {
                let print = (pe.clone(), pc.clone(), pp.clone());
                let mut items: Vec<AnyView> = vec![Box::new(
                    Button::new("Imprimir")
                        .icon(IconName::Printer)
                        .size(ButtonSize::Small)
                        .on_click(move || print_report(&print.0, &print.1, &print.2)),
                )];
                for (label, icon, format) in [
                    ("PDF", IconName::FileText, preview::Export::Pdf),
                    ("Excel", IconName::Table, preview::Export::Xlsx),
                ] {
                    let (e, c, p) = (pe.clone(), pc.clone(), pp.clone());
                    items.push(Box::new(
                        Button::new(label)
                            .icon(icon)
                            .size(ButtonSize::Small)
                            .on_click(move || export_report(&e, &c, &p, format)),
                    ));
                }
                items
            });
        let viewer = cx.child(&viewer);
        let mut children = Vec::new();
        if !report.parameters.is_empty() {
            let (year, month, day) = DateTime::now().ymd();
            if let Some(today) = Date::new(year, month as u8, day as u8) {
                let panel = ParameterPanel::new(report.parameters.clone(), self.params.clone(), today)
                    .labels(ParameterLabels::pt_br());
                children.push(cx.child(&panel));
                children.push(cx.child(&Divider::horizontal()));
            }
        }
        children.push(viewer);
        cx.container(
            LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                overflow_hidden: true,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        )
    }
}

/// The report definition as JSON, with long sample data abbreviated.
struct JsonPane(Editor);

impl View for JsonPane {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let report = self.0.report.get();
        let full = report.to_json();
        let mut shown = report.clone();
        for s in &mut shown.data_sources {
            if let DataSourceKind::Json { data: serde_json::Value::Array(rows) } = &mut s.kind
                && rows.len() > 2
            {
                let total = rows.len();
                rows.truncate(2);
                rows.push(serde_json::json!(format!("… mais {} linhas", total - 2)));
            }
        }
        let clipboard = cx.clipboard();
        let copied = cx.use_state(|| false);
        let c2 = copied.clone();
        let lines: Vec<AnyView> = shown
            .to_json()
            .lines()
            .map(|l| Box::new(Text::new(l.replace(' ', "\u{a0}")).style(TextStyle::Callout)) as AnyView)
            .collect();
        cx.child(
            &VStack::new((
                HStack::new((
                    VStack::new((
                        Text::new("Definição do relatório").style(TextStyle::Headline),
                        Text::new("O layout e as fontes de dados ficam em um único arquivo .rvreport.json. O servidor de relatórios usa o mesmo arquivo.")
                            .style(TextStyle::Callout)
                            .foreground(Semantic::InkMuted),
                    ))
                    .spacing(SPACING.s1)
                    .grow(1.0),
                    Button::new(if copied.get() { "Copiado" } else { "Copiar JSON" })
                        .icon(if copied.get() { IconName::Check } else { IconName::Copy })
                        .style(ButtonStyle::Secondary)
                        .on_click(move || {
                            clipboard.write(&full);
                            c2.set(true);
                        }),
                ))
                .align(Align::Center)
                .spacing(SPACING.s3)
                .padding(Insets::symmetric(SPACING.s3, SPACING.s5)),
                ScrollView::new((VStack::new(lines).padding(Insets::symmetric(SPACING.s2, SPACING.s5)),)).horizontal().grow(1.0),
            ))
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}

struct StatusBar {
    /// Whether it offers the themes.
    themes: bool,
    editor: Editor,
    cache: Rc<RefCell<RenderCache>>,
}

impl View for StatusBar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.editor;
        let report = e.report.get();
        let cache = self.cache.borrow();
        let (page, render) = match (&cache.doc, e.tab.get()) {
            (Some(doc), 1) => {
                let pages = doc.pages.len().max(1);
                let current = e.page.get().min(pages - 1) + 1;
                let millis = format!("{:.1}", cache.millis).replace('.', ",");
                (Some(format!("Página {current} de {pages}")), format!("Gerado em {millis} ms"))
            }
            _ => (None, "Abra Visualizar para gerar o documento".to_owned()),
        };
        let rows =
            report.embedded_data().map_or(0, |d| wizard::rows_of(d, report.data_member.as_deref()).len());
        let paper = if report.page.width > report.page.height { "paisagem" } else { "retrato" };
        drop(cache);
        let muted = |text: String| Text::new(text).style(TextStyle::Callout).foreground(Semantic::InkMuted);
        cx.child(
            &HStack::new((
                muted(format!("{} · {} linhas de dados · {} · {}", report.name, rows, report.culture, paper)),
                Spacer::new(),
                page.map(muted),
                muted(render),
                ZoomBar(e.clone()),
                self.themes.then(ThemePicker::new),
            ))
            .spacing(SPACING.s3)
            .align(Align::Center)
            .padding(Insets::symmetric(SPACING.s1, SPACING.s4))
            .background(Semantic::Surface),
        )
    }
}

/// The zoom at the right of the status bar: out, a slider, in and the percentage.
struct ZoomBar(Editor);

impl View for ZoomBar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let zoom = &self.0.zoom;
        let (out, into) = (zoom.clone(), zoom.clone());
        cx.child(
            &HStack::new((
                IconButton::new(IconName::Minus)
                    .label("Afastar")
                    .on_click(move || out.update(|z| *z = (*z - 0.25).max(ZOOM_LEAST))),
                Slider::new(zoom.binding())
                    .range(ZOOM_LEAST, ZOOM_MOST)
                    .step(0.05)
                    .width(120.0)
                    .label("Zoom"),
                IconButton::new(IconName::Plus)
                    .label("Aproximar")
                    .on_click(move || into.update(|z| *z = (*z + 0.25).min(ZOOM_MOST))),
                Text::new(format!("{:.0}%", zoom.get() * 100.0))
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
            ))
            .spacing(SPACING.s1)
            .align(Align::Center),
        )
    }
}

/// The expression editor, opened from an expression's button: the expression with
/// the report's fields, calculated fields and parameters, applied or dropped as a whole.
struct ExpressionDialog(Editor);

impl View for ExpressionDialog {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.0;
        let report = e.report.get();
        let data = report.embedded_data().cloned().unwrap_or_default();
        let kind = |k: FieldKind| match k {
            FieldKind::Number => FilterFieldKind::Number,
            FieldKind::Date => FilterFieldKind::Date,
            FieldKind::Boolean => FilterFieldKind::Boolean,
            _ => FilterFieldKind::Text,
        };
        let mut fields: Vec<FilterField> = wizard::fields(&data, report.data_member.as_deref())
            .into_iter()
            .filter(|f| f.kind != FieldKind::Collection)
            .map(|f| FilterField::new(f.path.clone(), f.path.clone(), kind(f.kind)))
            .collect();
        fields.extend(
            report
                .calculated_fields
                .iter()
                .map(|f| FilterField::new(f.name.clone(), f.name.clone(), FilterFieldKind::Number)),
        );
        let parameters = report.parameters.iter().map(|p| {
            let k = match p.kind {
                ravenrs_report::ParameterKind::Number => FilterFieldKind::Number,
                ravenrs_report::ParameterKind::Date => FilterFieldKind::Date,
                ravenrs_report::ParameterKind::Boolean => FilterFieldKind::Boolean,
                _ => FilterFieldKind::Text,
            };
            FilterField::new(p.name.clone(), p.label.clone().unwrap_or_else(|| p.name.clone()), k)
        });
        let (read, write) = (e.expression.clone(), e.expression.clone());
        let open = Binding::new(
            move || read.get().is_some(),
            move |on: bool| {
                if !on {
                    write.set(None)
                }
            },
        );
        let apply = e.clone();
        cx.child(
            &Dialog::new(open, "Editor de expressões")
                .description("Clique num item para ver o que ele faz; clique de novo, ou em Inserir, para colocá-lo no cursor.")
                .width(860.0)
                .close_label("Cancelar")
                .content(Localized::new(
                    Labels::pt_br(),
                    ExpressionEditor::new(e.expression_draft.binding()).fields(fields).parameters(parameters).rows(6),
                ))
                .action(Button::new("Aplicar").icon(IconName::Check).on_click(move || apply.apply_expression())),
        )
    }
}

/// The template gallery shown at start, as commercial designers do.
struct Gallery {
    editor: Editor,
}

fn template_card<F: Fn() + 'static>(
    title: &str,
    text: &str,
    icon: IconName,
    on_click: F,
) -> impl View + use<F> {
    Card::new(
        VStack::new((
            Icon::new(icon).size(22.0).color(Semantic::Accent),
            Text::new(title).style(TextStyle::Headline),
            Text::new(text).style(TextStyle::Callout).foreground(Semantic::InkMuted),
        ))
        .spacing(SPACING.s2)
        .align(Align::Stretch),
    )
    .corner_radius(RADII.lg)
    .padding(SPACING.s4)
    .no_shadow()
    .fill(Semantic::Surface)
    .on_click(on_click)
}

impl View for Gallery {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.editor;
        let open = |report: fn() -> Report, tab: usize| {
            let e = e.clone();
            move || {
                e.load(report());
                e.tab.set(tab);
                e.gallery_open.set(false);
            }
        };
        let wizard = {
            let e = e.clone();
            move || {
                e.gallery_open.set(false);
                e.wizard_open.set(true);
            }
        };
        let blank = {
            let e = e.clone();
            move || {
                let data = samples::sales_data(300);
                let r = Report::new("Novo relatório")
                    .culture("pt-BR")
                    .data_source(DataSource { name: "Vendas".into(), kind: DataSourceKind::Json { data } })
                    .calculated("Total", "[Quantity] * [UnitPrice]")
                    .report_header(ravenrs_report::Band::new(40.0))
                    .detail(ravenrs_report::Band::new(20.0))
                    .page_footer(ravenrs_report::Band::new(20.0));
                e.load(r);
                e.tab.set(0);
                e.gallery_open.set(false);
            }
        };
        let cards = Grid::new(
            3,
            (
                template_card(
                    "Fatura",
                    "Emitente, cliente, itens (mestre-detalhe), totais e observações.",
                    IconName::FileText,
                    open(samples::invoice, 1),
                ),
                template_card(
                    "Vendas por região",
                    "Grupos aninhados, subtotais, zebra, destaque condicional e parâmetro de valor mínimo.",
                    IconName::ChartBar,
                    open(|| samples::sales_by_region(300), 1),
                ),
                template_card(
                    "Vendas · 10.000 linhas",
                    "O mesmo relatório com 10 mil pedidos: veja o tempo de geração.",
                    IconName::Zap,
                    open(|| samples::sales_by_region(10_000), 1),
                ),
                template_card(
                    "Painel de vendas",
                    "Gráficos de área, barras e rosca sobre os mesmos pedidos.",
                    IconName::ChartColumn,
                    open(samples::sales_dashboard, 1),
                ),
                template_card(
                    "Lista de clientes",
                    "Agrupada pela inicial, com caixa de seleção.",
                    IconName::Users,
                    open(samples::customer_list, 1),
                ),
                template_card(
                    "Assistente",
                    "Escolha os campos, o agrupamento e os totais: o relatório fica pronto.",
                    IconName::Sparkles,
                    wizard,
                ),
                template_card(
                    "Em branco",
                    "Comece do zero e arraste os campos para as bandas.",
                    IconName::FilePlus,
                    blank,
                ),
            ),
        )
        .spacing(SPACING.s4);
        cx.child(
            &Dialog::new(e.gallery_open.binding(), "Novo relatório")
                .description("Comece por um modelo pronto ou deixe o assistente montar o relatório a partir dos seus dados.")
                .width(880.0)
                .close_label("Fechar")
                .content(cards),
        )
    }
}

/// The report wizard dialog.
struct WizardDialog {
    editor: Editor,
    state: WizardState,
}

impl View for WizardDialog {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.editor;
        let s = &self.state;
        // A few rows are enough to list the fields; the data is generated on demand.
        let fields = wizard::fields(&samples::sales_data(5), None);
        let chosen = s.chosen.get();
        let checks: Vec<AnyView> = fields
            .iter()
            .map(|f| {
                let (read, write, path) = (s.chosen.clone(), s.chosen.clone(), f.path.clone());
                let p2 = path.clone();
                Box::new(Checkbox::new(
                    wizard::caption(&f.path),
                    Binding::new(
                        move || read.with(|c| c.contains(&path)),
                        move |on: bool| {
                            write.update(|c| {
                                c.retain(|x| x != &p2);
                                if on {
                                    c.push(p2.clone());
                                }
                            })
                        },
                    ),
                )) as AnyView
            })
            .collect();
        let groups: Vec<String> = std::iter::once("Sem agrupamento".to_owned())
            .chain(fields.iter().filter(|f| f.kind == FieldKind::Text).map(|f| wizard::caption(&f.path)))
            .collect();
        let text_fields: Vec<String> =
            fields.iter().filter(|f| f.kind == FieldKind::Text).map(|f| f.path.clone()).collect();
        let (e2, st, all_fields) = (e.clone(), s.clone(), fields.clone());
        let generate = Button::new("Gerar relatório").icon(IconName::Sparkles).on_click(move || {
            let (name, rows) = WIZARD_SOURCES[st.source.get().min(WIZARD_SOURCES.len() - 1)];
            let data = samples::sales_data(rows);
            let chosen = st.chosen.get();
            let columns: Vec<FieldInfo> =
                chosen.iter().filter_map(|p| all_fields.iter().find(|f| &f.path == p).cloned()).collect();
            let group_by = st.group.get().checked_sub(1).and_then(|i| text_fields.get(i).cloned());
            let totals = if st.totals.get() {
                columns.iter().filter(|c| c.kind == FieldKind::Number).map(|c| c.path.clone()).collect()
            } else {
                Vec::new()
            };
            let options = WizardOptions {
                title: st.title.get(),
                culture: "pt-BR".into(),
                columns,
                group_by,
                totals,
                ..WizardOptions::default()
            };
            let report = wizard::table_report(
                DataSource { name: name.into(), kind: DataSourceKind::Json { data } },
                &options,
            );
            e2.load(report);
            e2.tab.set(1);
            e2.wizard_open.set(false);
        });
        let content = VStack::new((
            FormField::new("Título", TextField::new(s.title.binding())),
            FormField::new(
                "Dados",
                Select::new(s.source.binding(), WIZARD_SOURCES.iter().map(|(n, _)| *n))
                    .width(300.0)
                    .label("Dados"),
            ),
            Text::new(format!("Campos ({} escolhidos)", chosen.len()))
                .style(TextStyle::Callout)
                .font_weight(Weight::SEMIBOLD),
            Grid::new(3, checks).spacing(SPACING.s2),
            FormField::new(
                "Agrupar por",
                Select::new(s.group.binding(), groups).width(300.0).label("Agrupar por"),
            ),
            Toggle::new(s.totals.binding()).label("Totais dos campos numéricos"),
        ))
        .spacing(SPACING.s4)
        .align(Align::Stretch);
        cx.child(
            &Dialog::new(e.wizard_open.binding(), "Assistente de relatório")
                .description("Escolha os campos e o agrupamento. O resto (cabeçalhos, zebra, totais, numeração de páginas) o assistente faz.")
                .close_label("Fechar")
                .content(content)
                .action(generate),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_menu_is_offered_alone_and_inside_an_application_unless_it_says_no() {
        assert!(Designer::new().theme_menu);
        let report = || samples::sales_by_region(3);
        assert!(Designer::embedded(report(), "Salvar", "Fechar", |_| {}, || {}).theme_menu);
        let kept = Designer::embedded(report(), "Salvar", "Fechar", |_| {}, || {}).theme_menu(false);
        assert!(!kept.theme_menu);
    }

    #[test]
    fn several_controls_line_up_and_size_to_the_first_one_picked_and_go_together() {
        // #46: Shift- or Cmd-click picks more controls of the band; the Ribbon lines them up.
        let label = |x: f32, y: f32, w: f32, h: f32| Control::label("Texto").at(x, y, w, h);
        let report = Report::new("Teste").detail(
            ravenrs_report::Band::new(80.0)
                .control(label(10.0, 4.0, 100.0, 18.0))
                .control(label(150.0, 30.0, 60.0, 24.0))
                .control(label(260.0, 12.0, 80.0, 18.0)),
        );
        let e = Editor::new(report);
        let boxes = |e: &Editor| {
            e.report
                .with(|r| r.detail.controls.iter().map(|c| (c.x, c.y, c.width, c.height)).collect::<Vec<_>>())
        };
        e.select_control(BandRef::Detail, 0);
        assert!(!e.several_selected());
        e.toggle_selected(BandRef::Detail, 1);
        e.toggle_selected(BandRef::Detail, 2);
        assert!(e.several_selected());
        assert_eq!(e.selected_indices(), Some((BandRef::Detail, vec![0, 1, 2])));
        e.toggle_selected(BandRef::Detail, 2);
        assert_eq!(e.selected_indices(), Some((BandRef::Detail, vec![0, 1])), "a second click takes it out");

        e.arrange_selected(editor::Arrange::Left);
        e.arrange_selected(editor::Arrange::Top);
        e.arrange_selected(editor::Arrange::SameWidth);
        assert_eq!(boxes(&e)[1], (10.0, 4.0, 100.0, 24.0), "to the first one picked");
        assert_eq!(boxes(&e)[2], (260.0, 12.0, 80.0, 18.0), "the one taken out stays");
        e.arrange_selected(editor::Arrange::Bottom);
        assert_eq!(boxes(&e)[1].1, 4.0 + 18.0 - 24.0, "bottoms lined up");
        e.undo();
        e.undo();
        assert_eq!(boxes(&e)[1], (10.0, 4.0, 60.0, 24.0), "each one undoes on its own");

        // A plain click on one already picked keeps the others, as a drag of the group would; on
        // another, it picks that one alone.
        e.toggle_selected(BandRef::Detail, 1);
        e.select_control(BandRef::Detail, 0);
        assert_eq!(e.selected_indices(), Some((BandRef::Detail, vec![0, 1])));
        e.select_control(BandRef::Detail, 2);
        assert_eq!(e.selected_indices(), Some((BandRef::Detail, vec![2])));
        e.toggle_selected(BandRef::Detail, 0);
        e.delete_selected();
        assert_eq!(e.report.with(|r| r.detail.controls.len()), 1, "both go together, the third stays");
        assert!(!e.several_selected());
    }
}
