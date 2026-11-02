//! Things side by side: tiles to rearrange, and cards in as many columns as fit.

use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, CursorIcon, Insets, Key, Point, Quad, Rect, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, DragPhase, NodeId, Paint, QuadPaint, Semantics};
use crate::{AnyView, SelectionMode, View, util};

const TILE_GAP: f32 = 12.0;
const TILE_HEADER: f32 = 36.0;
const CARD_OVERSCAN: usize = 2;

/// Where a tile stands: which tile, and how many cells it takes. The order of a list
/// of these is the order on the screen.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TileSpot {
    /// The tile, by [`Tile::new`]'s key.
    pub key: String,
    /// Columns it takes.
    pub columns: u16,
    /// Rows it takes.
    pub rows: u16,
}

impl TileSpot {
    /// A tile taking this many cells.
    pub fn new(key: impl Into<String>, columns: u16, rows: u16) -> Self {
        Self { key: key.into(), columns: columns.max(1), rows: rows.max(1) }
    }
}

/// One tile of a [`TileLayout`].
pub struct Tile {
    key: String,
    title: String,
    content: AnyView,
}

impl fmt::Debug for Tile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tile").field("key", &self.key).finish_non_exhaustive()
    }
}

impl Tile {
    /// A tile known by `key`, under this title.
    pub fn new(key: impl Into<String>, title: impl Into<String>, content: impl View + 'static) -> Self {
        Self { key: key.into(), title: title.into(), content: Box::new(content) }
    }
}

/// Puts each tile in the first place it fits, going row by row: `(column, row)`.
/// A tile wider than the grid takes the whole width.
fn place(spots: &[TileSpot], columns: usize) -> Vec<(usize, usize)> {
    let columns = columns.max(1);
    let mut taken: Vec<Vec<bool>> = Vec::new();
    let mut places = Vec::with_capacity(spots.len());
    for spot in spots {
        let (wide, tall) = ((spot.columns as usize).min(columns), spot.rows as usize);
        let found = (0usize..)
            .flat_map(|row| (0..=columns - wide).map(move |column| (column, row)))
            .find(|&(column, row)| {
                (row..row + tall)
                    .all(|r| (column..column + wide).all(|c| !taken.get(r).is_some_and(|line| line[c])))
            })
            .unwrap_or((0, taken.len()));
        for r in found.1..found.1 + tall {
            if taken.len() <= r {
                taken.resize(r + 1, vec![false; columns]);
            }
            for cell in &mut taken[r][found.0..found.0 + wide] {
                *cell = true;
            }
        }
        places.push(found);
    }
    places
}

fn rows_used(spots: &[TileSpot], places: &[(usize, usize)]) -> usize {
    spots.iter().zip(places).map(|(s, p)| p.1 + s.rows as usize).max().unwrap_or(0)
}

/// Spots in the order of the tiles, for tiles a saved layout has not heard of yet,
/// dropping what is no longer there.
fn reconcile(saved: &[TileSpot], defaults: &[TileSpot]) -> Vec<TileSpot> {
    let mut spots: Vec<TileSpot> =
        saved.iter().filter(|s| defaults.iter().any(|d| d.key == s.key)).cloned().collect();
    for default in defaults {
        if !spots.iter().any(|s| s.key == default.key) {
            spots.push(default.clone());
        }
    }
    spots
}

/// A dashboard: tiles in a grid of cells, that people can drag to another place and
/// resize. As the width falls, it uses fewer columns. From the keyboard, on a tile's
/// handle: arrows move it, Shift with arrows resizes it.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Text, Tile, TileLayout, TileSpot, Ui};
///
/// let spots = State::new(vec![TileSpot::new("sales", 2, 1)]);
/// let tiles = TileLayout::new(spots.binding())
///     .tile(Tile::new("sales", "Sales", Text::new("R$ 1.000")))
///     .tile(Tile::new("stock", "Stock", Text::new("42")));
/// let mut ui = Ui::new(tiles, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct TileLayout {
    spots: Binding<Vec<TileSpot>>,
    tiles: Vec<Tile>,
    columns: usize,
    min_column_width: f32,
    row_height: f32,
    locked: bool,
}

