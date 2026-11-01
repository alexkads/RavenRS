//! Document viewer for RavenRS Reporting.
//!
//! * [`DocumentViewer`] shows a rendered [`Document`]: pages, zoom, thumbnails
//!   and text search that finds words regardless of case and accents.
//! * [`ParameterPanel`] asks for a report's parameters and applies them together.
//! * [`ViewerState`] holds what the user has chosen, so it survives rebuilds and
//!   can be shared with the rest of an app.
//!
//! ```
//! use std::rc::Rc;
//! use ravenrs_report::{Band, Control, Report, ReportEngine};
//! use ravenrs_report_viewer::{DocumentViewer, ViewerState};
//! use serde_json::json;
//!
//! let report = Report::new("Sales").detail(Band::new(18.0).control(Control::field("[Name]").at(0.0, 0.0, 200.0, 18.0)));
//! let document = Rc::new(ReportEngine::new().render(&report, &json!([{ "Name": "Ana" }])));
//! let state = ViewerState::new();
//! let viewer = DocumentViewer::new(document, &state);
//! # let _ = viewer;
//! ```

mod parameters;
mod print;
mod search;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ravenrs_core::{Color, CursorIcon, Insets, Key, Point, Primitive, Quad, Rect, Role, Scene};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_report::{Document, Item, TextBlock, paint_page};
use ravenrs_style::{SPACING, Semantic, TextStyle, Weight};
use ravenrs_text::{TextAttrs, TextSystem};
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, ButtonSize, ButtonStyle, Canvas, Divider, HStack, IconButton, IconName,
    NodeId, Pagination, Paint, QuadPaint, ScrollView, Semantics, Spacer, Text, TextField, Tone, VStack, View,
    ViewExt,
};

pub use parameters::{ParameterLabels, ParameterPanel, ParameterValues};
pub use print::print;
pub use search::{Match, find_matches};

const ZOOM_STEP: f32 = 0.25;
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 3.0;
const THUMBNAIL_ZOOM: f32 = 0.18;
/// Thumbnails rendered at once; a long document shows those around the current page.
const THUMBNAILS_MAX: usize = 12;
/// Page scenes kept; each is cheap to redraw and costly to build.
const SCENES_MAX: usize = 96;
/// Room left out when working out how many thumbnails fit.
const CHROME_HEIGHT: f32 = 200.0;
/// Problems in the report shown above the page; the rest are summed up.
const ERRORS_SHOWN: usize = 3;

/// The pages, centered, in a column that can get wider than its scroll view.
struct PagesColumn {
    pages: Vec<AnyView>,
    min_width: f32,
}

impl View for PagesColumn {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let pages = cx.children(&self.pages);
        cx.container(
            LayoutStyle {
                spacing: SPACING.s3,
                padding: SPACING.s6.into(),
                min_width: Some(self.min_width),
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &pages,
            Paint::None,
        )
    }
}

/// The text a viewer shows.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ViewerLabels {
    /// "Previous" in the page navigation.
    pub previous: String,
    /// "Next" in the page navigation.
    pub next: String,
    /// "Page" in the page navigation.
    pub page: String,
    /// The search field's hint.
    pub search: String,
    /// The previous search result.
    pub previous_match: String,
    /// The next search result.
    pub next_match: String,
    /// Shown when a search finds nothing.
    pub no_matches: String,
    /// Between a result's position and the total.
    pub of: String,
    /// Shows or hides the thumbnails.
    pub thumbnails: String,
    /// Before the number in a thumbnail's name, for screen readers.
    pub thumbnail_of_page: String,
    /// Zooms out.
    pub zoom_out: String,
    /// Zooms in.
    pub zoom_in: String,
    /// Fits the page to the window's width.
    pub fit_width: String,
    /// Prints the document.
    pub print: String,
    /// The title of a problem in the report.
    pub report_error: String,
}

