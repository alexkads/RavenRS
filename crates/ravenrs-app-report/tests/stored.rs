//! Reports people create, keep and run, and panels, with permissions.
#![allow(clippy::expect_used, clippy::panic, clippy::field_reassign_with_default)]

use std::rc::Rc;

use ravenrs_app::{
    Access, ActionInput, Application, ApplicationBuilder, EntityInfo, FieldInfo, FieldKind, Identity,
    MemoryDatabase, ObjectSpace, Operation, PermissionPolicy, Query, Role, RoleMerge, SecuredObjectSpace,
    SecurityContext, SpaceMaker, UserDirectory, Value, security_entities,
};
use ravenrs_app_report::{
    ANALYTICS_DASHBOARD, ANALYTICS_TILE, AnalyticsModule, AnalyticsPanel, IndicatorLevel, PanelChart,
    REPORT_DEFINITION, ReportWords, StoredReportsModule, Thresholds, analytics_entities, report_entity,
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
    secured_with(database, role, |builder| builder)
}

fn secured_with(
    database: &MemoryDatabase,
    role: Role,
    setup: impl FnOnce(ApplicationBuilder) -> ApplicationBuilder,
) -> World {
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
        setup,
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

fn sales_panel() -> AnalyticsPanel {
    AnalyticsPanel::new("Sales", "Painel de vendas")
        .navigation("Sales")
        .number_in("Order", "Total vendido", "cost:Sum")
        .number_in("Order", "Só a mesa", "cost:Sum")
        .filtered("[name] = 'Mesa'")
        .chart_in("Order", PanelChart::Bar, "Por pedido", "name", "cost:Sum")
}

#[test]
fn a_panel_declared_in_code_is_in_the_navigation_with_the_sums_of_the_records() {
    let database = database();
    let panel = sales_panel();
    let info = panel.info();
    let source = database.clone();
    let mut world = World::over(move || source.space(), move |builder| builder.dashboard(info), None);
    panel.register(&world.session);
    world.settle();
    assert!(world.says("Painel de vendas"), "in the navigation: {:?}", world.texts());
    world.session.open_view("Sales_DashboardView");
    let said = world.texts().join(" | ");
    assert!(said.contains("Total vendido") && said.contains("150"), "the sum of both orders: {said}");
    assert!(said.contains("Só a mesa") && said.contains("100"), "the filtered tile: {said}");
    assert!(said.contains("Por pedido") && said.contains("Cadeira"), "the chart's points: {said}");
}

#[test]
fn a_panel_declared_in_code_counts_only_what_the_person_can_read() {
    let database = database();
    let role = Role::new("r").policy(PermissionPolicy::ReadOnlyAll).object_permission(
        "Order",
        "[name] = 'Mesa'",
        [Operation::Read],
        Access::Deny,
    );
    let panel = sales_panel();
    let mut world = secured_with(&database, role, {
        let info = panel.info();
        move |builder| builder.dashboard(info)
    });
    panel.register(&world.session);
    world.session.open_view("Sales_DashboardView");
    let said = world.texts().join(" | ");
    assert!(said.contains("50"), "{said}");
    assert!(!said.contains("150") && !said.contains("Mesa"), "{said}");
}

#[test]
fn a_panel_names_one_control_per_tile_from_the_struct_of_the_record() {
    #[derive(ravenrs_app::BusinessObject)]
    struct Sale {
        #[field(key)]
        id: i64,
        item: String,
        amount: f64,
    }
    let _ = |sale: Sale| (sale.id, sale.item, sale.amount);
    let info = AnalyticsPanel::new("Shop", "Loja")
        .number::<Sale>("Total", "amount:Sum")
        .pie::<Sale>("Divisão", "item", "amount:Sum")
        .info();
    let captions: Vec<_> = info.items.iter().map(|item| (item.caption.as_str(), item.id.as_str())).collect();
    assert_eq!(captions, [("Total", "Shop.tile0"), ("Divisão", "Shop.tile1"), ("Loja", "Shop.print")]);
    assert_eq!(info.icon.as_deref(), Some("layout-dashboard"));
}

/// The records `order` holds sum 150 in whole units and count two.
fn levels_panel() -> AnalyticsPanel {
    AnalyticsPanel::new("Levels", "Gestão")
        .number_in("Order", "Total", "cost:Sum")
        .level(Thresholds::at_least(120.0, 60.0))
        .number_in("Order", "Média", "cost:Average")
        .level(Thresholds::at_least(100.0, 60.0))
        .number_in("Order", "Pedidos", "")
        .level(Thresholds::at_most(0.0, 0.0))
        .number_in("Order", "Só a cadeira", "cost:Sum")
        .filtered("[name] = 'Cadeira'")
        .level(Thresholds::at_least(75.0, 50.0))
}

fn open_levels(world: &mut World) -> String {
    levels_panel().register(&world.session);
    world.settle();
    world.session.open_view("Levels_DashboardView");
    world.texts().join(" | ")
}

#[test]
fn a_number_shows_its_level_by_the_thresholds() {
    assert_eq!(Thresholds::at_least(75.0, 50.0).level(80.0), Some(IndicatorLevel::Good));
    assert_eq!(Thresholds::at_least(75.0, 50.0).level(60.0), Some(IndicatorLevel::Attention));
    assert_eq!(Thresholds::at_least(75.0, 50.0).level(40.0), Some(IndicatorLevel::Critical));
    assert_eq!(Thresholds::at_most(0.0, 0.0).level(0.0), Some(IndicatorLevel::Good));
    assert_eq!(Thresholds::at_most(0.0, 0.0).level(1.0), Some(IndicatorLevel::Critical));
    assert_eq!(Thresholds::level_with(|_| None).level(1.0), None);
    assert_eq!(
        IndicatorLevel::worst([IndicatorLevel::Good, IndicatorLevel::Critical, IndicatorLevel::Attention]),
        Some(IndicatorLevel::Critical)
    );
    let database = database();
    let info = levels_panel().info();
    let source = database.clone();
    let mut world = World::over(move || source.space(), move |builder| builder.dashboard(info), None);
    let said = open_levels(&mut world);
    // 150 total, 75 on average, two orders, 50 for the chair.
    assert!(said.contains("On track"), "the total: {said}");
    assert!(said.contains("Attention"), "the average: {said}");
    assert!(said.contains("Critical"), "the count above zero, and the chair at 50: {said}");
    let tree = world.ui.access_tree();
    let badges = tree
        .nodes
        .iter()
        .filter(|node| matches!(node.name.as_deref(), Some("On track" | "Attention" | "Critical")))
        .count();
    assert_eq!(badges, 4, "{said}");
}

#[test]
fn a_number_its_reader_may_not_see_is_restricted_never_zero() {
    let database = database();
    let info = levels_panel().info();
    let no_cost = Role::new("Secretaria").policy(PermissionPolicy::ReadOnlyAll).member_permission(
        "Order",
        ["cost"],
        None,
        [Operation::Read],
        Access::Deny,
    );
    let mut world = secured_with(&database, no_cost, move |builder| builder.dashboard(info.clone()));
    let said = open_levels(&mut world);
    assert!(said.contains("Restricted to your role"), "{said}");
    assert!(
        !said.contains("Attention") && !said.contains("On track"),
        "a restricted tile has no level: {said}"
    );
    assert!(said.contains("Pedidos") && said.contains("Critical"), "the count needs no cost: {said}");

    let info = levels_panel().info();
    let nothing = Role::new("Visitante")
        .policy(PermissionPolicy::DenyAll)
        .navigation_permission("Levels_DashboardView", Access::Allow);
    let mut world = secured_with(&database, nothing, move |builder| builder.dashboard(info.clone()));
    let said = open_levels(&mut world);
    assert!(!said.contains(" 0 ") && !said.contains("R$ 0") && !said.contains("Critical"), "{said}");
}

/// Two areas: sales, which a role without `cost` may not read, and orders, which needs nothing.
fn grouped_panel() -> AnalyticsPanel {
    AnalyticsPanel::new("Areas", "Gestão à vista")
        .group("Vendas")
        .group_note("Em dia a partir de R$ 120,00 vendidos.")
        .group_view("Order_ListView", "Ver pedidos")
        .number_in("Order", "Total vendido", "cost:Sum")
        .level(Thresholds::at_least(120.0, 60.0))
        .number_with("Ticket médio", &["Order.cost"], |space| {
            let orders = space.query("Order", &Query::all()).map_err(|e| e.to_string())?;
            let total: f64 = orders.iter().filter_map(|order| order.number("cost")).sum();
            Ok(total / 100.0 / orders.len().max(1) as f64)
        })
        .level(Thresholds::at_least(100.0, 50.0))
        .group("Pedidos")
        .number_in("Order", "Pedidos abertos", "")
        .level(Thresholds::at_most(0.0, 0.0))
}

fn open_grouped(world: &mut World) -> String {
    grouped_panel().register(&world.session);
    world.settle();
    world.session.open_view("Areas_DashboardView");
    world.texts().join(" | ")
}

#[test]
fn a_group_shows_the_worst_level_its_rule_and_its_view() {
    let database = database();
    let info = grouped_panel().info();
    let captions: Vec<_> = info.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(captions, ["Areas.group0", "Areas.group1", "Areas.print"]);
    let source = database.clone();
    let mut world = World::over(move || source.space(), move |builder| builder.dashboard(info), None);
    let said = open_grouped(&mut world);
    // 150 sold is on track, 75 on average needs attention: the group is at attention.
    assert!(
        said.contains("Vendas") && said.contains("Total vendido") && said.contains("Ticket médio"),
        "{said}"
    );
    assert!(said.contains("Em dia a partir de R$ 120,00 vendidos."), "{said}");
    assert!(said.contains("75"), "the computed number: {said}");
    let tree = world.ui.access_tree();
    let named = |name: &str| tree.nodes.iter().filter(|node| node.name.as_deref() == Some(name)).count();
    assert_eq!(named("Attention"), 2, "the average and its group: {said}");
    assert_eq!(named("Critical"), 2, "two open orders and their group: {said}");
    world.press("Ver pedidos");
    assert!(world.says("Mesa") && world.says("Cadeira"), "the button opens the list: {:?}", world.texts());
}

#[test]
fn a_group_all_restricted_says_only_that_and_the_print_follows_the_screen() {
    let database = database();
    let info = grouped_panel().info();
    let no_cost = Role::new("Secretaria").policy(PermissionPolicy::ReadOnlyAll).member_permission(
        "Order",
        ["cost"],
        None,
        [Operation::Read],
        Access::Deny,
    );
    let mut world = secured_with(&database, no_cost, move |builder| builder.dashboard(info.clone()));
    let said = open_grouped(&mut world);
    assert!(said.contains("Restricted to your role"), "{said}");
    assert!(!said.contains("Em dia a partir") && !said.contains("Ver pedidos"), "no rule nor button: {said}");
    assert!(said.contains("Pedidos abertos") && said.contains("Critical"), "the other group stays: {said}");

    let document = grouped_panel().document(&world.session);
    let text = format!("{document:?}");
    assert!(text.contains("Restricted to your role") && text.contains("Pedidos abertos"), "{text}");
    assert!(!text.contains("150"), "no value of a restricted number on paper");
    world.press("Print panel");
    let shown = world.texts().join(" ");
    assert!(shown.contains("Page 1 of 1") && shown.contains("Print"), "the viewer opens: {shown}");
    assert!(
        shown.contains("Vendas Total vendido Restricted to your role")
            && shown.contains("Pedidos abertos 2 Critical"),
        "the same reading as the screen: {shown}"
    );
}

#[test]
fn a_kept_tile_shows_the_level_its_record_declares() {
    let database = database();
    let mut space = database.space();
    let mut panel = space.new_object(ANALYTICS_DASHBOARD).expect("known");
    panel.set("name", "Metas");
    let key = panel.get("id");
    space.save(panel.clone()).expect("saved");
    for (at, title, series, good, attention, higher) in
        [(0.0, "Vendido", "cost:Sum", 120.0, 60.0, true), (1.0, "Pedidos", "", 0.0, 0.0, false)]
    {
        let mut tile = space.new_object(ANALYTICS_TILE).expect("known");
        tile.set("dashboard_id", key.clone());
        tile.set("title", title);
        tile.set("position", at);
        tile.set("target_entity", "Order");
        tile.set("kind", "number");
        tile.set("series", series);
        tile.set("good_from", good);
        tile.set("attention_from", attention);
        tile.set("higher_is_better", higher);
        space.save(tile).expect("saved");
    }
    space.commit().expect("committed");
    let mut world = World::plain(&database);
    world.session.open_view("AnalyticsDashboard_ListView");
    world.settle();
    world.run("OpenPanel", Some(panel));
    let said = world.texts().join(" | ");
    // 150 sold, from 120 up: on track; two orders, lower being better from zero: critical.
    assert!(said.contains("On track") && said.contains("Critical"), "{said}");
}
