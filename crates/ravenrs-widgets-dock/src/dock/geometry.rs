//! Where everything of a [`DockArea`](crate::DockArea) goes, from the arrangement and the room, and
//! what lies under the pointer during a drag. No drawing here: rectangles only, relative to the
//! area.

use ravenrs_core::{Point, Rect, Size};
use ravenrs_layout::Axis;

use super::layout::{DockChild, DockDrop, DockEdge, DockLayout, DockNode, DockSide};
use crate::reorder;

/// How far inside a group, as a part of it, a drop still means its edge.
const EDGE_PART: f32 = 0.25;
/// How close to the border of the area a drop means a dock that is not showing.
const BORDER_ZONE: f32 = 28.0;
/// How close to the top a drop opens the top dock. Much thinner than the other borders: the tabs of
/// the groups at the top lie under it, and dropping among them has to keep working.
const TOP_ZONE: f32 = 4.0;
/// How thick the rail is where the panels hidden at the edge of a dock have their tabs.
#[doc(hidden)]
pub const RAIL: f32 = 28.0;
/// How thick the line that marks a gap between tabs is.
const GAP_LINE: f32 = 2.0;

/// The measures an arrangement is laid out with.
#[derive(Clone, Copy, Debug, PartialEq)]
#[doc(hidden)]
pub struct Metrics {
    /// How thick a bar between parts is.
    pub bar: f32,
    /// How thick the strip that collapses a dock is, or zero without one.
    pub toggle: f32,
    /// The smallest and the largest the left dock gets.
    pub left: (f32, f32),
    /// The smallest and the largest the right dock gets.
    pub right: (f32, f32),
    /// The smallest and the largest the bottom dock gets.
    pub bottom: (f32, f32),
    /// The smallest and the largest the top dock gets.
    pub top: (f32, f32),
    /// The room the center keeps, which docks do not take from it.
    pub center_min: (f32, f32),
    /// The smallest a group gets along a split.
    pub pane_min: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Metrics {
    fn limits(&self, side: DockSide) -> (f32, f32) {
        match side {
            DockSide::Left => self.left,
            DockSide::Right => self.right,
            DockSide::Bottom => self.bottom,
            DockSide::Top => self.top,
            _ => (0.0, f32::MAX),
        }
    }
}

/// A group of tabs and where it goes.
#[derive(Clone, Debug, PartialEq)]
#[doc(hidden)]
pub struct GroupPlace {
    pub panels: Vec<String>,
    pub active: usize,
    pub rect: Rect,
}

/// What a bar changes.
#[derive(Clone, Debug, PartialEq)]
#[doc(hidden)]
pub enum BarKind {
    /// The size of a dock.
    Dock(DockSide),
    /// A pair in a split, of `both` logical pixels in all.
    Split { side: DockSide, path: Vec<usize>, index: usize, both: f32, names: (String, String) },
}

/// A bar between two parts and where it goes. It lies across `axis`.
#[derive(Clone, Debug, PartialEq)]
#[doc(hidden)]
pub struct BarPlace {
    pub kind: BarKind,
    pub axis: Axis,
    pub rect: Rect,
    /// The size it controls now, and the limits of that size.
    pub value: f32,
    pub min: f32,
    pub max: f32,
}

/// Everything laid out.
#[derive(Clone, Debug, Default, PartialEq)]
#[doc(hidden)]
pub struct Arranged {
    pub size: Size,
    pub groups: Vec<GroupPlace>,
    pub bars: Vec<BarPlace>,
    /// The strips that collapse the docks.
    pub toggles: Vec<(DockSide, Rect)>,
    /// The rails at the edge of the docks that have panels hidden there.
    pub rails: Vec<(DockSide, Rect)>,
    /// What the center gets, whatever is in it.
    pub center: Rect,
    /// Where each dock that holds nothing or is collapsed would go, for a drop that opens it.
    pub ghosts: Vec<(DockSide, Rect)>,
}

/// The sizes the parts of a split get in `room`: their shares, with each part at least at its
/// smallest and the difference taken from those that have some to spare.
#[doc(hidden)]
pub fn distribute(room: f32, shares: &[f32], mins: &[f32]) -> Vec<f32> {
    let mut sizes: Vec<f32> = shares.iter().zip(mins).map(|(share, min)| (share * room).max(*min)).collect();
    let mut over = sizes.iter().sum::<f32>() - room;
    if over > 0.0 {
        let mut spare: f32 = sizes.iter().zip(mins).map(|(size, min)| size - min).sum();
        if spare > 0.0 {
            let part = (over / spare).min(1.0);
            for (size, min) in sizes.iter_mut().zip(mins) {
                *size -= (*size - min) * part;
            }
            spare = over.min(spare);
            over -= spare;
        }
        // Too little room for every smallest: the last parts give way, down to nothing.
        for size in sizes.iter_mut().rev() {
            if over <= 0.0 {
                break;
            }
            let taken = size.min(over);
            *size -= taken;
            over -= taken;
        }
    }
    sizes
}

/// The least a node can be along `axis`.
fn least(node: &DockNode, axis: Axis, metrics: &Metrics) -> f32 {
    match node {
        DockNode::Tabs { .. } => metrics.pane_min,
        DockNode::Split { axis: own, children } => {
            let parts = children.iter().map(|c| least(&c.node, axis, metrics));
            if *own == axis {
                parts.sum::<f32>() + children.len().saturating_sub(1) as f32 * metrics.bar
            } else {
                parts.fold(0.0, f32::max)
            }
        }
    }
}

/// The name of a pair: what shows on either side of a bar.
fn shown_name(node: &DockNode) -> String {
    match node {
        DockNode::Tabs { panels, active } => panels.get(*active).cloned().unwrap_or_default(),
        DockNode::Split { children, .. } => children.first().map(|c| shown_name(&c.node)).unwrap_or_default(),
    }
}

fn place_node(
    node: &DockNode,
    rect: Rect,
    side: DockSide,
    path: &mut Vec<usize>,
    metrics: &Metrics,
    out: &mut Arranged,
) {
    match node {
        DockNode::Tabs { panels, active } => {
            out.groups.push(GroupPlace { panels: panels.clone(), active: *active, rect });
        }
        DockNode::Split { axis, children } => place_split(*axis, children, rect, side, path, metrics, out),
    }
}

fn place_split(
    axis: Axis,
    children: &[DockChild],
    rect: Rect,
    side: DockSide,
    path: &mut Vec<usize>,
    metrics: &Metrics,
    out: &mut Arranged,
) {
    let vertical = axis == Axis::Vertical;
    let length = if vertical { rect.height() } else { rect.width() };
    let room = (length - children.len().saturating_sub(1) as f32 * metrics.bar).max(0.0);
    let shares: Vec<f32> = children.iter().map(|c| c.share).collect();
    let mins: Vec<f32> = children.iter().map(|c| least(&c.node, axis, metrics)).collect();
    let sizes = distribute(room, &shares, &mins);
    let mut at = if vertical { rect.y() } else { rect.x() };
    for (index, child) in children.iter().enumerate() {
        let size = sizes.get(index).copied().unwrap_or(0.0);
        let part = if vertical {
            Rect::new(rect.x(), at, rect.width(), size)
        } else {
            Rect::new(at, rect.y(), size, rect.height())
        };
        path.push(index);
        place_node(&child.node, part, side, path, metrics, out);
        path.pop();
        at += size;
        if let (Some(next), Some(next_size), Some(next_min)) =
            (children.get(index + 1), sizes.get(index + 1), mins.get(index + 1))
        {
            let bar = if vertical {
                Rect::new(rect.x(), at, rect.width(), metrics.bar)
            } else {
                Rect::new(at, rect.y(), metrics.bar, rect.height())
            };
            let both = size + next_size;
            let min = mins[index].min(both);
            out.bars.push(BarPlace {
                kind: BarKind::Split {
                    side,
                    path: path.clone(),
                    index,
                    both,
                    names: (shown_name(&child.node), shown_name(&next.node)),
                },
                axis,
                rect: bar,
                value: size,
                min,
                max: (both - next_min).max(min),
            });
            at += metrics.bar;
        }
    }
}

/// How large a dock gets out of `room`, and how large it could get while the center and `taken`
/// keep theirs.
fn dock_size(
    layout: &DockLayout,
    side: DockSide,
    room: f32,
    taken: f32,
    keep: f32,
    metrics: &Metrics,
) -> (f32, f32) {
    let (min, max) = metrics.limits(side);
    let most = (room - taken - keep - metrics.bar).clamp(min, max.max(min));
    (layout.size(side).clamp(min, most.max(min)).min((room - taken).max(0.0)), most)
}

/// Lays an arrangement out in an area of `size`.
#[doc(hidden)]
pub fn arrange(layout: &DockLayout, size: Size, metrics: &Metrics) -> Arranged {
    let mut out = Arranged { size, ..Arranged::default() };
    let has = |side| layout.node(side).is_some();
    let open = |side| has(side) && layout.is_open(side);
    let mut x0 = 0.0;
    let mut x1 = size.width;
    let mut y1 = size.height;
    let toggle = metrics.toggle;
    let toggles = |side| has(side) && toggle > 0.0;
    let rails = |side| !layout.auto_hidden_panels(side).is_empty();
    // The rails go outermost, then the strips, then the docks.
    if rails(DockSide::Left) {
        out.rails.push((DockSide::Left, Rect::new(0.0, 0.0, RAIL, size.height)));
        x0 += RAIL;
    }
    if rails(DockSide::Right) {
        x1 -= RAIL;
        out.rails.push((DockSide::Right, Rect::new(x1, 0.0, RAIL, size.height)));
    }
    if toggles(DockSide::Left) {
        out.toggles.push((DockSide::Left, Rect::new(x0, 0.0, toggle, size.height)));
        x0 += toggle;
    }
    if toggles(DockSide::Right) {
        x1 -= toggle;
        out.toggles.push((DockSide::Right, Rect::new(x1, 0.0, toggle, size.height)));
    }
    let width = (x1 - x0).max(0.0);
    let (center_w, center_h) = metrics.center_min;
    // The right gives way first when a narrow window has too little for both.
    let left_size = open(DockSide::Left).then(|| {
        let right_least = if open(DockSide::Right) { metrics.right.0 + metrics.bar } else { 0.0 };
        dock_size(layout, DockSide::Left, width, right_least, center_w, metrics)
    });
    let right_size = open(DockSide::Right).then(|| {
        let taken = left_size.map_or(0.0, |(left, _)| left + metrics.bar);
        dock_size(layout, DockSide::Right, width, taken, center_w, metrics)
    });
    let mut path = Vec::new();
    if let (Some((left, most)), Some(node)) = (left_size, layout.node(DockSide::Left)) {
        place_node(node, Rect::new(x0, 0.0, left, size.height), DockSide::Left, &mut path, metrics, &mut out);
        x0 += left;
        let (min, _) = metrics.left;
        out.bars.push(BarPlace {
            kind: BarKind::Dock(DockSide::Left),
            axis: Axis::Horizontal,
            rect: Rect::new(x0, 0.0, metrics.bar, size.height),
            value: left,
            min,
            max: most,
        });
        x0 += metrics.bar;
    } else if !open(DockSide::Left) {
        let (min, max) = metrics.left;
        let ghost = layout.size(DockSide::Left).clamp(min, max).min(width / 2.0);
        out.ghosts.push((DockSide::Left, Rect::new(x0, 0.0, ghost, size.height)));
    }
    if let (Some((right, most)), Some(node)) = (right_size, layout.node(DockSide::Right)) {
        x1 -= right;
        place_node(
            node,
            Rect::new(x1, 0.0, right, size.height),
            DockSide::Right,
            &mut path,
            metrics,
            &mut out,
        );
        x1 -= metrics.bar;
        let (min, _) = metrics.right;
        out.bars.push(BarPlace {
            kind: BarKind::Dock(DockSide::Right),
            axis: Axis::Horizontal,
            rect: Rect::new(x1, 0.0, metrics.bar, size.height),
            value: right,
            min,
            max: most,
        });
    } else if !open(DockSide::Right) {
        let (min, max) = metrics.right;
        let ghost = layout.size(DockSide::Right).clamp(min, max).min(width / 2.0);
        out.ghosts.push((DockSide::Right, Rect::new(x1 - ghost, 0.0, ghost, size.height)));
    }
    let middle = (x1 - x0).max(0.0);
    let mut y0 = 0.0;
    if rails(DockSide::Top) {
        out.rails.push((DockSide::Top, Rect::new(x0, 0.0, middle, RAIL)));
        y0 += RAIL;
    }
    if rails(DockSide::Bottom) {
        y1 -= RAIL;
        out.rails.push((DockSide::Bottom, Rect::new(x0, y1, middle, RAIL)));
    }
    if toggles(DockSide::Top) {
        out.toggles.push((DockSide::Top, Rect::new(x0, y0, middle, toggle)));
        y0 += toggle;
    }
    if toggles(DockSide::Bottom) {
        y1 -= toggle;
        out.toggles.push((DockSide::Bottom, Rect::new(x0, y1, middle, toggle)));
    }
    let height = (y1 - y0).max(0.0);
    // The top gives way first when a short window has too little for both.
    let bottom_size = open(DockSide::Bottom).then(|| {
        let top_least = if open(DockSide::Top) { metrics.top.0 + metrics.bar } else { 0.0 };
        dock_size(layout, DockSide::Bottom, height, top_least, center_h, metrics)
    });
    let top_size = open(DockSide::Top).then(|| {
        let taken = bottom_size.map_or(0.0, |(bottom, _)| bottom + metrics.bar);
        dock_size(layout, DockSide::Top, height, taken, center_h, metrics)
    });
    if let (Some((bottom, most)), Some(node)) = (bottom_size, layout.node(DockSide::Bottom)) {
        y1 -= bottom;
        place_node(node, Rect::new(x0, y1, middle, bottom), DockSide::Bottom, &mut path, metrics, &mut out);
        y1 -= metrics.bar;
        let (min, _) = metrics.bottom;
        out.bars.push(BarPlace {
            kind: BarKind::Dock(DockSide::Bottom),
            axis: Axis::Vertical,
            rect: Rect::new(x0, y1, middle, metrics.bar),
            value: bottom,
            min,
            max: most,
        });
    } else if !open(DockSide::Bottom) {
        let (min, max) = metrics.bottom;
        let ghost = layout.size(DockSide::Bottom).clamp(min, max).min(height / 2.0);
        out.ghosts.push((DockSide::Bottom, Rect::new(x0, y1 - ghost, middle, ghost)));
    }
    if let (Some((top, most)), Some(node)) = (top_size, layout.node(DockSide::Top)) {
        place_node(node, Rect::new(x0, y0, middle, top), DockSide::Top, &mut path, metrics, &mut out);
        y0 += top;
        let (min, _) = metrics.top;
        out.bars.push(BarPlace {
            kind: BarKind::Dock(DockSide::Top),
            axis: Axis::Vertical,
            rect: Rect::new(x0, y0, middle, metrics.bar),
            value: top,
            min,
            max: most,
        });
        y0 += metrics.bar;
    } else if !open(DockSide::Top) {
        let (min, max) = metrics.top;
        let ghost = layout.size(DockSide::Top).clamp(min, max).min(height / 2.0);
        out.ghosts.push((DockSide::Top, Rect::new(x0, y0, middle, ghost)));
    }
    out.center = Rect::new(x0, y0, middle, (y1 - y0).max(0.0));
    if let Some(node) = layout.node(DockSide::Center) {
        place_node(node, out.center, DockSide::Center, &mut path, metrics, &mut out);
    }
    out
}

/// What a tab being dragged would do if dropped here.
#[derive(Clone, Debug, PartialEq)]
#[doc(hidden)]
pub struct DropAim {
    pub target: DockDrop,
    /// What to mark, relative to the area.
    pub mark: Rect,
}

/// Which edge of `rect` the point counts for, if it is close enough to one.
fn edge_of(rect: Rect, at: Point) -> Option<DockEdge> {
    let part = |near: f32, length: f32| if length > 0.0 { near / length } else { 1.0 };
    let candidates = [
        (part(at.x - rect.x(), rect.width()), DockEdge::Left),
        (part(rect.max_x() - at.x, rect.width()), DockEdge::Right),
        (part(at.y - rect.y(), rect.height()), DockEdge::Top),
        (part(rect.max_y() - at.y, rect.height()), DockEdge::Bottom),
    ];
    candidates
        .into_iter()
        .filter(|(near, _)| *near < EDGE_PART)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, edge)| edge)
}

