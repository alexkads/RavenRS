//! The stories of the components of this crate, for RavenComponentBook: see
//! `ravenrs_widgets_base::book`.

mod nav;

use ravenrs_widgets_base::book::Meta;

/// The stories of the components of this crate that give them, in the order the book lists
/// them.
pub fn stories() -> Vec<Meta> {
    use ravenrs_widgets_base::book::ComponentBook;
    vec![crate::Tabs::book()]
}
