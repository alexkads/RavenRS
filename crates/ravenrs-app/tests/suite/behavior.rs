//! Actions, controllers, frames and modules, with no window.
#![allow(clippy::expect_used)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs_app::*;

type Log = Rc<RefCell<Vec<String>>>;

fn say(log: &Log, text: impl Into<String>) {
    log.borrow_mut().push(text.into());
}

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

fn bill(key: f64, status: &str) -> Object {
    let values = BTreeMap::from([
        ("id".to_owned(), Value::Number(key)),
        ("status".to_owned(), Value::Text(status.to_owned())),
    ]);
    Object::from_values("Bill", values)
}

fn entities() -> Vec<EntityInfo> {
    vec![
        EntityInfo::new("Bill")
            .navigation("Finance")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("status", FieldKind::Text)),
        EntityInfo::new("Account")
            .navigation("Finance")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text)),
    ]
}

fn space() -> Rc<RefCell<dyn ObjectSpace>> {
    Rc::new(RefCell::new(MemoryDatabase::new(entities()).expect("valid").space()))
}

/// Says what happens to it, and brings the actions it is given.
struct Noisy {
    id: String,
    scope: TargetScope,
    actions: Vec<Action>,
    log: Log,
}

impl Noisy {
    fn boxed(id: &str, scope: TargetScope, actions: Vec<Action>, log: &Log) -> Box<dyn ViewController> {
        Box::new(Self { id: id.into(), scope, actions, log: log.clone() })
    }
}

impl ViewController for Noisy {
    fn id(&self) -> &str {
        &self.id
    }
    fn scope(&self) -> TargetScope {
        self.scope.clone()
    }
    fn actions(&self) -> Vec<Action> {
        self.actions.clone()
    }
    fn activated(&mut self, _frame: &Frame) {
        say(&self.log, format!("{} on", self.id));
    }
    fn deactivated(&mut self, _frame: &Frame) {
        say(&self.log, format!("{} off", self.id));
    }
    fn view_controls_created(&mut self, _frame: &Frame) {
        say(&self.log, format!("{} controls", self.id));
    }
}

fn frame_with(actions: Vec<Action>) -> Frame {
    let log = Log::default();
    Frame::new(
        FrameContext::Main,
        space(),
        None,
        vec![Noisy::boxed("c", TargetScope::any(), actions, &log)],
        Vec::new(),
    )
}

fn list(entity: &str) -> ViewInfo {
    ViewInfo::new(format!("{entity}_ListView"), ViewKind::List, Some(entity))
}

fn detail(entity: &str) -> ViewInfo {
    ViewInfo::new(format!("{entity}_DetailView"), ViewKind::Detail, Some(entity))
}

#[test]
fn changing_the_view_goes_in_order() {
    let log = Log::default();
    let frame = Frame::new(
        FrameContext::Main,
        space(),
        None,
        vec![
            Noisy::boxed("any", TargetScope::any(), vec![], &log),
            Noisy::boxed("bills", TargetScope::any().entity("Bill"), vec![], &log),
            Noisy::boxed("lists", TargetScope::any().view_kind(ViewKind::List), vec![], &log),
        ],
        Vec::new(),
    );
    let watching = log.clone();
    frame.on_event(move |event| say(&watching, format!("{event:?}")));
    assert!(taken(&log).is_empty(), "with no view, nothing is on");

    frame.set_view(Some(list("Bill")));
    assert_eq!(
        taken(&log),
        [
            "ViewChanging(Some(\"Bill_ListView\"))",
            "any on",
            "bills on",
            "lists on",
            "ViewChanged(Some(\"Bill_ListView\"))"
        ]
    );
    frame.view_controls_created();
    assert_eq!(taken(&log), ["any controls", "bills controls", "lists controls"]);

    frame.set_view(Some(detail("Account")));
    assert_eq!(
        taken(&log),
        [
            "ViewChanging(Some(\"Account_DetailView\"))",
            "any off",
            "bills off",
            "lists off",
            "any on",
            "ViewChanged(Some(\"Account_DetailView\"))"
        ],
        "everything turns off before the view changes, and only what fits comes back"
    );
    frame.view_controls_created();
    assert_eq!(taken(&log), ["any controls"], "only those that are on");
}

/// Asks for another view as soon as it turns on.
struct Redirect {
    log: Log,
    to: ViewInfo,
}

