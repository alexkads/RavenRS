//! What every application needs: creating, saving, removing and moving between records.

use ravenrs_app::{
    Action, ActionCategory, ActionInput, Frame, Module, Nesting, Selection, TargetScope, Value,
    ViewController, ViewKind,
};

use crate::nested::NestedOwner;

use crate::session::{Session, Words};

/// The ids of the actions of [`SystemModule`].
pub mod ids {
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
    /// Puts an existing record on a list inside a form.
    pub const LINK: &str = "LinkObject";
    /// Takes records off a list inside a form, without removing them.
    pub const UNLINK: &str = "UnlinkObject";
}

/// The actions every application needs. Searching, filtering and exporting a list come with
/// [`DataGrid`](ravenrs_widgets::DataGrid), and are not repeated here.
#[derive(Debug, Default)]
pub struct SystemModule {
    words: Words,
}

impl SystemModule {
    /// With its words in English.
    pub fn new() -> Self {
        Self::default()
    }

    /// With these words.
    pub fn with_words(words: Words) -> Self {
        Self { words }
    }
}

fn session_of(cx: &ravenrs_app::ActionContext) -> Result<Session, String> {
    Session::of(cx).ok_or_else(|| "this action needs the application's screens".to_owned())
}

fn entity_of(cx: &ravenrs_app::ActionContext) -> Result<String, String> {
    cx.view.as_ref().and_then(|view| view.entity.clone()).ok_or_else(|| "this view has no entity".to_owned())
}

/// A dashboard has no records of its own to act on.
fn off_on_a_dashboard(frame: &Frame, ids: &[&str]) {
    let on = frame.view().is_none_or(|view| view.kind != ViewKind::Dashboard);
    for id in ids {
        if let Some(action) = frame.action(id) {
            action.set_active("dashboard", on);
        }
    }
}

struct NewObjectController(Words);

impl ViewController for NewObjectController {
    fn id(&self) -> &str {
        "NewObject"
    }
    fn scope(&self) -> TargetScope {
        TargetScope::any().nesting(Nesting::Root)
    }
    fn activated(&mut self, frame: &Frame) {
        off_on_a_dashboard(frame, &[ids::NEW]);
    }
    fn actions(&self) -> Vec<Action> {
        vec![
            Action::simple(ids::NEW, self.0.new.clone())
                .category(ActionCategory::New)
                .icon("plus")
                .on_execute(|cx| {
                    let (session, entity) = (session_of(cx)?, entity_of(cx)?);
                    let opener = session.clone();
                    session.leaving(move || opener.open_new(&entity));
                    Ok(())
                }),
        ]
    }
}

struct DeleteController(Words);

impl ViewController for DeleteController {
    fn id(&self) -> &str {
        "DeleteObjects"
    }
    fn activated(&mut self, frame: &Frame) {
        off_on_a_dashboard(frame, &[ids::DELETE]);
    }
    fn actions(&self) -> Vec<Action> {
        vec![
            Action::simple(ids::DELETE, self.0.delete.clone())
                .category(ActionCategory::Edit)
                .icon("trash-2")
                .selection(Selection::RequireMultiple)
                .confirmation(self.0.delete_question.clone())
                .on_execute(|cx| session_of(cx)?.delete(&cx.selected)),
        ]
    }
}

struct ModificationsController(Words);

impl ViewController for ModificationsController {
    fn id(&self) -> &str {
        "Modifications"
    }
    fn scope(&self) -> TargetScope {
        TargetScope::any().view_kind(ViewKind::Detail)
    }
    fn actions(&self) -> Vec<Action> {
        vec![
            Action::simple(ids::SAVE, self.0.save.clone())
                .category(ActionCategory::Save)
                .icon("save")
                .hotkey("command+s")
                .on_execute(|cx| session_of(cx)?.save()),
            Action::simple(ids::SAVE_AND_CLOSE, self.0.save_and_close.clone())
                .category(ActionCategory::Save)
                .hotkey("command+shift+s")
                .on_execute(|cx| {
                    let session = session_of(cx)?;
                    session.save()?;
                    session.back_now();
                    Ok(())
                }),
            Action::simple(ids::SAVE_AND_NEW, self.0.save_and_new.clone())
                .category(ActionCategory::Save)
                .on_execute(|cx| {
                    let (session, entity) = (session_of(cx)?, entity_of(cx)?);
                    session.save()?;
                    session.open_new(&entity);
                    Ok(())
                }),
            Action::simple(ids::CANCEL, self.0.cancel.clone()).category(ActionCategory::Save).on_execute(
                |cx| {
                    session_of(cx)?.cancel();
                    Ok(())
                },
            ),
        ]
    }
}

struct RecordsNavigationController(Words);

