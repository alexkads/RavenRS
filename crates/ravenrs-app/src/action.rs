//! What people can do: actions, when they apply, and what happens when they run.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use ravenrs_criteria::{Criteria, Environment, Scope, Value};
use ravenrs_reactive::State;

use crate::frame::{FrameContext, ViewInfo, ViewKind};
use crate::{FieldKind, ModelError, Object, ObjectSpace};

/// The reason an action or a controller is off because its view is not the one it targets.
pub const REASON_SCOPE: &str = "scope";
/// The reason an action is off because its controller is.
pub const REASON_CONTROLLER: &str = "controller";
/// The reason an action is off because of what is selected.
pub const REASON_SELECTION: &str = "selection";
/// The reason an action is off because the selected records do not meet its criteria.
pub const REASON_CRITERIA: &str = "criteria";

/// The reason an action is off because of an appearance rule.
pub const REASON_APPEARANCE: &str = "appearance";

/// Why something is on or off: pairs of reason and answer. It is on only if every answer is yes,
/// so one part of the application turning it off cannot be undone by another.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reasons(BTreeMap<String, bool>);

impl Reasons {
    /// Whether every answer is yes.
    pub fn all(&self) -> bool {
        self.0.values().all(|on| *on)
    }

    /// Gives the answer for a reason.
    pub fn set(&mut self, reason: &str, on: bool) {
        self.0.insert(reason.to_owned(), on);
    }

    /// The answer for a reason, if it was given.
    pub fn get(&self, reason: &str) -> Option<bool> {
        self.0.get(reason).copied()
    }

    /// The reasons that say no.
    pub fn against(&self) -> impl Iterator<Item = &str> {
        self.0.iter().filter(|(_, on)| !**on).map(|(reason, _)| reason.as_str())
    }
}

/// What an action needs selected to be enabled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Selection {
    /// Nothing in particular.
    #[default]
    Independent,
    /// Exactly one record.
    RequireSingle,
    /// One record or more.
    RequireMultiple,
}

impl Selection {
    fn allows(self, count: usize) -> bool {
        match self {
            Self::Independent => true,
            Self::RequireSingle => count == 1,
            Self::RequireMultiple => count >= 1,
        }
    }
}

/// How [`Action::target_criteria`] counts the selected records.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CriteriaMode {
    /// All of them have to meet it.
    #[default]
    TrueForAll,
    /// At least one has to.
    TrueAtLeastForOne,
}

/// Where an action belongs, which decides the place it shows in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ActionCategory {
    /// Creating records.
    New,
    /// Changing records.
    #[default]
    Edit,
    /// Saving and canceling.
    Save,
    /// Filtering and searching.
    Filter,
    /// Moving between records and views.
    Navigation,
    /// How the view shows things.
    View,
    /// Taking data out.
    Export,
    /// Anything else, with its name.
    Custom(String),
}

/// Whether views inside others count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Nesting {
    /// Any view.
    #[default]
    Any,
    /// Only a view of its own window.
    Root,
    /// Only a view inside another.
    Nested,
}

/// Which views an action or a controller is for. Anything left out does not narrow it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct TargetScope {
    /// Only views of this entity.
    pub entity: Option<String>,
    /// Only the view with this id.
    pub view_id: Option<String>,
    /// Only views of this kind.
    pub view_kind: Option<ViewKind>,
    /// Root or nested.
    pub nesting: Nesting,
}

impl TargetScope {
    /// Every view.
    pub fn any() -> Self {
        Self::default()
    }

    /// Only views of this entity.
    pub fn entity(mut self, entity: impl Into<String>) -> Self {
        self.entity = Some(entity.into());
        self
    }

    /// Only the view with this id.
    pub fn view_id(mut self, id: impl Into<String>) -> Self {
        self.view_id = Some(id.into());
        self
    }

