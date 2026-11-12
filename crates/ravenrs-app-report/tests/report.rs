//! A list becoming a report, driven the way people use it.
#![allow(clippy::expect_used)]

use ravenrs_app::{Application, EntityInfo, FieldInfo, FieldKind, MemoryDatabase, ObjectSpace};
use ravenrs_app_report::ReportModule;
use ravenrs_app_ui::{AppShell, SystemModule};
use ravenrs_core::{AccessAction, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn account() -> EntityInfo {
    EntityInfo::new("Account")
        .caption("Accounts")
        .navigation("Records")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Name"))
        .field(FieldInfo::new("opening", FieldKind::Money).caption("Opening"))
}

#[test]
fn the_report_action_shows_the_list_in_the_viewer() {
    let database = MemoryDatabase::new(vec![account()]).expect("valid");
    let mut space = database.space();
    let mut object = space.new_object("Account").expect("known");
    object.set("name", "Corrente");
    object.set("opening", 12_345.0);
    space.save(object).expect("saves");
    space.commit().expect("keeps");
    let application = Application::builder("Test")
        .module(SystemModule::new())
        .module(ReportModule::new("Report"))
        .entity(account())
        .build()
        .expect("valid");
    let source = database.clone();
    let shell = AppShell::new(application, move || source.space());
    let session = shell.session();
    let table = session.frame().view().and_then(|view| session.list_table(&view)).expect("a table");
    assert_eq!(table.title, "Accounts");
    assert_eq!(table.columns.len(), 2, "what the list shows, without its hidden key");
    assert_eq!(table.rows.len(), 1);

    let mut ui = Ui::new(shell, Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 900.0), 1.0);
    ui.update();
    ui.update();
    let tree = ui.access_tree();
    let button = tree.nodes.iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some("Report"));
    ui.perform_action(button.expect("the action").id, AccessAction::Click);
    ui.update();
    ui.update();
    let shows = ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Print"));
    assert!(shows, "the viewer opened, with its print button");
}

mod as_the_list_shows {
    //! The report has the rows the list shows: its search, filters and sorting, as the person can
    //! read them.
    use std::rc::Rc;

    use ravenrs_app::{
        Access, Application, DataAccess, EntityInfo, FieldInfo, FieldKind, Identity, MemoryDatabase,
        ObjectSpace, Operation, PermissionPolicy, Role as SecurityRole, RoleMerge, SecuredObjectSpace,
        SecurityContext, SpaceMaker, UserDirectory, security_entities,
    };
    use ravenrs_app_report::ReportModule;
    use ravenrs_app_ui::{AppShell, Session, SystemModule};
    use ravenrs_core::{AccessAction, Event, Key, Modifiers, Role, Size};
    use ravenrs_style::Theme;
    use ravenrs_widgets::Ui;
    use ravenrs_widgets::grid::Value as Cell;

