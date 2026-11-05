//! The arrangement of a [`DockArea`](crate::DockArea), with no drawing: which panel is where, what
//! happens as they move, and the text it is saved as.

use std::collections::HashSet;
use std::fmt;

use ravenrs_layout::Axis;
use serde::{Deserialize, Serialize};

use crate::lists::reorder;

/// The version of the saved text this build writes, and the newest it reads.
const FORMAT_VERSION: u64 = 1;
/// How far the shares of a split may stray from a sum of one before they are put right. Saved
/// shares land within it, so reading a text and writing it again gives the same text.
const SHARES_SUM_TOLERANCE: f32 = 1e-3;

/// One of the four places a [`DockArea`](crate::DockArea) has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum DockSide {
    /// The column on the left.
    Left,
    /// The column on the right.
    Right,
    /// The strip under the center, between the columns.
    Bottom,
    /// What takes the room left.
    Center,
}

impl DockSide {
    /// The three docks that collapse and have a size of their own.
    pub(crate) const DOCKS: [Self; 3] = [Self::Left, Self::Right, Self::Bottom];
    /// All four, in the order they are searched.
    pub(crate) const ALL: [Self; 4] = [Self::Center, Self::Left, Self::Right, Self::Bottom];
    /// All four, in the order a menu lists them.
    pub(crate) const ALL_IN_MENU: [Self; 4] = [Self::Left, Self::Right, Self::Bottom, Self::Center];
}

/// A side of a group of tabs, where a split opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DockEdge {
    /// To its left.
    Left,
    /// To its right.
    Right,
    /// Above it.
    Top,
    /// Below it.
    Bottom,
}

/// Where a panel is sent by [`DockLayout::move_panel`]. Places are named by a panel that is there,
/// since the id of a panel is what stays the same as the tree changes around it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DockDrop {
    /// Into the group of `beside`, in the gap `at`: 0 is before its first tab, and anything past
    /// the end is after its last.
    Tab {
        /// A panel of the group.
        beside: String,
        /// The gap among its tabs.
        at: usize,
    },
    /// A group of its own, opened on one side of the group of `beside`.
    Split {
        /// A panel of the group to split.
        beside: String,
        /// The side the new group opens on.
        edge: DockEdge,
    },
    /// Into a place as a whole: its first group, or a new one if it holds nothing.
    Side(DockSide),
}

/// A part of the tree: tabs, or parts side by side or stacked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum DockNode {
    /// A group of panels, of which one shows.
    Tabs {
        /// The ids, in the order of the tabs.
        panels: Vec<String>,
        /// Which of them shows.
        #[serde(default)]
        active: usize,
    },
    /// Parts along an axis.
    Split {
        /// `Horizontal` puts them side by side, and `Vertical` stacks them.
        #[serde(with = "axis_name")]
        axis: Axis,
        /// The parts, in order.
        children: Vec<DockChild>,
    },
}

/// A part of a split and how much of it it takes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockChild {
    /// Its part of the room, as a fraction: the parts of a split add up to one.
    pub share: f32,
    /// What is there.
    pub node: DockNode,
}

impl DockNode {
    /// A group of these panels, with the first one showing.
    pub fn tabs<S: Into<String>>(panels: impl IntoIterator<Item = S>) -> Self {
        Self::Tabs { panels: panels.into_iter().map(Into::into).collect(), active: 0 }
    }

    /// These parts side by side, in equal shares.
    pub fn horizontal(children: impl IntoIterator<Item = DockNode>) -> Self {
        Self::split(Axis::Horizontal, children)
    }

    /// These parts stacked, in equal shares.
    pub fn vertical(children: impl IntoIterator<Item = DockNode>) -> Self {
        Self::split(Axis::Vertical, children)
    }

    fn split(axis: Axis, children: impl IntoIterator<Item = DockNode>) -> Self {
        let nodes: Vec<DockNode> = children.into_iter().collect();
        let share = 1.0 / nodes.len().max(1) as f32;
        Self::Split { axis, children: nodes.into_iter().map(|node| DockChild { share, node }).collect() }
    }

    /// The group under this node that has this panel.
    fn group_of(&mut self, id: &str) -> Option<(&mut Vec<String>, &mut usize)> {
        match self {
            Self::Tabs { panels, active } => {
                if panels.iter().any(|panel| panel == id) {
                    Some((panels, active))
                } else {
                    None
                }
            }
            Self::Split { children, .. } => children.iter_mut().find_map(|c| c.node.group_of(id)),
        }
    }

    /// Whether this panel is what its group shows, if the panel is under this node.
    fn shows(&self, id: &str) -> Option<bool> {
        match self {
            Self::Tabs { panels, active } => {
                panels.iter().any(|panel| panel == id).then(|| panels.get(*active).is_some_and(|p| p == id))
            }
            Self::Split { children, .. } => children.iter().find_map(|c| c.node.shows(id)),
        }
    }

    fn holds(&self, id: &str) -> bool {
        match self {
            Self::Tabs { panels, .. } => panels.iter().any(|panel| panel == id),
            Self::Split { children, .. } => children.iter().any(|c| c.node.holds(id)),
        }
    }