    /// Only views of this kind.
    pub fn view_kind(mut self, kind: ViewKind) -> Self {
        self.view_kind = Some(kind);
        self
    }

    /// Root or nested.
    pub fn nesting(mut self, nesting: Nesting) -> Self {
        self.nesting = nesting;
        self
    }

    /// Whether it covers this view, in a frame of this context. No view is covered
    /// only by what asks nothing of one.
    pub fn covers(&self, view: Option<&ViewInfo>, context: FrameContext) -> bool {
        let nesting = match self.nesting {
            Nesting::Any => true,
            Nesting::Root => context != FrameContext::Nested,
            Nesting::Nested => context == FrameContext::Nested,
        };
        nesting
            && match view {
                Some(view) => {
                    self.entity.as_ref().is_none_or(|e| view.entity.as_ref() == Some(e))
                        && self.view_id.as_ref().is_none_or(|id| *id == view.id)
                        && self.view_kind.is_none_or(|kind| kind == view.kind)
                }
                None => self.entity.is_none() && self.view_id.is_none() && self.view_kind.is_none(),
            }
    }
}

/// One option of a single-choice action, which can hold others.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceItem {
    /// What the handler receives.
    pub id: String,
    /// What people see.
    pub caption: String,
    /// The options under this one.
    pub items: Vec<ChoiceItem>,
}

impl ChoiceItem {
    /// An option with nothing under it.
    pub fn new(id: impl Into<String>, caption: impl Into<String>) -> Self {
        Self { id: id.into(), caption: caption.into(), items: Vec::new() }
    }

    /// With this option under it.
    pub fn item(mut self, item: ChoiceItem) -> Self {
        self.items.push(item);
        self
    }

    fn has(&self, id: &str) -> bool {
        self.id == id || self.items.iter().any(|item| item.has(id))
    }
}

/// How an action asks for what it needs.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ActionKind {
    /// A button: it just runs.
    Simple,
    /// Opens a view in a window, and runs on OK.
    Popup {
        /// The view it opens.
        view: String,
    },
    /// Offers options, and runs with the one picked.
    SingleChoice(Vec<ChoiceItem>),
    /// Asks for a value, and runs with it.
    Parametrized(FieldKind),
    /// Opens an address made from the selected record, where `{field}` takes its value.
    Url(String),
}

/// What the running of an action asks for next, by whoever shows the application.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ActionResult {
    /// Nothing else.
    Done,
    /// Open this view in a window; [`Frame::accept_popup`](crate::Frame::accept_popup) or
    /// [`Frame::cancel_popup`](crate::Frame::cancel_popup) ends it.
    Popup(String),
    /// Show this view in the frame.
    ShowView(String),
    /// Open this address.
    OpenUrl(String),
}

/// What people gave an action that asks for something.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum ActionInput {
    /// Nothing.
    #[default]
    None,
    /// The option picked.
    Choice(String),
    /// The value typed.
    Parameter(Value),
}

/// What an action sees while running.
pub struct ActionContext {
    /// The frame it runs in.
    pub frame: crate::Frame,
    /// The records to read and write. A handler commits when it means to.
    pub space: Rc<RefCell<dyn ObjectSpace>>,
    /// The records selected.
    pub selected: Vec<Object>,
    /// The view it runs on.
    pub view: Option<ViewInfo>,
    /// What people gave it.
    pub input: ActionInput,
    next: RefCell<Option<ActionResult>>,
}

impl fmt::Debug for ActionContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActionContext").field("selected", &self.selected.len()).finish_non_exhaustive()
    }
}

impl ActionContext {
    pub(crate) fn new(
        frame: crate::Frame,
        space: Rc<RefCell<dyn ObjectSpace>>,
        selected: Vec<Object>,
        view: Option<ViewInfo>,
        input: ActionInput,
    ) -> Self {
        Self { frame, space, selected, view, input, next: RefCell::new(None) }
    }