impl ViewController for Redirect {
    fn id(&self) -> &str {
        "redirect"
    }
    fn scope(&self) -> TargetScope {
        TargetScope::any().entity("Bill")
    }
    fn activated(&mut self, frame: &Frame) {
        say(&self.log, "redirect on");
        frame.set_view(Some(self.to.clone()));
        say(&self.log, "redirect asked");
    }
    fn deactivated(&mut self, _frame: &Frame) {
        say(&self.log, "redirect off");
    }
}

#[test]
fn a_controller_asking_for_another_view_waits_for_the_change_to_end() {
    let log = Log::default();
    let frame = Frame::new(
        FrameContext::Main,
        space(),
        None,
        vec![Box::new(Redirect { log: log.clone(), to: list("Account") })],
        Vec::new(),
    );
    frame.set_view(Some(list("Bill")));
    assert_eq!(taken(&log), ["redirect on", "redirect asked", "redirect off"]);
    assert_eq!(frame.view().map(|v| v.id).as_deref(), Some("Account_ListView"));
}

#[test]
fn each_part_of_an_actions_scope_decides_if_it_is_in_place() {
    let bills = Action::simple("bills", "x").target_entity("Bill");
    let by_id = Action::simple("byid", "x").target_view("Bill_ListView");
    let lists = Action::simple("lists", "x").target_view_kind(ViewKind::List);
    let roots = Action::simple("roots", "x").nesting(Nesting::Root);
    let nested = Action::simple("nested", "x").nesting(Nesting::Nested);
    let all = vec![bills.clone(), by_id.clone(), lists.clone(), roots.clone(), nested.clone()];
    let frame = frame_with(all);
    let state = || [&bills, &by_id, &lists, &roots, &nested].map(Action::is_active);
    assert_eq!(state(), [false; 5], "no view, and its controller is off");
    frame.set_view(Some(list("Bill")));
    assert_eq!(state(), [true, true, true, true, false]);
    frame.set_view(Some(detail("Bill")));
    assert_eq!(state(), [true, false, false, true, false]);
    frame.set_view(Some(list("Account")));
    assert_eq!(state(), [false, false, true, true, false]);

    let inner = Action::simple("inner", "x").nesting(Nesting::Nested);
    let root_only = Action::simple("root", "x").nesting(Nesting::Root);
    let log = Log::default();
    let nested_frame = Frame::new(
        FrameContext::Nested,
        space(),
        None,
        vec![Noisy::boxed("c", TargetScope::any(), vec![inner.clone(), root_only.clone()], &log)],
        Vec::new(),
    );
    nested_frame.set_view(Some(list("Bill")));
    assert!(inner.is_active());
    assert!(!root_only.is_active());
}

#[test]
fn a_controllers_scope_covers_all_its_actions() {
    let log = Log::default();
    let action = Action::simple("a", "x");
    let frame = Frame::new(
        FrameContext::Main,
        space(),
        None,
        vec![Noisy::boxed("bills", TargetScope::any().entity("Bill"), vec![action.clone()], &log)],
        Vec::new(),
    );
    frame.set_view(Some(list("Account")));
    assert!(!action.is_active());
    assert_eq!(action.active_reasons().get("controller"), Some(false));
    frame.set_view(Some(list("Bill")));
    assert!(action.is_active());
}

#[test]
fn a_controller_turned_off_by_a_reason_takes_its_actions_along() {
    let log = Log::default();
    let action = Action::simple("a", "x");
    let frame = Frame::new(
        FrameContext::Main,
        space(),
        None,
        vec![Noisy::boxed("c", TargetScope::any(), vec![action.clone()], &log)],
        Vec::new(),
    );
    frame.set_view(Some(list("Bill")));
    taken(&log);
    frame.set_controller_active("c", "license", false);
    assert!(!frame.is_controller_active("c"));
    assert!(!action.is_active());
    assert_eq!(taken(&log), ["c off"]);
    frame.set_view(Some(list("Account")));
    assert!(!action.is_active(), "the reason stays across views");
    assert!(taken(&log).is_empty());
    frame.set_controller_active("c", "license", true);
    assert!(action.is_active());
    assert_eq!(taken(&log), ["c on"]);
}