    fn first_group(&mut self) -> Option<(&mut Vec<String>, &mut usize)> {
        match self {
            Self::Tabs { panels, active } => Some((panels, active)),
            Self::Split { children, .. } => children.first_mut().and_then(|c| c.node.first_group()),
        }
    }

    fn collect_ids<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            Self::Tabs { panels, .. } => out.extend(panels.iter().map(String::as_str)),
            Self::Split { children, .. } => children.iter().for_each(|c| c.node.collect_ids(out)),
        }
    }

    /// Takes the panel out, and says who stayed in its group. It can leave a group empty.
    fn remove(&mut self, id: &str) -> Option<Option<String>> {
        match self {
            Self::Tabs { panels, active } => {
                let at = panels.iter().position(|panel| panel == id)?;
                panels.remove(at);
                // The tab showing goes on showing, and a closed one gives way to its neighbor.
                if at < *active {
                    *active -= 1;
                }
                *active = (*active).min(panels.len().saturating_sub(1));
                Some(panels.first().cloned())
            }
            Self::Split { children, .. } => children.iter_mut().find_map(|c| c.node.remove(id)),
        }
    }

    /// Opens a group of `id` on the edge of the group of `beside`.
    fn split_group(&mut self, beside: &str, id: &str, edge: DockEdge) -> bool {
        match self {
            Self::Tabs { panels, .. } if panels.iter().any(|panel| panel == beside) => {
                let axis = match edge {
                    DockEdge::Left | DockEdge::Right => Axis::Horizontal,
                    DockEdge::Top | DockEdge::Bottom => Axis::Vertical,
                };
                let old = std::mem::replace(self, Self::tabs(Vec::<String>::new()));
                let new = Self::tabs([id]);
                let pair = match edge {
                    DockEdge::Left | DockEdge::Top => [new, old],
                    DockEdge::Right | DockEdge::Bottom => [old, new],
                };
                *self = Self::split(axis, pair);
                true
            }
            Self::Tabs { .. } => false,
            Self::Split { children, .. } => children.iter_mut().any(|c| c.node.split_group(beside, id, edge)),
        }
    }

    /// The split at `path`, each step an index among the children.
    fn split_at(&mut self, path: &[usize]) -> Option<&mut Vec<DockChild>> {
        let Self::Split { children, .. } = self else { return None };
        match path.split_first() {
            None => Some(children),
            Some((first, rest)) => children.get_mut(*first)?.node.split_at(rest),
        }
    }
}

/// Puts a node in its simplest form: panels seen once, no empty group, no split of one part or
/// inside another along the same axis, and shares that add up to one.
fn tidied(node: DockNode, seen: &mut HashSet<String>) -> Option<DockNode> {
    match node {
        DockNode::Tabs { panels, active } => {
            let shown = panels.get(active).cloned();
            let mut kept = Vec::with_capacity(panels.len());
            for panel in panels {
                if seen.insert(panel.clone()) {
                    kept.push(panel);
                }
            }
            if kept.is_empty() {
                return None;
            }
            let active = shown
                .and_then(|shown| kept.iter().position(|panel| *panel == shown))
                .unwrap_or_else(|| active.min(kept.len() - 1));
            Some(DockNode::Tabs { panels: kept, active })
        }
        DockNode::Split { axis, children } => {
            let mut flat: Vec<DockChild> = Vec::with_capacity(children.len());
            let valid = children.iter().filter(|c| c.share.is_finite() && c.share > 0.0).count().max(1);
            // A share that makes no sense becomes an even one, and what is lost stays in the sum.
            let fallback = 1.0 / valid as f32;
            for child in children {
                let share = if child.share.is_finite() && child.share > 0.0 { child.share } else { fallback };
                let Some(node) = tidied(child.node, seen) else { continue };
                match node {
                    DockNode::Split { axis: inner, children: grand } if inner == axis => {
                        flat.extend(
                            grand.into_iter().map(|g| DockChild { share: g.share * share, node: g.node }),
                        );
                    }
                    node => flat.push(DockChild { share, node }),
                }
            }
            match flat.len() {
                0 => None,
                1 => flat.pop().map(|only| only.node),
                _ => {
                    let sum: f32 = flat.iter().map(|c| c.share).sum();
                    if (sum - 1.0).abs() > SHARES_SUM_TOLERANCE {
                        flat.iter_mut().for_each(|c| c.share /= sum);
                    }
                    Some(DockNode::Split { axis, children: flat })
                }
            }
        }
    }
}

mod axis_name {
    use ravenrs_layout::Axis;
    use serde::{Deserialize, Deserializer, Serializer, de};

    pub(super) fn serialize<S: Serializer>(axis: &Axis, out: S) -> Result<S::Ok, S::Error> {
        out.serialize_str(if *axis == Axis::Vertical { "vertical" } else { "horizontal" })
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(input: D) -> Result<Axis, D::Error> {
        match String::deserialize(input)?.as_str() {
            "horizontal" => Ok(Axis::Horizontal),
            "vertical" => Ok(Axis::Vertical),
            other => Err(de::Error::custom(format!("no axis called {other}"))),
        }
    }
}

/// A dock: what is there, its size in logical pixels, and whether it shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Region {
    #[serde(default)]
    node: Option<DockNode>,
    size: f32,
    #[serde(default = "shown")]
    open: bool,
}

