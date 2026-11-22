//! Arranging a diagram by itself, in layers: every connection runs one way, from one layer to a
//! later one, and the nodes of a layer are ordered so that lines cross as little as they can.
//!
//! It is the usual layered method, kept small: cycles are broken by turning back the
//! connections that close them, each node goes to the layer after the longest path that reaches
//! it, the nodes of each layer are ordered by the average place of their neighbours over a few
//! sweeps, and each node then moves towards its neighbours as far as the room in its layer lets
//! it, so a chain comes out straight.

use super::{FlowGraph, FlowNode};

/// The way the layers follow one another.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum FlowDirection {
    /// From left to right: the first layer is the leftmost column.
    #[default]
    Right,
    /// From top to bottom: the first layer is the top row.
    Down,
}

/// The arrangement [`FlowLayout`] makes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum FlowLayoutKind {
    /// Layers in which every connection runs one way and few lines cross: for flows.
    #[default]
    Layered,
    /// A tree, as an org chart: each node centred over the nodes it leads to, which sit side by
    /// side. A node reached from two others hangs under the first; a forest is trees side by
    /// side.
    Tree,
    /// A mind map: the first node nothing leads to in the middle, half its branches growing to
    /// the right and half to the left, each as a tree.
    MindMap,
}

/// How [`FlowLayout::apply`] arranges a diagram: the kind of arrangement, the direction and
/// the room between layers and between the nodes of a layer, in units of the diagram.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowLayout {
    /// Layers or a tree.
    pub kind: FlowLayoutKind,
    /// The way the layers follow one another.
    pub direction: FlowDirection,
    /// The room between one layer and the next.
    pub layer_gap: f32,
    /// The room between two nodes of a layer.
    pub node_gap: f32,
}

impl Default for FlowLayout {
    fn default() -> Self {
        Self::new(FlowDirection::Right)
    }
}

