//! The layout of a sankey diagram: where each node goes and the ribbon of each flow,
//! from a graph that must not go in circles. Pure logic, in the coordinates asked for.

use std::fmt;

use ravenrs_core::{Point, Rect};

use crate::vector::PathSegment;

/// Which column a node goes to, among those its flows allow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum SankeyAlign {
    /// As far left as its flows allow.
    Left,
    /// As far right as its flows allow.
    Right,
    /// Like [`Self::Left`], except that a node with nothing coming in sits right
    /// before the first thing it feeds.
    Center,
    /// Like [`Self::Left`], except that a node with nothing going out goes to the last
    /// column.
    #[default]
    Justify,
}

/// A flow from one node to another, given by their places in the list of nodes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SankeyLink {
    /// Where it comes from.
    pub source: usize,
    /// Where it goes.
    pub target: usize,
    /// How much. One that is not above zero has no ribbon, and does not count.
    pub value: f64,
}

/// How a graph is laid out.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct SankeyStyle {
    /// How wide a node is.
    pub node_width: f32,
    /// The room between two nodes of a column.
    pub node_gap: f32,
    /// Which column a node goes to.
    pub align: SankeyAlign,
    /// How many passes put nodes closer to what they connect to.
    pub passes: usize,
}

impl Default for SankeyStyle {
    fn default() -> Self {
        Self { node_width: 12.0, node_gap: 14.0, align: SankeyAlign::default(), passes: 16 }
    }
}

/// Why a graph has no layout.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SankeyError {
    /// The flows go in a circle, through these nodes, the first one again at the end.
    Cycle(Vec<usize>),
    /// A flow names a node past the end of the list.
    NoSuchNode(usize),
}

impl fmt::Display for SankeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cycle(nodes) => {
                let names: Vec<String> = nodes.iter().map(ToString::to_string).collect();
                write!(f, "the flows go in a circle: {}", names.join(" -> "))
            }
            Self::NoSuchNode(node) => write!(f, "a flow names node {node}, which does not exist"),
        }
    }
}

impl std::error::Error for SankeyError {}

/// A node as it landed.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct SankeyNode {
    /// Where it is.
    pub rect: Rect,
    /// The larger of what comes in and what goes out.
    pub value: f64,
    /// Which column.
    pub column: usize,
    /// The flows coming in, as places in [`SankeyLayout::flows`].
    pub incoming: Vec<usize>,
    /// The flows going out.
    pub outgoing: Vec<usize>,
}

/// A flow as it landed: a ribbon from the side of a node to the side of another.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct SankeyFlow {
    /// Where it comes from.
    pub source: usize,
    /// Where it goes.
    pub target: usize,
    /// How much.
    pub value: f64,
    /// Where it starts: on this side, at this height, its middle.
    pub from: Point,
    /// Where it ends.
    pub to: Point,
    /// How thick it is.
    pub width: f32,
}

impl SankeyFlow {
    /// The two control points' place along the way, halfway across.
    fn middle(&self) -> f32 {
        (self.from.x + self.to.x) / 2.0
    }

    /// The outline of the ribbon.
    pub fn path(&self) -> Vec<PathSegment> {
        let (half, middle) = (self.width / 2.0, self.middle());
        let at = Point::new;
        let (x0, x1, y0, y1) = (self.from.x, self.to.x, self.from.y, self.to.y);
        vec![
            PathSegment::Move(at(x0, y0 - half)),
            PathSegment::Cubic(at(middle, y0 - half), at(middle, y1 - half), at(x1, y1 - half)),
            PathSegment::Line(at(x1, y1 + half)),
            PathSegment::Cubic(at(middle, y1 + half), at(middle, y0 + half), at(x0, y0 + half)),
            PathSegment::Close,
        ]
    }

    /// Whether the ribbon covers this point.
    pub fn contains(&self, point: Point) -> bool {
        if point.x < self.from.x || point.x > self.to.x {
            return false;
        }
        let (x0, x1, middle) = (self.from.x, self.to.x, self.middle());
        let x_at = |t: f32| {
            let u = 1.0 - t;
            u * u * u * x0 + 3.0 * u * t * middle * (u + t) + t * t * t * x1
        };
        // Across, a curve only ever goes forward, so halving finds the place on it.
        let (mut low, mut high) = (0.0_f32, 1.0_f32);
        for _ in 0..16 {
            let t = (low + high) / 2.0;
            if x_at(t) < point.x {
                low = t;
            } else {
                high = t;
            }
        }
        let t = (low + high) / 2.0;
        let share = t * t * (3.0 - 2.0 * t);
        let y = self.from.y + (self.to.y - self.from.y) * share;
        (point.y - y).abs() <= self.width / 2.0
    }
}

