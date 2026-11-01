//! RavenRS Report Designer: an end-user report designer running on the
//! desktop and in the browser. Pick a template or run the wizard, drag fields
//! onto bands, edit properties, and preview the paginated result instantly.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ravenrs::core::{Border, Key};
use ravenrs::layout::{Align, Axis, LayoutStyle};
use ravenrs::prelude::*;
use ravenrs::widgets::{DragPhase, Paint, Placement, QuadPaint};
use ravenrs_report::wizard::{self, FieldInfo, FieldKind, WizardOptions};
use ravenrs_report::{Control, DataSource, DataSourceKind, Line, Report, Style, Table, TableRow, samples};

mod bands;
mod editor;
mod preview;
mod props;
mod surface;

use bands::BandRef;
use editor::{Editor, FieldDrag, Selection};
use preview::RenderCache;

/// The designer app.
pub struct Designer {
    editor: Editor,
    cache: Rc<RefCell<RenderCache>>,
    params: State<HashMap<String, String>>,
    left_tab: State<usize>,
    wizard: WizardState,
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

impl Designer {
    /// A designer showing the sales report, with the template gallery open.
    pub fn new() -> Self {
        Self {
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

/// The app, for the native runner and the web entry point.
pub fn app() -> impl View {
    Designer::new()
}

/// The data sets the wizard offers, with their row counts.
const WIZARD_SOURCES: [(&str, usize); 2] =
    [("Vendas (300 pedidos)", 300), ("Vendas (10.000 pedidos)", 10_000)];

impl Component for Designer {
    fn body(&self) -> impl View {
        let e = self.editor.clone();
        VStack::new((
            self.ribbon(),
            FillRow((
                self.left_panel(),
                Divider::vertical(),
                CenterPane { editor: e.clone(), cache: self.cache.clone(), params: self.params.clone() },
                Divider::vertical(),
                VStack::new((props::Properties(e.clone()),)).width(320.0).align(Align::Stretch),
            )),
            StatusBar { editor: e.clone(), cache: self.cache.clone() },
            Gallery { editor: e.clone() },
            WizardDialog { editor: e.clone(), state: self.wizard.clone() },
            FieldGhost(e.clone()),
            Shortcuts { editor: e, cache: self.cache.clone(), params: self.params.clone() },
        ))
        .spacing(0.0)
        .align(Align::Stretch)
        .grow(1.0)
    }
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

/// Renders the report as it stands and sends it to the printer.
fn print_report(editor: &Editor, cache: &Rc<RefCell<RenderCache>>, params: &State<HashMap<String, String>>) {
    let doc = preview::document(editor, cache, &params.get());
    preview::print(&doc);
}

impl Designer {
    fn ribbon(&self) -> impl View + use<> {
        let e = self.editor.clone();
        let printer = |item: RibbonItem| {
            let (ed, cache, params) = (e.clone(), self.cache.clone(), self.params.clone());
            item.on_click(move || print_report(&ed, &cache, &params))
        };
        let (pm_e, pm_c, pm_p) = (e.clone(), self.cache.clone(), self.params.clone());
        let insert = |label: &'static str, icon: IconName, make: fn() -> Control| {
            let e = e.clone();
            RibbonItem::button(label, icon).on_click(move || {
                let band = e.target_band();
                let mut c = make();
                let (x, y) = e.free_spot(band, c.height);
                c.x = x;
                c.y = y;
                e.add_control(band, c);
                e.tab.set(0);
            })
        };
        let (e1, e2, e3, e4, e5, e6) = (e.clone(), e.clone(), e.clone(), e.clone(), e.clone(), e.clone());
        let (z1, z2) = (e.clone(), e.clone());
        let (t0, t1, t2) = (e.clone(), e.clone(), e.clone());
        let tab_toggle = |i: usize, label: &'static str, icon: IconName, ed: Editor| {
            let (read, write) = (ed.tab.clone(), ed.tab.clone());
            RibbonItem::toggle(label, icon, Binding::new(move || read.get() == i, move |_| write.set(i)))
                .large()
        };
        Ribbon::new()
            .app_menu(
                "Arquivo",
                vec![
                    MenuItem::new("Novo a partir de modelo…")
                        .icon(IconName::FilePlus)
                        .on_select(move || e1.gallery_open.set(true)),
                    MenuItem::new("Assistente de relatório…")
                        .icon(IconName::Sparkles)
                        .on_select(move || e2.wizard_open.set(true)),
                    MenuItem::new("Imprimir…")
                        .icon(IconName::Printer)
                        .shortcut("⌘P")
                        .on_select(move || print_report(&pm_e, &pm_c, &pm_p)),
                    MenuItem::new("Ver o JSON do relatório")
                        .icon(IconName::Code)
                        .on_select(move || e3.tab.set(2)),
                ],
            )
            .quick_access(RibbonItem::button("Desfazer", IconName::Undo2).on_click(move || e4.undo()))
            .quick_access(RibbonItem::button("Refazer", IconName::Redo2).on_click(move || e5.redo()))
            .quick_access(printer(RibbonItem::button("Imprimir", IconName::Printer)))
            .page(
                RibbonPage::new("Início")
                    .group(
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
                    .group(
                        RibbonGroup::new("Exibir")
                            .item(tab_toggle(0, "Design", IconName::Pencil, t0))
                            .item(tab_toggle(1, "Visualizar", IconName::Eye, t1))
                            .item(tab_toggle(2, "JSON", IconName::Code, t2)),
                    )
                    .group(
                        RibbonGroup::new("Saída")
                            .item(printer(RibbonItem::button("Imprimir", IconName::Printer).large())),
                    )
                    .group(
                        RibbonGroup::new("Zoom")
                            .item(
                                RibbonItem::button("Aproximar", IconName::ZoomIn)
                                    .small()
                                    .on_click(move || z1.zoom.update(|z| *z = (*z + 0.25).min(3.0))),
                            )
                            .item(
                                RibbonItem::button("Afastar", IconName::ZoomOut)
                                    .small()
                                    .on_click(move || z2.zoom.update(|z| *z = (*z - 0.25).max(0.5))),
                            ),
                    ),
            )
            .page(
                RibbonPage::new("Inserir").group(
                    RibbonGroup::new("Controles")
                        .item(
                            insert("Rótulo", IconName::Type, || {
                                Control::label("Texto").at(0.0, 0.0, 140.0, 18.0)
                            })
                            .large(),
                        )
                        .item(
                            insert("Campo", IconName::Database, || {
                                Control::field("[Customer]").at(0.0, 0.0, 140.0, 18.0)
                            })
                            .large(),
                        )
                        .item(
                            insert("Tabela", IconName::Table, || {
                                Control::table(
                                    Table::new().row(
                                        TableRow::new(18.0)
                                            .text(120.0, "Coluna 1")
                                            .text(120.0, "Coluna 2")
                                            .text(120.0, "Coluna 3"),
                                    ),
                                )
                            })
                            .large(),
                        )
                        .item(
                            insert("Linha", IconName::Minus, || Control {
                                kind: ravenrs_report::ControlKind::Line(Line::default()),
                                ..Control::line().at(0.0, 0.0, 200.0, 4.0)
                            })
                            .small(),
                        )
                        .item(
                            insert("Retângulo", IconName::Square, || {
                                Control::rect(4.0)
                                    .at(0.0, 0.0, 120.0, 40.0)
                                    .style(Style::new().background("#f4f6fb").border("all", 0.75, "#9aa1b2"))
                            })
                            .small(),
                        )
                        .item(
                            insert("Caixa de seleção", IconName::CircleCheck, || {
                                Control::check_box("[Paid]", "Pago").at(0.0, 0.0, 100.0, 18.0)
                            })
                            .small(),
                        )
                        .item(
                            insert("Total", IconName::Sigma, || {
                                Control::field("Sum([Total])")
                                    .format("C2")
                                    .at(0.0, 0.0, 140.0, 18.0)
                                    .style(Style::new().bold())
                            })
                            .small(),
                        )
                        .item(
                            insert("Página N de M", IconName::FileText, || {
                                Control::field("FormatString('Página {0} de {1}', PageNumber(), PageCount())")
                                    .at(0.0, 0.0, 160.0, 16.0)
                            })
                            .small(),
                        ),
                ),
            )
    }

    fn left_panel(&self) -> impl View + use<> {
        let e = self.editor.clone();
        VStack::new((
            SegmentedControl::new(self.left_tab.binding(), ["Campos", "Bandas"]).label("Painel"),
            LeftContent { editor: e, tab: self.left_tab.clone() },
        ))
        .spacing(10.0)
        .padding(12.0)
        .width(250.0)
        .align(Align::Stretch)
    }
}

struct LeftContent {
    editor: Editor,
    tab: State<usize>,
}

impl View for LeftContent {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.editor;
        let content: AnyView =
            if self.tab.get() == 0 { Box::new(FieldList(e.clone())) } else { Box::new(BandList(e.clone())) };
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
            .spacing(6.0)
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
        sections.push(Box::new(VStack::new(rows).spacing(2.0).align(Align::Stretch)));
        sections.push(Box::new(
            Text::new("Arraste um campo até uma banda, ou clique para inserir na banda selecionada.")
                .style(TextStyle::Caption)
                .foreground(Semantic::InkMuted),
        ));
        cx.child(&VStack::new(sections).spacing(10.0).align(Align::Stretch))
    }
}

struct FieldItem {
    editor: Editor,
    field: FieldInfo,
    collection: Option<String>,
}

impl View for FieldItem {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let hovered = cx.is_hovered();
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
        let row = cx.child(
            &HStack::new((
                Icon::new(icon).size(14.0).color(Semantic::Accent),
                Text::new(label).style(TextStyle::Callout),
            ))
            .spacing(8.0)
            .align(Align::Center)
            .padding(Insets::symmetric(5.0, 8.0))
            .background(if hovered {
                theme.color(Semantic::Ink).with_alpha(0.06)
            } else {
                Color::TRANSPARENT
            })
            .corner_radius(6.0)
            .stretch(),
        );
        if self.field.kind == FieldKind::Collection {
            return row;
        }
        let (e, path, kind, collection) =
            (self.editor.clone(), self.field.path.clone(), self.field.kind, self.collection.clone());
        cx.on_drag(
            row,
            Rc::new(move |d| match d.phase {
                DragPhase::Start => {
                    e.drag.set(Some(FieldDrag { path: path.clone(), start: d.position, at: d.position }))
                }
                DragPhase::Move => e.drag.update(|g| {
                    if let Some(g) = g {
                        g.at = d.position;
                    }
                }),
                _ => {
                    let Some(g) = e.drag.get() else { return };
                    e.drag.set(None);
                    let moved = (g.at.x - g.start.x).hypot(g.at.y - g.start.y) > 4.0;
                    let control = field_control(&path, kind);
                    if moved {
                        if let Some((band, at)) = e.band_at(d.position) {
                            let c = control.at(
                                editor::snap(at.x.max(0.0)),
                                editor::snap((at.y - 9.0).max(0.0)),
                                140.0,
                                18.0,
                            );
                            e.add_control(band, c);
                        }
                    } else {
                        // A click: into the selected band, or the band that lists this collection.
                        let band = match &collection {
                            Some(c) => e
                                .report
                                .with(|r| r.detail_reports.iter().position(|d| &d.data_member == c))
                                .map_or(e.target_band(), BandRef::DetailReportDetail),
                            None => e.target_band(),
                        };
                        let (x, y) = e.free_spot(band, 18.0);
                        e.add_control(band, control.at(x, y, 140.0, 18.0));
                    }
                    e.tab.set(0);
                }
            }),
        );
        cx.cursor(row, CursorIcon::Pointer);
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
                        .count(count.to_string())
                        .selected(selected)
                        .on_click(move || sel.set(Selection::Band(b))),
                ) as AnyView
            })
            .collect();
        cx.child(&VStack::new(items).spacing(2.0).align(Align::Stretch))
    }
}

