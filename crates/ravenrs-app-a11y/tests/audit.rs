//! The audit finds what keeps someone out, and stays quiet when nothing does.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::rc::Rc;

use ravenrs_app_a11y::{Audit, Problem};
use ravenrs_core::{Key, Role, Size};
use ravenrs_layout::LayoutStyle;
use ravenrs_reactive::Binding;
use ravenrs_style::Theme;
use ravenrs_widgets::{
    BuildCx, Button, FormField, Hotkey, NodeId, Paint, Semantics, TextField, Toolbar, ToolbarAction, Ui,
    VStack, View,
};

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui
}

/// A control that keeps Tab for itself, as a careless editor would.
struct Holds;

impl View for Holds {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let node = cx.container(
            LayoutStyle { width: Some(80.0), height: Some(30.0), ..LayoutStyle::leaf() },
            &[],
            Paint::None,
        );
        cx.focusable(node, 4.0);
        cx.on_key(node, Rc::new(|key, _| *key == Key::Tab));
        cx.semantics(node, Semantics::new(Role::TextInput).name("Holds Tab"));
        node
    }
}

#[test]
fn named_controls_that_let_tab_go_pass() {
    let view = VStack::new((
        Button::new("Salvar"),
        Button::new("Cancelar"),
        FormField::new("Nome", TextField::new(Binding::constant(String::new()))),
    ));
    Audit::new().screen(&mut ui(view), "form").assert_clean();
}

#[test]
fn a_control_with_no_name_is_found() {
    let view = VStack::new((Button::new("Salvar"), TextField::new(Binding::constant(String::new()))));
    let report = Audit::new().without_focus().screen(&mut ui(view), "form");
    assert!(
        report.findings.iter().any(|f| matches!(f.problem, Problem::Unnamed { role: Role::TextInput, .. })),
        "{report}"
    );
}

#[test]
fn a_control_that_keeps_tab_is_found() {
    let view = VStack::new((Button::new("Antes"), Holds, Button::new("Depois")));
    let report = Audit::new().screen(&mut ui(view), "form");
    assert!(
        report
            .findings
            .iter()
            .any(|f| matches!(f.problem, Problem::FocusStuck { .. } | Problem::FocusTrapped { .. })),
        "{report}"
    );
}

#[test]
fn two_commands_on_the_same_keys_are_found() {
    let save = || ToolbarAction::new("Salvar").hotkey(Hotkey::command("s")).on_click(|| {});
    let view = VStack::new((Toolbar::new().action(save()), Toolbar::new().action(save())));
    let report = Audit::new().without_focus().screen(&mut ui(view), "form");
    let keys = Hotkey::command("s").label();
    assert!(
        report.findings.iter().any(|f| f.problem == Problem::SharedShortcut { keys: keys.clone(), times: 2 }),
        "{report}"
    );
}

#[test]
fn a_command_with_keys_says_them() {
    let view = Toolbar::new().action(
        ToolbarAction::new("Salvar")
            .hotkey(Hotkey::command("s"))
            .accessible_description("Guarda o registro")
            .on_click(|| {}),
    );
    let tree = ui(view).access_tree();
    let save = tree.find(Role::Button, "Salvar").expect("the command");
    let said = format!("Guarda o registro. {}", Hotkey::command("s").label());
    assert_eq!(save.description.as_deref(), Some(said.as_str()));
}
