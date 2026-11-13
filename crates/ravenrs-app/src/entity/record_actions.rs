//! What an action declared on an entity says: what it sets, when it is enabled and on how many records.
//! The entity keeps them; the code that runs them lives in its own module.

/// What an action needs selected to be enabled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Selection {
    /// Nothing in particular.
    #[default]
    Independent,
    /// Exactly one record.
    RequireSingle,
    /// One record or more.
    RequireMultiple,
}

/// An action of an entity that sets fields of the records selected.
///
/// ```
/// use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, RecordAction};
///
/// let task = EntityInfo::new("Task")
///     .field(FieldInfo::new("id", FieldKind::Integer).key())
///     .field(FieldInfo::new("state", FieldKind::Text))
///     .action(
///         RecordAction::new("Finish")
///             .enabled("[state] <> 'done'")
///             .set("state = 'done'")
///             .confirm("Finish the tasks?"),
///     );
/// assert!(task.check().is_ok());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct RecordAction {
    /// What its button says.
    pub caption: String,
    /// Its id; `Entity.Caption` unless given.
    pub id: Option<String>,
    /// How many records it takes: one or more unless given.
    pub selection: Selection,
    /// An expression every record selected has to meet for it to be on.
    pub enabled: Option<String>,
    /// What it sets: `field = expression; ...`, each read from the record as it was.
    pub set: String,
    /// The question asked before it runs.
    pub confirm: Option<String>,
}
