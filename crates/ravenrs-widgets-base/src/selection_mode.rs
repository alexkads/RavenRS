//! [`SelectionMode`]: how many of a list's items can be chosen at once. Grids, lists and tiles
//! share it.

/// How rows can be selected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum SelectionMode {
    /// Rows cannot be selected.
    None,
    /// One row at a time; arrow keys move the selection.
    #[default]
    Single,
    /// Check boxes; Ctrl/Cmd-click toggles, Shift-click selects a range.
    Multiple,
}
