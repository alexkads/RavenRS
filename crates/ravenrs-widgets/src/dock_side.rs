//! [`DockSide`]: the places of a dock area, with their words in [`Labels`].

use serde::{Deserialize, Serialize};

use crate::labels::Labels;

/// One of the four places a [`DockArea`](crate::DockArea) has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
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

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DockSide {
    /// The three docks that collapse and have a size of their own.
    pub(crate) const DOCKS: [Self; 3] = [Self::Left, Self::Right, Self::Bottom];
    /// All four, in the order they are searched.
    pub(crate) const ALL: [Self; 4] = [Self::Center, Self::Left, Self::Right, Self::Bottom];
    /// All four, in the order a menu lists them.
    pub(crate) const ALL_IN_MENU: [Self; 4] = [Self::Left, Self::Right, Self::Bottom, Self::Center];
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Labels {
    /// A place of a [`DockArea`](crate::DockArea).
    pub fn dock_place(&self, side: DockSide) -> &str {
        match side {
            DockSide::Left => &self.dock_left,
            DockSide::Right => &self.dock_right,
            DockSide::Bottom => &self.dock_bottom,
            _ => &self.dock_center,
        }
    }

    /// A place in the middle of a sentence.
    fn dock_place_lower(&self, side: DockSide) -> String {
        self.dock_place(side).to_lowercase()
    }

    /// The name of the strip that collapses a dock.
    pub fn dock_hide_name(&self, side: DockSide) -> String {
        self.dock_hide.replace("{place}", &self.dock_place_lower(side))
    }

    /// The name of the strip that brings a dock back.
    pub fn dock_show_name(&self, side: DockSide) -> String {
        self.dock_show.replace("{place}", &self.dock_place_lower(side))
    }

    /// The name of the bar that resizes a dock.
    pub fn dock_resize_name(&self, side: DockSide) -> String {
        self.dock_resize.replace("{place}", &self.dock_place_lower(side))
    }

    /// What is announced after a panel went to a place.
    pub fn dock_moved_to(&self, side: DockSide) -> String {
        self.dock_moved.replace("{place}", &self.dock_place_lower(side))
    }
}