impl Default for ViewerLabels {
    fn default() -> Self {
        let text = |s: &str| s.to_owned();
        Self {
            previous: text("Previous"),
            next: text("Next"),
            page: text("Page"),
            search: text("Search"),
            previous_match: text("Previous result"),
            next_match: text("Next result"),
            no_matches: text("Not found"),
            of: text("of"),
            thumbnails: text("Thumbnails"),
            thumbnail_of_page: text("Thumbnail of page"),
            zoom_out: text("Zoom out"),
            zoom_in: text("Zoom in"),
            fit_width: text("Fit width"),
            print: text("Print"),
            report_error: text("Problem in the report"),
        }
    }
}

impl ViewerLabels {
    /// The labels in Brazilian Portuguese.
    pub fn pt_br() -> Self {
        let text = |s: &str| s.to_owned();
        Self {
            previous: text("Anterior"),
            next: text("Próxima"),
            page: text("Página"),
            search: text("Buscar"),
            previous_match: text("Resultado anterior"),
            next_match: text("Próximo resultado"),
            no_matches: text("Não encontrado"),
            of: text("de"),
            thumbnails: text("Miniaturas"),
            thumbnail_of_page: text("Miniatura da página"),
            zoom_out: text("Afastar"),
            zoom_in: text("Aproximar"),
            fit_width: text("Ajustar à largura"),
            print: text("Imprimir"),
            report_error: text("Erro no relatório"),
        }
    }
}

/// What a viewer remembers. Clones share the same state.
#[derive(Clone)]
pub struct ViewerState {
    page: State<usize>,
    zoom: State<f32>,
    query: State<String>,
    current_match: State<usize>,
    thumbnails: State<bool>,
    cache: Rc<RefCell<Cache>>,
}

impl std::fmt::Debug for ViewerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewerState")
            .field("page", &self.page.get())
            .field("zoom", &self.zoom.get())
            .finish_non_exhaustive()
    }
}

impl Default for ViewerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewerState {
    /// A viewer on the first page at 100%.
    pub fn new() -> Self {
        Self::sharing(State::new(0), State::new(1.0))
    }

    /// A viewer whose page and zoom are states the app also uses.
    pub fn sharing(page: State<usize>, zoom: State<f32>) -> Self {
        Self {
            page,
            zoom,
            query: State::new(String::new()),
            current_match: State::new(0),
            thumbnails: State::new(false),
            cache: Rc::new(RefCell::new(Cache::default())),
        }
    }

    /// The current page, counted from 0.
    pub fn page(&self) -> &State<usize> {
        &self.page
    }

    /// The zoom: 1.0 is one logical pixel per point.
    pub fn zoom(&self) -> &State<f32> {
        &self.zoom
    }

    /// The text being searched for.
    pub fn query(&self) -> &State<String> {
        &self.query
    }

    /// Whether the thumbnails are shown.
    pub fn thumbnails(&self) -> &State<bool> {
        &self.thumbnails
    }
}

/// What is worth keeping between builds, for one document.
#[derive(Default)]
struct Cache {
    /// Identifies the document; everything here is cleared when it changes.
    document: usize,
    scenes: HashMap<(usize, u32), Rc<Scene>>,
    query: String,
    matches: Rc<Vec<Match>>,
    text: Option<TextSystem>,
}

impl Cache {
    fn for_document(&mut self, document: &Rc<Document>) {
        let id = Rc::as_ptr(document) as usize;
        if self.document != id {
            self.document = id;
            self.scenes.clear();
            self.query.clear();
            self.matches = Rc::new(Vec::new());
        }
    }

    fn scene(&mut self, document: &Document, page: usize, zoom: f32) -> Rc<Scene> {
        let key = (page, zoom.to_bits());
        if let Some(scene) = self.scenes.get(&key) {
            return scene.clone();
        }
        let mut scene = Scene::new();
        if let Some(sheet) = document.pages.get(page) {
            paint_page(sheet, self.text.get_or_insert_with(TextSystem::new), &mut scene, Point::ZERO, zoom);
        }
        if self.scenes.len() >= SCENES_MAX {
            self.scenes.clear();
        }
        let scene = Rc::new(scene);
        self.scenes.insert(key, scene.clone());
        scene
    }