    /// Asks the frame to show this view when the action ends.
    pub fn show_view(&self, id: impl Into<String>) {
        *self.next.borrow_mut() = Some(ActionResult::ShowView(id.into()));
    }

    pub(crate) fn take_next(&self) -> Option<ActionResult> {
        self.next.borrow_mut().take()
    }
}

/// Why an action did not run.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ActionError {
    /// No action has this id.
    Unknown(String),
    /// It is out of place here, with the reasons.
    Inactive(Vec<String>),
    /// It is shown but off, with the reasons.
    Disabled(Vec<String>),
    /// Something asked first said no.
    Canceled,
    /// What people gave it is not what it asks for.
    BadInput(String),
    /// It ran and failed, in words for people.
    Failed(String),
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(id) => write!(f, "there is no action {id}"),
            Self::Inactive(why) => write!(f, "the action is out of place: {}", why.join(", ")),
            Self::Disabled(why) => write!(f, "the action is off: {}", why.join(", ")),
            Self::Canceled => f.write_str("the action was canceled"),
            Self::BadInput(why) | Self::Failed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for ActionError {}

type Handler = Rc<dyn Fn(&ActionContext) -> Result<(), String>>;
type Guard = Rc<dyn Fn(&ActionContext) -> bool>;
type Notice = Rc<dyn Fn(&ActionContext)>;

struct ActionInner {
    id: String,
    caption: String,
    kind: ActionKind,
    category: ActionCategory,
    scope: TargetScope,
    selection: Selection,
    criteria: Option<(String, CriteriaMode)>,
    validation_contexts: Vec<String>,
    confirmation: Option<String>,
    hotkey: Option<String>,
    icon: Option<String>,
    on_execute: Option<Handler>,
    on_cancel: Option<Notice>,
    on_executing: Option<Guard>,
    on_executed: Option<Notice>,
    active: State<Reasons>,
    enabled: State<Reasons>,
}

/// Something people can do. A handle: the frame and its controller hold the same action, so a
/// controller can turn it off by a reason of its own.
///
/// ```
/// use ravenrs_app::{Action, ActionCategory, CriteriaMode, Selection};
///
/// let pay = Action::simple("Pay", "Pagar")
///     .category(ActionCategory::Edit)
///     .target_entity("Bill")
///     .selection(Selection::RequireMultiple)
///     .target_criteria("[Status] = 'Open'", CriteriaMode::TrueForAll)
///     .on_execute(|cx| {
///         for bill in &cx.selected {
///             let mut bill = bill.clone();
///             bill.set("Status", "Paid");
///             cx.space.borrow_mut().save(bill).map_err(|e| e.to_string())?;
///         }
///         cx.space.borrow_mut().commit().map_err(|e| e.to_string())
///     });
/// assert_eq!(pay.caption(), "Pagar");
/// ```
#[derive(Clone)]
pub struct Action(Rc<ActionInner>);

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Action").field("id", &self.0.id).finish_non_exhaustive()
    }
}

impl Action {
    fn new(id: impl Into<String>, caption: impl Into<String>, kind: ActionKind) -> Self {
        Self(Rc::new(ActionInner {
            id: id.into(),
            caption: caption.into(),
            kind,
            category: ActionCategory::default(),
            scope: TargetScope::default(),
            selection: Selection::default(),
            criteria: None,
            validation_contexts: Vec::new(),
            confirmation: None,
            hotkey: None,
            icon: None,
            on_execute: None,
            on_cancel: None,
            on_executing: None,
            on_executed: None,
            active: State::new(Reasons::default()),
            enabled: State::new(Reasons::default()),
        }))
    }

    /// A button: it just runs.
    pub fn simple(id: impl Into<String>, caption: impl Into<String>) -> Self {
        Self::new(id, caption, ActionKind::Simple)
    }