/// The floating chip that follows a dragged field.
struct FieldGhost(Editor);

impl View for FieldGhost {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let anchor = cx.empty();
        if let Some(g) = self.0.drag.get().filter(|g| (g.at.x - g.start.x).hypot(g.at.y - g.start.y) > 4.0) {
            let over = self.0.band_at(g.at).is_some();
            let chip = HStack::new((
                Icon::new(if over { IconName::Plus } else { IconName::Database })
                    .size(13.0)
                    .color(Color::WHITE),
                Text::new(format!("[{}]", g.path))
                    .style(TextStyle::Callout)
                    .font_weight(Weight::SEMIBOLD)
                    .foreground(Color::WHITE),
            ))
            .spacing(6.0)
            .align(Align::Center)
            .padding(Insets::symmetric(5.0, 10.0))
            .background(if over {
                Color::rgba(0.298, 0.4, 0.933, 1.0)
            } else {
                Color::rgba(0.2, 0.22, 0.28, 0.9)
            })
            .corner_radius(8.0);
            cx.overlay(anchor, Placement::At(Point::new(g.at.x + 12.0, g.at.y + 8.0)), &chip, None);
        }
        anchor
    }
}

/// Undo, redo and delete from the keyboard.
struct Shortcuts {
    editor: Editor,
    cache: Rc<RefCell<RenderCache>>,
    params: State<HashMap<String, String>>,
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
    params: State<HashMap<String, String>>,
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
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Canvas).lerp(theme.color(Semantic::Ink), 0.04))),
        );
        // A click on the empty canvas selects the report itself.
        let sel = e.selection.clone();
        cx.on_click(node, Rc::new(move || sel.set(Selection::None)));
        node
    }
}