/// The half of `rect` a split on `edge` would take.
fn half(rect: Rect, edge: DockEdge) -> Rect {
    let (w, h) = (rect.width() / 2.0, rect.height() / 2.0);
    match edge {
        DockEdge::Left => Rect::new(rect.x(), rect.y(), w, rect.height()),
        DockEdge::Right => Rect::new(rect.x() + w, rect.y(), w, rect.height()),
        DockEdge::Top => Rect::new(rect.x(), rect.y(), rect.width(), h),
        DockEdge::Bottom => Rect::new(rect.x(), rect.y() + h, rect.width(), h),
    }
}

/// What lies under the pointer at `at` for the panel `dragged`. `strip` says how tall the tabs of
/// a group are, and `tab` where a tab is on screen now, relative to the area. It finds a place
/// whether or not dropping there changes anything; the caller tries that.
#[doc(hidden)]
pub fn aim(
    arranged: &Arranged,
    at: Point,
    strip: &dyn Fn(&GroupPlace) -> f32,
    tab: &dyn Fn(&str) -> Option<Rect>,
) -> Option<DropAim> {
    let size = arranged.size;
    if at.x < 0.0 || at.y < 0.0 || at.x > size.width || at.y > size.height {
        return None;
    }
    // A dock that does not show opens from the border it belongs to.
    for (side, rect) in &arranged.ghosts {
        let near = match side {
            DockSide::Left => at.x < BORDER_ZONE,
            DockSide::Right => at.x > size.width - BORDER_ZONE,
            DockSide::Bottom => at.y > size.height - BORDER_ZONE,
            DockSide::Top => at.y < TOP_ZONE && at.x >= rect.x() && at.x <= rect.max_x(),
            _ => false,
        };
        if near {
            return Some(DropAim { target: DockDrop::Side(*side), mark: *rect });
        }
    }
    let group = arranged.groups.iter().find(|group| group.rect.contains(at));
    let Some(group) = group else {
        return arranged
            .center
            .contains(at)
            .then_some(DropAim { target: DockDrop::Side(DockSide::Center), mark: arranged.center });
    };
    let first = group.panels.first()?;
    let height = strip(group);
    if at.y < group.rect.y() + height {
        return Some(aim_tab(group, first, at, height, tab));
    }
    let body = Rect::new(
        group.rect.x(),
        group.rect.y() + height,
        group.rect.width(),
        (group.rect.height() - height).max(0.0),
    );
    Some(match edge_of(body, at) {
        Some(edge) => {
            DropAim { target: DockDrop::Split { beside: first.clone(), edge }, mark: half(body, edge) }
        }
        None => {
            DropAim { target: DockDrop::Tab { beside: first.clone(), at: group.panels.len() }, mark: body }
        }
    })
}