    /// Opens a view in a window, and runs on OK.
    pub fn popup(id: impl Into<String>, caption: impl Into<String>, view: impl Into<String>) -> Self {
        Self::new(id, caption, ActionKind::Popup { view: view.into() })
    }

    /// Offers these options, and runs with the one picked.
    pub fn single_choice(id: impl Into<String>, caption: impl Into<String>, items: Vec<ChoiceItem>) -> Self {
        Self::new(id, caption, ActionKind::SingleChoice(items))
    }

    /// Asks for a value of this kind, and runs with it.
    pub fn parametrized(id: impl Into<String>, caption: impl Into<String>, kind: FieldKind) -> Self {
        Self::new(id, caption, ActionKind::Parametrized(kind))
    }

    /// Opens an address made from the selected record, where `{field}` takes its value.
    pub fn url(id: impl Into<String>, caption: impl Into<String>, template: impl Into<String>) -> Self {
        Self::new(id, caption, ActionKind::Url(template.into()))
    }

    fn change(mut self, f: impl FnOnce(&mut ActionInner)) -> Self {
        // While it is being declared, nothing else holds it.
        if let Some(inner) = Rc::get_mut(&mut self.0) {
            f(inner);
        }
        self
    }

    /// Where it belongs.
    pub fn category(self, category: ActionCategory) -> Self {
        self.change(|a| a.category = category)
    }

    /// Only on views of this entity.
    pub fn target_entity(self, entity: impl Into<String>) -> Self {
        self.change(|a| a.scope.entity = Some(entity.into()))
    }

    /// Only on the view with this id.
    pub fn target_view(self, id: impl Into<String>) -> Self {
        self.change(|a| a.scope.view_id = Some(id.into()))
    }

    /// Only on views of this kind.
    pub fn target_view_kind(self, kind: ViewKind) -> Self {
        self.change(|a| a.scope.view_kind = Some(kind))
    }

    /// Only on root views, or only on nested ones.
    pub fn nesting(self, nesting: Nesting) -> Self {
        self.change(|a| a.scope.nesting = nesting)
    }

    /// What it needs selected.
    pub fn selection(self, selection: Selection) -> Self {
        self.change(|a| a.selection = selection)
    }

    /// What the selected records have to meet. With nothing selected, it does not stop anything;
    /// [`selection`](Self::selection) says whether something has to be.
    pub fn target_criteria(self, criteria: impl Into<String>, mode: CriteriaMode) -> Self {
        self.change(|a| a.criteria = Some((criteria.into(), mode)))
    }

    /// The validation contexts to pass before it runs.
    pub fn validation_contexts(self, contexts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.change(|a| a.validation_contexts = contexts.into_iter().map(Into::into).collect())
    }

    /// A question to ask before it runs.
    pub fn confirmation(self, text: impl Into<String>) -> Self {
        self.change(|a| a.confirmation = Some(text.into()))
    }

    /// The keys that run it, in the words of whoever shows it.
    pub fn hotkey(self, keys: impl Into<String>) -> Self {
        self.change(|a| a.hotkey = Some(keys.into()))
    }

    /// The name of its icon.
    pub fn icon(self, icon: impl Into<String>) -> Self {
        self.change(|a| a.icon = Some(icon.into()))
    }

    /// What it does.
    pub fn on_execute(self, handler: impl Fn(&ActionContext) -> Result<(), String> + 'static) -> Self {
        self.change(|a| a.on_execute = Some(Rc::new(handler)))
    }

    /// What a popup does when canceled.
    pub fn on_cancel(self, handler: impl Fn(&ActionContext) + 'static) -> Self {
        self.change(|a| a.on_cancel = Some(Rc::new(handler)))
    }

    /// Asked before it runs; false stops it.
    pub fn on_executing(self, guard: impl Fn(&ActionContext) -> bool + 'static) -> Self {
        self.change(|a| a.on_executing = Some(Rc::new(guard)))
    }

