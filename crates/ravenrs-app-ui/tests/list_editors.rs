//! A list shown as a chart, a month, a timeline and a tree, picked only by the model.
#![allow(clippy::expect_used, clippy::panic)]

use ravenrs_app::{
    Access, Application, ApplicationBuilder, EntityInfo, EnumOption, FieldInfo, FieldKind, Identity,
    MemoryDatabase, ObjectSpace, Operation, PermissionPolicy, Query, Role, RoleMerge, SecuredObjectSpace,
    SecurityContext, SpaceMaker, UserDirectory, Value, ViewVariant, security_entities,
};
use ravenrs_app_ui::{AppShell, Session, SystemModule};
use ravenrs_core::{AccessAction, Event, Point, PointerButton, Role as AccessRole, Size};
use ravenrs_criteria::DateTime;
use ravenrs_style::Theme;
use ravenrs_widgets::{Date, Ui};

fn person() -> EntityInfo {
    EntityInfo::new("Person")
        .caption("People")
        .navigation("Work")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
}

fn task() -> EntityInfo {
    EntityInfo::new("Task")
        .caption("Tasks")
        .navigation("Work")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("title", FieldKind::Text).default_property())
        .field(FieldInfo::new("owner", FieldKind::Reference("Person".to_owned())))
        .field(FieldInfo::new("parent", FieldKind::Reference("Task".to_owned())))
        .field(FieldInfo::new("starts", FieldKind::DateTime))
        .field(FieldInfo::new("ends", FieldKind::DateTime))
        .field(FieldInfo::new("cost", FieldKind::Money))
        .field(FieldInfo::new("done", FieldKind::Boolean).default_value(false))
        .field(FieldInfo::new(
            "state",
            FieldKind::Enum(vec![EnumOption::new("open", "Open"), EnumOption::new("late", "Late")]),
        ))
}

fn at(day: u8, hour: u32) -> Value {
    // In this month, which is what a calendar and a timeline open on.
    let today = Date::from_days(DateTime::now().days);
    let date = Date::new(today.year(), today.month(), day).expect("a day");
    Value::Date(DateTime { days: date.to_days(), seconds: hour * 3600 })
}

fn database(entities: Vec<EntityInfo>) -> MemoryDatabase {
    let database = MemoryDatabase::new(entities).expect("valid");
    let mut space = database.space();
    for (id, name) in [(1.0, "Ana"), (2.0, "Bia")] {
        let mut person = space.new_object("Person").expect("known");
        person.set("id", id);
        person.set("name", name);
        space.save(person).expect("saved");
    }
    for (id, title, owner, parent, day, cost) in [
        (10.0, "Build", Value::Number(1.0), Value::Null, 3, 10_000.0),
        (11.0, "Walls", Value::Number(1.0), Value::Number(10.0), 3, 2_500.0),
        (12.0, "Roof", Value::Number(2.0), Value::Number(10.0), 5, 5_000.0),
    ] {
        let mut task = space.new_object("Task").expect("known");
        task.set("id", id);
        task.set("title", title);
        task.set("owner", owner);
        task.set("parent", parent);
        task.set("starts", at(day, 9));
        task.set("ends", at(day, 12));
        task.set("cost", cost);
        space.save(task).expect("saved");
    }
    space.commit().expect("committed");
    database
}

struct World {
    ui: Ui,
    session: Session,
}

