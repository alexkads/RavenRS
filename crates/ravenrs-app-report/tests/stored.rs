//! Reports people create, keep and run, and panels, with permissions.
#![allow(clippy::expect_used, clippy::panic, clippy::field_reassign_with_default)]

use std::rc::Rc;

use ravenrs_app::{
    Access, ActionInput, Application, ApplicationBuilder, EntityInfo, FieldInfo, FieldKind, Identity,
    MemoryDatabase, ObjectSpace, Operation, PermissionPolicy, Query, Role, RoleMerge, SecuredObjectSpace,
    SecurityContext, SpaceMaker, UserDirectory, Value, security_entities,
};
use ravenrs_app_report::{
    ANALYTICS_DASHBOARD, ANALYTICS_TILE, AnalyticsModule, REPORT_DEFINITION, ReportWords,
    StoredReportsModule, analytics_entities, report_entity,
};
use ravenrs_app_ui::{AppShell, Session, SystemModule};
use ravenrs_core::{AccessAction, Role as AccessRole, Size};
use ravenrs_report::{DataSource, DataSourceKind, Parameter, ParameterKind, Report};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn order() -> EntityInfo {
    EntityInfo::new("Order")
        .caption("Orders")
        .navigation("Sales")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
        .field(FieldInfo::new("cost", FieldKind::Money))
}