/// How many times the order of the layers is swept, down and back up.
const SWEEPS: usize = 4;

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowLayout {
    /// Layers in this direction, 96 units apart, with 48 between the nodes of a layer.
    pub fn new(direction: FlowDirection) -> Self {
        Self { kind: FlowLayoutKind::Layered, direction, layer_gap: 96.0, node_gap: 48.0 }
    }

    /// A tree growing in this direction, as an org chart (down) or a mind map (right).
    pub fn tree(direction: FlowDirection) -> Self {
        Self { kind: FlowLayoutKind::Tree, ..Self::new(direction) }
    }

    /// A mind map growing to both sides of its middle idea.
    pub fn mind_map() -> Self {
        Self { kind: FlowLayoutKind::MindMap, ..Self::new(FlowDirection::Right) }
    }

    /// With this room between layers and between the nodes of a layer.
    pub fn gaps(mut self, layer_gap: f32, node_gap: f32) -> Self {
        self.layer_gap = layer_gap.max(0.0);
        self.node_gap = node_gap.max(0.0);
        self
    }

    /// Where each node goes, its left and top edges, in the order of `graph.nodes`. The
    /// arranged diagram starts where the diagram started: its box keeps its top left corner.
    ///
    /// Only the nodes outside any group are arranged, with a group standing for everything in
    /// it and its connections; what a group holds moves with it and keeps its own arrangement.
    pub fn positions(&self, graph: &FlowGraph) -> Vec<(f32, f32)> {
        let outer = FlowGraph {
            // The app's data of the whole diagram stays with the diagram.
            data: serde_json::Value::Null,
            nodes: graph.nodes.iter().filter(|n| n.parent.is_none()).cloned().collect(),
            edges: graph
                .edges
                .iter()
                .map(|e| {
                    let mut e = e.clone();
                    e.from.node = graph.top(&e.from.node);
                    e.to.node = graph.top(&e.to.node);
                    e
                })
                .collect(),
        };
        let placed = self.arrange(&outer);
        graph
            .nodes
            .iter()
            .map(|n| {
                let top = graph.top(&n.id);
                let moved = outer.nodes.iter().zip(&placed).find(|(o, _)| o.id == top);
                match moved {
                    Some((o, (x, y))) => (n.x + x - o.x, n.y + y - o.y),
                    None => (n.x, n.y),
                }
            })
            .collect()
    }

    /// Where each node of a diagram with no groups goes.
    fn arrange(&self, graph: &FlowGraph) -> Vec<(f32, f32)> {
        let n = graph.nodes.len();
        if n == 0 {
            return Vec::new();
        }
        let links = links(graph);
        let places = match self.kind {
            FlowLayoutKind::Tree => self.tree_places(&graph.nodes, &links),
            FlowLayoutKind::MindMap => self.mind_places(&graph.nodes, &links),
            _ => {
                let layer = layers(n, &links);
                let mut order = order(n, &links, &layer);
                self.place(&graph.nodes, &links, &layer, &mut order)
            }
        };
        let origin = graph.bounds().map_or((0.0, 0.0), |b| (b.x(), b.y()));
        let (least_x, least_y) =
            places.iter().fold((f32::INFINITY, f32::INFINITY), |(x, y), p| (x.min(p.0), y.min(p.1)));
        places.into_iter().map(|(x, y)| (x - least_x + origin.0, y - least_y + origin.1)).collect()
    }

    /// Moves every node of `graph` where [`Self::positions`] puts it. Says whether any moved.
    /// A mind map also turns the ports of the nodes left of the middle to face it: their way in
    /// on the right, their way out on the left.
    pub fn apply(&self, graph: &mut FlowGraph) -> bool {
        let places = self.positions(graph);
        let mut moved = false;
        for (node, (x, y)) in graph.nodes.iter_mut().zip(places) {
            if node.x != x || node.y != y {
                node.x = x;
                node.y = y;
                moved = true;
            }
        }
        if self.kind == FlowLayoutKind::MindMap
            && let Some(middle) = middle_of(graph)
        {
            let centre = graph.nodes[middle].rect().center().x;
            for (i, node) in graph.nodes.iter_mut().enumerate() {
                let left = i != middle && node.rect().center().x < centre;
                for port in &mut node.ports {
                    if !port.side.is_horizontal() {
                        continue;
                    }
                    let wanted = match (port.direction == super::PortDirection::Input, left) {
                        (true, false) | (false, true) => super::PortSide::Left,
                        _ => super::PortSide::Right,
                    };
                    if port.side != wanted {
                        port.side = wanted;
                        moved = true;
                    }
                }
            }
        }
        moved
    }

    /// Where each node of a mind map goes: the branches of the middle split between the right
    /// and the left, each half a tree, the left one mirrored.
    fn mind_places(&self, nodes: &[FlowNode], links: &[(usize, usize)]) -> Vec<(f32, f32)> {
        let n = nodes.len();
        let reached: Vec<bool> = (0..n).map(|i| links.iter().any(|(_, b)| *b == i)).collect();
        let middle = (0..n).find(|i| !reached[*i]).unwrap_or(0);
        let branches: Vec<usize> = links.iter().filter(|(a, _)| *a == middle).map(|(_, b)| *b).collect();
        let half = branches.len().div_ceil(2);
        let under = |from: usize| -> Vec<usize> {
            let mut found = vec![from];
            let mut i = 0;
            while i < found.len() {
                for (a, b) in links {
                    if *a == found[i] && !found.contains(b) && *b != middle {
                        found.push(*b);
                    }
                }
                i += 1;
            }
            found
        };
        let side = |chosen: &[usize]| -> Vec<usize> {
            let mut all = vec![middle];
            for b in chosen {
                for i in under(*b) {
                    if !all.contains(&i) {
                        all.push(i);
                    }
                }
            }
            all
        };
        let (right, left) = (side(&branches[..half]), side(&branches[half..]));
        let tree = |members: &[usize]| -> Vec<(f32, f32)> {
            let sub: Vec<FlowNode> = members.iter().map(|i| nodes[*i].clone()).collect();
            let sub_links: Vec<(usize, usize)> = links
                .iter()
                .filter_map(|(a, b)| {
                    Some((members.iter().position(|m| m == a)?, members.iter().position(|m| m == b)?))
                })
                .collect();
            Self { kind: FlowLayoutKind::Tree, direction: FlowDirection::Right, ..*self }
                .tree_places(&sub, &sub_links)
        };
        let mut places: Vec<(f32, f32)> = nodes.iter().map(|n| (n.x, n.y)).collect();
        let r = tree(&right);
        let (rx, ry) = r[0];
        for (k, i) in right.iter().enumerate() {
            places[*i] = (r[k].0 - rx, r[k].1 - ry);
        }
        if left.len() > 1 {
            let l = tree(&left);
            let (lx, ly) = l[0];
            let w = nodes[middle].width;
            for (k, i) in left.iter().enumerate().skip(1) {
                // Mirrored: as far left of the middle's left edge as the tree put it right.
                let dx = l[k].0 - lx;
                places[*i] = (w - dx - nodes[*i].width, l[k].1 - ly);
            }
        }
        // What no branch reaches stays below, in a row.
        let placed: Vec<usize> = right.iter().chain(&left).copied().collect();
        let bottom = placed.iter().map(|i| places[*i].1 + nodes[*i].height).fold(0.0_f32, f32::max);
        let mut x = 0.0;
        for i in (0..n).filter(|i| !placed.contains(i)) {
            places[i] = (x, bottom + self.layer_gap);
            x += nodes[i].width + self.node_gap;
        }
        places
    }

    /// Where each node goes in a tree: the left and top edges.
    fn tree_places(&self, nodes: &[FlowNode], links: &[(usize, usize)]) -> Vec<(f32, f32)> {
        let n = nodes.len();
        // Who hangs under whom: walked breadth first from the nodes nothing leads to, then from
        // the first node not reached yet, so that a cycle still becomes a tree.
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut depth = vec![0usize; n];
        let mut seen = vec![false; n];
        let mut walked: Vec<usize> = Vec::with_capacity(n);
        let mut roots: Vec<usize> = Vec::new();
        let starts: Vec<usize> =
            (0..n).filter(|&i| !links.iter().any(|&(_, b)| b == i)).chain(0..n).collect();
        for start in starts {
            if seen[start] {
                continue;
            }
            seen[start] = true;
            roots.push(start);
            let mut next = std::collections::VecDeque::from([start]);
            while let Some(u) = next.pop_front() {
                walked.push(u);
                for &(a, b) in links {
                    if a == u && !seen[b] {
                        seen[b] = true;
                        depth[b] = depth[u] + 1;
                        children[u].push(b);
                        next.push_back(b);
                    }
                }
            }
        }
        // How wide each subtree is across the layers, the deepest first.
        let mut span = vec![0.0_f32; n];
        for &u in walked.iter().rev() {
            let own = self.extents(&nodes[u]).1;
            let below: f32 = children[u].iter().map(|&c| span[c]).sum::<f32>()
                + self.node_gap * children[u].len().saturating_sub(1) as f32;
            span[u] = own.max(below);
        }
        // Along the layers: each depth as deep as its deepest node.
        let levels = depth.iter().copied().max().map_or(0, |d| d + 1);
        let mut along = vec![0.0_f32; levels];
        let mut at = 0.0;
        for (d, slot) in along.iter_mut().enumerate() {
            *slot = at;
            let deepest =
                (0..n).filter(|&i| depth[i] == d).map(|i| self.extents(&nodes[i]).0).fold(0.0, f32::max);
            at += deepest + self.layer_gap;
        }
        // Across: each subtree in its own band, its root centred over it.
        let mut centre = vec![0.0_f32; n];
        let mut bands: Vec<(usize, f32)> = Vec::new();
        let mut edge = 0.0;
        for &r in &roots {
            bands.push((r, edge));
            edge += span[r] + self.node_gap;
        }
        while let Some((u, start)) = bands.pop() {
            centre[u] = start + span[u] / 2.0;
            let below: f32 = children[u].iter().map(|&c| span[c]).sum::<f32>()
                + self.node_gap * children[u].len().saturating_sub(1) as f32;
            let mut child = start + (span[u] - below) / 2.0;
            for &c in &children[u] {
                bands.push((c, child));
                child += span[c] + self.node_gap;
            }
        }
        (0..n)
            .map(|i| {
                let (_, across) = self.extents(&nodes[i]);
                let (a, c) = (along[depth[i]], centre[i] - across / 2.0);
                match self.direction {
                    FlowDirection::Right => (a, c),
                    FlowDirection::Down => (c, a),
                }
            })
            .collect()
    }

    /// The size of a node along the layers (its extent in the direction they follow) and across.
    fn extents(&self, node: &FlowNode) -> (f32, f32) {
        match self.direction {
            FlowDirection::Right => (node.width, node.height),
            FlowDirection::Down => (node.height, node.width),
        }
    }

    /// The left and top edge of each node, with layers and orders already chosen.
    fn place(
        &self,
        nodes: &[FlowNode],
        links: &[(usize, usize)],
        layer: &[usize],
        order: &mut [Vec<usize>],
    ) -> Vec<(f32, f32)> {
        // Along the layers: each layer as deep as its deepest node.
        let mut along = Vec::with_capacity(order.len());
        let mut at = 0.0;
        for members in order.iter() {
            along.push(at);
            let depth = members.iter().map(|&i| self.extents(&nodes[i]).0).fold(0.0, f32::max);
            at += depth + self.layer_gap;
        }
        // Across: the centre of each node, the first layer packed, each later one pulled
        // towards the centres of the neighbours before it.
        let mut centre = vec![0.0_f32; nodes.len()];
        for (l, members) in order.iter().enumerate() {
            let wanted: Vec<Option<f32>> = members
                .iter()
                .map(|&i| {
                    let near: Vec<f32> = links
                        .iter()
                        .filter_map(|&(a, b)| match () {
                            _ if b == i && layer[a] < l => Some(centre[a]),
                            _ if a == i && layer[b] < l => Some(centre[b]),
                            _ => None,
                        })
                        .collect();
                    (!near.is_empty()).then(|| near.iter().sum::<f32>() / near.len() as f32)
                })
                .collect();
            self.spread(nodes, members, &wanted, &mut centre);
        }
        order.iter().enumerate().flat_map(|(l, members)| members.iter().map(move |&i| (l, i))).fold(
            vec![(0.0, 0.0); nodes.len()],
            |mut out, (l, i)| {
                let (_, across) = self.extents(&nodes[i]);
                let (a, c) = (along[l], centre[i] - across / 2.0);
                out[i] = match self.direction {
                    FlowDirection::Right => (a, c),
                    FlowDirection::Down => (c, a),
                };
                out
            },
        )
    }

    /// Places the centres of one layer, in its order: as near as each wants to be, at least a
    /// gap apart, and shifted as a whole so that, on average, each is where it wanted.
    fn spread(&self, nodes: &[FlowNode], members: &[usize], wanted: &[Option<f32>], centre: &mut [f32]) {
        let half = |i: usize| self.extents(&nodes[i]).1 / 2.0;
        // Packed from zero, for the nodes that want nowhere in particular.
        let mut packed = Vec::with_capacity(members.len());
        let mut edge = 0.0;
        for &i in members {
            packed.push(edge + half(i));
            edge += half(i) * 2.0 + self.node_gap;
        }
        let mut placed: Vec<f32> = Vec::with_capacity(members.len());
        for (k, &i) in members.iter().enumerate() {
            let want = wanted[k].unwrap_or(packed[k]);
            let least = match k {
                0 => f32::NEG_INFINITY,
                _ => placed[k - 1] + half(members[k - 1]) + self.node_gap + half(i),
            };
            placed.push(want.max(least));
        }
        let pulled: Vec<(f32, f32)> =
            placed.iter().zip(wanted).filter_map(|(p, w)| w.map(|w| (*p, w))).collect();
        let shift = match pulled.len() {
            0 => 0.0,
            count => pulled.iter().map(|(p, w)| w - p).sum::<f32>() / count as f32,
        };
        for (&i, p) in members.iter().zip(placed) {
            centre[i] = p + shift;
        }
    }
}