#[test]
fn selection_and_criteria_turn_actions_off_without_taking_them_away() {
    let pay = Action::simple("pay", "Pagar")
        .selection(Selection::RequireMultiple)
        .target_criteria("[status] = 'Open'", CriteriaMode::TrueForAll);
    let any = Action::simple("any", "x")
        .selection(Selection::RequireMultiple)
        .target_criteria("[status] = 'Open'", CriteriaMode::TrueAtLeastForOne);
    let one = Action::simple("one", "x").selection(Selection::RequireSingle);
    let frame = frame_with(vec![pay.clone(), any.clone(), one.clone()]);
    frame.set_view(Some(list("Bill")));
    let state = || [pay.is_enabled(), any.is_enabled(), one.is_enabled()];
    assert_eq!(state(), [false, false, false], "nothing selected");
    assert!(pay.is_active(), "off, but still in place");
    frame.set_selected(vec![bill(1.0, "Open")]);
    assert_eq!(state(), [true, true, true]);
    frame.set_selected(vec![bill(1.0, "Open"), bill(2.0, "Paid")]);
    assert_eq!(state(), [false, true, false]);
    assert_eq!(pay.enabled_reasons().get("criteria"), Some(false));
    frame.set_selected(vec![bill(1.0, "Paid")]);
    assert_eq!(state(), [false, false, true]);
    frame.set_view(Some(detail("Bill")));
    assert!(frame.selected().is_empty(), "a new view starts with nothing selected");
}

#[test]
fn reasons_from_a_controller_add_to_the_rest() {
    let action = Action::simple("a", "x");
    let frame = frame_with(vec![action.clone()]);
    frame.set_view(Some(list("Bill")));
    action.set_enabled("mine", false);
    frame.set_selected(vec![]);
    assert!(!action.is_enabled(), "updating the rest does not undo it");
    assert!(
        matches!(frame.execute("a", ActionInput::None), Err(ActionError::Disabled(why)) if why == ["mine"])
    );
    action.set_enabled("mine", true);
    assert_eq!(frame.execute("a", ActionInput::None), Ok(ActionResult::Done));
}

#[test]
fn an_action_that_is_off_does_not_run() {
    let runs = Rc::new(RefCell::new(0));
    let count = runs.clone();
    let action = Action::simple("a", "x")
        .target_entity("Bill")
        .selection(Selection::RequireSingle)
        .on_execute(move |_| {
            *count.borrow_mut() += 1;
            Ok(())
        });
    let frame = frame_with(vec![action]);
    frame.set_view(Some(list("Account")));
    assert!(matches!(frame.execute("a", ActionInput::None), Err(ActionError::Inactive(_))));
    frame.set_view(Some(list("Bill")));
    assert!(matches!(frame.execute("a", ActionInput::None), Err(ActionError::Disabled(_))));
    assert_eq!(*runs.borrow(), 0);
    frame.set_selected(vec![bill(1.0, "Open")]);
    assert_eq!(frame.execute("a", ActionInput::None), Ok(ActionResult::Done));
    assert_eq!(*runs.borrow(), 1);
    assert_eq!(frame.execute("nope", ActionInput::None), Err(ActionError::Unknown("nope".into())));
}

#[test]
fn an_action_runs_between_who_asks_and_who_is_told_and_can_fail() {
    let log = Log::default();
    let (a, b, c) = (log.clone(), log.clone(), log.clone());
    let allow = Rc::new(std::cell::Cell::new(false));
    let gate = allow.clone();
    let fail = Rc::new(std::cell::Cell::new(false));
    let broken = fail.clone();
    let action = Action::simple("a", "x")
        .on_executing(move |_| {
            say(&a, "asked");
            gate.get()
        })
        .on_execute(move |_| {
            say(&b, "ran");
            if broken.get() { Err("no money".to_owned()) } else { Ok(()) }
        })
        .on_executed(move |_| say(&c, "told"));
    let frame = frame_with(vec![action]);
    frame.set_view(Some(list("Bill")));
    assert_eq!(frame.execute("a", ActionInput::None), Err(ActionError::Canceled));
    assert_eq!(taken(&log), ["asked"]);
    allow.set(true);
    frame.execute("a", ActionInput::None).expect("runs");
    assert_eq!(taken(&log), ["asked", "ran", "told"]);
    fail.set(true);
    assert_eq!(frame.execute("a", ActionInput::None), Err(ActionError::Failed("no money".into())));
    assert_eq!(taken(&log), ["asked", "ran"], "no one is told of what failed");
}