fn database() -> MemoryDatabase {
    let mut entities = vec![order(), report_entity()];
    entities.extend(analytics_entities());
    entities.extend(security_entities());
    let database = MemoryDatabase::new(entities).expect("valid");
    let mut space = database.space();
    for (id, name, cost) in [(1.0, "Mesa", 10_000.0), (2.0, "Cadeira", 5_000.0)] {
        let mut record = space.new_object("Order").expect("known");
        record.set("id", id);
        record.set("name", name);
        record.set("cost", cost);
        space.save(record).expect("saved");
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
        space: impl Fn() -> S + 'static,
        setup: impl FnOnce(ApplicationBuilder) -> ApplicationBuilder,
        security: Option<Rc<SecurityContext>>,
    ) -> Self {
        let builder = Application::builder("Test")
            .module(SystemModule::new())
            .module(StoredReportsModule::new(ReportWords::default()))
            .module(AnalyticsModule::new(ReportWords::default()))
            .entity(order());
        let mut application = setup(builder).build().expect("valid");
        if let Some(security) = security {
            application.set_security(security);
        }
        let shell = AppShell::new(application, space);
        let session = shell.session();
        let mut ui = Ui::new(shell, Theme::raven_dark());
        ui.set_viewport(Size::new(1500.0, 1000.0), 1.0);
        let mut world = Self { ui, session };
        world.settle();
        world
    }

    fn plain(database: &MemoryDatabase) -> Self {
        let source = database.clone();
        Self::over(move || source.space(), |builder| builder, None)
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.ui.update();
        }
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

    fn press(&mut self, name: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let found =
            tree.nodes.iter().find(|n| n.role == AccessRole::Button && n.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no button {name} in {:?}", self.texts())).id;
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }

    fn run(&mut self, action: &str, record: Option<ravenrs_app::Object>) {
        let frame = self.session.frame();
        if let Some(record) = record {
            frame.set_selected(vec![record]);
        }
        let action = frame.action(action).unwrap_or_else(|| panic!("no action {action}"));
        self.session.run_action(&action, ActionInput::None);
        self.settle();
    }
}

fn add_report(database: &MemoryDatabase, name: &str, layout: Option<String>) -> ravenrs_app::Object {
    let mut space = database.space();
    let mut record = space.new_object(REPORT_DEFINITION).expect("known");
    record.set("name", name);
    record.set("target_entity", "Order");
    if let Some(layout) = layout {
        record.set("layout", layout);
    }
    let key = record.get("id");
    space.save(record).expect("saved");
    space.commit().expect("committed");
    space.find(REPORT_DEFINITION, &key).expect("reads").expect("there")
}

#[test]
fn a_person_creates_saves_and_reopens_a_report_without_compiling() {
    let database = database();
    let record = add_report(&database, "Pedidos", None);
    let mut world = World::plain(&database);
    world.session.open_view("ReportDefinition_ListView");
    world.settle();
    // The designer opens on a first layout of the entity, with its fields at hand.
    world.run("DesignReport", Some(record.clone()));
    assert!(world.says("cost"), "{:?}", world.texts());
    world.press("Save");
    assert!(world.says("Report saved"));
    let stored = database.space().find(REPORT_DEFINITION, &record.get("id")).expect("reads").expect("there");
    let Value::Text(layout) = stored.get("layout") else { panic!("a layout was kept") };
    let report = Report::from_json(&layout).expect("it reads back");
    assert!(!layout.contains("Mesa"), "the layout keeps no one's data");
    assert!(!report.data_sources.is_empty());
    world.press("Close");

    // Opened again, it shows the records as they are now.
    world.run("ShowReport", Some(stored));
    assert!(world.says("Mesa"), "{:?}", world.texts());
    assert!(world.says("PDF"));
    world.press("Close");
    assert!(!world.says("PDF"));
}

#[test]
fn a_layout_that_does_not_read_or_an_entity_that_is_gone_says_so() {
    let database = database();
    let broken = add_report(&database, "Quebrado", Some("{ not a report".to_owned()));
    let mut world = World::plain(&database);
    world.session.open_view("ReportDefinition_ListView");
    world.settle();
    world.run("ShowReport", Some(broken));
    assert!(world.says("cannot be read"), "{:?}", world.texts());

    let mut space = database.space();
    let mut gone = space.new_object(REPORT_DEFINITION).expect("known");
    gone.set("name", "Sumiu");
    gone.set("target_entity", "Nothing");
    space.save(gone.clone()).expect("saved");
    space.commit().expect("committed");
    world.run("ShowReport", Some(gone));
    assert!(world.says("does not exist"), "{:?}", world.texts());
}

#[test]
fn a_report_with_parameters_asks_for_them() {
    let database = database();
    let mut report = Report::default();
    report.parameters.push(Parameter {
        name: "Least".into(),
        label: Some("Least cost".into()),
        kind: ParameterKind::Number,
        default: None,
    });
    let record = add_report(&database, "Com parâmetro", Some(report.to_json()));
    let mut world = World::plain(&database);
    world.session.open_view("ReportDefinition_ListView");
    world.settle();
    world.run("ShowReport", Some(record));
    assert!(world.says("Least cost"));
}

#[test]
fn a_list_shows_in_its_report_and_says_when_there_is_none() {
    let database = database();
    let mut world = World::plain(&database);
    world.session.open_view("Order_ListView");
    world.settle();
    world.run("ShowInReport", None);
    assert!(world.says("no report"), "{:?}", world.texts());
    add_report(&database, "Um", None);
    world.run("ShowInReport", None);
    assert!(world.says("PDF"), "one report opens right away");
    world.press("Close");
    add_report(&database, "Dois", None);
    world.run("ShowInReport", None);
    assert!(world.says("Which report"), "with several it asks");
    world.press("Dois");
    assert!(world.says("PDF"));
}

/// Someone with this role, over the same records.
fn secured(database: &MemoryDatabase, role: Role) -> World {
    let mut entities = vec![order(), report_entity()];
    entities.extend(analytics_entities());
    entities.extend(security_entities());
    let raw = database.clone();
    let maker: SpaceMaker = Rc::new(move || Box::new(raw.space()) as Box<dyn ObjectSpace>);
    let directory = UserDirectory::new(maker, Vec::new()).expect("roles");
    let context = Rc::new(SecurityContext::new(
        Identity::new("bia", "bia", vec![role]),
        directory,
        entities,
        RoleMerge::AnyRole,
    ));
    let (source, spaces) = (database.clone(), context.clone());
    World::over(
        move || SecuredObjectSpace::new(Box::new(source.space()), spaces.clone()),
        |builder| builder,
        Some(context),
    )
}

#[test]
fn who_has_no_permission_does_not_see_a_report_and_cannot_change_one() {
    let database = database();
    add_report(&database, "Secreto", None);
    let shut = Role::new("r").policy(PermissionPolicy::ReadOnlyAll).type_permission(
        REPORT_DEFINITION,
        [Operation::Read],
        Access::Deny,
    );
    let mut world = secured(&database, shut);
    world.session.open_view("ReportDefinition_ListView");
    world.settle();
    assert!(!world.says("Secreto"));
    world.session.open_view("Order_ListView");
    world.settle();
    world.run("ShowInReport", None);
    assert!(world.says("no report"), "and it is not offered on a list");

    // Reading is not changing: the space refuses a save from the designer.
    let reader = Role::new("r").policy(PermissionPolicy::ReadOnlyAll);
    let mut world = secured(&database, reader);
    let record = database.space().query(REPORT_DEFINITION, &Query::all()).expect("reads").remove(0);
    world.session.open_view("ReportDefinition_ListView");
    world.settle();
    world.session.frame().set_selected(vec![record]);
    world.settle();
    let design = world.session.frame().action("DesignReport").expect("there");
    assert!(!design.is_enabled());
}

#[test]
fn a_report_only_shows_what_the_person_can_read_whatever_its_layout_says() {
    let database = database();
    // A layout that wants its data from an address, and carries some of its own.
    let mut report = Report::default();
    report.data_sources = vec![
        DataSource {
            name: "Elsewhere".into(),
            kind: DataSourceKind::Url { url: "https://example.com/all".into(), headers: Default::default() },
        },
        DataSource {
            name: "Kept".into(),
            kind: DataSourceKind::Json { data: serde_json::json!([{"name": "Vazou"}]) },
        },
    ];
    report.data_source = Some("Kept".into());
    report.detail.controls.push(ravenrs_report::Control::field("[name]").at(0.0, 0.0, 200.0, 18.0));
    report.detail.height = 20.0;
    let record = add_report(&database, "Tudo", Some(report.to_json()));
    let role = Role::new("r").policy(PermissionPolicy::ReadOnlyAll).object_permission(
        "Order",
        "[name] = 'Cadeira'",
        [Operation::Read],
        Access::Deny,
    );
    let mut world = secured(&database, role);
    world.session.open_view("ReportDefinition_ListView");
    world.settle();
    world.run("ShowReport", Some(record));
    let said = world.texts().join(" | ");
    assert!(said.contains("Mesa"), "what they can read shows: {said}");
    assert!(!said.contains("Vazou"), "{said}");
    assert!(!said.contains("Cadeira"), "{said}");
}

fn add_panel(database: &MemoryDatabase) -> ravenrs_app::Object {
    let mut space = database.space();
    let mut panel = space.new_object(ANALYTICS_DASHBOARD).expect("known");
    panel.set("name", "Vendas");
    let key = panel.get("id");
    space.save(panel.clone()).expect("saved");
    for (at, title, kind, series, filter) in [
        (0.0, "Total", "number", "cost:Sum", ""),
        (1.0, "Quantos", "number", "", ""),
        (2.0, "Por pedido", "bar", "cost:Sum", ""),
        (3.0, "Filtro ruim", "number", "cost:Sum", "(("),
        (4.0, "Campo ruim", "bar", "nothing:Sum", ""),
    ] {
        let mut tile = space.new_object(ANALYTICS_TILE).expect("known");
        tile.set("dashboard_id", key.clone());
        tile.set("title", title);
        tile.set("position", at);
        tile.set("target_entity", "Order");
        tile.set("kind", kind);
        tile.set("series", series);
        tile.set("filter", filter);
        space.save(tile).expect("saved");
    }
    space.commit().expect("committed");
    panel
}

#[test]
fn a_panel_people_made_shows_numbers_and_charts_and_says_what_is_wrong_with_a_tile() {
    let database = database();
    let panel = add_panel(&database);
    let mut world = World::plain(&database);
    world.session.open_view("AnalyticsDashboard_ListView");
    world.settle();
    world.run("OpenPanel", Some(panel));
    let said = world.texts().join(" | ");
    assert!(said.contains("150"), "cents become units: {said}");
    assert!(said.contains("Mesa"), "the chart's points: {said}");
    assert!(said.contains("Filtro ruim"), "{said}");
    assert!(said.contains("nothing"), "{said}");
    world.press("Close");
}

#[test]
fn a_panel_only_counts_what_the_person_can_read() {
    let database = database();
    let panel = add_panel(&database);
    let role = Role::new("r").policy(PermissionPolicy::ReadOnlyAll).object_permission(
        "Order",
        "[name] = 'Mesa'",
        [Operation::Read],
        Access::Deny,
    );
    let mut world = secured(&database, role);
    world.session.open_view("AnalyticsDashboard_ListView");
    world.settle();
    world.run("OpenPanel", Some(panel));
    let said = world.texts().join(" | ");
    assert!(said.contains("50"), "{said}");
    assert!(!said.contains("150"), "{said}");
    assert!(!said.contains("Mesa"), "{said}");
}

#[test]
fn a_report_that_comes_with_the_application_is_added_once() {
    let database = database();
    let module =
        || StoredReportsModule::new(ReportWords::default()).predefined("Padrão", "Order", Report::default());
    let source = database.clone();
    let _first = World::over(move || source.space(), |builder| builder, None);
    let application = Application::builder("Test").module(module()).entity(order()).build().expect("valid");
    let mut space = database.space();
    application.run_initial_records(&mut space).expect("runs");
    application.run_initial_records(&mut space).expect("runs");
    let found = database.space().query(REPORT_DEFINITION, &Query::all()).expect("reads");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].get("target_entity"), Value::Text("Order".into()));
}
