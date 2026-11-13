//! The ids of the actions of [`SystemModule`](crate::SystemModule).

/// Creates a record.
pub const NEW: &str = "New";
/// Removes what is selected.
pub const DELETE: &str = "Delete";
/// Keeps the changes.
pub const SAVE: &str = "Save";
/// Keeps the changes and leaves.
pub const SAVE_AND_CLOSE: &str = "SaveAndClose";
/// Keeps the changes and starts another.
pub const SAVE_AND_NEW: &str = "SaveAndNew";
/// Throws away the changes and leaves.
pub const CANCEL: &str = "Cancel";
/// The record before.
pub const PREVIOUS: &str = "PreviousObject";
/// The record after.
pub const NEXT: &str = "NextObject";
/// Reads again.
pub const REFRESH: &str = "Refresh";
/// Goes to the screen before.
pub const BACK: &str = "Back";
/// Picks a way to see the list.
pub const VARIANTS: &str = "ViewVariants";
/// Picks a filter ready on the list.
pub const FILTERS: &str = "SetFilter";
/// Makes a new record from the one selected.
pub const CLONE: &str = "CloneObject";
/// Puts an existing record on a list inside a form.
pub const LINK: &str = "LinkObject";
/// Takes records off a list inside a form, without removing them.
pub const UNLINK: &str = "UnlinkObject";