/// What a floating panel dragged by its header would do if let go here. Only the tabs of a group
/// and the borders of the docks that do not show take it, so that it passes over the groups
/// without landing in them; with `splits`, everything that takes a tab takes it.
#[doc(hidden)]
pub fn float_aim(
    arranged: &Arranged,
    at: Point,
    strip: &dyn Fn(&GroupPlace) -> f32,
    tab: &dyn Fn(&str) -> Option<Rect>,
    splits: bool,
) -> Option<DropAim> {
    let found = aim(arranged, at, strip, tab)?;
    if splits {
        return Some(found);
    }
    let on_tabs =
        arranged.groups.iter().any(|group| group.rect.contains(at) && at.y < group.rect.y() + strip(group));
    (on_tabs || matches!(found.target, DockDrop::Side(_))).then_some(found)
}

/// Where a floating panel shows in an area of `size`: moved to fit, and made smaller only when it
/// cannot fit at all, never below `least` while the area has room for that.
#[doc(hidden)]
pub fn clamp_floating(rect: Rect, size: Size, least: Size) -> Rect {
    let room = |want: f32, least: f32, room: f32| want.max(least.min(room)).min(room.max(0.0));
    let width = room(rect.width(), least.width, size.width);
    let height = room(rect.height(), least.height, size.height);
    let x = rect.x().clamp(0.0, (size.width - width).max(0.0));
    let y = rect.y().clamp(0.0, (size.height - height).max(0.0));
    Rect::new(x, y, width, height)
}