fn shown() -> bool {
    true
}

impl Region {
    fn new(size: f32) -> Self {
        Self { node: None, size, open: true }
    }
}

fn left() -> Region {
    Region::new(260.0)
}

fn right() -> Region {
    Region::new(280.0)
}

fn bottom() -> Region {
    Region::new(200.0)
}

/// A panel out of the tree, with where to bring it back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Closed {
    panel: String,
    place: DockSide,
    /// Who stayed in its group as it closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    with: Option<String>,
}

/// The arrangement of a [`DockArea`](crate::DockArea): which panel is where, the sizes, and what
/// is collapsed or closed. It holds ids only, so it can be kept in a `State`, compared and saved;
/// what a panel shows comes from the app, by id, each time it is drawn.
///
/// ```
/// use ravenrs_widgets::{DockDrop, DockLayout, DockNode, DockSide};
///
/// let mut layout = DockLayout::new()
///     .center(DockNode::tabs(["photo"]))
///     .left(DockNode::tabs(["folders", "search"]), 240.0);
/// layout.move_panel("search", DockDrop::Side(DockSide::Center));
/// assert_eq!(layout.side_of("search"), Some(DockSide::Center));
///
/// let saved = layout.to_text();
/// assert_eq!(DockLayout::parse(&saved), Ok(layout));
/// assert!(DockLayout::parse("not a layout").is_err());
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockLayout {
    #[serde(default)]
    center: Option<DockNode>,
    #[serde(default = "left")]
    left: Region,
    #[serde(default = "right")]
    right: Region,
    #[serde(default = "bottom")]
    bottom: Region,
    #[serde(default)]
    closed: Vec<Closed>,
}

impl Default for DockLayout {
    fn default() -> Self {
        Self::new()
    }
}

/// Why a saved text could not be read. Nothing was changed; the app goes on with the arrangement
/// it has, or the one it starts from.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DockLayoutError {
    /// It is not well-formed text in the format.
    Syntax {
        /// The line of the problem, from one.
        line: usize,
        /// The column of the problem, from one.
        column: usize,
        /// What was wrong.
        message: String,
    },
    /// It is well-formed, but says nothing of being an arrangement: it has no version.
    NotALayout,
    /// It was saved by a newer build, in a version this one cannot read.
    UnsupportedVersion(u64),
    /// It has the version, but a part of it is not what the version says.
    Invalid(String),
}

impl fmt::Display for DockLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { line, column, message } => {
                write!(f, "the dock layout cannot be read at line {line}, column {column}: {message}")
            }
            Self::NotALayout => f.write_str("this text is not a dock layout: it has no version"),
            Self::UnsupportedVersion(version) => write!(
                f,
                "this dock layout has version {version}, and this build reads up to {FORMAT_VERSION}"
            ),
            Self::Invalid(message) => write!(f, "this dock layout is invalid: {message}"),
        }
    }
}

impl std::error::Error for DockLayoutError {}

#[derive(Serialize)]
struct Saved<'a> {
    version: u64,
    #[serde(flatten)]
    layout: &'a DockLayout,
}

impl DockLayout {
    /// Nothing anywhere yet, with the docks at their usual sizes.
    pub fn new() -> Self {
        Self { center: None, left: left(), right: right(), bottom: bottom(), closed: Vec::new() }
    }

    /// Puts this in the center.
    pub fn center(mut self, node: DockNode) -> Self {
        self.center = Some(node);
        self.tidy();
        self
    }

    /// Puts this in the left dock, `size` logical pixels wide.
    pub fn left(self, node: DockNode, size: f32) -> Self {
        self.dock(DockSide::Left, node, size)
    }

    /// Puts this in the right dock, `size` logical pixels wide.
    pub fn right(self, node: DockNode, size: f32) -> Self {
        self.dock(DockSide::Right, node, size)
    }

    /// Puts this in the bottom dock, `size` logical pixels tall.
    pub fn bottom(self, node: DockNode, size: f32) -> Self {
        self.dock(DockSide::Bottom, node, size)
    }

    /// Starts with these panels closed, ready to be reopened in `place`.
    pub fn closed<S: Into<String>>(mut self, panels: impl IntoIterator<Item = S>, place: DockSide) -> Self {
        for panel in panels {
            self.closed.push(Closed { panel: panel.into(), place, with: None });
        }
        self.tidy();
        self
    }

    fn dock(mut self, side: DockSide, node: DockNode, size: f32) -> Self {
        if let Some(region) = self.region_mut(side) {
            region.node = Some(node);
        }
        self.set_size(side, size);
        self.tidy();
        self
    }

    fn region(&self, side: DockSide) -> Option<&Region> {
        match side {
            DockSide::Left => Some(&self.left),
            DockSide::Right => Some(&self.right),
            DockSide::Bottom => Some(&self.bottom),
            DockSide::Center => None,
        }
    }

    fn region_mut(&mut self, side: DockSide) -> Option<&mut Region> {
        match side {
            DockSide::Left => Some(&mut self.left),
            DockSide::Right => Some(&mut self.right),
            DockSide::Bottom => Some(&mut self.bottom),
            DockSide::Center => None,
        }
    }