struct PreviewPane {
    editor: Editor,
    cache: Rc<RefCell<RenderCache>>,
    params: State<HashMap<String, String>>,
}

impl View for PreviewPane {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.editor;
        let params = self.params.get();
        let doc = preview::document(e, &self.cache, &params);
        let pages = doc.pages.len().max(1);
        let current = e.page.get().min(pages - 1);
        let zoom = e.zoom.get();
        let report = e.report.get();
        let mut bar: Vec<AnyView> = Vec::new();
        let nav = |label: &'static str, icon: IconName, to: usize, enabled: bool, page: State<usize>| {
            IconButton::new(icon).label(label).on_click(move || {
                if enabled {
                    page.set(to)
                }
            })
        };
        bar.push(Box::new(nav("Primeira página", IconName::ChevronLeft, 0, current > 0, e.page.clone())));
        bar.push(Box::new(
            Text::new(format!("Página {} de {}", current + 1, pages)).font_weight(Weight::SEMIBOLD),
        ));
        bar.push(Box::new(nav(
            "Próxima página",
            IconName::ChevronRight,
            (current + 1).min(pages - 1),
            current + 1 < pages,
            e.page.clone(),
        )));
        bar.push(Box::new(Spacer::new()));
        let (pe, pc, pp) = (e.clone(), self.cache.clone(), self.params.clone());
        let print_button = Button::new("Imprimir")
            .icon(IconName::Printer)
            .style(ButtonStyle::Secondary)
            .size(ButtonSize::Small)
            .on_click(move || print_report(&pe, &pc, &pp));
        for p in &report.parameters {
            let name = p.name.clone();
            let (read, write) = (self.params.clone(), self.params.clone());
            let n2 = name.clone();
            let default = p.default.as_ref().map(|d| d.to_string()).unwrap_or_default();
            let field = TextField::new(Binding::new(
                move || read.with(|m| m.get(&name).cloned()).unwrap_or_else(|| default.clone()),
                move |s: String| {
                    write.update(|m| {
                        m.insert(n2.clone(), s);
                    })
                },
            ))
            .placeholder(format!("Parâmetro {}", p.name))
            .compact()
            .width(110.0);
            bar.push(Box::new(
                HStack::new((Text::new(format!("?{}", p.name)).style(TextStyle::Callout), field))
                    .spacing(6.0)
                    .align(Align::Center),
            ));
        }
        bar.push(Box::new(print_button));
        let toolbar = cx.child(
            &HStack::new(bar).spacing(10.0).align(Align::Center).padding(Insets::symmetric(8.0, 16.0)),
        );
        let page_views: Vec<AnyView> = doc
            .pages
            .get(current)
            .map(|p| {
                let scene = preview::page_scene(e, &self.cache, &doc, current, zoom);
                // Screen readers get the page's text; everyone else sees the drawing.
                let text = doc.page_text(current).join(" ");
                vec![Box::new(PageView {
                    canvas: preview::page_view(scene, p.width * zoom, p.height * zoom),
                    text,
                }) as AnyView]
            })
            .unwrap_or_default();
        let errors: Vec<AnyView> = doc
            .errors
            .iter()
            .take(3)
            .map(|m| Box::new(Alert::new("Erro no relatório", m.clone()).tone(Tone::Danger)) as AnyView)
            .collect();
        let body = cx.child(
            &ScrollView::new((VStack::new((VStack::new(errors).spacing(8.0), VStack::new(page_views)))
                .spacing(12.0)
                .padding(24.0)
                .align(Align::Center),))
            .horizontal()
            .grow(1.0),
        );
        // Page Up and Page Down turn pages.
        let (up, down) = (e.page.clone(), e.page.clone());
        cx.shortcut(Key::PageUp, false, Rc::new(move || up.update(|p| *p = p.saturating_sub(1))));
        cx.shortcut(Key::PageDown, false, Rc::new(move || down.update(|p| *p = (*p + 1).min(pages - 1))));
        cx.container(
            LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                overflow_hidden: true,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[toolbar, body],
            Paint::None,
        )
    }
}