impl fmt::Debug for TileLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TileLayout").field("tiles", &self.tiles).finish_non_exhaustive()
    }
}

impl TileLayout {
    /// A layout bound to where the tiles stand, to keep between sessions. Tiles not in
    /// the list take one cell, after the others.
    pub fn new(spots: Binding<Vec<TileSpot>>) -> Self {
        Self {
            spots,
            tiles: Vec::new(),
            columns: 4,
            min_column_width: 200.0,
            row_height: 160.0,
            locked: false,
        }
    }

    /// Appends a tile.
    pub fn tile(mut self, tile: Tile) -> Self {
        self.tiles.push(tile);
        self
    }

    /// The most columns, when there is room. 4 by default.
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns.max(1);
        self
    }

    /// A column narrower than this makes the layout use one column less. 200 by default.
    pub fn min_column_width(mut self, width: f32) -> Self {
        self.min_column_width = width.max(1.0);
        self
    }

    /// Height of a row of cells in logical pixels. 160 by default.
    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(TILE_HEADER);
        self
    }

    /// Keeps tiles where they are, with no handles.
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }
}

/// The geometry of the grid in this build, for what handles a drag.
#[derive(Clone, Copy)]
struct Cells {
    columns: usize,
    column_step: f32,
    row_step: f32,
}

impl Cells {
    /// The cell under a point inside the layout.
    fn at(&self, x: f32, y: f32) -> (usize, usize) {
        let column = ((x / self.column_step).floor().max(0.0) as usize).min(self.columns - 1);
        let row = (y / self.row_step).floor().max(0.0) as usize;
        (column, row)
    }
}

/// The grip of a tile. Focus and drags go by view, so it is one.
struct TileHandle {
    name: String,
    key: String,
    origin: Point,
    spots: Binding<Vec<TileSpot>>,
    cells: Cells,
}

impl View for TileHandle {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let muted = theme.color(Semantic::InkMuted);
        let handle = cx.container(
            LayoutStyle { width: Some(24.0), height: Some(TILE_HEADER), ..LayoutStyle::leaf() },
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                let center = f.center();
                for column in [-3.0, 2.0] {
                    for row in [-6.0, -1.0, 4.0] {
                        let dot = Rect::new(center.x + column, center.y + row, 2.0, 2.0);
                        scene.push_quad(Quad::new(dot, muted).radius(1.0));
                    }
                }
            })),
        );
        cx.semantics(handle, Semantics::new(Role::Button).name(self.name.clone()));
        cx.focusable(handle, theme.radii.sm);
        cx.cursor(handle, CursorIcon::Pointer);
        let (spots, key, cells, origin) = (self.spots.clone(), self.key.clone(), self.cells, self.origin);
        cx.on_drag(
            handle,
            Rc::new(move |d| {
                if d.phase != DragPhase::Move {
                    return;
                }
                // The handle sits at the tile's corner, so this is where the pointer is
                // inside the layout, near enough to pick a cell.
                let at = Point::new(
                    origin.x + d.position.x - d.container.x(),
                    origin.y + d.position.y - d.container.y(),
                );
                move_to_cell(&spots, &key, cells, cells.at(at.x, at.y));
            }),
        );
        let (spots, key, wide) = (self.spots.clone(), self.key.clone(), self.cells.columns);
        cx.on_key(
            handle,
            Rc::new(move |pressed, modifiers| {
                if modifiers.shift {
                    let (columns, rows) = match pressed {
                        Key::ArrowLeft => (-1, 0),
                        Key::ArrowRight => (1, 0),
                        Key::ArrowUp => (0, -1),
                        Key::ArrowDown => (0, 1),
                        _ => return false,
                    };
                    resize(&spots, &key, columns, rows, wide);
                } else {
                    let by = match pressed {
                        Key::ArrowLeft | Key::ArrowUp => -1,
                        Key::ArrowRight | Key::ArrowDown => 1,
                        _ => return false,
                    };
                    shift(&spots, &key, by);
                }
                true
            }),
        );
        handle
    }
}