    /// What is in this place.
    pub fn node(&self, side: DockSide) -> Option<&DockNode> {
        match side {
            DockSide::Center => self.center.as_ref(),
            dock => self.region(dock).and_then(|region| region.node.as_ref()),
        }
    }

    fn node_mut(&mut self, side: DockSide) -> Option<&mut DockNode> {
        match side {
            DockSide::Center => self.center.as_mut(),
            dock => self.region_mut(dock).and_then(|region| region.node.as_mut()),
        }
    }

    /// The size of a dock in logical pixels, kept while it is collapsed. The center has none.
    pub fn size(&self, side: DockSide) -> f32 {
        self.region(side).map_or(0.0, |region| region.size)
    }

    /// Sets the size of a dock. A size that is no number is ignored.
    pub fn set_size(&mut self, side: DockSide, size: f32) {
        if size.is_finite()
            && let Some(region) = self.region_mut(side)
        {
            region.size = size.max(0.0);
        }
    }

    /// Whether a dock shows; the center always does.
    pub fn is_open(&self, side: DockSide) -> bool {
        self.region(side).is_none_or(|region| region.open)
    }

    /// Shows or collapses a dock. It keeps its size and its panels for when it comes back.
    pub fn set_open(&mut self, side: DockSide, open: bool) {
        if let Some(region) = self.region_mut(side) {
            region.open = open;
        }
    }

    /// Whether a panel is in the tree, whether or not it is what shows.
    pub fn contains(&self, id: &str) -> bool {
        self.side_of(id).is_some()
    }

    /// The place a panel is in, if it is not closed.
    pub fn side_of(&self, id: &str) -> Option<DockSide> {
        DockSide::ALL.into_iter().find(|side| self.node(*side).is_some_and(|node| node.holds(id)))
    }

    /// The ids in the tree, center first.
    pub fn panels(&self) -> Vec<&str> {
        let mut out = Vec::new();
        for side in DockSide::ALL {
            if let Some(node) = self.node(side) {
                node.collect_ids(&mut out);
            }
        }
        out
    }

    /// The ids closed, in the order they closed.
    pub fn closed_panels(&self) -> Vec<&str> {
        self.closed.iter().map(|closed| closed.panel.as_str()).collect()
    }

    /// Whether this panel is the one its group shows.
    pub fn is_active(&self, id: &str) -> bool {
        self.side_of(id).and_then(|side| self.node(side)).and_then(|node| node.shows(id)) == Some(true)
    }

    fn group_of(&mut self, id: &str) -> Option<(&mut Vec<String>, &mut usize)> {
        let side = self.side_of(id)?;
        self.node_mut(side)?.group_of(id)
    }

    /// Brings a panel to the front of its group, and shows its dock if that was collapsed. It
    /// says whether the panel is there at all; for one that was closed, see [`Self::reopen`].
    pub fn activate(&mut self, id: &str) -> bool {
        let Some(side) = self.side_of(id) else { return false };
        if let Some((panels, active)) = self.group_of(id) {
            *active = panels.iter().position(|panel| panel == id).unwrap_or(*active);
        }
        self.set_open(side, true);
        true
    }

    /// Closes a panel, remembering where it was for [`Self::reopen`]. A group left empty goes
    /// away, and a split left with one part gives way to it. The last panel can close too: an
    /// arrangement with nothing open is still valid.
    pub fn close(&mut self, id: &str) -> bool {
        let Some((place, with)) = self.take_out(id) else { return false };
        self.closed.retain(|closed| closed.panel != id);
        self.closed.push(Closed { panel: id.to_owned(), place, with });
        self.tidy();
        true
    }

    /// Brings a closed panel back to the group it was in, or to the place if that group is gone,
    /// and shows it. It says whether there was such a panel to bring back.
    pub fn reopen(&mut self, id: &str) -> bool {
        let Some(at) = self.closed.iter().position(|closed| closed.panel == id) else { return false };
        let closed = self.closed.remove(at);
        let group = closed.with.filter(|with| self.side_of(with) == Some(closed.place));
        match group {
            Some(beside) => {
                self.insert_tab(id, &beside, usize::MAX);
            }
            None => self.insert_in(id, closed.place),
        }
        self.tidy();
        true
    }

    /// Sends a panel somewhere: into a group, beside it, or into a place; a closed one reopens
    /// there. It says whether anything changed, which it does not if the target is gone, or the
    /// panel is already what it would become.
    pub fn move_panel(&mut self, id: &str, target: DockDrop) -> bool {
        let closed = self.closed.iter().any(|closed| closed.panel == id);
        if !closed && !self.contains(id) {
            return false;
        }
        let done = match target {
            DockDrop::Tab { beside, at } => self.move_to_tab(id, &beside, at),
            DockDrop::Split { beside, edge } => self.move_to_split(id, &beside, edge),
            DockDrop::Side(side) => self.move_to_side(id, side),
        };
        if done {
            self.closed.retain(|closed| closed.panel != id);
            self.tidy();
        }
        done
    }