/// A rendered page, described to assistive technology by its text.
struct PageView {
    canvas: Canvas,
    text: String,
}

impl View for PageView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let canvas = cx.child(&self.canvas);
        let node = cx.container(LayoutStyle::stack(Axis::Vertical), &[canvas], Paint::None);
        cx.semantics(node, ravenrs::widgets::Semantics::new(Role::Image).name(self.text.clone()));
        node
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
                    .spacing(4.0)
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
                .spacing(12.0)
                .padding(Insets::symmetric(12.0, 20.0)),
                ScrollView::new((VStack::new(lines).padding(Insets::symmetric(8.0, 20.0)),)).horizontal().grow(1.0),
            ))
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}

struct StatusBar {
    editor: Editor,
    cache: Rc<RefCell<RenderCache>>,
}

impl View for StatusBar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.editor;
        let report = e.report.get();
        let cache = self.cache.borrow();
        let render = match (&cache.doc, e.tab.get()) {
            (Some(doc), 1) => format!("{} páginas geradas em {:.1} ms", doc.pages.len(), cache.millis),
            _ => "Abra Visualizar para gerar o documento".to_owned(),
        };
        let rows =
            report.embedded_data().map_or(0, |d| wizard::rows_of(d, report.data_member.as_deref()).len());
        let paper = if report.page.width > report.page.height { "paisagem" } else { "retrato" };
        drop(cache);
        cx.child(
            &HStack::new((
                Icon::new(IconName::FileText).size(13.0).color(Semantic::InkMuted),
                Text::new(format!(
                    "{} · {} linhas de dados · {} · {}",
                    report.name, rows, report.culture, paper
                ))
                .style(TextStyle::Caption)
                .foreground(Semantic::InkMuted),
                Spacer::new(),
                Text::new(render).style(TextStyle::Caption).foreground(Semantic::InkMuted),
                Text::new(format!("Zoom {:.0}%", e.zoom.get() * 100.0))
                    .style(TextStyle::Caption)
                    .foreground(Semantic::InkMuted),
            ))
            .spacing(10.0)
            .align(Align::Center)
            .padding(Insets::symmetric(6.0, 14.0)),
        )
    }
}