    /// Told after it ran.
    pub fn on_executed(self, notice: impl Fn(&ActionContext) + 'static) -> Self {
        self.change(|a| a.on_executed = Some(Rc::new(notice)))
    }

    /// Its id.
    pub fn id(&self) -> &str {
        &self.0.id
    }

    /// What people see.
    pub fn caption(&self) -> &str {
        &self.0.caption
    }

    /// How it asks for what it needs.
    pub fn kind(&self) -> &ActionKind {
        &self.0.kind
    }

    /// Where it belongs.
    pub fn action_category(&self) -> &ActionCategory {
        &self.0.category
    }

    /// The question to ask before it runs.
    pub fn confirmation_text(&self) -> Option<&str> {
        self.0.confirmation.as_deref()
    }

    /// The keys that run it.
    pub fn hotkey_text(&self) -> Option<&str> {
        self.0.hotkey.as_deref()
    }

    /// The name of its icon.
    pub fn icon_name(&self) -> Option<&str> {
        self.0.icon.as_deref()
    }

    /// The validation contexts to pass before it runs.
    pub fn contexts(&self) -> &[String] {
        &self.0.validation_contexts
    }

    /// Whether it is in place. If not, it is not shown.
    pub fn is_active(&self) -> bool {
        self.0.active.with(Reasons::all)
    }

    /// Whether it can be used. If not, it is shown faded.
    pub fn is_enabled(&self) -> bool {
        self.0.enabled.with(Reasons::all)
    }

    /// Why it is or is not in place.
    pub fn active_reasons(&self) -> Reasons {
        self.0.active.get()
    }

    /// Why it can or cannot be used.
    pub fn enabled_reasons(&self) -> Reasons {
        self.0.enabled.get()
    }

    /// Gives a reason for it being in place or not.
    pub fn set_active(&self, reason: &str, on: bool) {
        if self.0.active.peek().get(reason) != Some(on) {
            self.0.active.update(|reasons| reasons.set(reason, on));
        }
    }

    /// Gives a reason for it being usable or not. Prefer this to
    /// [`set_active`](Self::set_active) when it depends on the record: taking a button away
    /// moves the ones beside it, and people click the wrong one.
    pub fn set_enabled(&self, reason: &str, on: bool) {
        if self.0.enabled.peek().get(reason) != Some(on) {
            self.0.enabled.update(|reasons| reasons.set(reason, on));
        }
    }

    /// Says what is wrong with how it was declared.
    pub(crate) fn check(&self) -> Result<(), ModelError> {
        if let Some((text, _)) = &self.0.criteria {
            Criteria::parse(text).map_err(|e| ModelError(format!("the action {}: {e}", self.0.id)))?;
        }
        Ok(())
    }

    /// Answers the reasons that follow from the frame.
    pub(crate) fn update(
        &self,
        view: Option<&ViewInfo>,
        context: FrameContext,
        selected: &[Object],
        environment: &Environment,
    ) {
        self.set_active(REASON_SCOPE, self.0.scope.covers(view, context));
        self.set_enabled(REASON_SELECTION, self.0.selection.allows(selected.len()));
        self.set_enabled(REASON_CRITERIA, self.meets_criteria(selected, environment));
    }

    fn meets_criteria(&self, selected: &[Object], environment: &Environment) -> bool {
        let Some((text, mode)) = &self.0.criteria else { return true };
        if selected.is_empty() {
            return true;
        }
        // `check` refuses what does not parse, when the application is put together.
        let Ok(criteria) = Criteria::parse(text) else { return false };
        let meets = |object: &Object| Scope::new(environment).row(object).matches(&criteria);
        match mode {
            CriteriaMode::TrueForAll => selected.iter().all(meets),
            CriteriaMode::TrueAtLeastForOne => selected.iter().any(meets),
        }
    }