    fn move_to_tab(&mut self, id: &str, beside: &str, at: usize) -> bool {
        let Some((panels, active)) = self.group_of(beside) else { return false };
        if let Some(from) = panels.iter().position(|panel| panel == id) {
            // Within the group the gaps count the tab itself, as in any strip.
            let Some(to) = reorder::landing(from, at.min(panels.len())) else { return false };
            let shown = panels.get(*active).cloned();
            let moved = panels.remove(from);
            panels.insert(to, moved);
            *active = shown.and_then(|s| panels.iter().position(|p| *p == s)).unwrap_or(to);
            return true;
        }
        self.take_out(id);
        self.insert_tab(id, beside, at)
    }

    fn move_to_split(&mut self, id: &str, beside: &str, edge: DockEdge) -> bool {
        // Splitting its own group leaves the rest of it as the other part.
        let beside = if beside == id {
            let Some((panels, _)) = self.group_of(id) else { return false };
            let Some(other) = panels.iter().find(|panel| *panel != id).cloned() else { return false };
            other
        } else {
            beside.to_owned()
        };
        let Some(side) = self.side_of(&beside) else { return false };
        self.take_out(id);
        let done = self.node_mut(side).is_some_and(|node| node.split_group(&beside, id, edge));
        self.set_open(side, true);
        done
    }

    fn move_to_side(&mut self, id: &str, side: DockSide) -> bool {
        if self.side_of(id) == Some(side) {
            return false;
        }
        self.take_out(id);
        self.insert_in(id, side);
        true
    }

    /// Takes a panel out of the tree, leaving the rest to be tidied.
    fn take_out(&mut self, id: &str) -> Option<(DockSide, Option<String>)> {
        let side = self.side_of(id)?;
        let with = self.node_mut(side)?.remove(id)?;
        Some((side, with))
    }

    fn insert_tab(&mut self, id: &str, beside: &str, at: usize) -> bool {
        let side = self.side_of(beside);
        let Some((panels, active)) = self.group_of(beside) else { return false };
        let at = at.min(panels.len());
        panels.insert(at, id.to_owned());
        *active = at;
        if let Some(side) = side {
            self.set_open(side, true);
        }
        true
    }

    fn insert_in(&mut self, id: &str, side: DockSide) {
        if self.node(side).is_none() {
            let node = Some(DockNode::tabs([id]));
            match self.region_mut(side) {
                Some(region) => region.node = node,
                None => self.center = node,
            }
        } else if let Some((panels, active)) = self.node_mut(side).and_then(DockNode::first_group) {
            panels.push(id.to_owned());
            *active = panels.len() - 1;
        }
        self.set_open(side, true);
    }

    /// Fits the arrangement to the panels that exist: an id not among them is left out, as if it
    /// had never been saved, and one of them that is nowhere becomes closed, ready to be reopened.
    /// It says whether anything changed.
    pub fn reconcile(&mut self, known: &[&str]) -> bool {
        let before = self.clone();
        let unknown: Vec<String> =
            self.panels().into_iter().filter(|id| !known.contains(id)).map(str::to_owned).collect();
        for id in unknown {
            self.take_out(&id);
        }
        self.closed.retain(|closed| known.contains(&closed.panel.as_str()));
        for id in known {
            if !self.contains(id) && !self.closed.iter().any(|closed| closed.panel == *id) {
                self.closed.push(Closed { panel: (*id).to_owned(), place: DockSide::Center, with: None });
            }
        }
        self.tidy();
        *self != before
    }

    /// The first part of a pair in a split gets `first` out of `both` logical pixels; the pair
    /// keeps the share it has together. Other parts do not move.
    pub(crate) fn resize_split(
        &mut self,
        side: DockSide,
        path: &[usize],
        index: usize,
        first: f32,
        both: f32,
    ) {
        if both <= 0.0 || !first.is_finite() {
            return;
        }
        let Some(children) = self.node_mut(side).and_then(|node| node.split_at(path)) else { return };
        let total = match (children.get(index), children.get(index + 1)) {
            (Some(a), Some(b)) => a.share + b.share,
            _ => return,
        };
        // Never quite nothing: a share of zero would read as a share that makes no sense.
        let part = (first / both).clamp(0.01, 0.99) * total;
        children[index].share = part;
        children[index + 1].share = total - part;
    }

    /// Writes it as text to keep, in a versioned format: JSON with a `version` at the top.
    pub fn to_text(&self) -> String {
        // Every map here has string keys and sizes are kept finite, so this cannot fail.
        serde_json::to_string_pretty(&Saved { version: FORMAT_VERSION, layout: self }).unwrap_or_default()
    }

