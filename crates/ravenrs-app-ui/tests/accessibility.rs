//! What screen readers hear comes from the model, with nothing written for them: names, focus
//! order, and what changed.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ravenrs_app::*;
use ravenrs_app_a11y::Audit;
use ravenrs_app_ui::*;
use ravenrs_core::{AccessAction, AccessNode, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn period() -> EntityInfo {
    EntityInfo::new("Period")
        .caption("Periods")
        .navigation("Records")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("from", FieldKind::Date).caption("From").accessible_name("Start of the period"))
        .field(
            FieldInfo::new("until", FieldKind::Date)
                .caption("Until")
                .accessible_name("End of the period")
                .accessible_description("The last day that counts"),
        )
        .field(FieldInfo::new("name", FieldKind::Text).caption("Name").default_property())
        .field(
            FieldInfo::new("closed", FieldKind::Boolean).caption("Closed").accessible_name("Period closed"),
        )
        .focus_order(["name"])
        .rule(Rule::required("name"))
}

struct World {
    ui: Ui,
    session: Session,
}

fn world() -> World {
    let database = MemoryDatabase::new(vec![period()]).expect("valid");
    let application =
        Application::builder("Test").module(SystemModule::new()).entity(period()).build().expect("valid");
    let shell = AppShell::new(application, move || database.space());
    let session = shell.session();
    let mut ui = Ui::new(shell, Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    ui.update();
    ui.update();
    World { ui, session }
}

impl World {
    fn settle(&mut self) {
        for _ in 0..3 {
            self.ui.update();
        }
    }

    fn node(&mut self, role: Role, name: &str) -> AccessNode {
        self.settle();
        let tree = self.ui.access_tree();
        tree.find(role, name).cloned().unwrap_or_else(|| {
            let names: Vec<_> = tree.nodes.iter().filter_map(|n| n.name.clone()).collect();
            panic!("no {role:?} {name}: {names:?}")
        })
    }

    fn press(&mut self, name: &str) {
        let id = self.node(Role::Button, name).id;
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }

    fn heard(&mut self) -> Vec<(Role, String)> {
        self.settle();
        let tree = self.ui.access_tree();
        tree.nodes
            .iter()
            .filter(|n| matches!(n.role, Role::Status | Role::Alert))
            .filter_map(|n| n.name.clone().map(|name| (n.role, name)))
            .collect()
    }
}

#[test]
fn fields_are_called_what_the_model_says_and_the_caption_otherwise() {
    let mut w = world();
    w.press("New");
    let until = w.node(Role::ComboBox, "End of the period");
    assert_eq!(until.description.as_deref(), Some("The last day that counts"));
    w.node(Role::ComboBox, "Start of the period");
    w.node(Role::TextInput, "Name");
    w.node(Role::CheckBox, "Period closed");
}

#[test]
fn the_focus_order_leads_the_form_on_the_screen_and_for_tab() {
    let mut w = world();
    w.press("New");
    let name = w.node(Role::TextInput, "Name").bounds;
    let from = w.node(Role::ComboBox, "Start of the period").bounds;
    assert!(
        (name.y(), name.x()) < (from.y(), from.x()),
        "the field put first shows first: {name:?} before {from:?}"
    );
}

#[test]
fn saving_deleting_and_opening_a_screen_are_heard() {
    let mut w = world();
    w.session.open_view("Period_ListView");
    assert!(w.heard().iter().any(|(_, said)| said == "Showing Periods"), "{:?}", w.heard());

    w.press("New");
    w.press("Save");
    let heard = w.heard();
    assert!(
        heard.iter().any(|(role, said)| *role == Role::Alert && said.starts_with("Not saved.")),
        "what went wrong interrupts: {heard:?}"
    );

    w.session.set_form_value("name", Value::Text("March".into()));
    w.press("Save");
    assert!(w.heard().contains(&(Role::Status, "Saved: March".into())), "{:?}", w.heard());

    w.press("Delete");
    let tree = w.ui.access_tree();
    let answer = tree.nodes.iter().rfind(|n| n.role == Role::Button && n.name.as_deref() == Some("Delete"));
    let answer = answer.expect("the question").id;
    w.ui.perform_action(answer, AccessAction::Click);
    w.settle();
    assert!(w.heard().contains(&(Role::Status, "1 record deleted".into())), "{:?}", w.heard());
}

#[test]
fn an_application_can_be_quiet_and_say_things_of_its_own() {
    let mut w = world();
    w.session.announcer().polite("Imported 3 periods");
    assert!(w.heard().contains(&(Role::Status, "Imported 3 periods".into())));
    w.session.announcer().set_quiet(true);
    let before = w.session.announcer().latest();
    w.session.open_view("Period_ListView");
    w.settle();
    assert_eq!(w.session.announcer().latest(), before, "nothing new is said");
}

#[test]
fn the_generated_application_passes_the_audit() {
    let mut w = world();
    Audit::new().theme(Theme::raven_light()).open("New").app(&mut w.ui).assert_clean();
}