/// The corner that resizes a tile with the pointer; the keyboard does it on the handle.
struct TileCorner {
    key: String,
    /// Where the tile stands, in cells.
    place: (usize, usize),
    at: (f32, f32),
    spots: Binding<Vec<TileSpot>>,
    cells: Cells,
}

impl View for TileCorner {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let ink = cx.theme().color(Semantic::InkMuted);
        let corner = cx.container(
            LayoutStyle {
                width: Some(16.0),
                height: Some(16.0),
                absolute: Some(self.at),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                for i in 0..3 {
                    let step = i as f32 * 4.0;
                    scene.push_quad(Quad::new(
                        Rect::new(f.max_x() - 3.0 - step, f.max_y() - 3.0, 2.0, 2.0),
                        ink,
                    ));
                    scene.push_quad(Quad::new(
                        Rect::new(f.max_x() - 3.0, f.max_y() - 3.0 - step, 2.0, 2.0),
                        ink,
                    ));
                }
            })),
        );
        cx.semantics(corner, Semantics::hidden());
        cx.cursor(corner, CursorIcon::ColResize);
        let (spots, key, cells, (column, row)) =
            (self.spots.clone(), self.key.clone(), self.cells, self.place);
        cx.on_drag(
            corner,
            Rc::new(move |d| {
                if d.phase != DragPhase::Move {
                    return;
                }
                // The corner's parent is the tile, which stands at its own cell.
                let (end_column, end_row) = cells.at(
                    d.position.x - d.container.x() + column as f32 * cells.column_step,
                    d.position.y - d.container.y() + row as f32 * cells.row_step,
                );
                let wide = (end_column + 1).saturating_sub(column).clamp(1, cells.columns) as u16;
                let tall = (end_row + 1).saturating_sub(row).max(1) as u16;
                spots.update(|all| {
                    if let Some(spot) = all.iter_mut().find(|s| s.key == key) {
                        spot.columns = wide;
                        spot.rows = tall;
                    }
                });
            }),
        );
        corner
    }
}

struct TileCard<'a> {
    tile: &'a Tile,
    key: &'a str,
    place: (usize, usize),
    frame: (f32, f32, f32, f32),
    spots: Binding<Vec<TileSpot>>,
    cells: Cells,
    locked: bool,
}

impl View for TileCard<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (x, y, width, height) = self.frame;
        let title = cx.text_truncated(
            &self.tile.title,
            &util::attrs(&theme, TextStyle::Callout, Some(Weight::SEMIBOLD)),
            theme.color(Semantic::Ink),
            LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::leaf() },
        );
        let mut header = Vec::new();
        if !self.locked {
            header.push(cx.child(&TileHandle {
                name: format!("{} {}", cx.labels().tile_move, self.tile.title),
                key: self.key.to_owned(),
                origin: Point::new(x, y),
                spots: self.spots.clone(),
                cells: self.cells,
            }));
        }
        header.push(title);
        let bar = cx.container(
            LayoutStyle {
                height: Some(TILE_HEADER),
                padding: Insets { left: if self.locked { 12.0 } else { 4.0 }, right: 12.0, ..Insets::ZERO },
                spacing: 4.0,
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &header,
            Paint::None,
        );
        let content = cx.child(self.tile.content.as_ref());
        let body = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                overflow_hidden: true,
                padding: Insets { left: 12.0, right: 12.0, top: 0.0, bottom: 12.0 },
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::None,
        );
        cx.clip(body);
        let mut parts = vec![bar, body];
        if !self.locked {
            parts.push(cx.child(&TileCorner {
                key: self.key.to_owned(),
                place: self.place,
                at: (width - 18.0, height - 18.0),
                spots: self.spots.clone(),
                cells: self.cells,
            }));
        }
        let node = cx.container(
            LayoutStyle {
                absolute: Some((x, y)),
                width: Some(width),
                height: Some(height),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &parts,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        );
        cx.semantics(node, Semantics::new(Role::Group).name(self.tile.title.clone()));
        node
    }
}