    /// Reads a text from [`Self::to_text`]. What makes no sense but does no harm, such as a panel
    /// twice, an empty group or a share below zero, is put right; what cannot be read is an error,
    /// never a panic.
    pub fn parse(text: &str) -> Result<Self, DockLayoutError> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|error| {
            DockLayoutError::Syntax { line: error.line(), column: error.column(), message: error.to_string() }
        })?;
        let version = value.get("version").and_then(serde_json::Value::as_u64);
        match version {
            None => return Err(DockLayoutError::NotALayout),
            Some(version) if version > FORMAT_VERSION || version == 0 => {
                return Err(DockLayoutError::UnsupportedVersion(version));
            }
            Some(_) => {}
        }
        let mut layout: Self =
            serde_json::from_value(value).map_err(|error| DockLayoutError::Invalid(error.to_string()))?;
        for side in DockSide::DOCKS {
            let size = layout.size(side);
            layout.set_size(side, size);
        }
        layout.tidy();
        Ok(layout)
    }

    /// Back to the simplest form after any change.
    fn tidy(&mut self) {
        let mut seen = HashSet::new();
        self.center = self.center.take().and_then(|node| tidied(node, &mut seen));
        for side in DockSide::DOCKS {
            if let Some(region) = self.region_mut(side) {
                region.node = region.node.take().and_then(|node| tidied(node, &mut seen));
            }
        }
        // The tree wins over a closed entry: a panel cannot be in both.
        self.closed.retain(|closed| seen.insert(closed.panel.clone()));
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn sample() -> DockLayout {
        DockLayout::new()
            .center(DockNode::horizontal([DockNode::tabs(["a", "b"]), DockNode::tabs(["c"])]))
            .left(DockNode::tabs(["d", "e"]), 240.0)
            .bottom(DockNode::tabs(["f"]), 180.0)
    }

    #[test]
    fn a_group_left_empty_goes_and_its_split_gives_way() {
        let mut layout = sample();
        assert!(layout.close("c"));
        assert_eq!(layout.node(DockSide::Center), Some(&DockNode::tabs(["a", "b"])));
    }

    #[test]
    fn closing_keeps_the_tab_that_shows_and_a_neighbor_takes_over() {
        let mut layout = DockLayout::new().center(DockNode::tabs(["a", "b", "c"]));
        layout.activate("c");
        layout.close("a");
        assert!(layout.is_active("c"), "what showed goes on showing");
        layout.close("c");
        assert!(layout.is_active("b"));
    }

    #[test]
    fn the_last_panel_can_close_and_comes_back_where_it_was() {
        let mut layout = DockLayout::new().left(DockNode::tabs(["a"]), 200.0);
        layout.close("a");
        assert!(layout.panels().is_empty());
        assert_eq!(layout.closed_panels(), ["a"]);
        assert_eq!(DockLayout::parse(&layout.to_text()), Ok(layout.clone()));
        assert!(layout.reopen("a"));
        assert_eq!(layout.side_of("a"), Some(DockSide::Left));
        assert!(layout.closed_panels().is_empty());
        assert!(!layout.reopen("a"), "it is open already");
    }

    #[test]
    fn a_panel_reopens_in_the_group_it_left() {
        let mut layout = sample();
        layout.close("b");
        layout.reopen("b");
        let Some(DockNode::Split { children, .. }) = layout.node(DockSide::Center) else { panic!("a split") };
        assert_eq!(children[0].node, DockNode::Tabs { panels: vec!["a".into(), "b".into()], active: 1 });
    }

    #[test]
    fn a_tab_reorders_within_its_group_by_gaps() {
        let mut layout = DockLayout::new().center(DockNode::tabs(["a", "b", "c"]));
        assert!(layout.move_panel("a", DockDrop::Tab { beside: "c".into(), at: 3 }));
        assert_eq!(
            layout.node(DockSide::Center),
            Some(&DockNode::Tabs { panels: vec!["b".into(), "c".into(), "a".into()], active: 2 })
        );
        assert!(!layout.move_panel("a", DockDrop::Tab { beside: "a".into(), at: 3 }), "it is there already");
        assert!(!layout.move_panel("a", DockDrop::Tab { beside: "a".into(), at: 2 }), "the gap beside it");
    }

    #[test]
    fn a_tab_joins_another_group_where_it_is_dropped_and_shows() {
        let mut layout = sample();
        assert!(layout.move_panel("d", DockDrop::Tab { beside: "a".into(), at: 1 }));
        assert!(layout.is_active("d"));
        let Some(DockNode::Split { children, .. }) = layout.node(DockSide::Center) else { panic!("a split") };
        assert_eq!(
            children[0].node,
            DockNode::Tabs { panels: vec!["a".into(), "d".into(), "b".into()], active: 1 }
        );
    }

    #[test]
    fn a_split_opens_on_the_edge_asked_for() {
        let mut layout = DockLayout::new().center(DockNode::tabs(["a", "b"]));
        assert!(layout.move_panel("b", DockDrop::Split { beside: "a".into(), edge: DockEdge::Top }));
        let Some(DockNode::Split { axis, children }) = layout.node(DockSide::Center) else {
            panic!("a split")
        };
        assert_eq!(*axis, Axis::Vertical);
        assert_eq!(children[0].node, DockNode::tabs(["b"]));
        assert_eq!(children[1].node, DockNode::tabs(["a"]));
    }

    #[test]
    fn a_panel_alone_cannot_split_from_itself() {
        let mut layout = DockLayout::new().center(DockNode::tabs(["a"]));
        assert!(!layout.move_panel("a", DockDrop::Split { beside: "a".into(), edge: DockEdge::Left }));
        assert!(layout.contains("a"));
        let mut two = DockLayout::new().center(DockNode::tabs(["a", "b"]));
        assert!(two.move_panel("a", DockDrop::Split { beside: "a".into(), edge: DockEdge::Left }));
    }

    #[test]
    fn a_split_along_the_same_axis_joins_its_parent_with_the_same_room() {
        let mut layout = DockLayout::new()
            .center(DockNode::horizontal([DockNode::tabs(["a", "x"]), DockNode::tabs(["b"])]));
        assert!(layout.move_panel("x", DockDrop::Split { beside: "a".into(), edge: DockEdge::Right }));
        let Some(DockNode::Split { children, .. }) = layout.node(DockSide::Center) else { panic!("a split") };
        let shares: Vec<f32> = children.iter().map(|c| c.share).collect();
        assert_eq!(shares, [0.25, 0.25, 0.5]);
    }

    #[test]
    fn moving_into_a_collapsed_dock_shows_it() {
        let mut layout = sample();
        layout.set_open(DockSide::Left, false);
        layout.move_panel("a", DockDrop::Side(DockSide::Left));
        assert!(layout.is_open(DockSide::Left));
        assert!(!layout.move_panel("a", DockDrop::Side(DockSide::Left)), "it is there already");
    }

    #[test]
    fn a_target_that_is_gone_changes_nothing() {
        let mut layout = sample();
        let before = layout.clone();
        assert!(!layout.move_panel("a", DockDrop::Tab { beside: "nobody".into(), at: 0 }));
        assert!(!layout.move_panel("a", DockDrop::Split { beside: "nobody".into(), edge: DockEdge::Top }));
        assert!(!layout.move_panel("nobody", DockDrop::Side(DockSide::Left)));
        assert_eq!(layout, before);
    }

    #[test]
    fn an_unknown_id_is_left_out_and_a_new_panel_is_ready_to_reopen() {
        let mut layout = sample();
        layout.close("e");
        assert!(layout.reconcile(&["a", "c", "d", "f", "new"]));
        assert_eq!(layout.panels(), ["a", "c", "d", "f"]);
        assert_eq!(layout.closed_panels(), ["new"]);
        assert!(!layout.reconcile(&["a", "c", "d", "f", "new"]), "nothing is left to do");
    }

    #[test]
    fn a_saved_text_reads_back_the_same_and_stays_the_same_text() {
        let mut layout = sample();
        layout.set_open(DockSide::Left, false);
        layout.close("e");
        layout.resize_split(DockSide::Center, &[], 0, 100.0, 300.0);
        let text = layout.to_text();
        let read = DockLayout::parse(&text).expect("it reads");
        assert_eq!(read.to_text(), text);
        assert_eq!(read, layout);
    }

    #[test]
    fn what_cannot_be_read_gives_a_clear_error() {
        let syntax = DockLayout::parse("{ not json").expect_err("broken");
        assert!(matches!(syntax, DockLayoutError::Syntax { line: 1, .. }));
        assert!(syntax.to_string().contains("line 1"));
        assert_eq!(DockLayout::parse("{}"), Err(DockLayoutError::NotALayout));
        assert_eq!(DockLayout::parse("[1, 2]"), Err(DockLayoutError::NotALayout));
        assert_eq!(DockLayout::parse(r#"{"version": 99}"#), Err(DockLayoutError::UnsupportedVersion(99)));
        assert_eq!(DockLayout::parse(r#"{"version": 0}"#), Err(DockLayoutError::UnsupportedVersion(0)));
        let wrong = r#"{"version": 1, "center": {"split": {"axis": "sideways", "children": []}}}"#;
        let invalid = DockLayout::parse(wrong).expect_err("no such axis");
        assert!(invalid.to_string().contains("sideways"), "{invalid}");
        let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
        assert!(DockLayout::parse(&deep).is_err(), "too deep is an error, not an overflow of the stack");
    }

    #[test]
    fn nonsense_in_a_text_is_put_right() {
        let text = r#"{"version": 1,
            "center": {"split": {"axis": "horizontal", "children": [
                {"share": -3, "node": {"tabs": {"panels": ["a", "a", "b"], "active": 9}}},
                {"share": 1e30, "node": {"tabs": {"panels": []}}},
                {"share": 2, "node": {"tabs": {"panels": ["a", "c"]}}}
            ]}},
            "left": {"size": -5}}"#;
        let layout = DockLayout::parse(text).expect("it reads");
        assert_eq!(layout.panels(), ["a", "b", "c"], "each panel once");
        assert_eq!(layout.size(DockSide::Left), 0.0);
        let Some(DockNode::Split { children, .. }) = layout.node(DockSide::Center) else { panic!("a split") };
        assert_eq!(children.len(), 2, "the empty group went");
        let sum: f32 = children.iter().map(|c| c.share).sum();
        assert!((sum - 1.0).abs() < 1e-3);
        assert_eq!(children[0].node, DockNode::Tabs { panels: vec!["a".into(), "b".into()], active: 1 });
    }

    const IDS: [&str; 7] = ["a", "b", "c", "d", "e", "f", "g"];

    #[derive(Clone, Debug)]
    enum Op {
        Close(usize),
        Reopen(usize),
        Activate(usize),
        Tab(usize, usize, usize),
        Split(usize, usize, u8),
        Side(usize, u8),
        Toggle(u8),
        Resize(f32),
    }

    fn op() -> impl Strategy<Value = Op> {
        let id = 0..IDS.len();
        prop_oneof![
            id.clone().prop_map(Op::Close),
            id.clone().prop_map(Op::Reopen),
            id.clone().prop_map(Op::Activate),
            (id.clone(), id.clone(), 0..9_usize).prop_map(|(a, b, at)| Op::Tab(a, b, at)),
            (id.clone(), id.clone(), 0..4_u8).prop_map(|(a, b, edge)| Op::Split(a, b, edge)),
            (id, 0..4_u8).prop_map(|(a, side)| Op::Side(a, side)),
            (0..3_u8).prop_map(Op::Toggle),
            (0.0..800.0_f32).prop_map(Op::Resize),
        ]
    }

    fn side(index: u8) -> DockSide {
        [DockSide::Left, DockSide::Right, DockSide::Bottom, DockSide::Center][usize::from(index % 4)]
    }

    fn apply(layout: &mut DockLayout, op: &Op) {
        match *op {
            Op::Close(a) => {
                layout.close(IDS[a]);
            }
            Op::Reopen(a) => {
                layout.reopen(IDS[a]);
            }
            Op::Activate(a) => {
                layout.activate(IDS[a]);
            }
            Op::Tab(a, b, at) => {
                layout.move_panel(IDS[a], DockDrop::Tab { beside: IDS[b].into(), at });
            }
            Op::Split(a, b, edge) => {
                let edge =
                    [DockEdge::Left, DockEdge::Right, DockEdge::Top, DockEdge::Bottom][usize::from(edge % 4)];
                layout.move_panel(IDS[a], DockDrop::Split { beside: IDS[b].into(), edge });
            }
            Op::Side(a, s) => {
                layout.move_panel(IDS[a], DockDrop::Side(side(s)));
            }
            Op::Toggle(s) => layout.set_open(side(s), !layout.is_open(side(s))),
            Op::Resize(px) => layout.resize_split(DockSide::Center, &[], 0, px, 800.0),
        }
    }

    fn check_node(node: &DockNode, parent: Option<Axis>) -> Result<(), TestCaseError> {
        match node {
            DockNode::Tabs { panels, active } => {
                prop_assert!(!panels.is_empty(), "no empty group");
                prop_assert!(*active < panels.len(), "what shows is there");
            }
            DockNode::Split { axis, children } => {
                prop_assert!(children.len() >= 2, "no split of one part");
                prop_assert!(parent != Some(*axis), "no split inside one along the same axis");
                let sum: f32 = children.iter().map(|c| c.share).sum();
                prop_assert!((sum - 1.0).abs() < 0.01, "shares add up, got {sum}");
                for child in children {
                    prop_assert!(child.share.is_finite() && child.share >= 0.0);
                    check_node(&child.node, Some(*axis))?;
                }
            }
        }
        Ok(())
    }

    proptest! {
        #[test]
        fn any_series_of_moves_keeps_the_tree_valid_and_no_panel_lost(ops in proptest::collection::vec(op(), 0..60)) {
            let mut layout = DockLayout::new()
                .center(DockNode::horizontal([DockNode::tabs(["a", "b"]), DockNode::vertical([DockNode::tabs(["c"]), DockNode::tabs(["d"])])]))
                .left(DockNode::tabs(["e", "f"]), 200.0)
                .bottom(DockNode::tabs(["g"]), 160.0);
            for op in &ops {
                apply(&mut layout, op);
                let mut all: Vec<&str> = layout.panels();
                all.extend(layout.closed_panels());
                all.sort_unstable();
                prop_assert_eq!(all, IDS.to_vec(), "each panel exactly once, after {:?}", op);
                for side in DockSide::ALL {
                    if let Some(node) = layout.node(side) {
                        check_node(node, None)?;
                    }
                }
            }
            let text = layout.to_text();
            let read = DockLayout::parse(&text);
            prop_assert!(read.is_ok(), "it reads back: {:?}", read);
            prop_assert_eq!(read.map(|r| r.to_text()).unwrap_or_default(), text);
        }

        #[test]
        fn no_text_makes_it_panic(text in "\\PC{0,200}") {
            let _ = DockLayout::parse(&text);
        }

        #[test]
        fn no_tree_in_a_text_makes_it_panic(
            active in 0..50_usize,
            share in proptest::num::f32::ANY,
            size in proptest::num::f32::ANY,
            name in "[a-z\"\\\\é]{0,6}",
        ) {
            let share = serde_json::to_string(&share).unwrap_or_default();
            let size = serde_json::to_string(&size).unwrap_or_default();
            let name = serde_json::to_string(&name).unwrap_or_default();
            let text = format!(
                r#"{{"version":1,"center":{{"split":{{"axis":"vertical","children":[
                {{"share":{share},"node":{{"tabs":{{"panels":[{name},{name}],"active":{active}}}}}}},
                {{"share":{share},"node":{{"tabs":{{"panels":["z"]}}}}}}]}}}},"left":{{"size":{size}}}}}"#
            );
            if let Ok(layout) = DockLayout::parse(&text) {
                prop_assert!(layout.size(DockSide::Left).is_finite());
                prop_assert!(DockLayout::parse(&layout.to_text()).is_ok());
            }
        }
    }
}
