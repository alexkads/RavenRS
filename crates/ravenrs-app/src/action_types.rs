//! What an action is given and how much what it says matters: data that work running in the
//! background shares with actions, below both.

use ravenrs_criteria::Value;

use crate::Object;

/// What people gave an action that asks for something.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum ActionInput {
    /// Nothing.
    #[default]
    None,
    /// The option picked.
    Choice(String),
    /// The value typed.
    Parameter(Value),
    /// The record filled in on a form.
    Record(Object),
}

/// How much a [`Message`](crate::Message) matters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum MessageKind {
    /// Something to know.
    #[default]
    Information,
    /// It went well.
    Success,
    /// Something to look at.
    Warning,
    /// It went badly.
    Error,
}
