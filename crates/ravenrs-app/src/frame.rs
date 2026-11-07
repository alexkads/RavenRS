//! Where a view lives: its controllers, its actions and its records.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fmt;
use std::rc::Rc;

use ravenrs_reactive::State;

use crate::action::{
    ActionContext, ActionError, ActionInput, ActionKind, ActionResult, REASON_CONTROLLER, fill_url,
};
use crate::controller::{ViewController, WindowController};
use crate::{Action, ModelError, ModelNode, Object, ObjectSpace};

/// What a view shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewKind {
    /// Many records.
    List,
    /// One record.
    Detail,
    /// Other views together.
    Dashboard,
}

/// A view, as the model describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ViewInfo {
    /// Its id in the model.
    pub id: String,
    /// What it shows.
    pub kind: ViewKind,
    /// The entity it shows; a dashboard has none.
    pub entity: Option<String>,
}

impl ViewInfo {
    /// A view with this id.
    pub fn new(id: impl Into<String>, kind: ViewKind, entity: Option<&str>) -> Self {
        Self { id: id.into(), kind, entity: entity.map(str::to_owned) }
    }

    /// Reads a view from the model.
    pub fn from_model(root: &ModelNode, id: &str) -> Result<Self, ModelError> {
        let node =
            root.at(&["Views", id]).ok_or_else(|| ModelError(format!("the model has no view {id}")))?;
        let kind = match node.text("Kind") {
            Some("ListView") => ViewKind::List,
            Some("DetailView") => ViewKind::Detail,
            Some("DashboardView") => ViewKind::Dashboard,
            other => return Err(ModelError(format!("the view {id} is of an unknown kind: {other:?}"))),
        };
        Ok(Self::new(id, kind, node.text("Entity")))
    }
}

/// Where a frame is, which decides its template and what targets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameContext {
    /// The main window.
    Main,
    /// Inside another view.
    Nested,
    /// A window over the others.
    Popup,
    /// A window to pick a record.
    Lookup,
}

/// What a frame goes through, in order, for whoever shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FrameEvent {
    /// The view is about to change, to this one.
    ViewChanging(Option<String>),
    /// The view changed, and the controllers that fit it are on.
    ViewChanged(Option<String>),
}

type Validator = Rc<dyn Fn(&[String], &ActionContext) -> Result<(), String>>;
type FrameWatcher = Rc<dyn Fn(&FrameEvent)>;

/// A view waiting for its turn, with the unit of work to take on, if a new one.
type PendingView = (Option<ViewInfo>, Option<Rc<RefCell<dyn ObjectSpace>>>);

struct ViewSlot {
    controller: RefCell<Box<dyn ViewController>>,
    id: String,
    actions: Vec<Action>,
    /// Reasons given from outside, and whether it is on now.
    reasons: RefCell<crate::action::Reasons>,
    on: Cell<bool>,
}

struct WindowSlot {
    controller: RefCell<Box<dyn WindowController>>,
    actions: Vec<Action>,
}

type Messenger = Rc<dyn Fn(crate::Message)>;

struct FrameInner {
    context: FrameContext,
    space: RefCell<Rc<RefCell<dyn ObjectSpace>>>,
    model: Option<Rc<ModelNode>>,
    extensions: RefCell<Vec<Rc<dyn Any>>>,
    entities: RefCell<Rc<Vec<crate::EntityInfo>>>,
    view: State<Option<ViewInfo>>,
    selected: State<Vec<Object>>,
    popup: State<Option<(String, String)>>,
    views: Vec<ViewSlot>,
    windows: Vec<WindowSlot>,
    watchers: RefCell<Vec<FrameWatcher>>,
    validator: RefCell<Option<Validator>>,
    messenger: RefCell<Option<Messenger>>,
    security: RefCell<Option<Rc<crate::SecurityContext>>>,
    /// Views asked for while one was being changed to, which wait for it to end.
    pending: RefCell<VecDeque<PendingView>>,
    changing: Cell<bool>,
    /// Background work that started here and did not end yet.
    jobs: RefCell<Vec<crate::Job>>,
    job_waker: RefCell<Option<crate::JobWaker>>,
}

