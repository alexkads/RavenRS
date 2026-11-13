//! What an appearance rule says: which items, in which context, look how, and the colors it uses.
//! The entity keeps them; the code that runs them lives in its own module.

use std::rc::Rc;

use crate::Object;

/// A color with a meaning, which every theme has its own version of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Tint {
    /// Something wrong.
    Danger,
    /// Something to watch.
    Warning,
    /// Something right.
    Success,
    /// What stands out.
    Accent,
    /// What steps back.
    Muted,
}

/// What a rule changes the look of.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum AppearanceTarget {
    /// A field: a cell of a list, an editor on a form.
    #[default]
    Item,
    /// A group or a tab of a form.
    Layout,
    /// An action.
    Action,
}

/// Which views a rule applies in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum AppearanceContext {
    /// Every view.
    #[default]
    Any,
    /// Only lists.
    List,
    /// Only forms.
    Detail,
    /// Only the view with this id.
    View(String),
}

pub(crate) type Condition = Rc<dyn Fn(&Object) -> bool>;

/// Changes how things look or behave, for records that meet a condition. It is about looks:
/// it does not protect data, which is what permissions are for.
#[derive(Clone)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct AppearanceRule {
    /// What it is known by.
    pub id: String,
    /// The kind of thing it changes.
    pub target: AppearanceTarget,
    /// The names of those things, where `*` stands for any text. A list that starts with a
    /// lone `*` means all but the names after it.
    pub names: Vec<String>,
    /// Where it applies.
    pub context: AppearanceContext,
    /// The expression a record has to meet. With none, and no function, it always applies.
    pub criteria: Option<String>,
    /// Whichever is higher wins, for what both change.
    pub priority: i32,
    /// The color of the text.
    pub font: Option<Tint>,
    /// The color behind it.
    pub back: Option<Tint>,
    /// Heavier text.
    pub bold: Option<bool>,
    /// Whether it can be used.
    pub enabled: Option<bool>,
    /// Whether it shows.
    pub visible: Option<bool>,
    pub(crate) function: Option<Condition>,
}