/// Moves a tile to the place in the order that puts it at this cell.
fn move_to_cell(spots: &Binding<Vec<TileSpot>>, key: &str, cells: Cells, target: (usize, usize)) {
    let all = spots.get();
    let places = place(&all, cells.columns);
    let Some(from) = all.iter().position(|s| s.key == key) else { return };
    let over = all.iter().zip(&places).position(|(spot, &(column, row))| {
        (column..column + (spot.columns as usize).min(cells.columns)).contains(&target.0)
            && (row..row + spot.rows as usize).contains(&target.1)
    });
    if let Some(to) = over
        && to != from
    {
        spots.update(|all| {
            let spot = all.remove(from);
            all.insert(to, spot);
        });
    }
}

fn shift(spots: &Binding<Vec<TileSpot>>, key: &str, by: i32) {
    spots.update(|all| {
        if let Some(from) = all.iter().position(|s| s.key == key) {
            let to = (from as i32 + by).clamp(0, all.len() as i32 - 1) as usize;
            let spot = all.remove(from);
            all.insert(to, spot);
        }
    });
}

fn resize(spots: &Binding<Vec<TileSpot>>, key: &str, columns: i32, rows: i32, most_columns: usize) {
    spots.update(|all| {
        if let Some(spot) = all.iter_mut().find(|s| s.key == key) {
            spot.columns = (i32::from(spot.columns) + columns).clamp(1, most_columns as i32) as u16;
            spot.rows = (i32::from(spot.rows) + rows).max(1) as u16;
        }
    });
}

impl View for TileLayout {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let defaults: Vec<TileSpot> = self.tiles.iter().map(|t| TileSpot::new(t.key.clone(), 1, 1)).collect();
        let spots = reconcile(&self.spots.get(), &defaults);
        // The first build has only the window to go by.
        let width = cx.scroll_viewport().map_or(cx.viewport().width, |size| size.width).max(1.0);
        let columns = (((width + TILE_GAP) / (self.min_column_width + TILE_GAP)).floor() as usize)
            .clamp(1, self.columns);
        let step = (width + TILE_GAP) / columns as f32;
        let row_step = self.row_height + TILE_GAP;
        let cells = Cells { columns, column_step: step, row_step };
        let places = place(&spots, columns);
        let total = rows_used(&spots, &places);
        // The list the handles change, with every tile in it.
        let (read, write) = (self.spots.clone(), self.spots.clone());
        let known = defaults.clone();
        let bound = Binding::new(move || reconcile(&read.get(), &known), move |all| write.set(all));
        let mut children = Vec::with_capacity(spots.len());
        for (index, spot) in spots.iter().enumerate() {
            let Some(tile) = self.tiles.iter().find(|t| t.key == spot.key) else { continue };
            let (column, row) = places[index];
            let wide = (spot.columns as usize).min(columns) as f32;
            let card = TileCard {
                tile,
                key: &spot.key,
                place: places[index],
                frame: (
                    column as f32 * step,
                    row as f32 * row_step,
                    wide * step - TILE_GAP,
                    spot.rows as f32 * row_step - TILE_GAP,
                ),
                spots: bound.clone(),
                cells,
                locked: self.locked,
            };
            children.push(cx.keyed_child(util::hash_text(&spot.key), &card));
        }
        let node = cx.container(
            LayoutStyle {
                height: Some((total as f32 * row_step - TILE_GAP).max(0.0)),
                align_self: Some(Align::Stretch),
                ..LayoutStyle::leaf()
            },
            &children,
            Paint::None,
        );
        cx.report_frame(node);
        node
    }
}