/// Where a view lives: it holds the view, the controllers that fit it, their actions and a
/// unit of work. A handle, so controllers and actions can keep one.
///
/// Changing the view tells whoever watches, turns off the controllers that were on, changes the
/// view, turns on those that fit the new one, and tells again.
#[derive(Clone)]
pub struct Frame(Rc<FrameInner>);

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame").field("context", &self.0.context).finish_non_exhaustive()
    }
}

impl Frame {
    /// A frame with no view, with these controllers. Window controllers turn on now, if
    /// the context fits them, and stay on.
    pub fn new(
        context: FrameContext,
        space: Rc<RefCell<dyn ObjectSpace>>,
        model: Option<Rc<ModelNode>>,
        view_controllers: Vec<Box<dyn ViewController>>,
        window_controllers: Vec<Box<dyn WindowController>>,
    ) -> Self {
        let views = view_controllers
            .into_iter()
            .map(|controller| ViewSlot {
                id: controller.id().to_owned(),
                actions: controller.actions(),
                controller: RefCell::new(controller),
                reasons: RefCell::default(),
                on: Cell::new(false),
            })
            .collect();
        let windows: Vec<WindowSlot> = window_controllers
            .into_iter()
            .map(|controller| WindowSlot {
                actions: controller.actions(),
                controller: RefCell::new(controller),
            })
            .collect();
        let frame = Self(Rc::new(FrameInner {
            context,
            space: RefCell::new(space),
            model,
            extensions: RefCell::default(),
            entities: RefCell::default(),
            view: State::new(None),
            selected: State::new(Vec::new()),
            popup: State::new(None),
            views,
            windows,
            watchers: RefCell::default(),
            validator: RefCell::default(),
            messenger: RefCell::default(),
            security: RefCell::default(),
            pending: RefCell::default(),
            changing: Cell::new(false),
            jobs: RefCell::default(),
            job_waker: RefCell::default(),
        }));
        for slot in &frame.0.windows {
            let fits = slot.controller.borrow().contexts().contains(&context);
            for action in &slot.actions {
                action.set_active(REASON_CONTROLLER, fits);
            }
            if fits {
                slot.controller.borrow_mut().activated(&frame);
            }
        }
        frame.refresh_controllers();
        frame.refresh_actions();
        frame
    }

    /// Gives messages to this, which shows them.
    pub fn set_messenger(&self, messenger: impl Fn(crate::Message) + 'static) {
        *self.0.messenger.borrow_mut() = Some(Rc::new(messenger));
    }

    /// Tells people something, as long as someone showing the application listens.
    pub fn show_message(&self, message: crate::Message) {
        let messenger = self.0.messenger.borrow().clone();
        if let Some(messenger) = messenger {
            messenger(message);
        }
    }

    /// Where it is.
    pub fn context(&self) -> FrameContext {
        self.0.context
    }

    /// The unit of work of its view.
    pub fn space(&self) -> Rc<RefCell<dyn ObjectSpace>> {
        self.0.space.borrow().clone()
    }

    /// The entities whose appearance rules turn actions on and off.
    pub fn set_entities(&self, entities: Rc<Vec<crate::EntityInfo>>) {
        *self.0.entities.borrow_mut() = entities;
        self.refresh_actions();
    }

    /// Turns actions off by what this person can do. What protects records is the secured space;
    /// this is what keeps people from reaching for what would be refused.
    pub fn set_security(&self, security: Rc<crate::SecurityContext>) {
        *self.0.security.borrow_mut() = Some(security);
        self.refresh_actions();
    }

    /// What this person can do, when the application has permissions.
    pub fn security(&self) -> Option<Rc<crate::SecurityContext>> {
        self.0.security.borrow().clone()
    }