    fn matches(&mut self, document: &Document, query: &str) -> Rc<Vec<Match>> {
        if self.query != query {
            self.query = query.to_owned();
            self.matches = Rc::new(find_matches(document, query));
        }
        self.matches.clone()
    }

    /// Where a match is on its page, in points.
    fn match_rect(&mut self, document: &Document, found: &Match) -> Option<Rect> {
        let Item::Text(block) = document.pages.get(found.page)?.items.get(found.item)? else { return None };
        let line = block.lines.get(found.line)?;
        let text = self.text.get_or_insert_with(TextSystem::new);
        let start = prefix_width(text, block, line.text.get(..found.range.start)?);
        let end = prefix_width(text, block, line.text.get(..found.range.end)?);
        Some(Rect::new(line.x + start, line.top, (end - start).max(1.0), block.line_height))
    }
}

fn prefix_width(text: &mut TextSystem, block: &TextBlock, prefix: &str) -> f32 {
    let attrs = TextAttrs {
        size: block.size,
        line_height: block.line_height,
        weight: if block.bold { 600.0 } else { 400.0 },
        ..TextAttrs::default()
    };
    text.layout(prefix, &attrs).measure(None).width
}

/// Copies a cached page into the frame being painted, moved to `origin`.
fn draw_at(source: &Scene, origin: Point, scene: &mut Scene) {
    let shift = |r: Rect| Rect::from_origin_size(Point::new(r.x() + origin.x, r.y() + origin.y), r.size);
    for primitive in source.primitives() {
        match primitive {
            Primitive::Quad(q) => scene.push_quad(Quad { rect: shift(q.rect), ..*q }),
            Primitive::Glyphs(run) => {
                let mut run = run.clone();
                for glyph in &mut run.glyphs {
                    glyph.x += origin.x;
                    glyph.y += origin.y;
                }
                scene.push_glyphs(run);
            }
            Primitive::Image(image) => {
                scene.push_image(ravenrs_core::ImageQuad { rect: shift(image.rect), ..image.clone() });
            }
            Primitive::PushClip(r) => scene.push_clip(shift(*r)),
            Primitive::PopClip => scene.pop_clip(),
            _ => {}
        }
    }
}

/// Builds the extra controls of the toolbar.
type ToolbarItems = Rc<dyn Fn() -> Vec<AnyView>>;

/// Shows a rendered document.
pub struct DocumentViewer {
    document: Rc<Document>,
    state: ViewerState,
    labels: ViewerLabels,
    leading: Option<ToolbarItems>,
    trailing: Option<ToolbarItems>,
    print_button: bool,
}

impl std::fmt::Debug for DocumentViewer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentViewer").field("title", &self.document.title).finish_non_exhaustive()
    }
}

impl DocumentViewer {
    /// A viewer of `document`.
    pub fn new(document: Rc<Document>, state: &ViewerState) -> Self {
        Self {
            document,
            state: state.clone(),
            labels: ViewerLabels::default(),
            leading: None,
            trailing: None,
            print_button: true,
        }
    }

    /// Sets the text the viewer shows.
    pub fn labels(mut self, labels: ViewerLabels) -> Self {
        self.labels = labels;
        self
    }

    /// Whether the toolbar offers to print (on by default).
    pub fn print_button(mut self, show: bool) -> Self {
        self.print_button = show;
        self
    }

    /// Adds controls at the start of the toolbar.
    pub fn toolbar_leading(mut self, items: impl Fn() -> Vec<AnyView> + 'static) -> Self {
        self.leading = Some(Rc::new(items));
        self
    }

    /// Adds controls at the end of the toolbar, such as print and export buttons.
    pub fn toolbar_trailing(mut self, items: impl Fn() -> Vec<AnyView> + 'static) -> Self {
        self.trailing = Some(Rc::new(items));
        self
    }