/// What is under a point of a diagram.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SankeyHit {
    /// This node.
    Node(usize),
    /// This flow.
    Flow(usize),
}

/// A diagram laid out.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SankeyLayout {
    /// Every node, in the order given.
    pub nodes: Vec<SankeyNode>,
    /// Every flow with a value, in the order given.
    pub flows: Vec<SankeyFlow>,
    /// How many columns there are.
    pub columns: usize,
}

impl SankeyLayout {
    /// What is under `point`: a node, over the flows that reach it.
    pub fn hit(&self, point: Point) -> Option<SankeyHit> {
        if let Some(node) = self.nodes.iter().position(|node| node.rect.contains(point)) {
            return Some(SankeyHit::Node(node));
        }
        // The last one drawn is the one on top.
        self.flows.iter().rposition(|flow| flow.contains(point)).map(SankeyHit::Flow)
    }
}

/// Each node's order so that flows only go forward, or the circle that prevents it.
fn in_order(count: usize, links: &[SankeyLink]) -> Result<Vec<usize>, SankeyError> {
    let mut waiting = vec![0_usize; count];
    for link in links {
        waiting[link.target] += 1;
    }
    let mut ready: Vec<usize> = (0..count).filter(|node| waiting[*node] == 0).collect();
    let mut order = Vec::with_capacity(count);
    while let Some(node) = ready.pop() {
        order.push(node);
        for link in links.iter().filter(|link| link.source == node) {
            waiting[link.target] -= 1;
            if waiting[link.target] == 0 {
                ready.push(link.target);
            }
        }
    }
    if order.len() == count {
        return Ok(order);
    }
    // What is left all has something coming in from what is left: going backwards
    // from any of them comes to a node seen before.
    let Some(start) = (0..count).find(|node| waiting[*node] > 0) else { return Ok(order) };
    let mut path = vec![start];
    loop {
        let last = path[path.len() - 1];
        let previous = links.iter().find(|l| l.target == last && waiting[l.source] > 0).map(|l| l.source);
        let Some(previous) = previous else { return Err(SankeyError::Cycle(path)) };
        if let Some(seen) = path.iter().position(|node| *node == previous) {
            let mut circle: Vec<usize> = path[seen..].to_vec();
            circle.push(previous);
            circle.reverse();
            return Err(SankeyError::Cycle(circle));
        }
        path.push(previous);
    }
}

fn usable(links: &[SankeyLink]) -> Vec<SankeyLink> {
    links.iter().copied().filter(|link| link.value.is_finite() && link.value > 0.0).collect()
}

/// Whether a graph can be laid out, without laying it out.
pub fn check(count: usize, links: &[SankeyLink]) -> Result<(), SankeyError> {
    let links = usable(links);
    if let Some(link) = links.iter().find(|link| link.source >= count || link.target >= count) {
        return Err(SankeyError::NoSuchNode(link.source.max(link.target)));
    }
    in_order(count, &links).map(|_| ())
}

/// The column of each node, and how many there are.
fn columns_of(
    count: usize,
    links: &[SankeyLink],
    order: &[usize],
    align: SankeyAlign,
) -> (Vec<usize>, usize) {
    let mut depth = vec![0_usize; count];
    for node in order {
        for link in links.iter().filter(|link| link.source == *node) {
            depth[link.target] = depth[link.target].max(depth[*node] + 1);
        }
    }
    let mut height = vec![0_usize; count];
    for node in order.iter().rev() {
        for link in links.iter().filter(|link| link.target == *node) {
            height[link.source] = height[link.source].max(height[*node] + 1);
        }
    }
    let columns = depth.iter().copied().max().map_or(0, |deepest| deepest + 1);
    let has_in = |node: usize| links.iter().any(|link| link.target == node);
    let has_out = |node: usize| links.iter().any(|link| link.source == node);
    let column = (0..count)
        .map(|node| match align {
            SankeyAlign::Left => depth[node],
            SankeyAlign::Right => columns - 1 - height[node],
            SankeyAlign::Center if !has_in(node) && has_out(node) => {
                let first = links.iter().filter(|l| l.source == node).map(|l| depth[l.target]).min();
                first.map_or(0, |at| at.saturating_sub(1))
            }
            SankeyAlign::Center => depth[node],
            SankeyAlign::Justify if !has_out(node) && has_in(node) => columns - 1,
            _ => depth[node],
        })
        .collect();
    (column, columns)
}