    /// Keeps something for whoever holds the frame to find by its type, as what shows it
    /// does for its controllers and actions.
    pub fn set_extension<T: 'static>(&self, value: Rc<T>) {
        self.0.extensions.borrow_mut().retain(|kept| !kept.is::<T>());
        self.0.extensions.borrow_mut().push(value);
    }

    pub(crate) fn keep_extension(&self, value: Rc<dyn Any>) {
        self.0.extensions.borrow_mut().push(value);
    }

    /// What was kept of this type.
    pub fn extension<T: 'static>(&self) -> Option<Rc<T>> {
        let kept = self.0.extensions.borrow().iter().find(|kept| kept.is::<T>()).cloned();
        kept.and_then(|kept| kept.downcast().ok())
    }

    /// The application model, when it was given one.
    pub fn model(&self) -> Option<Rc<ModelNode>> {
        self.0.model.clone()
    }

    /// The view it holds.
    pub fn view(&self) -> Option<ViewInfo> {
        self.0.view.get()
    }

    /// The records selected in the view.
    pub fn selected(&self) -> Vec<Object> {
        self.0.selected.get()
    }

    /// The popup open, as its action's id and its view's id.
    pub fn open_popup(&self) -> Option<(String, String)> {
        self.0.popup.get()
    }

    /// Calls this as the view changes.
    pub fn on_event(&self, watcher: impl Fn(&FrameEvent) + 'static) {
        self.0.watchers.borrow_mut().push(Rc::new(watcher));
    }

    /// What passes actions' validation contexts before they run.
    pub fn set_validator(
        &self,
        validator: impl Fn(&[String], &ActionContext) -> Result<(), String> + 'static,
    ) {
        *self.0.validator.borrow_mut() = Some(Rc::new(validator));
    }

    fn tell(&self, event: &FrameEvent) {
        let watchers = self.0.watchers.borrow().clone();
        for watcher in watchers {
            watcher(event);
        }
    }

    /// Shows the view with this id in the model.
    pub fn show_view(&self, id: &str) -> Result<(), ModelError> {
        let model = self.0.model.as_ref().ok_or_else(|| ModelError("this frame has no model".into()))?;
        self.set_view(Some(ViewInfo::from_model(model, id)?));
        Ok(())
    }

    /// Changes the view. Asked while a change is under way, as from a controller turning on,
    /// it happens after that one ends.
    pub fn set_view(&self, view: Option<ViewInfo>) {
        self.queue_view(view, None);
    }

    /// Changes the view and takes on a new unit of work with it, as each view has its own.
    /// The controllers that turn on already see it.
    pub fn set_view_in(&self, view: Option<ViewInfo>, space: Rc<RefCell<dyn ObjectSpace>>) {
        self.queue_view(view, Some(space));
    }

    fn queue_view(&self, view: Option<ViewInfo>, space: Option<Rc<RefCell<dyn ObjectSpace>>>) {
        self.0.pending.borrow_mut().push_back((view, space));
        if self.0.changing.replace(true) {
            return;
        }
        loop {
            let next = self.0.pending.borrow_mut().pop_front();
            let Some((next, space)) = next else { break };
            self.change_view(next, space);
        }
        self.0.changing.set(false);
    }

    fn change_view(&self, view: Option<ViewInfo>, space: Option<Rc<RefCell<dyn ObjectSpace>>>) {
        let id = view.as_ref().map(|v| v.id.clone());
        self.tell(&FrameEvent::ViewChanging(id.clone()));
        for slot in &self.0.views {
            self.turn(slot, false);
        }
        if let Some(space) = space {
            *self.0.space.borrow_mut() = space;
        }
        self.0.view.set(view);
        self.0.selected.set(Vec::new());
        self.refresh_controllers();
        self.refresh_actions();
        self.tell(&FrameEvent::ViewChanged(id));
    }

    fn turn(&self, slot: &ViewSlot, on: bool) {
        if slot.on.replace(on) != on {
            if on {
                slot.controller.borrow_mut().activated(self);
            } else {
                slot.controller.borrow_mut().deactivated(self);
            }
        }
    }

    fn refresh_controllers(&self) {
        let view = self.0.view.peek().clone();
        for slot in &self.0.views {
            let fits = slot.controller.borrow().scope().covers(view.as_ref(), self.0.context);
            let on = fits && view.is_some() && slot.reasons.borrow().all();
            for action in &slot.actions {
                action.set_active(REASON_CONTROLLER, on);
            }
            self.turn(slot, on);
        }
    }

    fn refresh_actions(&self) {
        let view = self.0.view.peek().clone();
        let selected = self.0.selected.peek().clone();
        let entities = self.0.entities.borrow().clone();
        let environment = crate::environment();
        let entity = view
            .as_ref()
            .and_then(|view| view.entity.as_deref())
            .and_then(|name| entities.iter().find(|entity| entity.name == name));
        for action in self.actions() {
            action.update(view.as_ref(), self.0.context, &selected, &environment);
            let (mut enabled, mut visible) = (true, true);
            if let (Some(entity), Some(view)) = (entity, view.as_ref()) {
                // One record that does not fit is enough to turn it off.
                let records: Vec<Option<&Object>> =
                    if selected.is_empty() { vec![None] } else { selected.iter().map(Some).collect() };
                for record in records {
                    let found =
                        entity.appearance_of(crate::AppearanceTarget::Action, action.id(), view, record);
                    enabled &= found.enabled.unwrap_or(true);
                    visible &= found.visible.unwrap_or(true);
                }
            }
            // Among many records, what fits changes from row to row: taking the button away
            // would move its neighbors, so it only fades.
            if view.as_ref().is_some_and(|view| view.kind == ViewKind::List) && !visible {
                enabled = false;
                visible = true;
            }
            action.set_enabled(crate::REASON_APPEARANCE, enabled);
            action.set_active(crate::REASON_APPEARANCE, visible);
            if let Some(security) = self.0.security.borrow().as_ref() {
                let (exists, can) = security.with(|permissions| {
                    let can = match (action.required_operation(), entity) {
                        (Some(operation), Some(entity)) if selected.is_empty() => {
                            permissions.can(operation, &entity.name, None)
                        }
                        (Some(operation), Some(entity)) => selected
                            .iter()
                            .all(|record| permissions.can(operation, &entity.name, Some(record))),
                        _ => true,
                    };
                    (permissions.can_use_action(action.id()), can)
                });
                action.set_active(crate::REASON_SECURITY, exists);
                action.set_enabled(crate::REASON_SECURITY, can);
            }
        }
    }

    /// Tells the controllers that are on that the view's controls exist now.
    pub fn view_controls_created(&self) {
        for slot in self.0.views.iter().filter(|slot| slot.on.get()) {
            slot.controller.borrow_mut().view_controls_created(self);
        }
    }

    /// Gives a reason for a controller being on or off. Off, it turns off all its actions.
    pub fn set_controller_active(&self, id: &str, reason: &str, on: bool) {
        if let Some(slot) = self.0.views.iter().find(|slot| slot.id == id) {
            slot.reasons.borrow_mut().set(reason, on);
            self.refresh_controllers();
        }
    }

    /// Whether a controller is on.
    pub fn is_controller_active(&self, id: &str) -> bool {
        self.0.views.iter().any(|slot| slot.id == id && slot.on.get())
    }

    /// Sets what is selected, which turns actions on and off.
    pub fn set_selected(&self, selected: Vec<Object>) {
        self.0.selected.set(selected);
        self.refresh_actions();
    }

    /// Every action of its controllers, whether on or off, in the order they came.
    pub fn actions(&self) -> Vec<Action> {
        let window = self.0.windows.iter().flat_map(|slot| slot.actions.iter());
        let view = self.0.views.iter().flat_map(|slot| slot.actions.iter());
        window.chain(view).cloned().collect()
    }

    /// The action with this id.
    pub fn action(&self, id: &str) -> Option<Action> {
        self.actions().into_iter().find(|action| action.id() == id)
    }

    fn context_for(&self, input: ActionInput) -> ActionContext {
        ActionContext::new(self.clone(), self.space(), self.0.selected.get(), self.0.view.get(), input)
    }

    /// Runs an action, if it is on: asks whoever guards it, passes its validation contexts, runs
    /// it, and says what comes next. Asking the question first is up to whoever shows it.
    pub fn execute(&self, id: &str, input: ActionInput) -> Result<ActionResult, ActionError> {
        let action = self.action(id).ok_or_else(|| ActionError::Unknown(id.to_owned()))?;
        let against = |reasons: crate::action::Reasons| reasons.against().map(str::to_owned).collect();
        if !action.is_active() {
            return Err(ActionError::Inactive(against(action.active_reasons())));
        }
        if !action.is_enabled() {
            return Err(ActionError::Disabled(against(action.enabled_reasons())));
        }
        action.check_input(&input)?;
        let cx = self.context_for(input);
        if !action.run_guard(&cx) {
            return Err(ActionError::Canceled);
        }
        match action.kind() {
            ActionKind::Popup { view } => {
                self.0.popup.set(Some((action.id().to_owned(), view.clone())));
                return Ok(ActionResult::Popup(view.clone()));
            }
            ActionKind::Url(template) => {
                let address = fill_url(template, cx.selected.first());
                action.run(&cx).map_err(ActionError::Failed)?;
                action.run_notice(&cx);
                return Ok(ActionResult::OpenUrl(address));
            }
            _ => {}
        }
        self.finish(&action, &cx)
    }

    fn finish(&self, action: &Action, cx: &ActionContext) -> Result<ActionResult, ActionError> {
        if !action.contexts().is_empty() {
            let validator = self.0.validator.borrow().clone();
            if let Some(validator) = validator {
                validator(action.contexts(), cx).map_err(ActionError::Failed)?;
            }
        }
        action.run(cx).map_err(ActionError::Failed)?;
        if let Some(work) = action.work() {
            let waker = self.0.job_waker.borrow().clone();
            let job = crate::Job::start(
                action.id(),
                action.caption(),
                work,
                cx.selected.clone(),
                cx.input.clone(),
                waker,
            );
            let id = job.id();
            action.set_enabled(crate::REASON_RUNNING, false);
            self.0.jobs.borrow_mut().push(job);
            return Ok(ActionResult::Started(id));
        }
        action.run_notice(cx);
        let next = cx.take_next().unwrap_or(ActionResult::Done);
        if let ActionResult::ShowView(id) = &next {
            self.show_view(id).map_err(|e| ActionError::Failed(e.0))?;
        }
        Ok(next)
    }

    /// Wakes whoever shows the frame, from any thread, when background work has something to show.
    pub fn set_job_waker(&self, waker: crate::JobWaker) {
        for job in self.0.jobs.borrow().iter() {
            job.set_waker(Some(waker.clone()));
        }
        *self.0.job_waker.borrow_mut() = Some(waker);
    }

    /// The background work that runs now, in the order it started.
    pub fn jobs(&self) -> Vec<crate::Job> {
        self.0.jobs.borrow().clone()
    }

    /// Keeps what background work handed back, tells what it asked to, and ends the work that
    /// ended: its action turns on again and says it ran. Says whether records changed, so what
    /// shows them reads again. Whoever shows the frame calls it once it is woken.
    pub fn apply_jobs(&self) -> bool {
        let jobs = self.jobs();
        let mut changed = false;
        for job in &jobs {
            changed |= self.keep_changes(job.take_changes());
            let Some(end) = job.take_end() else { continue };
            self.0.jobs.borrow_mut().retain(|running| running.id() != job.id());
            let action = self.action(job.action());
            if let Some(action) = &action {
                action.set_enabled(crate::REASON_RUNNING, true);
            }
            match end {
                Ok(()) => {
                    if let Some(action) = action {
                        action.run_notice(&self.context_for(ActionInput::None));
                    }
                }
                Err(error) => self.show_message(crate::Message::new(error, crate::MessageKind::Error)),
            }
        }
        changed
    }

    fn keep_changes(&self, changes: Vec<crate::JobChange>) -> bool {
        let space = self.space();
        let mut wrote = false;
        let mut failed = None;
        for change in changes {
            let done = match change {
                crate::JobChange::Save(record) => space.borrow_mut().save(record).map_err(|e| e.to_string()),
                crate::JobChange::Delete(record) => {
                    let entities = self.0.entities.borrow().clone();
                    let key = entities
                        .iter()
                        .find(|entity| entity.name == record.entity())
                        .and_then(|entity| entity.key_field())
                        .map(|field| record.get(&field.name));
                    match key {
                        Some(key) => {
                            space.borrow_mut().delete(record.entity(), &key).map_err(|e| e.to_string())
                        }
                        None => Err(format!("{} has no key to remove it by", record.entity())),
                    }
                }
                crate::JobChange::Tell(text, kind) => {
                    self.show_message(crate::Message::new(text, kind));
                    continue;
                }
            };
            match done {
                Ok(()) => wrote = true,
                Err(error) => {
                    failed = Some(error);
                    break;
                }
            }
        }
        if let Some(error) = failed {
            space.borrow_mut().rollback();
            self.show_message(crate::Message::new(error, crate::MessageKind::Error));
            return false;
        }
        if wrote && let Err(error) = space.borrow_mut().commit() {
            space.borrow_mut().rollback();
            self.show_message(crate::Message::new(error.to_string(), crate::MessageKind::Error));
            return false;
        }
        wrote
    }

    /// Opens a popup on this view for this action, for when the view depends on what it is run
    /// on. OK runs the action again, with what was picked.
    pub fn open_popup_on(&self, action: &str, view: &str) {
        self.0.popup.set(Some((action.to_owned(), view.to_owned())));
    }

    /// Fills in the record the form of the open popup starts with.
    pub fn prepare_popup(&self, object: &mut Object) {
        let Some((id, _)) = self.0.popup.get() else { return };
        if let Some(action) = self.action(&id) {
            action.run_prepare(&self.context_for(ActionInput::None), object);
        }
    }

    /// Ends the open popup with OK, which runs its action.
    pub fn accept_popup(&self, input: ActionInput) -> Result<ActionResult, ActionError> {
        let (id, _) = self.0.popup.get().ok_or_else(|| ActionError::Unknown("popup".to_owned()))?;
        let action = self.action(&id).ok_or(ActionError::Unknown(id))?;
        let cx = self.context_for(input);
        let done = self.finish(&action, &cx);
        // A failure leaves it open, with the reason, to try again.
        if done.is_ok() {
            self.0.popup.set(None);
        }
        done
    }

    /// Ends the open popup with Cancel, and nothing runs but what cancels.
    pub fn cancel_popup(&self) {
        if let Some((id, _)) = self.0.popup.get()
            && let Some(action) = self.action(&id)
        {
            action.run_cancel(&self.context_for(ActionInput::None));
        }
        self.0.popup.set(None);
    }

    /// Ends it: turns off its controllers, the window's last.
    pub fn close(&self) {
        self.set_view(None);
        for slot in &self.0.views {
            self.turn(slot, false);
        }
        for slot in self.0.windows.iter().rev() {
            if slot.controller.borrow().contexts().contains(&self.0.context) {
                slot.controller.borrow_mut().deactivated(self);
            }
        }
    }
}