    fn search_bar(&self, matches: &[Match], current: usize) -> impl View + use<> {
        let labels = &self.labels;
        let (read, write) = (self.state.clone(), self.state.clone());
        let document = self.document.clone();
        let field = TextField::new(Binding::new(
            move || read.query.get(),
            move |query: String| {
                let found = write.cache.borrow_mut().matches(&document, &query);
                write.query.set(query);
                write.current_match.set(0);
                if let Some(first) = found.first() {
                    write.page.set(first.page);
                }
            },
        ))
        .placeholder(labels.search.clone())
        .icon(IconName::Search)
        .compact()
        .width(180.0)
        .on_submit({
            let step = self.stepper(1);
            move |_| step()
        });
        let mut items: Vec<AnyView> = vec![Box::new(field)];
        if !self.state.query.with(|q| q.trim().is_empty()) {
            let status = if matches.is_empty() {
                labels.no_matches.clone()
            } else {
                format!("{} {} {}", current + 1, labels.of, matches.len())
            };
            items.push(Box::new(Text::new(status).style(TextStyle::Callout).foreground(Semantic::InkMuted)));
            if !matches.is_empty() {
                let (back, forward) = (self.stepper(-1), self.stepper(1));
                items.push(Box::new(
                    IconButton::new(IconName::ChevronUp)
                        .label(labels.previous_match.clone())
                        .size(ButtonSize::Small)
                        .on_click(back),
                ));
                items.push(Box::new(
                    IconButton::new(IconName::ChevronDown)
                        .label(labels.next_match.clone())
                        .size(ButtonSize::Small)
                        .on_click(forward),
                ));
            }
        }
        HStack::new(items).spacing(SPACING.s2).align(Align::Center)
    }

    /// A step through the search results, going around at the ends.
    fn stepper(&self, delta: isize) -> impl Fn() + use<> {
        let state = self.state.clone();
        let document = self.document.clone();
        move || {
            let query = state.query.get();
            let found = state.cache.borrow_mut().matches(&document, &query);
            if found.is_empty() {
                return;
            }
            let count = found.len() as isize;
            let next = (state.current_match.get() as isize + delta).rem_euclid(count) as usize;
            state.current_match.set(next);
            state.page.set(found[next].page);
        }
    }

    fn zoom_controls(&self, zoom: f32, fit_width: f32) -> impl View + use<> {
        let labels = &self.labels;
        let (out, into, fit) = (self.state.zoom.clone(), self.state.zoom.clone(), self.state.zoom.clone());
        HStack::new((
            IconButton::new(IconName::ZoomOut)
                .label(labels.zoom_out.clone())
                .size(ButtonSize::Small)
                .on_click(move || out.update(|z| *z = (*z - ZOOM_STEP).max(ZOOM_MIN))),
            Text::new(format!("{:.0}%", zoom * 100.0))
                .style(TextStyle::Callout)
                .font_weight(Weight::SEMIBOLD),
            IconButton::new(IconName::ZoomIn)
                .label(labels.zoom_in.clone())
                .size(ButtonSize::Small)
                .on_click(move || into.update(|z| *z = (*z + ZOOM_STEP).min(ZOOM_MAX))),
            IconButton::new(IconName::Maximize2)
                .label(labels.fit_width.clone())
                .size(ButtonSize::Small)
                .on_click(move || fit.set(fit_width.clamp(ZOOM_MIN, ZOOM_MAX))),
        ))
        .spacing(SPACING.s1)
        .align(Align::Center)
    }

    fn thumbnails_column(&self, current: usize, available_height: f32, cx: &mut BuildCx<'_>) -> NodeId {
        let pages = self.document.pages.len();
        let tall =
            self.document.pages.first().map_or(842.0, |p| p.height) * THUMBNAIL_ZOOM + 2.0 * SPACING.s6;
        let fits = ((available_height / tall) as usize).clamp(1, THUMBNAILS_MAX);
        let start = current.saturating_sub(fits / 2).min(pages.saturating_sub(fits));
        let items: Vec<AnyView> = (start..(start + fits).min(pages))
            .map(|page| {
                Box::new(Thumbnail {
                    document: self.document.clone(),
                    state: self.state.clone(),
                    page,
                    current: page == current,
                    label: format!("{} {}", self.labels.thumbnail_of_page, page + 1),
                }) as AnyView
            })
            .collect();
        cx.child(
            &VStack::new(items)
                .spacing(SPACING.s2)
                .padding(SPACING.s3)
                .align(Align::Center)
                .background(Semantic::Surface),
        )
    }
}