/// The template gallery shown at start, as commercial designers do.
struct Gallery {
    editor: Editor,
}

impl View for Gallery {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let anchor = cx.empty();
        if self.editor.gallery_open.get() {
            let close = self.editor.gallery_open.clone();
            cx.overlay(
                anchor,
                Placement::Fill,
                &GalleryLayer(self.editor.clone()),
                Some(Rc::new(move || close.set(false))),
            );
        }
        anchor
    }
}

struct GalleryLayer(Editor);

fn template_card(title: &str, text: &str, icon: IconName, on_click: impl Fn() + 'static) -> Clickable {
    let card = VStack::new((
        HStack::new((Icon::new(icon).size(22.0).color(Semantic::Accent), Spacer::new())).align(Align::Center),
        Text::new(title).style(TextStyle::Headline),
        Text::new(text).style(TextStyle::Callout).foreground(Semantic::InkMuted),
    ))
    .spacing(8.0)
    .padding(16.0)
    .align(Align::Stretch);
    Clickable { child: Box::new(card), on_click: Rc::new(on_click) }
}

/// A card that is a button.
struct Clickable {
    child: AnyView,
    on_click: Rc<dyn Fn()>,
}

impl View for Clickable {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let hovered = cx.is_hovered();
        let focused = cx.is_focus_visible();
        let child = cx.child(self.child.as_ref());
        let node = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[child],
            Paint::Quad(QuadPaint {
                corner_radius: 12.0,
                border: Some(Border {
                    width: if hovered || focused { 1.5 } else { 1.0 },
                    color: if hovered || focused {
                        theme.color(Semantic::Accent)
                    } else {
                        theme.color(Semantic::Border)
                    },
                }),
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        );
        cx.semantics(node, ravenrs::widgets::Semantics::new(Role::Button));
        cx.on_click(node, self.on_click.clone());
        cx.focusable(node, 12.0);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

impl View for GalleryLayer {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.0;
        let theme = *cx.theme();
        let viewport = cx.viewport();
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
        let close = e.gallery_open.clone();
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
        .spacing(14.0);
        let card = cx.child(
            &VStack::new((
                HStack::new((
                    VStack::new((
                        Text::new("Novo relatório").style(TextStyle::Title),
                        Text::new("Comece por um modelo pronto ou deixe o assistente montar o relatório a partir dos seus dados.")
                            .foreground(Semantic::InkMuted),
                    ))
                    .spacing(4.0)
                    .grow(1.0),
                    IconButton::new(IconName::X).label("Fechar").on_click(move || close.set(false)),
                ))
                .align(Align::Start),
                cards,
            ))
            .spacing(20.0)
            .padding(28.0)
            .width((viewport.width - 48.0).clamp(320.0, 880.0))
            .background(theme.color(Semantic::SurfaceElevated))
            .corner_radius(18.0)
            .shadow(theme.elevation.raised),
        );
        cx.semantics(card, ravenrs::widgets::Semantics::new(Role::Dialog).name("Novo relatório").modal());
        cx.cursor(card, CursorIcon::Default);
        let backdrop = cx.container(
            LayoutStyle {
                width: Some(viewport.width),
                height: Some(viewport.height),
                align: Align::Center,
                justify: ravenrs::layout::Justify::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[card],
            Paint::Quad(QuadPaint::new(Color::rgba(0.0, 0.0, 0.0, 0.45))),
        );
        let close = e.gallery_open.clone();
        cx.on_click(backdrop, Rc::new(move || close.set(false)));
        backdrop
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
            FormField::new("Título", TextField::new(s.title.binding()).compact()),
            FormField::new(
                "Dados",
                Select::new(s.source.binding(), WIZARD_SOURCES.iter().map(|(n, _)| *n))
                    .width(300.0)
                    .label("Dados"),
            ),
            Text::new(format!("Campos ({} escolhidos)", chosen.len()))
                .style(TextStyle::Callout)
                .font_weight(Weight::SEMIBOLD),
            Grid::new(3, checks).spacing(8.0),
            FormField::new(
                "Agrupar por",
                Select::new(s.group.binding(), groups).width(300.0).label("Agrupar por"),
            ),
            Toggle::new(s.totals.binding()).label("Totais dos campos numéricos"),
        ))
        .spacing(14.0)
        .align(Align::Stretch);
        cx.child(
            &Dialog::new(e.wizard_open.binding(), "Assistente de relatório")
                .description("Escolha os campos e o agrupamento. O resto (cabeçalhos, zebra, totais, numeração de páginas) o assistente faz.")
                .content(content)
                .action(generate),
        )
    }
}

#[cfg(target_arch = "wasm32")]
mod web_entry {
    use std::cell::RefCell;

    use wasm_bindgen::prelude::*;

    thread_local! {
        static APP: RefCell<Option<ravenrs::web::WebApp>> = const { RefCell::new(None) };
    }

    /// Runs automatically when the module is instantiated.
    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            let options =
                ravenrs::web::WebOptions::new("raven-canvas").theme(ravenrs::style::Theme::raven_light());
            match ravenrs::web::start(super::app(), options).await {
                Ok(app) => APP.with(|a| *a.borrow_mut() = Some(app)),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }
}