#[test]
fn an_action_sees_the_selection_and_writes_through_the_frames_space() {
    let db = MemoryDatabase::new(entities()).expect("valid");
    let mut setup = db.space();
    let mut object = setup.new_object("Bill").expect("the entity");
    object.set("status", "Open");
    let key = object.get("id");
    setup.save(object.clone()).expect("saves");
    setup.commit().expect("commits");
    let action = Action::simple("pay", "Pagar").selection(Selection::RequireMultiple).on_execute(|cx| {
        let mut space = cx.space.borrow_mut();
        for selected in &cx.selected {
            let mut changed = selected.clone();
            changed.set("status", "Paid");
            space.save(changed).map_err(|e| e.to_string())?;
        }
        Ok(())
    });
    let log = Log::default();
    let own: Rc<RefCell<dyn ObjectSpace>> = Rc::new(RefCell::new(db.space()));
    let frame = Frame::new(
        FrameContext::Main,
        own.clone(),
        None,
        vec![Noisy::boxed("c", TargetScope::any(), vec![action], &log)],
        Vec::new(),
    );
    frame.set_view(Some(list("Bill")));
    frame.set_selected(vec![object]);
    frame.execute("pay", ActionInput::None).expect("runs");
    let read =
        |space: &dyn ObjectSpace| space.find("Bill", &key).expect("reads").expect("there").get("status");
    assert_eq!(read(&*own.borrow()), Value::Text("Paid".into()), "its own space sees it");
    assert_eq!(read(&db.space()), Value::Text("Open".into()), "nothing is kept until the commit");
}

#[test]
fn validation_contexts_stop_an_action_before_it_runs() {
    let log = Log::default();
    let ran = log.clone();
    let action = Action::simple("a", "x").validation_contexts(["Pay"]).on_execute(move |_| {
        say(&ran, "ran");
        Ok(())
    });
    let plain = Action::simple("b", "x");
    let frame = frame_with(vec![action, plain]);
    frame.set_view(Some(list("Bill")));
    frame.set_validator(|contexts, _| Err(format!("{} failed", contexts.join(","))));
    assert_eq!(frame.execute("a", ActionInput::None), Err(ActionError::Failed("Pay failed".into())));
    assert!(taken(&log).is_empty());
    assert_eq!(frame.execute("b", ActionInput::None), Ok(ActionResult::Done), "it asks nothing");
}

#[test]
fn a_popup_opens_first_and_runs_on_ok() {
    let log = Log::default();
    let (ran, canceled) = (log.clone(), log.clone());
    let action = Action::popup("pick", "Escolher", "Account_ListView")
        .on_execute(move |cx| {
            if matches!(cx.input, ActionInput::None) {
                return Err("pick something".to_owned());
            }
            say(&ran, "accepted");
            Ok(())
        })
        .on_cancel(move |_| say(&canceled, "canceled"));
    let frame = frame_with(vec![action]);
    frame.set_view(Some(list("Bill")));
    assert_eq!(frame.execute("pick", ActionInput::None), Ok(ActionResult::Popup("Account_ListView".into())));
    assert!(taken(&log).is_empty(), "nothing runs until OK");
    assert_eq!(frame.open_popup(), Some(("pick".into(), "Account_ListView".into())));
    assert!(frame.accept_popup(ActionInput::None).is_err());
    assert!(frame.open_popup().is_some(), "a failure leaves it open");
    frame.accept_popup(ActionInput::Choice("x".into())).expect("runs");
    assert_eq!(taken(&log), ["accepted"]);
    assert_eq!(frame.open_popup(), None);

    frame.execute("pick", ActionInput::None).expect("opens");
    frame.cancel_popup();
    assert_eq!(taken(&log), ["canceled"]);
    assert_eq!(frame.open_popup(), None);
    assert!(frame.accept_popup(ActionInput::None).is_err(), "nothing open");
}