type CardBuilder = Rc<dyn Fn(usize) -> AnyView>;

/// Records as cards, in as many columns as fit, of which only those in sight are built.
/// Arrows move between cards, Enter opens one and Space selects it.
///
/// ```
/// use ravenrs_widgets::{AnyView, CardView, Text, Ui};
///
/// let cards = CardView::new(10_000, |i| Box::new(Text::new(format!("Card {i}"))) as AnyView).height(400.0);
/// let mut ui = Ui::new(cards, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct CardView {
    count: usize,
    card: CardBuilder,
    card_width: f32,
    card_height: f32,
    height: Option<f32>,
    mode: SelectionMode,
    selection: Option<Binding<BTreeSet<usize>>>,
    on_open: Option<Rc<dyn Fn(usize)>>,
    card_name: Option<Rc<dyn Fn(usize) -> String>>,
    label: Option<String>,
}

impl fmt::Debug for CardView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CardView").field("count", &self.count).finish_non_exhaustive()
    }
}

impl CardView {
    /// `count` cards, each the view `card` gives for its index.
    pub fn new(count: usize, card: impl Fn(usize) -> AnyView + 'static) -> Self {
        Self {
            count,
            card: Rc::new(card),
            card_width: 240.0,
            card_height: 160.0,
            height: None,
            mode: SelectionMode::Single,
            selection: None,
            on_open: None,
            card_name: None,
            label: None,
        }
    }

    /// The narrowest a card gets; they share what is left over. 240 by default.
    pub fn card_width(mut self, width: f32) -> Self {
        self.card_width = width.max(1.0);
        self
    }

    /// Height of every card in logical pixels. 160 by default.
    pub fn card_height(mut self, height: f32) -> Self {
        self.card_height = height.max(1.0);
        self
    }

    /// Scrolls inside this height. Without it, the view fills what its parent gives.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// How cards are selected. One at a time by default.
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.mode = mode;
        self
    }

    /// The indexes selected.
    pub fn selection(mut self, selection: Binding<BTreeSet<usize>>) -> Self {
        self.selection = Some(selection);
        self
    }

    /// Runs with the index on Enter or on a double click.
    pub fn on_open(mut self, handler: impl Fn(usize) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }

    /// What a screen reader says for each card, since the card's content is free.
    pub fn card_name(mut self, name: impl Fn(usize) -> String + 'static) -> Self {
        self.card_name = Some(Rc::new(name));
        self
    }

    /// The name screen readers announce.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Picks `index`, as a click or Space does in this mode.
fn select_card(selection: &Binding<BTreeSet<usize>>, mode: SelectionMode, index: usize) {
    match mode {
        SelectionMode::None => {}
        SelectionMode::Single => selection.set(BTreeSet::from([index])),
        _ => selection.update(|set| {
            if !set.remove(&index) {
                set.insert(index);
            }
        }),
    }
}

struct CardItem<'a> {
    index: usize,
    view: &'a CardView,
    width: f32,
    selection: Binding<BTreeSet<usize>>,
    selected: bool,
    active: bool,
    focused: bool,
    cursor: State<usize>,
}