/// The connections between two different nodes, by index, once each, ignoring the ports.
/// The middle of a mind map: the first node nothing leads to.
fn middle_of(graph: &FlowGraph) -> Option<usize> {
    (0..graph.nodes.len()).find(|i| !graph.edges.iter().any(|e| e.to.node == graph.nodes[*i].id))
}

fn links(graph: &FlowGraph) -> Vec<(usize, usize)> {
    let index = |id: &str| graph.nodes.iter().position(|n| n.id == id);
    let mut links: Vec<(usize, usize)> = Vec::new();
    for edge in &graph.edges {
        if let (Some(a), Some(b)) = (index(&edge.from.node), index(&edge.to.node))
            && a != b
            && !links.contains(&(a, b))
        {
            links.push((a, b));
        }
    }
    links
}

/// The layer of each node: one after the longest path that reaches it, once the connections
/// that close a cycle are turned back.
fn layers(n: usize, links: &[(usize, usize)]) -> Vec<usize> {
    let forward = acyclic(n, links);
    let mut incoming = vec![0usize; n];
    for &(_, b) in &forward {
        incoming[b] += 1;
    }
    let mut ready: Vec<usize> = (0..n).filter(|&i| incoming[i] == 0).rev().collect();
    let mut layer = vec![0usize; n];
    while let Some(a) = ready.pop() {
        for &(_, b) in forward.iter().filter(|(from, _)| *from == a) {
            layer[b] = layer[b].max(layer[a] + 1);
            incoming[b] -= 1;
            if incoming[b] == 0 {
                ready.push(b);
            }
        }
    }
    layer
}