#[test]
fn a_choice_and_a_parameter_reach_the_handler_or_it_does_not_run() {
    let log = Log::default();
    let (a, b) = (log.clone(), log.clone());
    let choice = Action::single_choice(
        "mode",
        "Modo",
        vec![
            ChoiceItem::new("day", "Dia"),
            ChoiceItem::new("more", "Mais").item(ChoiceItem::new("year", "Ano")),
        ],
    )
    .on_execute(move |cx| {
        say(&a, format!("{:?}", cx.input));
        Ok(())
    });
    let parameter = Action::parametrized("find", "Buscar", FieldKind::Text).on_execute(move |cx| {
        say(&b, format!("{:?}", cx.input));
        Ok(())
    });
    let frame = frame_with(vec![choice, parameter]);
    frame.set_view(Some(list("Bill")));
    assert!(matches!(frame.execute("mode", ActionInput::None), Err(ActionError::BadInput(_))));
    assert!(matches!(
        frame.execute("mode", ActionInput::Choice("week".into())),
        Err(ActionError::BadInput(_))
    ));
    frame.execute("mode", ActionInput::Choice("year".into())).expect("nested option");
    assert!(matches!(frame.execute("find", ActionInput::None), Err(ActionError::BadInput(_))));
    frame.execute("find", ActionInput::Parameter("abc".into())).expect("runs");
    assert_eq!(taken(&log), ["Choice(\"year\")", "Parameter(Text(\"abc\"))"]);
}

#[test]
fn a_url_comes_from_the_selected_record() {
    let action = Action::url("open", "Abrir", "https://x.dev/bills/{id}?s={status}");
    let frame = frame_with(vec![action]);
    frame.set_view(Some(list("Bill")));
    frame.set_selected(vec![bill(7.0, "Em aberto")]);
    assert_eq!(
        frame.execute("open", ActionInput::None),
        Ok(ActionResult::OpenUrl("https://x.dev/bills/7?s=Em%20aberto".into()))
    );
}

/// Stays on whatever the view is.
struct Shell {
    log: Log,
    action: Action,
}

impl WindowController for Shell {
    fn id(&self) -> &str {
        "shell"
    }
    fn actions(&self) -> Vec<Action> {
        vec![self.action.clone()]
    }
    fn activated(&mut self, _frame: &Frame) {
        say(&self.log, "shell on");
    }
    fn deactivated(&mut self, _frame: &Frame) {
        say(&self.log, "shell off");
    }
}

#[test]
fn a_window_controller_lasts_as_long_as_its_frame_and_fits_its_context() {
    let log = Log::default();
    let action = Action::simple("home", "Início");
    let make = |context| {
        Frame::new(
            context,
            space(),
            None,
            vec![Noisy::boxed("view", TargetScope::any(), vec![], &log)],
            vec![Box::new(Shell { log: log.clone(), action: action.clone() })],
        )
    };
    let popup = make(FrameContext::Popup);
    assert!(!action.is_active(), "only the main window");
    assert!(taken(&log).is_empty());
    drop(popup);

    let frame = make(FrameContext::Main);
    assert_eq!(taken(&log), ["shell on"]);
    assert!(action.is_active(), "before any view");
    frame.set_view(Some(list("Bill")));
    frame.set_view(Some(list("Account")));
    assert_eq!(taken(&log), ["view on", "view off", "view on"], "not bothered by views");
    frame.close();
    assert_eq!(taken(&log), ["view off", "shell off"]);
}

struct Part {
    name: &'static str,
    needs: Vec<String>,
    entities: Vec<EntityInfo>,
    actions: Vec<&'static str>,
    log: Log,
    difference: Option<&'static str>,
    criteria: Option<&'static str>,
}

impl Part {
    fn new(name: &'static str, log: &Log) -> Self {
        Self {
            name,
            needs: vec![],
            entities: vec![],
            actions: vec![],
            log: log.clone(),
            difference: None,
            criteria: None,
        }
    }
}

impl Module for Part {
    fn name(&self) -> &str {
        self.name
    }
    fn dependencies(&self) -> Vec<String> {
        self.needs.clone()
    }
    fn entities(&self) -> Vec<EntityInfo> {
        self.entities.clone()
    }
    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        let actions = self
            .actions
            .iter()
            .map(|id| {
                let action = Action::simple(*id, *id);
                match self.criteria {
                    Some(criteria) => action.target_criteria(criteria, CriteriaMode::TrueForAll),
                    None => action,
                }
            })
            .collect();
        vec![Noisy::boxed(self.name, TargetScope::any(), actions, &self.log)]
    }
    fn extend_model(&self, root: &mut ModelNode) {
        say(&self.log, format!("{} extends", self.name));
        root.children.push(ModelNode::new(self.name).with("From", "code"));
    }
    fn update_generated(&self, root: &mut ModelNode) {
        say(&self.log, format!("{} updates {}", self.name, root.children.len()));
    }
    fn model_difference(&self) -> Option<&'static str> {
        self.difference
    }
}