/// Keeps a column's nodes from covering each other and inside the area.
fn resolve(ys: &mut [f32], heights: &[f32], column: &mut [usize], area: Rect, gap: f32) {
    column.sort_by(|a, b| ys[*a].total_cmp(&ys[*b]));
    let mut next = area.y();
    for node in column.iter() {
        ys[*node] = ys[*node].max(next);
        next = ys[*node] + heights[*node] + gap;
    }
    let mut limit = area.max_y();
    for node in column.iter().rev() {
        ys[*node] = ys[*node].min(limit - heights[*node]);
        limit = ys[*node] - gap;
    }
}

/// Lays a graph of `count` nodes out inside `area`. Flows with no value are left out.
pub fn layout(
    count: usize,
    links: &[SankeyLink],
    area: Rect,
    style: &SankeyStyle,
) -> Result<SankeyLayout, SankeyError> {
    check(count, links)?;
    let links = usable(links);
    let order = in_order(count, &links)?;
    let (column_of, columns) = columns_of(count, &links, &order, style.align);
    if columns == 0 {
        return Ok(SankeyLayout::default());
    }
    let mut value = vec![0.0_f64; count];
    for (node, slot) in value.iter_mut().enumerate() {
        let coming: f64 = links.iter().filter(|l| l.target == node).map(|l| l.value).sum();
        let going: f64 = links.iter().filter(|l| l.source == node).map(|l| l.value).sum();
        *slot = coming.max(going);
    }
    let mut by_column: Vec<Vec<usize>> = vec![Vec::new(); columns];
    for node in 0..count {
        by_column[column_of[node]].push(node);
    }
    // One scale for the whole diagram, the one the fullest column allows.
    let scale = by_column
        .iter()
        .filter(|column| !column.is_empty())
        .map(|column| {
            let total: f64 = column.iter().map(|node| value[*node]).sum();
            let room = area.height() - style.node_gap * (column.len() - 1) as f32;
            if total > 0.0 { room.max(1.0) / total as f32 } else { f32::INFINITY }
        })
        .fold(f32::INFINITY, f32::min);
    let scale = if scale.is_finite() { scale } else { 1.0 };
    let heights: Vec<f32> = value.iter().map(|v| (*v as f32 * scale).max(1.0)).collect();

    // Each column starts in the middle, and each pass brings each node closer to what
    // it connects to, more boldly at first.
    let mut ys = vec![area.y(); count];
    for column in &by_column {
        let total: f32 = column.iter().map(|node| heights[*node]).sum::<f32>()
            + style.node_gap * column.len().saturating_sub(1) as f32;
        let mut y = area.y() + (area.height() - total).max(0.0) / 2.0;
        for node in column {
            ys[*node] = y;
            y += heights[*node] + style.node_gap;
        }
    }
    for pass in 0..style.passes {
        let strength = 0.99_f32.powi(pass as i32) * 0.8;
        for column in (1..columns).chain((0..columns.saturating_sub(1)).rev()) {
            let forward = pass % 2 == 0;
            let nodes: Vec<usize> = by_column[column].clone();
            for node in nodes {
                let toward = |other: usize| ys[other] + heights[other] / 2.0;
                let (mut sum, mut weight) = (0.0_f32, 0.0_f32);
                for link in &links {
                    let other = if forward && link.target == node {
                        Some(link.source)
                    } else if !forward && link.source == node {
                        Some(link.target)
                    } else {
                        None
                    };
                    if let Some(other) = other {
                        sum += toward(other) * link.value as f32;
                        weight += link.value as f32;
                    }
                }
                if weight > 0.0 {
                    let wanted = sum / weight - heights[node] / 2.0;
                    ys[node] += (wanted - ys[node]) * strength;
                }
            }
            resolve(&mut ys, &heights, &mut by_column[column], area, style.node_gap);
        }
    }

    // Many columns in little room: nodes get thinner, so flows still go forward.
    let node_width = style.node_width.min(area.width() / (columns * 2) as f32);
    let span = (area.width() - node_width).max(0.0);
    let x_of = |column: usize| {
        if columns > 1 { area.x() + span * column as f32 / (columns - 1) as f32 } else { area.x() }
    };
    let mut nodes: Vec<SankeyNode> = (0..count)
        .map(|node| SankeyNode {
            rect: Rect::new(x_of(column_of[node]), ys[node], node_width, heights[node]),
            value: value[node],
            column: column_of[node],
            incoming: Vec::new(),
            outgoing: Vec::new(),
        })
        .collect();
    for (place, link) in links.iter().enumerate() {
        nodes[link.source].outgoing.push(place);
        nodes[link.target].incoming.push(place);
    }
    let middle = |node: usize| ys[node] + heights[node] / 2.0;
    let mut starts = vec![0.0_f32; links.len()];
    let mut ends = vec![0.0_f32; links.len()];
    for node in nodes.iter_mut() {
        // Ribbons leave and arrive in the order of what is at their other ends, so
        // that they cross as little as they can.
        node.outgoing.sort_by(|a, b| middle(links[*a].target).total_cmp(&middle(links[*b].target)));
        node.incoming.sort_by(|a, b| middle(links[*a].source).total_cmp(&middle(links[*b].source)));
        let mut y = node.rect.y();
        for link in &node.outgoing {
            starts[*link] = y + links[*link].value as f32 * scale / 2.0;
            y += links[*link].value as f32 * scale;
        }
        let mut y = node.rect.y();
        for link in &node.incoming {
            ends[*link] = y + links[*link].value as f32 * scale / 2.0;
            y += links[*link].value as f32 * scale;
        }
    }
    let flows = links
        .iter()
        .enumerate()
        .map(|(place, link)| SankeyFlow {
            source: link.source,
            target: link.target,
            value: link.value,
            from: Point::new(nodes[link.source].rect.max_x(), starts[place]),
            to: Point::new(nodes[link.target].rect.x(), ends[place]),
            width: link.value as f32 * scale,
        })
        .collect();
    Ok(SankeyLayout { nodes, flows, columns })
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect::new(0.0, 0.0, 400.0, 300.0);

    fn links(list: &[(usize, usize, f64)]) -> Vec<SankeyLink> {
        list.iter()
            .map(|(source, target, value)| SankeyLink { source: *source, target: *target, value: *value })
            .collect()
    }

    fn laid(count: usize, list: &[(usize, usize, f64)]) -> SankeyLayout {
        layout(count, &links(list), AREA, &SankeyStyle::default()).expect("a layout")
    }

    #[test]
    fn a_chain_goes_from_left_to_right_with_a_column_each() {
        let graph = laid(3, &[(0, 1, 10.0), (1, 2, 10.0)]);
        assert_eq!(graph.columns, 3);
        assert_eq!(graph.nodes[0].rect.x(), 0.0);
        assert!((graph.nodes[2].rect.max_x() - AREA.max_x()).abs() < 0.01);
        assert!(graph.nodes[0].rect.max_x() < graph.nodes[1].rect.x());
        let flow = graph.flows[0];
        assert_eq!(flow.from.x, graph.nodes[0].rect.max_x());
        assert_eq!(flow.to.x, graph.nodes[1].rect.x());
    }

    #[test]
    fn a_circle_says_which_nodes_it_goes_through() {
        let error = layout(
            4,
            &links(&[(0, 1, 1.0), (1, 2, 1.0), (2, 3, 1.0), (3, 1, 1.0)]),
            AREA,
            &SankeyStyle::default(),
        );
        let Err(SankeyError::Cycle(nodes)) = error else { panic!("{error:?}") };
        assert_eq!(nodes.first(), nodes.last(), "it closes on itself");
        assert_eq!(nodes.len(), 4, "{nodes:?}");
        assert!(!nodes.contains(&0), "the node leading to it is not part of it");
        for pair in nodes.windows(2) {
            let found = [(1, 2), (2, 3), (3, 1)].contains(&(pair[0], pair[1]));
            assert!(found, "{pair:?} in the direction of the flows");
        }
    }

    #[test]
    fn a_node_that_flows_into_itself_is_a_circle() {
        assert!(matches!(check(2, &links(&[(1, 1, 1.0)])), Err(SankeyError::Cycle(_))));
    }

    #[test]
    fn a_flow_to_nowhere_is_an_error_and_says_so() {
        let error = check(2, &links(&[(0, 5, 1.0)])).expect_err("no such node");
        assert_eq!(error, SankeyError::NoSuchNode(5));
        assert!(error.to_string().contains('5'));
    }

    #[test]
    fn a_flow_with_no_value_does_not_count() {
        let graph = laid(3, &[(0, 1, 5.0), (1, 2, 0.0), (1, 2, f64::NAN), (2, 0, -1.0)]);
        assert_eq!(graph.flows.len(), 1, "and the negative one closes no circle");
    }

    #[test]
    fn what_goes_through_a_node_fills_its_side() {
        let graph = laid(4, &[(0, 2, 30.0), (1, 2, 10.0), (2, 3, 40.0)]);
        let middle = &graph.nodes[2];
        let width: f32 = middle.incoming.iter().map(|f| graph.flows[*f].width).sum();
        assert!((width - middle.rect.height()).abs() < 0.01);
        let width: f32 = middle.outgoing.iter().map(|f| graph.flows[*f].width).sum();
        assert!((width - middle.rect.height()).abs() < 0.01);
        assert!(graph.nodes[0].rect.height() > graph.nodes[1].rect.height() * 2.0);
    }

    #[test]
    fn nodes_of_a_column_stay_inside_and_apart() {
        let list: Vec<(usize, usize, f64)> = (1..12).map(|i| (0, i, f64::from(i as u32))).collect();
        let graph = laid(12, &list);
        let mut column: Vec<Rect> = graph.nodes[1..].iter().map(|n| n.rect).collect();
        column.sort_by(|a, b| a.y().total_cmp(&b.y()));
        for pair in column.windows(2) {
            assert!(pair[0].max_y() <= pair[1].y() + 0.01, "{pair:?}");
        }
        assert!(column[0].y() >= AREA.y() - 0.01);
        assert!(column[column.len() - 1].max_y() <= AREA.max_y() + 0.01);
    }

    #[test]
    fn the_alignment_decides_where_a_short_branch_goes() {
        let list = links(&[(0, 1, 5.0), (1, 2, 5.0), (3, 2, 5.0), (0, 4, 5.0)]);
        let of = |align| {
            let style = SankeyStyle { align, ..SankeyStyle::default() };
            layout(5, &list, AREA, &style).expect("a layout").nodes
        };
        assert_eq!(of(SankeyAlign::Left)[4].column, 1, "as soon as it can");
        assert_eq!(of(SankeyAlign::Justify)[4].column, 2, "nothing goes out of it: last");
        assert_eq!(of(SankeyAlign::Right)[3].column, 1, "right before what it feeds");
        assert_eq!(of(SankeyAlign::Left)[3].column, 0);
        assert_eq!(of(SankeyAlign::Center)[3].column, 1);
    }

    #[test]
    fn the_pointer_finds_a_node_over_a_flow() {
        let graph = laid(2, &[(0, 1, 10.0)]);
        let node = graph.nodes[0].rect.center();
        assert_eq!(graph.hit(node), Some(SankeyHit::Node(0)));
        let flow = graph.flows[0];
        let halfway = Point::new((flow.from.x + flow.to.x) / 2.0, (flow.from.y + flow.to.y) / 2.0);
        assert_eq!(graph.hit(halfway), Some(SankeyHit::Flow(0)));
        assert_eq!(graph.hit(Point::new(200.0, -10.0)), None);
    }

    #[test]
    fn a_ribbon_is_where_its_outline_says_at_any_point_across() {
        let graph = laid(3, &[(0, 2, 10.0), (1, 2, 30.0)]);
        let flow = graph.flows[0];
        let quarter = flow.from.x + (flow.to.x - flow.from.x) * 0.25;
        let inside = (0..300).map(|y| Point::new(quarter, y as f32)).filter(|p| flow.contains(*p)).count();
        assert!((inside as f32 - flow.width).abs() <= 2.0, "{inside} {}", flow.width);
    }

    #[test]
    fn nothing_lays_out_as_nothing() {
        assert_eq!(laid(0, &[]).nodes.len(), 0);
        let alone = laid(2, &[]);
        assert_eq!(alone.flows.len(), 0);
        assert_eq!(alone.nodes.len(), 2);
    }

    #[test]
    fn many_columns_in_little_room_still_flow_forward() {
        let list: Vec<(usize, usize, f64)> = (0..200).map(|i| (i, i + 1, 3.0)).collect();
        let graph = laid(201, &list);
        assert_eq!(graph.columns, 201);
        assert!(graph.flows.iter().all(|f| f.from.x < f.to.x));
    }
}