impl World {
    fn over<S: ObjectSpace + 'static>(
        entities: Vec<EntityInfo>,
        space: impl Fn() -> S + 'static,
        setup: impl FnOnce(ApplicationBuilder) -> ApplicationBuilder,
    ) -> Self {
        let builder = entities
            .into_iter()
            .fold(Application::builder("Test").module(SystemModule::new()), ApplicationBuilder::entity);
        let application = setup(builder).build().expect("valid");
        let shell = AppShell::new(application, space);
        let session = shell.session();
        let mut world = Self { ui: Ui::new(shell, Theme::raven_dark()), session };
        world.settle();
        world
    }

    fn with(task: EntityInfo) -> (Self, MemoryDatabase) {
        let database = database(vec![person(), task.clone()]);
        let source = database.clone();
        (Self::over(vec![person(), task], move || source.space(), |builder| builder), database)
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.ui.update();
        }
    }

    fn show(&mut self, editor: &str) {
        self.session.choose_list_editor("Task_ListView", editor).expect("a setting");
        self.session.open_view("Task_ListView");
        self.settle();
    }

    fn texts(&mut self) -> Vec<String> {
        self.settle();
        self.ui
            .access_tree()
            .nodes
            .iter()
            .flat_map(|node| [node.name.clone(), node.value.clone(), node.description.clone()])
            .flatten()
            .collect()
    }

    fn says(&mut self, text: &str) -> bool {
        self.texts().iter().any(|found| found.contains(text))
    }

    fn cells(&mut self) -> usize {
        self.ui.access_tree().nodes.iter().filter(|node| node.role == AccessRole::Cell).count()
    }

    fn click_on(&mut self, text: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let found =
            tree.nodes.iter().find(|node| node.name.as_deref().is_some_and(|name| name.contains(text)));
        let id = found.unwrap_or_else(|| panic!("nothing says {text} in {:?}", self.texts())).id;
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }
}

#[test]
fn a_chart_adds_up_by_what_it_is_told_and_money_reads_in_whole_units() {
    let entity = task().editor_setting("ChartSettings", "Argument", "owner").editor_setting(
        "ChartSettings",
        "Series",
        "cost:Sum",
    );
    let (mut world, _database) = World::with(entity);
    world.show("Chart");
    assert_eq!(world.cells(), 0, "no grid");
    let said = world.texts().join(" | ");
    // The chart reads out its points: two people, with what they add up to.
    assert!(said.contains("Ana"), "{said}");
    assert!(said.contains("Bia"), "{said}");
    assert!(said.contains("125"), "cents become units: {said}");
}

#[test]
fn a_chart_with_nothing_said_uses_the_kinds_of_the_fields_and_counts_if_there_is_no_number() {
    let (mut world, _database) = World::with(task());
    world.show("Chart");
    assert!(world.says("Build"), "the first field that is not a number goes along the axis");
    assert!(world.says("100"), "and the numbers shown are its series: {:?}", world.texts());

    let bare = EntityInfo::new("Task")
        .caption("Tasks")
        .navigation("Work")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("title", FieldKind::Text).default_property());
    let database = MemoryDatabase::new(vec![bare.clone()]).expect("valid");
    let mut space = database.space();
    let mut record = space.new_object("Task").expect("known");
    record.set("title", "One");
    space.save(record).expect("saved");
    space.commit().expect("committed");
    let source = database.clone();
    let mut world = World::over(vec![bare], move || source.space(), |builder| builder);
    world.show("Chart");
    assert!(world.says("One"), "{:?}", world.texts());
}

#[test]
fn a_setting_that_names_nothing_says_so_instead_of_showing_something_wrong() {
    let entity = task().editor_setting("ChartSettings", "Argument", "nothing");
    let (mut world, _database) = World::with(entity);
    world.show("Chart");
    assert!(world.says("ChartSettings/Argument: nothing"));

    let bare = EntityInfo::new("Task")
        .caption("Tasks")
        .navigation("Work")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("title", FieldKind::Text).default_property());
    let database = MemoryDatabase::new(vec![bare.clone()]).expect("valid");
    let source = database.clone();
    let mut world = World::over(vec![bare], move || source.space(), |builder| builder);
    world.show("Calendar");
    assert!(world.says("CalendarSettings/Start"));
    world.show("Scheduler");
    assert!(world.says("SchedulerSettings/Start"));
    world.show("Tree");
    assert!(world.says("TreeSettings/Parent"));
}

#[test]
fn a_tree_goes_by_the_reference_to_itself_and_opens_a_record() {
    let (mut world, _database) = World::with(task());
    world.show("Tree");
    assert_eq!(world.cells(), 0);
    for title in ["Build", "Walls", "Roof"] {
        assert!(world.says(title), "{title}: {:?}", world.texts());
    }
    let level_of = |world: &mut World, title: &str| {
        world.settle();
        let tree = world.ui.access_tree();
        tree.nodes
            .iter()
            .find(|node| node.name.as_deref() == Some(title))
            .map(|node| node.level)
            .unwrap_or_default()
    };
    assert!(level_of(&mut world, "Walls") > level_of(&mut world, "Build"));
}