/// A floating panel's box drawn to the edges near it: of the area, and of the other floating
/// panels, when one is within `reach`.
#[doc(hidden)]
pub fn snap_floating(rect: Rect, size: Size, others: &[Rect], reach: f32) -> Rect {
    let nearest = |at: f32, lines: &[f32]| {
        lines
            .iter()
            .copied()
            .filter(|line| (line - at).abs() <= reach)
            .min_by(|a, b| (a - at).abs().total_cmp(&(b - at).abs()))
    };
    let (width, height) = (rect.width(), rect.height());
    let mut xs = vec![0.0, size.width - width];
    let mut ys = vec![0.0, size.height - height];
    for other in others {
        xs.extend([other.max_x(), other.x() - width, other.x(), other.max_x() - width]);
        ys.extend([other.max_y(), other.y() - height, other.y(), other.max_y() - height]);
    }
    let x = nearest(rect.x(), &xs).unwrap_or(rect.x());
    let y = nearest(rect.y(), &ys).unwrap_or(rect.y());
    Rect::new(x, y, width, height)
}

fn aim_tab(
    group: &GroupPlace,
    first: &str,
    at: Point,
    height: f32,
    tab: &dyn Fn(&str) -> Option<Rect>,
) -> DropAim {
    let frames: Vec<Option<Rect>> = group.panels.iter().map(|panel| tab(panel)).collect();
    let start = frames.first().copied().flatten().map_or(group.rect.x(), |frame| frame.x());
    let extents: Vec<f32> = frames.iter().map(|frame| frame.map_or(0.0, |f| f.width())).collect();
    let spacing = match (frames.first().copied().flatten(), frames.get(1).copied().flatten()) {
        (Some(a), Some(b)) => (b.x() - a.max_x()).max(0.0),
        _ => 0.0,
    };
    let gap = reorder::gap_at(&extents, spacing, at.x - start);
    let line = frames
        .get(gap)
        .copied()
        .flatten()
        .map(|frame| frame.x())
        .or_else(|| frames.last().copied().flatten().map(|frame| frame.max_x()))
        .unwrap_or(group.rect.x());
    DropAim {
        target: DockDrop::Tab { beside: first.to_owned(), at: gap },
        mark: Rect::new(line - GAP_LINE / 2.0, group.rect.y(), GAP_LINE, height),
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const METRICS: Metrics = Metrics {
        bar: 10.0,
        toggle: 12.0,
        left: (120.0, 500.0),
        right: (120.0, 500.0),
        bottom: (80.0, 400.0),
        top: (60.0, 400.0),
        center_min: (160.0, 120.0),
        pane_min: 80.0,
    };

    fn full() -> DockLayout {
        DockLayout::new()
            .center(DockNode::horizontal([DockNode::tabs(["a", "b"]), DockNode::tabs(["c"])]))
            .left(DockNode::tabs(["d"]), 200.0)
            .right(DockNode::tabs(["e"]), 220.0)
            .bottom(DockNode::tabs(["f"]), 150.0)
    }

    fn no_strip(_: &GroupPlace) -> f32 {
        30.0
    }

    #[test]
    fn distribute_follows_the_shares_and_keeps_each_smallest() {
        assert_eq!(distribute(300.0, &[0.5, 0.5], &[10.0, 10.0]), [150.0, 150.0]);
        assert_eq!(distribute(300.0, &[0.9, 0.1], &[10.0, 100.0]), [200.0, 100.0]);
        let tight = distribute(100.0, &[0.5, 0.5], &[80.0, 80.0]);
        assert_eq!(tight.iter().sum::<f32>(), 100.0, "the room is what there is: {tight:?}");
    }

    #[test]
    fn the_pieces_fill_the_area_with_the_center_keeping_its_room() {
        let arranged = arrange(&full(), Size::new(1000.0, 700.0), &METRICS);
        assert_eq!(arranged.groups.len(), 5);
        assert_eq!(arranged.toggles.len(), 3);
        assert_eq!(arranged.bars.len(), 4);
        assert_eq!(
            arranged.center,
            Rect::new(12.0 + 200.0 + 10.0, 0.0, 1000.0 - 24.0 - 420.0 - 20.0, 700.0 - 12.0 - 150.0 - 10.0)
        );
    }

    #[test]
    fn the_top_dock_sits_between_the_columns_above_the_center() {
        let layout = full().top(DockNode::tabs(["t"]), 100.0);
        let arranged = arrange(&layout, Size::new(1000.0, 700.0), &METRICS);
        let top = arranged.groups.iter().find(|g| g.panels == ["t"]).expect("the top group");
        let (x0, middle) = (12.0 + 200.0 + 10.0, 1000.0 - 24.0 - 420.0 - 20.0);
        assert_eq!(top.rect, Rect::new(x0, 12.0, middle, 100.0), "under its strip, as wide as the center");
        let bar = arranged.bars.iter().find(|b| b.kind == BarKind::Dock(DockSide::Top)).expect("its bar");
        assert_eq!(bar.rect, Rect::new(x0, 112.0, middle, 10.0));
        assert_eq!(bar.axis, Axis::Vertical);
        assert_eq!(arranged.center, Rect::new(x0, 122.0, middle, 700.0 - 24.0 - 150.0 - 10.0 - 110.0));
        assert!(arranged.toggles.iter().any(|(side, _)| *side == DockSide::Top));
    }

    #[test]
    fn a_short_area_takes_from_the_top_before_the_bottom_and_keeps_the_center() {
        let layout = full().top(DockNode::tabs(["t"]), 300.0);
        let arranged = arrange(&layout, Size::new(1000.0, 520.0), &METRICS);
        assert!(arranged.center.height() >= METRICS.center_min.1, "{:?}", arranged.center);
        let size = |side| arranged.bars.iter().find(|b| b.kind == BarKind::Dock(side)).expect("a bar").value;
        assert_eq!(size(DockSide::Bottom), 150.0, "the bottom keeps what it asked for");
        assert!(size(DockSide::Top) < 300.0, "the top gives way");
    }

    #[test]
    fn a_rail_takes_the_outer_edge_of_a_dock_with_hidden_panels() {
        let mut layout = full().left(DockNode::tabs(["d", "h"]), 200.0);
        assert!(layout.auto_hide("h"));
        let arranged = arrange(&layout, Size::new(1000.0, 700.0), &METRICS);
        assert_eq!(arranged.rails, [(DockSide::Left, Rect::new(0.0, 0.0, RAIL, 700.0))]);
        let strip = arranged.toggles.iter().find(|(side, _)| *side == DockSide::Left).expect("its strip");
        assert_eq!(strip.1.x(), RAIL, "the strip comes after the rail");
        let left = arranged.groups.iter().find(|g| g.panels == ["d"]).expect("the left dock");
        assert_eq!(left.rect.x(), RAIL + 12.0);
        assert!(layout.auto_hide("f"));
        let arranged = arrange(&layout, Size::new(1000.0, 700.0), &METRICS);
        let bottom = arranged.rails.iter().find(|(side, _)| *side == DockSide::Bottom).expect("a rail below");
        assert_eq!(bottom.1.max_y(), 700.0);
        assert_eq!(bottom.1.height(), RAIL);
    }

    #[test]
    fn a_floating_panel_lands_only_on_tabs_and_borders_unless_asked_for_more() {
        let arranged = arrange(&full(), Size::new(1000.0, 700.0), &METRICS);
        let center = arranged.groups.iter().find(|g| g.panels == ["a", "b"]).expect("a group").rect;
        let middle = Point::new(center.x() + 60.0, center.y() + 200.0);
        assert_eq!(float_aim(&arranged, middle, &no_strip, &|_| None, false), None, "it passes over a group");
        let split = float_aim(&arranged, middle, &no_strip, &|_| None, true).expect("with Shift");
        assert!(matches!(split.target, DockDrop::Tab { .. } | DockDrop::Split { .. }));
        let tabs = Point::new(center.x() + 20.0, center.y() + 10.0);
        let on_tabs = float_aim(&arranged, tabs, &no_strip, &|_| None, false).expect("on the tabs");
        assert!(matches!(on_tabs.target, DockDrop::Tab { .. }));
    }

    #[test]
    fn a_floating_box_stays_in_the_area_and_draws_to_edges_near_it() {
        let area = Size::new(800.0, 600.0);
        let least = Size::new(200.0, 120.0);
        assert_eq!(
            clamp_floating(Rect::new(700.0, 550.0, 300.0, 200.0), area, least),
            Rect::new(500.0, 400.0, 300.0, 200.0)
        );
        assert_eq!(
            clamp_floating(Rect::new(0.0, 0.0, 900.0, 50.0), area, least),
            Rect::new(0.0, 0.0, 800.0, 120.0)
        );
        let small = Size::new(150.0, 100.0);
        assert_eq!(
            clamp_floating(Rect::new(10.0, 10.0, 300.0, 200.0), small, least),
            Rect::new(0.0, 0.0, 150.0, 100.0)
        );
        let other = Rect::new(400.0, 100.0, 200.0, 200.0);
        let near = snap_floating(Rect::new(5.0, 106.0, 100.0, 80.0), area, &[other], 8.0);
        assert_eq!(near, Rect::new(0.0, 100.0, 100.0, 80.0), "to the border and to the top of the other");
        let far = snap_floating(Rect::new(30.0, 40.0, 100.0, 80.0), area, &[other], 8.0);
        assert_eq!(far, Rect::new(30.0, 40.0, 100.0, 80.0));
    }

    #[test]
    fn a_narrow_area_takes_from_the_docks_not_the_center() {
        let arranged = arrange(&full(), Size::new(620.0, 500.0), &METRICS);
        assert!(arranged.center.width() >= METRICS.center_min.0, "{:?}", arranged.center);
        let bar = arranged.bars.iter().find(|b| b.kind == BarKind::Dock(DockSide::Left)).expect("a bar");
        assert!(bar.value >= METRICS.left.0);
        assert!(bar.max < METRICS.left.1, "it can go only as far as leaves the center its room");
    }

    #[test]
    fn a_collapsed_dock_gives_its_room_back_and_keeps_its_strip() {
        let mut layout = full();
        layout.set_open(DockSide::Left, false);
        let arranged = arrange(&layout, Size::new(1000.0, 700.0), &METRICS);
        assert_eq!(arranged.center.x(), 12.0);
        assert!(arranged.toggles.iter().any(|(side, _)| *side == DockSide::Left));
        assert!(arranged.ghosts.iter().any(|(side, _)| *side == DockSide::Left));
    }

    #[test]
    fn dropping_goes_by_the_part_of_the_group_under_the_pointer() {
        let arranged = arrange(&full(), Size::new(1000.0, 700.0), &METRICS);
        let group = arranged.groups.iter().find(|g| g.panels == ["c"]).expect("the group");
        let r = group.rect;
        let at = |x: f32, y: f32| {
            aim(
                &arranged,
                Point::new(r.x() + x * r.width(), r.y() + 30.0 + y * (r.height() - 30.0)),
                &no_strip,
                &|_| None,
            )
        };
        assert_eq!(at(0.5, 0.5).expect("aim").target, DockDrop::Tab { beside: "c".into(), at: 1 });
        assert_eq!(
            at(0.1, 0.5).expect("aim").target,
            DockDrop::Split { beside: "c".into(), edge: DockEdge::Left }
        );
        assert_eq!(
            at(0.9, 0.5).expect("aim").target,
            DockDrop::Split { beside: "c".into(), edge: DockEdge::Right }
        );
        assert_eq!(
            at(0.5, 0.1).expect("aim").target,
            DockDrop::Split { beside: "c".into(), edge: DockEdge::Top }
        );
        assert_eq!(
            at(0.5, 0.9).expect("aim").target,
            DockDrop::Split { beside: "c".into(), edge: DockEdge::Bottom }
        );
    }

    #[test]
    fn dropping_on_the_tabs_goes_by_the_gap_nearest() {
        let arranged = arrange(&full(), Size::new(1000.0, 700.0), &METRICS);
        let group = arranged.groups.iter().find(|g| g.panels == ["a", "b"]).expect("the group").rect;
        let frame = |id: &str| match id {
            "a" => Some(Rect::new(group.x(), group.y(), 60.0, 30.0)),
            "b" => Some(Rect::new(group.x() + 62.0, group.y(), 60.0, 30.0)),
            _ => None,
        };
        let aim_at =
            aim(&arranged, Point::new(group.x() + 10.0, group.y() + 5.0), &no_strip, &frame).expect("aim");
        assert_eq!(aim_at.target, DockDrop::Tab { beside: "a".into(), at: 0 });
        assert_eq!(aim_at.mark.x(), group.x() - 1.0);
        let aim_at =
            aim(&arranged, Point::new(group.x() + 100.0, group.y() + 5.0), &no_strip, &frame).expect("aim");
        assert_eq!(aim_at.target, DockDrop::Tab { beside: "a".into(), at: 2 });
        assert_eq!(aim_at.mark.x(), group.x() + 122.0 - 1.0);
    }

    #[test]
    fn the_border_of_the_area_opens_a_dock_that_does_not_show() {
        let layout = DockLayout::new().center(DockNode::tabs(["a"]));
        let arranged = arrange(&layout, Size::new(800.0, 600.0), &METRICS);
        let left = aim(&arranged, Point::new(4.0, 300.0), &no_strip, &|_| None).expect("aim");
        assert_eq!(left.target, DockDrop::Side(DockSide::Left));
        let bottom = aim(&arranged, Point::new(400.0, 596.0), &no_strip, &|_| None).expect("aim");
        assert_eq!(bottom.target, DockDrop::Side(DockSide::Bottom));
        assert!(aim(&arranged, Point::new(900.0, 100.0), &no_strip, &|_| None).is_none(), "outside the area");
    }

    #[test]
    fn only_the_very_top_between_the_columns_opens_the_top_dock() {
        let arranged = arrange(&full(), Size::new(1000.0, 700.0), &METRICS);
        let x = arranged.center.x() + 40.0;
        let edge = aim(&arranged, Point::new(x, 2.0), &no_strip, &|_| None).expect("aim");
        assert_eq!(edge.target, DockDrop::Side(DockSide::Top));
        assert_eq!(edge.mark.y(), 0.0);
        let tabs = aim(&arranged, Point::new(x, 6.0), &no_strip, &|_| None).expect("aim");
        assert!(matches!(tabs.target, DockDrop::Tab { .. }), "a little lower it is the tabs again");
        let left = aim(&arranged, Point::new(60.0, 2.0), &no_strip, &|_| None).expect("aim");
        assert_ne!(left.target, DockDrop::Side(DockSide::Top), "above a column it is not the top dock");
    }

    #[test]
    fn an_empty_center_takes_a_drop_as_a_whole() {
        let layout = DockLayout::new().left(DockNode::tabs(["a"]), 200.0);
        let arranged = arrange(&layout, Size::new(800.0, 600.0), &METRICS);
        let aim_at = aim(&arranged, Point::new(500.0, 300.0), &no_strip, &|_| None).expect("aim");
        assert_eq!(aim_at.target, DockDrop::Side(DockSide::Center));
    }

    proptest! {
        #[test]
        fn nothing_lands_outside_the_area_or_on_a_number_that_is_not_one(
            width in 0.0..2000.0_f32,
            height in 0.0..1400.0_f32,
            left in 0.0..900.0_f32,
            right in 0.0..900.0_f32,
            share in 0.01..0.99_f32,
        ) {
            let mut layout = full();
            layout.set_size(DockSide::Left, left);
            layout.set_size(DockSide::Right, right);
            layout.resize_split(DockSide::Center, &[], 0, share * 100.0, 100.0);
            let arranged = arrange(&layout, Size::new(width, height), &METRICS);
            for group in &arranged.groups {
                let r = group.rect;
                prop_assert!(r.width().is_finite() && r.height().is_finite());
                prop_assert!(r.width() >= 0.0 && r.height() >= 0.0, "{:?}", r);
            }
            for bar in &arranged.bars {
                prop_assert!(bar.min <= bar.max + f32::EPSILON, "{:?}", bar);
            }
            // With room for everything's smallest, nothing goes past the border.
            if width > 800.0 && height > 500.0 {
                for group in &arranged.groups {
                    prop_assert!(group.rect.max_x() <= width + 0.5, "{:?}", group.rect);
                    prop_assert!(group.rect.max_y() <= height + 0.5, "{:?}", group.rect);
                }
            }
        }
    }
}