    /// Says whether what people gave it is what it asks for.
    pub(crate) fn check_input(&self, input: &ActionInput) -> Result<(), ActionError> {
        match (&self.0.kind, input) {
            (ActionKind::SingleChoice(items), ActionInput::Choice(id)) => {
                if items.iter().any(|item| item.has(id)) {
                    Ok(())
                } else {
                    Err(ActionError::BadInput(format!("{} has no option {id}", self.0.id)))
                }
            }
            (ActionKind::SingleChoice(_), _) => {
                Err(ActionError::BadInput(format!("{} needs an option", self.0.id)))
            }
            (ActionKind::Parametrized(_), ActionInput::Parameter(_)) => Ok(()),
            (ActionKind::Parametrized(_), _) => {
                Err(ActionError::BadInput(format!("{} needs a value", self.0.id)))
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn run_guard(&self, cx: &ActionContext) -> bool {
        self.0.on_executing.as_ref().is_none_or(|guard| guard(cx))
    }

    pub(crate) fn run(&self, cx: &ActionContext) -> Result<(), String> {
        self.0.on_execute.as_ref().map_or(Ok(()), |handler| handler(cx))
    }

    pub(crate) fn run_notice(&self, cx: &ActionContext) {
        if let Some(notice) = &self.0.on_executed {
            notice(cx);
        }
    }

    pub(crate) fn run_cancel(&self, cx: &ActionContext) {
        if let Some(notice) = &self.0.on_cancel {
            notice(cx);
        }
    }
}

/// Puts a value in an address, where it cannot change what the address means.
fn encoded(value: &Value) -> String {
    let text = match value {
        Value::Null => String::new(),
        Value::Text(text) => text.clone(),
        Value::Number(number) if number.fract() == 0.0 => format!("{}", *number as i64),
        Value::Number(number) => number.to_string(),
        Value::Bool(on) => on.to_string(),
        other => format!("{other:?}"),
    };
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The address of a [`ActionKind::Url`], with `{field}` taken from the record.
pub(crate) fn fill_url(template: &str, object: Option<&Object>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        out.push_str(&encoded(&object.map_or(Value::Null, |o| o.get(&after[..end]))));
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn object(status: &str) -> Object {
        let values = BTreeMap::from([("Status".to_owned(), Value::Text(status.to_owned()))]);
        Object::from_values("Bill", values)
    }

    fn view(id: &str, kind: ViewKind, entity: Option<&str>) -> ViewInfo {
        ViewInfo::new(id, kind, entity)
    }

    #[test]
    fn every_reason_has_to_say_yes() {
        let mut reasons = Reasons::default();
        assert!(reasons.all(), "with none, nothing says no");
        reasons.set("a", true);
        reasons.set("b", false);
        assert!(!reasons.all());
        assert_eq!(reasons.against().collect::<Vec<_>>(), ["b"]);
        reasons.set("b", true);
        assert!(reasons.all());
    }

    #[test]
    fn each_part_of_the_scope_narrows_it() {
        let list = view("Bill_ListView", ViewKind::List, Some("Bill"));
        let main = FrameContext::Main;
        assert!(TargetScope::any().covers(Some(&list), main));
        assert!(TargetScope::any().entity("Bill").covers(Some(&list), main));
        assert!(!TargetScope::any().entity("Other").covers(Some(&list), main));
        assert!(TargetScope::any().view_id("Bill_ListView").covers(Some(&list), main));
        assert!(!TargetScope::any().view_id("Other").covers(Some(&list), main));
        assert!(TargetScope::any().view_kind(ViewKind::List).covers(Some(&list), main));
        assert!(!TargetScope::any().view_kind(ViewKind::Detail).covers(Some(&list), main));
        assert!(TargetScope::any().nesting(Nesting::Root).covers(Some(&list), main));
        assert!(!TargetScope::any().nesting(Nesting::Nested).covers(Some(&list), main));
        assert!(!TargetScope::any().nesting(Nesting::Root).covers(Some(&list), FrameContext::Nested));
        assert!(TargetScope::any().nesting(Nesting::Nested).covers(Some(&list), FrameContext::Nested));
        assert!(TargetScope::any().entity("Bill").view_kind(ViewKind::List).covers(Some(&list), main));
        assert!(!TargetScope::any().entity("Bill").view_kind(ViewKind::Detail).covers(Some(&list), main));
    }

    #[test]
    fn with_no_view_only_what_asks_nothing_of_one_applies() {
        assert!(TargetScope::any().covers(None, FrameContext::Main));
        assert!(!TargetScope::any().entity("Bill").covers(None, FrameContext::Main));
        assert!(!TargetScope::any().view_kind(ViewKind::List).covers(None, FrameContext::Main));
    }

    #[test]
    fn selection_counts_what_is_selected() {
        let enabled = |selection: Selection, count: usize| selection.allows(count);
        assert!(enabled(Selection::Independent, 0));
        assert!(enabled(Selection::Independent, 5));
        assert!(!enabled(Selection::RequireSingle, 0));
        assert!(enabled(Selection::RequireSingle, 1));
        assert!(!enabled(Selection::RequireSingle, 2));
        assert!(!enabled(Selection::RequireMultiple, 0));
        assert!(enabled(Selection::RequireMultiple, 1));
        assert!(enabled(Selection::RequireMultiple, 2));
    }

    #[test]
    fn criteria_count_all_or_at_least_one() {
        let env = Environment::new();
        let open = "[Status] = 'Open'";
        let all = Action::simple("a", "a").target_criteria(open, CriteriaMode::TrueForAll);
        let one = Action::simple("b", "b").target_criteria(open, CriteriaMode::TrueAtLeastForOne);
        let mixed = [object("Open"), object("Paid")];
        assert!(!all.meets_criteria(&mixed, &env));
        assert!(one.meets_criteria(&mixed, &env));
        let open_only = [object("Open"), object("Open")];
        assert!(all.meets_criteria(&open_only, &env));
        let paid_only = [object("Paid")];
        assert!(!one.meets_criteria(&paid_only, &env));
        assert!(all.meets_criteria(&[], &env), "with nothing selected, it stops nothing");
        assert!(one.meets_criteria(&[], &env));
    }

    #[test]
    fn a_bad_criteria_is_found_when_it_is_checked() {
        let bad = Action::simple("a", "a").target_criteria("[Status] =", CriteriaMode::TrueForAll);
        assert!(bad.check().is_err_and(|e| e.0.contains("the action a")));
        assert!(Action::simple("b", "b").check().is_ok());
    }

    #[test]
    fn addresses_take_values_that_cannot_change_their_meaning() {
        let values = BTreeMap::from([
            ("q".to_owned(), Value::Text("a b&c=d/ção".to_owned())),
            ("n".to_owned(), Value::Number(12.0)),
        ]);
        let record = Object::from_values("X", values);
        assert_eq!(
            fill_url("https://x.dev/s?q={q}&n={n}", Some(&record)),
            "https://x.dev/s?q=a%20b%26c%3Dd%2F%C3%A7%C3%A3o&n=12"
        );
        assert_eq!(fill_url("https://x.dev/{none}", Some(&record)), "https://x.dev/");
        assert_eq!(fill_url("https://x.dev/{q}", None), "https://x.dev/");
        assert_eq!(fill_url("no close {q", Some(&record)), "no close {q");
    }

    #[test]
    fn options_are_found_at_any_depth() {
        let action = Action::single_choice(
            "Mode",
            "Modo",
            vec![ChoiceItem::new("a", "A").item(ChoiceItem::new("a1", "A1"))],
        );
        assert!(action.check_input(&ActionInput::Choice("a1".into())).is_ok());
        assert!(action.check_input(&ActionInput::Choice("z".into())).is_err());
        assert!(action.check_input(&ActionInput::None).is_err());
    }
}