#[test]
fn a_tree_shows_a_circle_and_a_parent_it_cannot_see() {
    let database = database(vec![person(), task()]);
    let mut space = database.space();
    for (id, parent) in [(11.0, 12.0), (12.0, 11.0), (10.0, 999.0)] {
        let mut record = space.find("Task", &Value::Number(id)).expect("reads").expect("there");
        record.set("parent", parent);
        space.save(record).expect("saved");
    }
    space.commit().expect("committed");
    let source = database.clone();
    let mut world = World::over(vec![person(), task()], move || source.space(), |builder| builder);
    world.show("Tree");
    for title in ["Build", "Walls", "Roof"] {
        assert!(world.says(title), "{title}");
    }
}

#[test]
fn a_month_shows_records_on_their_days_and_opens_them() {
    let (mut world, _database) = World::with(task());
    world.show("Calendar");
    assert_eq!(world.cells(), 0);
    assert!(world.says("Walls"));
    assert!(world.says("Roof"));
    world.click_on("Roof");
    let form = world.session.form_record().expect("the form opened");
    assert_eq!(form.get("id"), Value::Number(12.0));
}

#[test]
fn a_timeline_has_a_row_for_each_resource_and_one_for_what_points_nowhere() {
    let database = database(vec![person(), task()]);
    let mut space = database.space();
    let mut record = space.find("Task", &Value::Number(12.0)).expect("reads").expect("there");
    record.set("owner", Value::Null);
    space.save(record).expect("saved");
    space.commit().expect("committed");
    let source = database.clone();
    let mut world = World::over(vec![person(), task()], move || source.space(), |builder| builder);
    world.show("Scheduler");
    assert!(world.says("Ana"));
    assert!(world.says("Bia"), "a resource with nothing on it still has a row");
    assert!(world.says("No resource"));
}

impl World {
    /// Drags the bar of this record to the row of this person.
    fn drag_bar(&mut self, title: &str, row: &str, days: f32) {
        self.ui.set_viewport(Size::new(1600.0, 900.0), 1.0);
        self.settle();
        let tree = self.ui.access_tree();
        let bounds = |role: AccessRole, name: &str| {
            tree.nodes
                .iter()
                .find(|n| n.role == role && n.name.as_deref() == Some(name))
                .unwrap_or_else(|| panic!("no {role:?} {name}"))
                .bounds
        };
        let (bar, row) = (bounds(AccessRole::Button, title), bounds(AccessRole::Row, row));
        let from = bar.center();
        let to = Point::new(from.x + days * 40.0, row.center().y);
        let ui = &mut self.ui;
        ui.handle_event(&Event::PointerMove { position: from });
        ui.update();
        ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
        ui.update();
        ui.handle_event(&Event::PointerMove {
            position: Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0),
        });
        ui.update();
        ui.handle_event(&Event::PointerMove { position: to });
        ui.update();
        ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
        self.settle();
    }
}

#[test]
fn a_record_moved_on_the_timeline_is_kept_with_its_resource_and_times() {
    let (mut world, database) = World::with(task());
    world.show("Scheduler");
    let before = database.space().find("Task", &Value::Number(11.0)).expect("reads").expect("there");
    world.drag_bar("Walls", "Bia", 2.0);
    let after = database.space().find("Task", &Value::Number(11.0)).expect("reads").expect("there");
    assert_eq!(after.get("owner"), Value::Number(2.0), "the key keeps its kind");
    assert_ne!(after.get("starts"), before.get("starts"));
    assert_ne!(after.get("ends"), before.get("ends"));
    assert_eq!(after.get("title"), Value::Text("Walls".into()));
}

#[test]
fn a_move_the_rules_refuse_says_why_and_changes_nothing() {
    let entity = task().rule(ravenrs_app::Rule::criteria("only_ana", "[owner] = 1").message("Only Ana"));
    let (mut world, database) = World::with(entity);
    world.show("Scheduler");
    world.drag_bar("Walls", "Bia", 0.0);
    assert!(world.session.error().is_some_and(|error| error.contains("Only Ana")));
    let after = database.space().find("Task", &Value::Number(11.0)).expect("reads").expect("there");
    assert_eq!(after.get("owner"), Value::Number(1.0));
}