    fn class(access: DataAccess) -> EntityInfo {
        EntityInfo::new("Class")
            .caption("Classes")
            .navigation("School")
            .data_access(access)
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text).caption("Name").default_property())
            .field(FieldInfo::new("period", FieldKind::Text).caption("Period"))
    }

    fn database(access: DataAccess) -> MemoryDatabase {
        let mut entities = vec![class(access)];
        entities.extend(security_entities());
        let database = MemoryDatabase::new(entities).expect("valid");
        let mut space = database.space();
        for (name, period) in [("Adults", "2025"), ("Youth", "2025"), ("Kids", "2026"), ("Teens", "2026")] {
            let mut object = space.new_object("Class").expect("known");
            object.set("name", name);
            object.set("period", period);
            space.save(object).expect("saves");
        }
        space.commit().expect("keeps");
        database
    }

    struct World {
        ui: Ui,
        session: Session,
    }

    impl World {
        fn over(shell: AppShell) -> Self {
            let session = shell.session();
            let mut ui = Ui::new(shell, Theme::raven_dark());
            ui.set_viewport(Size::new(1200.0, 900.0), 1.0);
            let mut world = Self { ui, session };
            world.settle();
            world
        }

        fn plain(access: DataAccess) -> Self {
            let application = application(access);
            let source = database(access);
            Self::over(AppShell::new(application, move || source.space()))
        }

        fn settle(&mut self) {
            for _ in 0..6 {
                self.ui.update();
            }
        }

        fn search(&mut self, text: &str) {
            self.settle();
            let tree = self.ui.access_tree();
            let field = tree
                .nodes
                .iter()
                .find(|n| n.role == Role::TextInput && n.name.as_deref() == Some("Search all columns"))
                .expect("the search")
                .id;
            self.ui.perform_action(field, AccessAction::Focus);
            for character in text.chars() {
                self.ui.handle_event(&Event::KeyDown {
                    key: Key::Character(character.to_string()),
                    modifiers: Modifiers::default(),
                });
            }
            // A remote list waits for typing to rest before it asks again.
            self.ui.set_time(1.0);
            self.settle();
        }

        fn sort_by(&mut self, column: &str) {
            self.settle();
            let tree = self.ui.access_tree();
            let header = tree
                .nodes
                .iter()
                .find(|n| {
                    n.role == Role::ColumnHeader && n.name.as_deref().is_some_and(|t| t.starts_with(column))
                })
                .unwrap_or_else(|| panic!("no column {column}"))
                .id;
            self.ui.perform_action(header, AccessAction::Click);
            self.settle();
        }

        /// The names in the report, in its order.
        fn report(&mut self) -> Vec<String> {
            self.settle();
            let view = self.session.frame().view().expect("a list");
            let table = self.session.list_table(&view).expect("a table");
            let name = table.columns.iter().position(|c| c.title == "Name").expect("the name column");
            table
                .rows
                .iter()
                .map(|row| match &row[name] {
                    Cell::Text(text) => text.clone(),
                    other => format!("{other:?}"),
                })
                .collect()
        }
    }

    fn application(access: DataAccess) -> Application {
        Application::builder("School")
            .module(SystemModule::new())
            .module(ReportModule::new("Report"))
            .entity(class(access))
            .build()
            .expect("valid")
    }

    #[test]
    fn with_nothing_asked_it_has_every_row() {
        let mut world = World::plain(DataAccess::Client);
        assert_eq!(world.report().len(), 4);
    }

    #[test]
    fn a_search_leaves_in_only_what_it_finds() {
        let mut world = World::plain(DataAccess::Client);
        world.search("2026");
        let mut found = world.report();
        found.sort();
        assert_eq!(found, ["Kids", "Teens"]);
    }

    #[test]
    fn it_keeps_the_order_the_list_was_sorted_in() {
        let mut world = World::plain(DataAccess::Client);
        world.sort_by("Name");
        assert_eq!(world.report(), ["Adults", "Kids", "Teens", "Youth"]);
        world.sort_by("Name");
        assert_eq!(world.report(), ["Youth", "Teens", "Kids", "Adults"]);
    }

    #[test]
    fn a_remote_list_brings_every_row_its_search_lets_through_in_its_order() {
        let mut world = World::plain(DataAccess::Server);
        world.search("2025");
        world.sort_by("Name");
        world.sort_by("Name");
        assert_eq!(world.report(), ["Youth", "Adults"]);
    }

    #[test]
    fn someone_who_reads_part_of_the_records_gets_only_that_part() {
        let entities = {
            let mut all = vec![class(DataAccess::Client)];
            all.extend(security_entities());
            all
        };
        let database = database(DataAccess::Client);
        let role = SecurityRole::new("Secretary")
            .policy(PermissionPolicy::DenyAll)
            .object_permission("Class", "[period] = '2025'", [Operation::Read], Access::Allow)
            .navigation_permission("School", Access::Allow);
        let raw = database.clone();
        let maker: SpaceMaker = Rc::new(move || Box::new(raw.space()) as Box<dyn ObjectSpace>);
        let directory = UserDirectory::new(maker, Vec::new()).expect("roles");
        let context = Rc::new(SecurityContext::new(
            Identity::new("bia", "bia", vec![role]),
            directory,
            entities,
            RoleMerge::AnyRole,
        ));
        let source = database.clone();
        let shell = AppShell::new(application(DataAccess::Client), move || {
            SecuredObjectSpace::new(Box::new(source.space()), context.clone())
        });
        let mut world = World::over(shell);
        let mut seen = world.report();
        seen.sort();
        assert_eq!(seen, ["Adults", "Youth"]);
        world.search("Kids");
        assert!(world.report().is_empty(), "a search does not reach what they cannot read");
    }

    #[test]
    fn the_report_button_shows_the_filtered_rows() {
        let mut world = World::plain(DataAccess::Client);
        world.search("Kids");
        let tree = world.ui.access_tree();
        let button =
            tree.nodes.iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some("Report"));
        world.ui.perform_action(button.expect("the action").id, AccessAction::Click);
        world.settle();
        let names: Vec<String> = world.ui.access_tree().nodes.iter().filter_map(|n| n.name.clone()).collect();
        assert!(names.iter().any(|n| n == "Print"), "the viewer opened");
        assert_eq!(world.report(), ["Kids"]);
    }
}