impl View for CardItem<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let content = (self.view.card)(self.index);
        let inner = cx.child(content.as_ref());
        let accent = theme.color(Semantic::Accent);
        let edge = if self.selected || (self.active && self.focused) {
            Border { width: 1.5, color: accent }
        } else if cx.is_hovered() {
            Border { width: 1.0, color: theme.color(Semantic::Border).lerp(theme.color(Semantic::Ink), 0.25) }
        } else {
            Border { width: 1.0, color: theme.color(Semantic::Border) }
        };
        let fill = if self.selected { accent.with_alpha(0.08) } else { theme.color(Semantic::Surface) };
        let node = cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: Some(self.view.card_height),
                padding: Insets::uniform(12.0),
                overflow_hidden: true,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[inner],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(edge),
                ..QuadPaint::new(fill)
            }),
        );
        let (selection, mode, index, cursor) =
            (self.selection.clone(), self.view.mode, self.index, self.cursor.clone());
        cx.on_click(
            node,
            Rc::new(move || {
                cursor.set(index);
                select_card(&selection, mode, index);
            }),
        );
        if let Some(open) = self.view.on_open.clone() {
            cx.on_double_click(node, Rc::new(move || open(index)));
        }
        cx.cursor(node, CursorIcon::Pointer);
        let mut semantics = Semantics::new(Role::ListItem).selected(self.selected).active(self.active);
        if let Some(name) = &self.view.card_name {
            semantics = semantics.name(name(self.index));
        }
        cx.semantics(node, semantics);
        node
    }
}