impl View for DocumentViewer {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let document = &self.document;
        let state = &self.state;
        let pages = document.pages.len().max(1);
        let current = state.page.get().min(pages - 1);
        let zoom = state.zoom.get();
        let query = state.query.get();
        let matches = {
            let mut cache = state.cache.borrow_mut();
            cache.for_document(document);
            cache.matches(document, &query)
        };
        let current_match = state.current_match.get().min(matches.len().saturating_sub(1));
        let page_width = document.pages.get(current).map_or(595.0, |p| p.width);
        let viewport = cx.viewport();
        let thumbnails = state.thumbnails.get();
        let sidebar = if thumbnails { page_width * THUMBNAIL_ZOOM + 2.0 * SPACING.s3 + 1.0 } else { 0.0 };
        let fit_width = (viewport.width - sidebar - 2.0 * SPACING.s6) / page_width.max(1.0);

        let labels = &self.labels;
        let toggle = state.thumbnails.clone();
        let mut bar: Vec<AnyView> = vec![Box::new(
            IconButton::new(IconName::PanelLeft)
                .label(labels.thumbnails.clone())
                .size(ButtonSize::Small)
                .on_click(move || toggle.update(|shown| *shown = !*shown)),
        )];
        if let Some(leading) = &self.leading {
            bar.extend(leading());
        }
        bar.push(Box::new(Pagination::new(state.page.binding(), pages).labels(
            labels.previous.clone(),
            labels.next.clone(),
            labels.page.clone(),
        )));
        bar.push(Box::new(Spacer::new()));
        bar.push(Box::new(self.search_bar(&matches, current_match)));
        bar.push(Box::new(self.zoom_controls(zoom, fit_width)));
        if self.print_button {
            let printed = document.clone();
            bar.push(Box::new(
                Button::new(labels.print.clone())
                    .icon(IconName::Printer)
                    .style(ButtonStyle::Secondary)
                    .size(ButtonSize::Small)
                    .on_click(move || print(&printed)),
            ));
        }
        if let Some(trailing) = &self.trailing {
            bar.extend(trailing());
        }
        let toolbar = cx.child(
            &HStack::new(bar)
                .spacing(SPACING.s3)
                .align(Align::Center)
                .wrap()
                .padding(Insets::symmetric(SPACING.s2, SPACING.s4))
                .background(Semantic::Surface),
        );
        let line = cx.child(&Divider::horizontal());

        let theme = *cx.theme();
        let highlights: Vec<(Rect, bool)> = {
            let mut cache = state.cache.borrow_mut();
            matches
                .iter()
                .enumerate()
                .filter(|(_, found)| found.page == current)
                .filter_map(|(index, found)| {
                    Some((cache.match_rect(document, found)?, index == current_match))
                })
                .collect()
        };
        let mut body: Vec<AnyView> = Vec::new();
        for message in document.errors.iter().take(ERRORS_SHOWN) {
            body.push(Box::new(Alert::new(labels.report_error.clone(), message.clone()).tone(Tone::Danger)));
        }
        if let Some(sheet) = document.pages.get(current) {
            let scene = state.cache.borrow_mut().scene(document, current, zoom);
            body.push(Box::new(PageView {
                scene,
                highlights,
                highlight: with_alpha(theme.color(Semantic::Warning), 0.35),
                highlight_current: with_alpha(theme.color(Semantic::Warning), 0.7),
                zoom,
                width: sheet.width * zoom,
                height: sheet.height * zoom,
                text: document.page_text(current).join(" "),
            }));
        }
        // At least the width of the page, and otherwise the width of the view,
        // wherever the app has put it, so the page stays centered.
        let main = cx.child(
            &ScrollView::new((PagesColumn { pages: body, min_width: page_width * zoom + 2.0 * SPACING.s6 },))
                .horizontal()
                .align(Align::Stretch)
                .grow(1.0),
        );
        let row_children = if thumbnails {
            // The window less what is usually above the pages: toolbars and a status bar.
            let column = self.thumbnails_column(current, viewport.height - CHROME_HEIGHT, cx);
            let separator = cx.child(&Divider::vertical());
            vec![column, separator, main]
        } else {
            vec![main]
        };
        let row = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                align: Align::Stretch,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &row_children,
            Paint::None,
        );

        let (up, down) = (state.page.clone(), state.page.clone());
        // Whole percents: each zoom gets its pages drawn again, and a gesture passes through many.
        let pinched = state.zoom.clone();
        cx.on_pinch(
            row,
            Rc::new(move |delta| {
                pinched.update(|z| {
                    let next = (*z * (1.0 + delta) * 100.0).round() / 100.0;
                    *z = next.clamp(ZOOM_MIN, ZOOM_MAX);
                });
            }),
        );
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
            &[toolbar, line, row],
            Paint::None,
        )
    }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color::rgba(color.r, color.g, color.b, alpha)
}