/// The connections with those that close a cycle turned back, found by walking the nodes in
/// their order, depth first.
fn acyclic(n: usize, links: &[(usize, usize)]) -> Vec<(usize, usize)> {
    // 0 not seen, 1 on the path being walked, 2 done.
    let mut state = vec![0u8; n];
    let mut back: Vec<(usize, usize)> = Vec::new();
    for start in 0..n {
        if state[start] != 0 {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        state[start] = 1;
        while let Some(&mut (node, ref mut next)) = stack.last_mut() {
            let out: Vec<usize> = links.iter().filter(|(a, _)| *a == node).map(|(_, b)| *b).collect();
            if let Some(&target) = out.get(*next) {
                *next += 1;
                match state[target] {
                    0 => {
                        state[target] = 1;
                        stack.push((target, 0));
                    }
                    1 => back.push((node, target)),
                    _ => {}
                }
            } else {
                state[node] = 2;
                stack.pop();
            }
        }
    }
    links.iter().map(|&(a, b)| if back.contains(&(a, b)) { (b, a) } else { (a, b) }).collect()
}

/// The nodes of each layer, ordered so that lines cross little: first as the diagram has them,
/// then by the average place of their neighbours in the layer before (sweeping down) and after
/// (sweeping up).
fn order(n: usize, links: &[(usize, usize)], layer: &[usize]) -> Vec<Vec<usize>> {
    let count = layer.iter().copied().max().map_or(0, |m| m + 1);
    let mut order: Vec<Vec<usize>> = vec![Vec::new(); count];
    for i in 0..n {
        order[layer[i]].push(i);
    }
    let neighbours = |i: usize, of: usize| -> Vec<usize> {
        links
            .iter()
            .filter_map(|&(a, b)| match () {
                _ if a == i && layer[b] == of => Some(b),
                _ if b == i && layer[a] == of => Some(a),
                _ => None,
            })
            .collect()
    };
    for sweep in 0..SWEEPS * 2 {
        let down = sweep % 2 == 0;
        let layers: Vec<usize> =
            if down { (1..count).collect() } else { (0..count.saturating_sub(1)).rev().collect() };
        for l in layers {
            let other = if down { l - 1 } else { l + 1 };
            let place = |j: usize| order[other].iter().position(|&k| k == j).unwrap_or(0) as f32;
            let mut keyed: Vec<(f32, usize)> = order[l]
                .iter()
                .enumerate()
                .map(|(k, &i)| {
                    let near = neighbours(i, other);
                    let key = match near.len() {
                        0 => k as f32,
                        count => near.iter().map(|&j| place(j)).sum::<f32>() / count as f32,
                    };
                    (key, i)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
            order[l] = keyed.into_iter().map(|(_, i)| i).collect();
        }
    }
    order
}

#[cfg(test)]
mod mind_tests {
    use super::*;
    use crate::flow::{FlowEdge, FlowEndpoint, FlowPort};

    #[test]
    fn a_mind_map_grows_to_both_sides_and_turns_the_ports_of_the_left() {
        let idea = |id: &str| {
            FlowNode::new(id, "idea", id, 0.0, 0.0)
                .size(120.0, 40.0)
                .port(FlowPort::input("in").side(crate::flow::PortSide::Left))
                .port(FlowPort::output("out").side(crate::flow::PortSide::Right))
        };
        let mut g = FlowGraph::new();
        for id in ["m", "a", "b", "c", "d", "a1"] {
            g = g.node(idea(id));
        }
        for (i, (from, to)) in
            [("m", "a"), ("m", "b"), ("m", "c"), ("m", "d"), ("a", "a1")].iter().enumerate()
        {
            g = g.edge(FlowEdge::new(
                format!("e{i}"),
                FlowEndpoint::new(*from, "out"),
                FlowEndpoint::new(*to, "in"),
            ));
        }
        FlowLayout::mind_map().apply(&mut g);
        let x = |id: &str| g.find_node(id).map_or(0.0, |n| n.rect().center().x);
        let m = x("m");
        assert!(x("a") > m && x("b") > m && x("a1") > x("a"), "half to the right");
        assert!(x("c") < m && x("d") < m, "half to the left");
        let side = |id: &str, port: &str| g.find_node(id).and_then(|n| n.find_port(port)).map(|p| p.side);
        assert_eq!(side("c", "in"), Some(crate::flow::PortSide::Right), "facing the middle");
        assert_eq!(side("c", "out"), Some(crate::flow::PortSide::Left));
        assert_eq!(side("a", "in"), Some(crate::flow::PortSide::Left));
        let before = g.clone();
        FlowLayout::mind_map().apply(&mut g);
        assert_eq!(g, before, "arranging again changes nothing");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::{FlowEdge, FlowEndpoint, FlowPort};
    use ravenrs_core::{Point, Rect};

    fn node(id: &str) -> FlowNode {
        FlowNode::new(id, "step", id, 500.0, 500.0).port(FlowPort::input("in")).port(FlowPort::output("out"))
    }

    fn link(id: &str, from: &str, to: &str) -> FlowEdge {
        FlowEdge::new(id, FlowEndpoint::new(from, "out"), FlowEndpoint::new(to, "in"))
    }

    fn at(graph: &FlowGraph, id: &str) -> (f32, f32) {
        graph.find_node(id).map(|n| (n.x, n.y)).unwrap_or((f32::NAN, f32::NAN))
    }

    #[test]
    fn a_chain_comes_out_straight_from_where_the_diagram_was() {
        let mut g = FlowGraph::new()
            .node(node("c"))
            .node(FlowNode { x: 40.0, y: 24.0, ..node("a") })
            .node(node("b"))
            .edge(link("e1", "a", "b"))
            .edge(link("e2", "b", "c"));
        assert!(FlowLayout::default().apply(&mut g));
        // 200 wide and 96 apart: one column every 296 units, on one line.
        assert_eq!(at(&g, "a"), (40.0, 24.0));
        assert_eq!(at(&g, "b"), (336.0, 24.0));
        assert_eq!(at(&g, "c"), (632.0, 24.0));
        assert!(!FlowLayout::default().apply(&mut g), "arranging again changes nothing");
    }

    #[test]
    fn down_puts_the_layers_in_rows() {
        let mut g = FlowGraph::new().node(node("a")).node(node("b")).edge(link("e1", "a", "b"));
        FlowLayout::new(FlowDirection::Down).gaps(40.0, 20.0).apply(&mut g);
        assert_eq!(at(&g, "b"), (500.0, 500.0 + 84.0 + 40.0));
    }

    #[test]
    fn a_branch_spreads_and_its_source_sits_between_them() {
        let mut g = FlowGraph::new()
            .node(node("a"))
            .node(node("b"))
            .node(node("c"))
            .edge(link("e1", "a", "b"))
            .edge(link("e2", "a", "c"));
        FlowLayout::default().apply(&mut g);
        let (a, b, c) = (at(&g, "a"), at(&g, "b"), at(&g, "c"));
        assert_eq!(b.0, c.0);
        assert_eq!((c.1 - b.1).abs(), 84.0 + 48.0, "a gap apart");
        assert!((a.1 - (b.1 + c.1) / 2.0).abs() < 0.01, "{a:?} {b:?} {c:?}");
        let boxes: Vec<_> = g.nodes.iter().map(FlowNode::rect).collect();
        for (i, r) in boxes.iter().enumerate() {
            for s in &boxes[i + 1..] {
                assert!(r.max_x() <= s.x() || s.max_x() <= r.x() || r.max_y() <= s.y() || s.max_y() <= r.y());
            }
        }
    }

    #[test]
    fn a_cycle_still_gets_layers_and_crossings_are_undone() {
        // a → b → c → a, and two lines that would cross in the order given.
        let mut g = FlowGraph::new()
            .node(node("a"))
            .node(node("b"))
            .node(node("c"))
            .edge(link("e1", "a", "b"))
            .edge(link("e2", "b", "c"))
            .edge(link("e3", "c", "a"));
        FlowLayout::default().apply(&mut g);
        let xs = [at(&g, "a").0, at(&g, "b").0, at(&g, "c").0];
        assert!(xs[0] < xs[1] && xs[1] < xs[2], "{xs:?}");

        let mut crossed = FlowGraph::new()
            .node(node("p"))
            .node(node("q"))
            .node(node("x"))
            .node(node("y"))
            .edge(link("e1", "p", "y"))
            .edge(link("e2", "q", "x"));
        FlowLayout::default().apply(&mut crossed);
        let above = |a: &str, b: &str| at(&crossed, a).1 < at(&crossed, b).1;
        assert_eq!(above("p", "q"), above("y", "x"), "the lines run side by side");
    }

    #[test]
    fn a_group_is_arranged_as_one_box_and_what_it_holds_moves_with_it() {
        let mut g = FlowGraph::new()
            .node(FlowNode::new_group("g", "Checks", Rect::new(1000.0, 0.0, 248.0, 264.0)))
            .node(FlowNode { x: 1024.0, y: 48.0, ..node("b") }.parent("g"))
            .node(FlowNode { x: 1024.0, y: 156.0, ..node("c") }.parent("g"))
            .node(FlowNode { x: 0.0, y: 0.0, ..node("a") })
            .edge(link("e1", "a", "b"))
            .edge(link("e2", "b", "c"));
        FlowLayout::default().apply(&mut g);
        // The group comes after `a`, as the line into what it holds says, and keeps its inside.
        let (a, group, b, c) = (at(&g, "a"), at(&g, "g"), at(&g, "b"), at(&g, "c"));
        // The box of everything keeps its corner, and `a` lines up with the middle of the group.
        assert_eq!((a.0, group.1), (0.0, 0.0));
        assert_eq!(a.1 + 42.0, group.1 + 132.0);
        assert_eq!(group.0, 200.0 + 96.0);
        assert_eq!((b.0 - group.0, b.1 - group.1), (24.0, 48.0));
        assert_eq!((c.0 - b.0, c.1 - b.1), (0.0, 108.0));
    }

    #[test]
    fn a_tree_centres_each_node_over_what_it_leads_to() {
        // root → three children, each with two grandchildren, drawn down as an org chart.
        let mut g = FlowGraph::new().node(node("root"));
        let mut k = 0;
        for c in ["a", "b", "c"] {
            g = g.node(node(c)).edge(link(&format!("r{c}"), "root", c));
            for _ in 0..2 {
                k += 1;
                let id = format!("{c}{k}");
                g = g.node(node(&id)).edge(link(&format!("e{id}"), c, &id));
            }
        }
        FlowLayout::tree(FlowDirection::Down).apply(&mut g);
        let middle =
            |id: &str| g.find_node(id).map(|n| n.rect().center()).unwrap_or(Point::new(f32::NAN, 0.0));
        let (root, a, b, c) = (middle("root"), middle("a"), middle("b"), middle("c"));
        assert!((root.x - b.x).abs() < 0.01, "the root over the middle child: {root:?} {b:?}");
        assert!(a.y == b.y && b.y == c.y && a.y > root.y, "children on one row below");
        assert!((middle("a").x - (middle("a1").x + middle("a2").x) / 2.0).abs() < 0.01, "a over its two");
        assert!(middle("a1").y > a.y);
        let boxes: Vec<_> = g.nodes.iter().map(FlowNode::rect).collect();
        for (i, r) in boxes.iter().enumerate() {
            for s in &boxes[i + 1..] {
                assert!(
                    r.max_x() <= s.x() || s.max_x() <= r.x() || r.max_y() <= s.y() || s.max_y() <= r.y(),
                    "{r:?} {s:?}"
                );
            }
        }
    }

    #[test]
    fn a_tree_of_a_cycle_and_a_forest_still_places_every_node() {
        let mut g = FlowGraph::new()
            .node(node("a"))
            .node(node("b"))
            .node(node("c"))
            .node(node("lone"))
            .edge(link("e1", "a", "b"))
            .edge(link("e2", "b", "c"))
            .edge(link("e3", "c", "a"));
        FlowLayout::tree(FlowDirection::Right).apply(&mut g);
        let xs: Vec<f32> = ["a", "b", "c"].iter().map(|id| at(&g, id).0).collect();
        assert!(xs[0] < xs[1] && xs[1] < xs[2], "{xs:?}");
        assert!(g.nodes.iter().all(|n| n.x.is_finite() && n.y.is_finite()));
        let boxes: Vec<_> = g.nodes.iter().map(FlowNode::rect).collect();
        let apart = |r: &Rect, s: &Rect| {
            r.max_x() <= s.x() || s.max_x() <= r.x() || r.max_y() <= s.y() || s.max_y() <= r.y()
        };
        assert!(boxes[..3].iter().all(|b| apart(b, &boxes[3])), "the lone node beside the tree");
    }

    #[test]
    fn nothing_to_arrange() {
        assert!(FlowLayout::default().positions(&FlowGraph::new()).is_empty());
        let mut alone = FlowGraph::new().node(node("a"));
        assert!(!FlowLayout::default().apply(&mut alone));
    }
}