impl ViewController for RecordsNavigationController {
    fn id(&self) -> &str {
        "RecordsNavigation"
    }
    fn scope(&self) -> TargetScope {
        TargetScope::any().view_kind(ViewKind::Detail)
    }
    fn actions(&self) -> Vec<Action> {
        vec![
            Action::simple(ids::BACK, self.0.back.clone())
                .category(ActionCategory::Navigation)
                .icon("arrow-left")
                .on_execute(|cx| {
                    session_of(cx)?.back();
                    Ok(())
                }),
            Action::simple(ids::PREVIOUS, self.0.previous.clone())
                .category(ActionCategory::Navigation)
                .icon("chevron-left")
                .on_execute(|cx| {
                    session_of(cx)?.step(-1);
                    Ok(())
                }),
            Action::simple(ids::NEXT, self.0.next.clone())
                .category(ActionCategory::Navigation)
                .icon("chevron-right")
                .on_execute(|cx| {
                    session_of(cx)?.step(1);
                    Ok(())
                }),
        ]
    }
}

struct RefreshController(Words);

impl ViewController for RefreshController {
    fn id(&self) -> &str {
        "Refresh"
    }
    fn scope(&self) -> TargetScope {
        TargetScope::any().nesting(ravenrs_app::Nesting::Root)
    }
    fn actions(&self) -> Vec<Action> {
        vec![
            Action::simple(ids::REFRESH, self.0.refresh.clone())
                .category(ActionCategory::View)
                .icon("refresh-cw")
                .on_execute(|cx| {
                    session_of(cx)?.refresh();
                    Ok(())
                }),
        ]
    }
}

struct VariantsController(Words);

impl ViewController for VariantsController {
    fn id(&self) -> &str {
        "ViewVariants"
    }
    fn scope(&self) -> TargetScope {
        TargetScope::any().view_kind(ViewKind::List)
    }
    fn actions(&self) -> Vec<Action> {
        vec![
            Action::single_choice(ids::VARIANTS, self.0.variant.clone(), Vec::new())
                .category(ActionCategory::View)
                .on_execute(|cx| {
                    let session = session_of(cx)?;
                    let view = cx.view.as_ref().ok_or_else(|| "no view".to_owned())?;
                    match &cx.input {
                        ActionInput::Choice(variant) => session.choose_variant(&view.id, variant),
                        _ => Ok(()),
                    }
                }),
        ]
    }
    fn activated(&mut self, frame: &Frame) {
        // One way to see a list is no choice at all.
        let many = frame.view().zip(frame.model()).is_some_and(|(view, model)| {
            model.at(&["Views", &view.id, "Variants"]).is_some_and(|all| all.children.len() >= 2)
        });
        if let Some(action) = frame.action(ids::VARIANTS) {
            action.set_active("variants", many);
        }
    }
}

struct LinkController(Words);

/// Changes what a record points to, keeps that, and shows it.
fn point(
    cx: &ravenrs_app::ActionContext,
    record: &ravenrs_app::Object,
    field: &str,
    to: Value,
) -> Result<(), String> {
    let mut record = record.clone();
    record.set(field, to);
    cx.space.borrow_mut().save(record).map_err(|e| e.to_string())
}

fn keep(cx: &ravenrs_app::ActionContext) -> Result<(), String> {
    let kept = cx.space.borrow_mut().commit().map_err(|e| e.to_string());
    if kept.is_err() {
        cx.space.borrow_mut().rollback();
    }
    kept?;
    session_of(cx)?.touch();
    Ok(())
}

impl ViewController for LinkController {
    fn id(&self) -> &str {
        "LinkObjects"
    }
    fn scope(&self) -> TargetScope {
        TargetScope::any().view_kind(ViewKind::List).nesting(Nesting::Nested)
    }
    fn actions(&self) -> Vec<Action> {
        vec![
            Action::simple(ids::LINK, self.0.link.clone())
                .category(ActionCategory::Edit)
                .icon("link")
                .on_execute(|cx| {
                    let owner = cx.frame.extension::<NestedOwner>().ok_or_else(|| "no list".to_owned())?;
                    let ActionInput::Parameter(key) = &cx.input else {
                        // Asked for the first time: it opens the list to pick from, and OK comes back here.
                        cx.frame.open_popup_on(ids::LINK, &format!("{}_LookupListView", owner.target));
                        return Ok(());
                    };
                    let found = cx.space.borrow().find(&owner.target, key).map_err(|e| e.to_string())?;
                    let Some(record) = found else { return Ok(()) };
                    point(cx, &record, &owner.field, owner.key.clone())?;
                    keep(cx)
                }),
            Action::simple(ids::UNLINK, self.0.unlink.clone())
                .category(ActionCategory::Edit)
                .icon("unlink")
                .selection(Selection::RequireMultiple)
                .on_execute(|cx| {
                    let owner = cx.frame.extension::<NestedOwner>().ok_or_else(|| "no list".to_owned())?;
                    for record in &cx.selected {
                        point(cx, record, &owner.field, Value::Null)?;
                    }
                    keep(cx)
                }),
        ]
    }
}

impl Module for SystemModule {
    fn name(&self) -> &str {
        "System"
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        let words = || self.words.clone();
        vec![
            Box::new(RecordsNavigationController(words())),
            Box::new(NewObjectController(words())),
            Box::new(ModificationsController(words())),
            Box::new(DeleteController(words())),
            Box::new(RefreshController(words())),
            Box::new(VariantsController(words())),
            Box::new(LinkController(words())),
        ]
    }
}