/// One page, with what a search found on it.
struct PageView {
    scene: Rc<Scene>,
    highlights: Vec<(Rect, bool)>,
    highlight: Color,
    highlight_current: Color,
    zoom: f32,
    width: f32,
    height: f32,
    text: String,
}

impl View for PageView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let scene = self.scene.clone();
        let highlights = self.highlights.clone();
        let (normal, strong, zoom) = (self.highlight, self.highlight_current, self.zoom);
        let canvas = Canvas::new(move |frame, out| {
            draw_at(&scene, frame.origin, out);
            for (rect, current) in &highlights {
                let box_on_screen = Rect::new(
                    frame.x() + rect.x() * zoom,
                    frame.y() + rect.y() * zoom,
                    rect.width() * zoom,
                    rect.height() * zoom,
                );
                out.push_quad(Quad::new(box_on_screen, if *current { strong } else { normal }).radius(2.0));
            }
        })
        .size(self.width, self.height);
        let canvas = cx.child(&canvas);
        let node = cx.container(LayoutStyle::stack(Axis::Vertical), &[canvas], Paint::None);
        // Screen readers get the page's text; everyone else sees the drawing.
        cx.semantics(node, Semantics::new(Role::Image).name(self.text.clone()));
        node
    }
}

/// A small picture of a page that goes to it when clicked.
struct Thumbnail {
    document: Rc<Document>,
    state: ViewerState,
    page: usize,
    current: bool,
    label: String,
}

impl View for Thumbnail {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let scene = self.state.cache.borrow_mut().scene(&self.document, self.page, THUMBNAIL_ZOOM);
        let sheet = self.document.pages.get(self.page);
        let (width, height) =
            sheet.map_or((0.0, 0.0), |p| (p.width * THUMBNAIL_ZOOM, p.height * THUMBNAIL_ZOOM));
        let picture = Canvas::new(move |frame, out| draw_at(&scene, frame.origin, out)).size(width, height);
        let caption = Text::new((self.page + 1).to_string())
            .style(TextStyle::Caption)
            .foreground(if self.current { Semantic::Accent } else { Semantic::InkMuted });
        let content = cx.child(&VStack::new((picture, caption)).spacing(SPACING.s1).align(Align::Center));
        let border =
            self.current.then(|| ravenrs_core::Border { width: 2.0, color: theme.color(Semantic::Accent) });
        let hovered = cx.is_hover_within();
        let fill = if hovered && !self.current {
            theme.color(Semantic::SurfaceElevated)
        } else {
            Color::TRANSPARENT
        };
        let node = cx.container(
            LayoutStyle { padding: Insets::uniform(SPACING.s1), ..LayoutStyle::stack(Axis::Vertical) },
            &[content],
            Paint::Quad(QuadPaint { corner_radius: 6.0, border, ..QuadPaint::new(fill) }),
        );
        let (page, target) = (self.state.page.clone(), self.page);
        cx.on_click(node, Rc::new(move || page.set(target)));
        cx.focusable(node, 6.0);
        cx.cursor(node, CursorIcon::Pointer);
        cx.semantics(node, Semantics::new(Role::Button).name(self.label.clone()).selected(self.current));
        node
    }
}