#[test]
fn someone_who_only_reads_cannot_move_anything() {
    let mut entities = vec![person(), task()];
    entities.extend(security_entities());
    let database = database(entities.clone());
    let raw = database.clone();
    let maker: SpaceMaker = std::rc::Rc::new(move || Box::new(raw.space()) as Box<dyn ObjectSpace>);
    let directory = UserDirectory::new(maker, Vec::new()).expect("roles");
    let role = Role::new("r").policy(PermissionPolicy::ReadOnlyAll);
    let context = std::rc::Rc::new(SecurityContext::new(
        Identity::new("bia", "bia", vec![role]),
        directory,
        entities,
        RoleMerge::AnyRole,
    ));
    let source = database.clone();
    let spaces = context.clone();
    let mut world = World::over(
        vec![person(), task()],
        move || SecuredObjectSpace::new(Box::new(source.space()), spaces.clone()),
        |builder| builder,
    );
    world.session.frame().set_security(context);
    world.show("Scheduler");
    world.drag_bar("Walls", "Bia", 2.0);
    let after = database.space().find("Task", &Value::Number(11.0)).expect("reads").expect("there");
    assert_eq!(after.get("owner"), Value::Number(1.0));
}

#[test]
fn a_way_to_see_the_list_picks_the_editor_and_going_back_brings_the_grid() {
    let entity = task()
        .variant(ViewVariant::new("grid", "Grid"))
        .variant(ViewVariant::new("tree", "Tree").editor("Tree"))
        .variant(ViewVariant::new("chart", "Chart").editor("Chart").filter("[cost] > 3000"));
    let (mut world, _database) = World::with(entity);
    world.session.open_view("Task_ListView");
    world.settle();
    assert!(world.cells() > 0, "the first way is the grid");
    let frame = world.session.frame();
    let choose = |world: &mut World, choice: &str| {
        let action = frame.action("ViewVariants").expect("there");
        world.session.run_action(&action, ravenrs_app::ActionInput::Choice(choice.to_owned()));
        world.settle();
    };
    choose(&mut world, "tree");
    assert_eq!(world.cells(), 0);
    assert!(world.says("Walls"));
    choose(&mut world, "chart");
    assert_eq!(world.cells(), 0);
    assert!(!world.says("Walls"), "its filter still counts");
    choose(&mut world, "grid");
    assert!(world.cells() > 0);
    choose(&mut world, "tree");
    assert!(world.says("Roof"), "and back again");
}

#[test]
fn what_someone_cannot_read_is_not_on_a_chart_or_a_tree() {
    let mut entities = vec![person(), task()];
    entities.extend(security_entities());
    let database = database(entities.clone());
    let raw = database.clone();
    let maker: SpaceMaker = std::rc::Rc::new(move || Box::new(raw.space()) as Box<dyn ObjectSpace>);
    let directory = UserDirectory::new(maker, Vec::new()).expect("roles");
    let role = Role::new("r")
        .policy(PermissionPolicy::ReadOnlyAll)
        .object_permission("Task", "[title] = 'Roof'", [Operation::Read], Access::Deny)
        .member_permission("Task", ["cost"], None, [Operation::Read], Access::Deny);
    let context = std::rc::Rc::new(SecurityContext::new(
        Identity::new("bia", "bia", vec![role]),
        directory,
        entities.clone(),
        RoleMerge::AnyRole,
    ));
    let source = database.clone();
    let spaces = context.clone();
    let mut world = World::over(
        vec![person(), task()],
        move || SecuredObjectSpace::new(Box::new(source.space()), spaces.clone()),
        |builder| builder,
    );
    world.show("Tree");
    assert!(world.says("Walls"));
    assert!(!world.says("Roof"));
    world.show("Scheduler");
    assert!(!world.says("Roof"));
    assert_eq!(database.space().query("Task", &Query::all()).expect("reads").len(), 3);
}