impl View for CardView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let local: State<BTreeSet<usize>> = cx.use_state(BTreeSet::new);
        let selection = self.selection.clone().unwrap_or_else(|| local.binding());
        let cursor: State<usize> = cx.use_state(|| 0);
        let reveal: State<bool> = cx.use_state(|| false);
        let viewport = cx.scroll_viewport();
        let width = viewport.map_or(cx.viewport().width, |size| size.width).max(1.0);
        let height = viewport.map_or(self.height.unwrap_or(cx.viewport().height), |size| size.height);
        let per_row = (((width + TILE_GAP) / (self.card_width + TILE_GAP)).floor() as usize).max(1);
        let card_width = (width - TILE_GAP * (per_row - 1) as f32) / per_row as f32;
        let row_step = self.card_height + TILE_GAP;
        let rows = self.count.div_ceil(per_row);
        let active = cursor.get().min(self.count.saturating_sub(1));
        let mut offset = cx.scroll_offset();
        if *reveal.peek() {
            reveal.set_if_changed(false);
            let top = (active / per_row) as f32 * row_step;
            if top < offset.y {
                offset.y = top;
                cx.set_scroll_offset(offset);
            } else if top + self.card_height > offset.y + height {
                offset.y = top + self.card_height - height;
                cx.set_scroll_offset(offset);
            }
        }
        let first = ((offset.y / row_step).floor() as usize).saturating_sub(CARD_OVERSCAN).min(rows);
        let last = (((offset.y + height) / row_step).ceil() as usize + CARD_OVERSCAN).min(rows);
        let selected = selection.get();
        let focused = cx.is_focused();
        let mut lines = Vec::with_capacity(last.saturating_sub(first) + 2);
        lines.push(cx.container(
            LayoutStyle { height: Some(first as f32 * row_step), ..LayoutStyle::leaf() },
            &[],
            Paint::None,
        ));
        for row in first..last {
            let mut cards = Vec::with_capacity(per_row);
            for index in (row * per_row)..((row + 1) * per_row).min(self.count) {
                let item = CardItem {
                    index,
                    view: self,
                    width: card_width,
                    selection: selection.clone(),
                    selected: selected.contains(&index),
                    active: index == active,
                    focused,
                    cursor: cursor.clone(),
                };
                cards.push(cx.keyed_child(index as u64, &item));
            }
            lines.push(cx.container(
                LayoutStyle {
                    height: Some(row_step),
                    spacing: TILE_GAP,
                    align: Align::Start,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &cards,
                Paint::None,
            ));
        }
        lines.push(cx.container(
            LayoutStyle { height: Some(rows.saturating_sub(last) as f32 * row_step), ..LayoutStyle::leaf() },
            &[],
            Paint::None,
        ));
        let content = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &lines,
            Paint::None,
        );
        let body = cx.container(
            LayoutStyle {
                height: self.height,
                grow: if self.height.is_some() { 0.0 } else { 1.0 },
                min_height: Some(0.0),
                overflow_hidden: true,
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::None,
        );
        cx.scrollable_virtual(body, false);
        let name = self.label.clone().unwrap_or_else(|| cx.labels().cards_name.clone());
        cx.semantics(body, Semantics::new(Role::List).name(name).description(self.count.to_string()));
        cx.focusable(body, 4.0);
        let (count, mode, open) = (self.count, self.mode, self.on_open.clone());
        cx.on_key(
            body,
            Rc::new(move |key, _| {
                if count == 0 {
                    return false;
                }
                let go = |index: usize| {
                    cursor.set(index.min(count - 1));
                    reveal.set(true);
                };
                match key {
                    Key::ArrowRight => go(active + 1),
                    Key::ArrowLeft => go(active.saturating_sub(1)),
                    Key::ArrowDown => go((active + per_row).min(count - 1)),
                    Key::ArrowUp => go(active.saturating_sub(per_row)),
                    Key::Home => go(0),
                    Key::End => go(count - 1),
                    Key::Space => select_card(&selection, mode, active),
                    Key::Enter => {
                        if let Some(open) = &open {
                            open(active);
                        }
                    }
                    _ => return false,
                }
                true
            }),
        );
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spot(key: &str, columns: u16, rows: u16) -> TileSpot {
        TileSpot::new(key, columns, rows)
    }

    #[test]
    fn tiles_take_the_first_place_they_fit() {
        let spots = [spot("a", 2, 1), spot("b", 1, 2), spot("c", 1, 1), spot("d", 3, 1), spot("e", 1, 1)];
        let places = place(&spots, 3);
        assert_eq!(places, [(0, 0), (2, 0), (0, 1), (0, 2), (1, 1)]);
        assert_eq!(rows_used(&spots, &places), 3);
    }

    #[test]
    fn a_tile_wider_than_the_grid_takes_its_whole_width() {
        let spots = [spot("a", 4, 1), spot("b", 1, 1)];
        assert_eq!(place(&spots, 2), [(0, 0), (0, 1)]);
        assert_eq!(place(&spots, 1), [(0, 0), (0, 1)]);
    }

    #[test]
    fn a_saved_layout_keeps_up_with_the_tiles() {
        let saved = [spot("gone", 1, 1), spot("b", 2, 1)];
        let defaults = [spot("a", 1, 1), spot("b", 1, 1)];
        assert_eq!(reconcile(&saved, &defaults), [spot("b", 2, 1), spot("a", 1, 1)]);
    }

    #[test]
    fn moving_lands_on_the_tile_under_the_cell() {
        let state = ravenrs_reactive::State::new(vec![spot("a", 1, 1), spot("b", 1, 1), spot("c", 1, 1)]);
        let cells = Cells { columns: 3, column_step: 100.0, row_step: 100.0 };
        move_to_cell(&state.binding(), "a", cells, (2, 0));
        let order: Vec<String> = state.get().into_iter().map(|s| s.key).collect();
        assert_eq!(order, ["b", "c", "a"]);
    }

    #[test]
    fn selecting_a_card_follows_the_mode() {
        let state = ravenrs_reactive::State::new(BTreeSet::from([1]));
        select_card(&state.binding(), SelectionMode::Single, 3);
        assert_eq!(state.get(), BTreeSet::from([3]));
        select_card(&state.binding(), SelectionMode::Multiple, 4);
        assert_eq!(state.get(), BTreeSet::from([3, 4]));
        select_card(&state.binding(), SelectionMode::Multiple, 3);
        assert_eq!(state.get(), BTreeSet::from([4]));
        select_card(&state.binding(), SelectionMode::None, 9);
        assert_eq!(state.get(), BTreeSet::from([4]));
    }
}