#[test]
fn modules_come_after_what_they_depend_on() {
    let log = Log::default();
    let mut base = Part::new("base", &log);
    base.entities = entities();
    let top = Part { needs: vec!["middle".into()], ..Part::new("top", &log) };
    let middle = Part { needs: vec!["base".into()], ..Part::new("middle", &log) };
    let app = Application::builder("App").module(top).module(middle).module(base).build().expect("valid");
    assert_eq!(app.title(), "App");
    assert_eq!(app.entities().len(), 2);
    assert_eq!(
        taken(&log),
        [
            "base extends",
            "middle extends",
            "top extends",
            "base updates 6",
            "middle updates 6",
            "top updates 6"
        ],
        "extending is over before anyone updates"
    );
    let frame = app.new_frame(FrameContext::Main, space());
    frame.show_view("Bill_ListView").expect("in the model");
    assert_eq!(taken(&log), ["base on", "middle on", "top on"]);
    assert_eq!(frame.view().map(|v| v.kind), Some(ViewKind::List));
    assert!(frame.show_view("Nope").is_err());
}

#[test]
fn what_is_wrong_with_modules_comes_out_when_building() {
    let log = Log::default();
    let build = |modules: Vec<Part>| {
        modules.into_iter().fold(Application::builder("A"), ApplicationBuilder::module).build().map(|_| ())
    };
    let needs = |name, other: &str| Part { needs: vec![other.to_owned()], ..Part::new(name, &log) };
    assert!(build(vec![needs("a", "gone")]).is_err_and(|e| e.0.contains("gone")));
    assert!(build(vec![needs("a", "b"), needs("b", "a")]).is_err_and(|e| e.0.contains("each other")));
    assert!(build(vec![Part::new("a", &log), Part::new("a", &log)]).is_err_and(|e| e.0.contains("twice")));
    let twice = |name| Part { actions: vec!["same"], ..Part::new(name, &log) };
    assert!(build(vec![twice("a"), twice("b")]).is_err_and(|e| e.0.contains("same")));
    let bad = Part { actions: vec!["x"], criteria: Some("[a] ="), ..Part::new("a", &log) };
    assert!(build(vec![bad]).is_err_and(|e| e.0.contains("the action x")));
    let json = Part { difference: Some("{"), ..Part::new("a", &log) };
    assert!(build(vec![json]).is_err_and(|e| e.0.contains("the module a")));
}

#[test]
fn a_modules_layer_stacks_and_a_persons_wins() {
    let log = Log::default();
    let module = Part {
        entities: entities(),
        difference: Some(r#"{ "NavigationItems": { "Finance": { "Caption": "Finanças" } } }"#),
        ..Part::new("m", &log)
    };
    let mut app = Application::builder("A").module(module).build().expect("valid");
    let caption = |app: &Application| {
        app.model().at(&["NavigationItems", "Finance"]).and_then(|n| n.text("Caption")).map(str::to_owned)
    };
    assert_eq!(caption(&app).as_deref(), Some("Finanças"));
    app.layers_mut().set(&ModelLayer::User, &["NavigationItems", "Finance"], "Caption", "Meu".into());
    app.refresh_model().expect("still valid");
    assert_eq!(caption(&app).as_deref(), Some("Meu"));
    app.layers_mut().set(&ModelLayer::User, &["Views", "Gone"], "Caption", "x".into());
    app.refresh_model().expect("still valid");
    assert_eq!(app.unusable().len(), 1, "kept, and said");
}

#[test]
fn a_change_that_breaks_the_model_is_refused_and_the_old_model_stays() {
    let log = Log::default();
    let mut app = Application::builder("A")
        .module(Part { entities: entities(), ..Part::new("m", &log) })
        .build()
        .expect("valid");
    let difference = Diff::parse(r#"{ "NavigationItems": { "Finance": { "Bill": { "View": "Gone" } } } }"#);
    app.layers_mut().put(&ModelLayer::User, difference.expect("valid"));
    assert!(app.refresh_model().is_err_and(|e| e.0.contains("Gone")));
    let item = app.model();
    assert_eq!(
        item.at(&["NavigationItems", "Finance", "Bill"]).and_then(|n| n.text("View")),
        Some("Bill_ListView")
    );
}
