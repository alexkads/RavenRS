//! What screen readers see: roles, names, states and actions of the
//! accessibility tree, and actions performed through it.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{AccessAction, AccessNode, AccessTree, Event, Key, Modifiers, Role, Size, Toggled};
use ravenrs_reactive::State;
use ravenrs_style::{TextStyle, Theme};
use ravenrs_widgets::grid::{GridColumn, GridData, Value};
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui
}

fn key(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    ui.update();
}

fn node<'a>(tree: &'a AccessTree, role: Role, name: &str) -> &'a AccessNode {
    tree.find(role, name).unwrap_or_else(|| panic!("no {role:?} named {name:?} in {:#?}", summary(tree)))
}

fn summary(tree: &AccessTree) -> Vec<String> {
    tree.nodes.iter().map(|n| format!("{:?} {:?}", n.role, n.name)).collect()
}

fn act(ui: &mut Ui, id: u64, action: AccessAction) {
    ui.perform_action(id, action);
    ui.update();
}

#[test]
fn buttons_are_named_by_their_label_and_can_be_clicked() {
    let clicks = State::new(0);
    let c = clicks.clone();
    let mut ui = ui(VStack::new((
        Button::new("Save").icon(IconName::Check).on_click(move || c.update(|n| *n += 1)),
        Button::new("Archive").disabled(true),
    )));
    let tree = ui.access_tree();
    assert_eq!(tree.root, tree.nodes[0].id);
    assert_eq!(tree.nodes[0].role, Role::Window);
    let save = node(&tree, Role::Button, "Save");
    assert!(save.children.is_empty(), "the label is the name, not a separate node");
    assert!(save.actions.contains(&AccessAction::Click) && save.focusable);
    assert!(node(&tree, Role::Button, "Archive").disabled, "disabled buttons stay in the tree");
    let id = save.id;
    act(&mut ui, id, AccessAction::Click);
    assert_eq!(clicks.get(), 1);
}

#[test]
fn icon_buttons_take_their_name_from_a_label_or_tooltip() {
    let ui_tree = ui(HStack::new((
        IconButton::new(IconName::Trash2).label("Delete"),
        Tooltip::new(IconButton::new(IconName::Pencil), "Edit"),
        IconButton::new(IconName::Copy),
    )))
    .access_tree();
    node(&ui_tree, Role::Button, "Delete");
    node(&ui_tree, Role::Button, "Edit");
    node(&ui_tree, Role::Button, "copy");
    assert!(
        ui_tree.nodes.iter().all(|n| n
            .name
            .as_deref()
            .is_none_or(|t| !t.chars().any(|c| ('\u{e000}'..='\u{f8ff}').contains(&c)))),
        "icon glyphs never leak into names"
    );
}

#[test]
fn check_boxes_and_switches_report_and_change_their_state() {
    let (terms, wifi) = (State::new(false), State::new(true));
    let mut ui = ui(VStack::new((
        Checkbox::new("Accept terms", terms.binding()),
        Toggle::new(wifi.binding()).label("Wi-Fi"),
    )));
    let tree = ui.access_tree();
    assert_eq!(node(&tree, Role::CheckBox, "Accept terms").toggled, Some(Toggled::False));
    assert_eq!(node(&tree, Role::Switch, "Wi-Fi").toggled, Some(Toggled::True));
    let id = node(&tree, Role::CheckBox, "Accept terms").id;
    act(&mut ui, id, AccessAction::Click);
    assert!(terms.get());
    assert_eq!(node(&ui.access_tree(), Role::CheckBox, "Accept terms").toggled, Some(Toggled::True));
}

#[test]
fn radio_groups_and_segments_expose_the_selection() {
    let (plan, density) = (State::new(1usize), State::new(0usize));
    let tree = ui(VStack::new((
        RadioGroup::new(plan.binding(), ["Free", "Team"]).label("Plan"),
        SegmentedControl::new(density.binding(), ["Compact", "Comfortable"]).label("Density"),
    )))
    .access_tree();
    let group = node(&tree, Role::RadioGroup, "Plan");
    assert_eq!(group.children.len(), 2);
    assert_eq!(node(&tree, Role::RadioButton, "Team").selected, Some(true));
    assert_eq!(node(&tree, Role::RadioButton, "Free").selected, Some(false));
    node(&tree, Role::RadioGroup, "Density");
    assert_eq!(node(&tree, Role::RadioButton, "Compact").toggled, Some(Toggled::True));
}

#[test]
fn text_fields_take_the_form_label_value_and_error() {
    let email = State::new("ana@".to_owned());
    let tree = ui(FormField::new("Email", TextField::new(email.binding()).placeholder("you@example.com"))
        .validate(email.binding(), validate::email))
    .access_tree();
    let field = node(&tree, Role::TextInput, "Email");
    assert_eq!(field.value.as_deref(), Some("ana@"));
    assert!(field.description.as_deref().is_some_and(|d| !d.is_empty()), "the validation error is read");
    let alone = ui(TextField::new(State::new(String::new()).binding()).placeholder("Search")).access_tree();
    node(&alone, Role::TextInput, "Search");
}

#[test]
fn passwords_never_expose_their_text() {
    let secret = State::new("hunter2".to_owned());
    let tree = ui(TextField::new(secret.binding()).secure().placeholder("Password")).access_tree();
    let field = node(&tree, Role::TextInput, "Password");
    assert_eq!(field.value.as_deref(), Some("•••••••"));
    assert!(tree.nodes.iter().all(|n| n.name.as_deref() != Some("hunter2")));
}

#[test]
fn sliders_and_steppers_are_ranges_that_step_through_actions() {
    let (volume, seats) = (State::new(50.0_f32), State::new(3));
    let mut ui = ui(VStack::new((
        Slider::new(volume.binding()).range(0.0, 100.0).step(10.0).label("Volume"),
        Stepper::new(seats.binding()).range(1, 5).label("Seats"),
    )));
    let tree = ui.access_tree();
    let slider = node(&tree, Role::Slider, "Volume");
    let range = slider.numeric.expect("numeric");
    assert_eq!((range.value, range.min, range.max, range.step), (50.0, 0.0, 100.0, Some(10.0)));
    let spin = node(&tree, Role::SpinButton, "Seats");
    assert_eq!(spin.numeric.map(|v| v.value), Some(3.0));
    node(&tree, Role::Button, "Increase");
    let (slider, spin) = (slider.id, spin.id);
    act(&mut ui, slider, AccessAction::Increment);
    act(&mut ui, spin, AccessAction::Decrement);
    assert_eq!((volume.get(), seats.get()), (60.0, 2));
}

#[test]
fn select_is_a_combo_box_whose_list_opens_with_the_current_option() {
    let choice = State::new(1usize);
    let mut ui = ui(Select::new(choice.binding(), ["Red", "Green", "Blue"]).label("Color"));
    let tree = ui.access_tree();
    let combo = node(&tree, Role::ComboBox, "Color");
    assert_eq!((combo.value.as_deref(), combo.expanded), (Some("Green"), Some(false)));
    let id = combo.id;
    act(&mut ui, id, AccessAction::Click);
    let tree = ui.access_tree();
    assert_eq!(node(&tree, Role::ComboBox, "Color").expanded, Some(true));
    let list = tree.nodes.iter().find(|n| n.role == Role::ListBox).expect("listbox");
    assert_eq!(list.children.len(), 3);
    let green = node(&tree, Role::ListBoxOption, "Green");
    assert_eq!(green.selected, Some(true));
    assert_eq!(tree.focus, Some(green.id), "focus lands on the current option");
    let blue = node(&tree, Role::ListBoxOption, "Blue").id;
    act(&mut ui, blue, AccessAction::Click);
    assert_eq!(choice.get(), 2);
}

#[test]
fn an_open_dialog_hides_the_page_behind_it() {
    let open = State::new(true);
    let tree = ui(VStack::new((
        Button::new("Behind"),
        Dialog::new(open.binding(), "Delete file?")
            .description("This cannot be undone.")
            .action(Button::new("Delete")),
    )))
    .access_tree();
    let dialog = node(&tree, Role::Dialog, "Delete file?");
    assert!(dialog.modal);
    assert_eq!(dialog.description.as_deref(), Some("This cannot be undone."));
    node(&tree, Role::Button, "Close");
    node(&tree, Role::Button, "Delete");
    assert!(tree.find(Role::Button, "Behind").is_none(), "the page behind is inert");
    let focused = tree.focused().expect("focus moved into the dialog");
    assert_eq!(focused.role, Role::Button);
}

#[test]
fn tabs_expose_the_list_the_selection_and_the_panel() {
    let tab = State::new(1usize);
    let mut ui =
        ui(Tabs::new(tab.binding()).tab("Overview", Text::new("one")).tab("Activity", Text::new("two")));
    let tree = ui.access_tree();
    let list = tree.nodes.iter().find(|n| n.role == Role::TabList).expect("tablist");
    assert_eq!(list.children.len(), 2);
    assert_eq!(node(&tree, Role::Tab, "Activity").selected, Some(true));
    let panel = node(&tree, Role::TabPanel, "Activity");
    assert_eq!(
        panel
            .children
            .iter()
            .filter_map(|c| tree.get(*c))
            .filter_map(|c| c.name.as_deref())
            .collect::<Vec<_>>(),
        ["two"]
    );
    let id = node(&tree, Role::Tab, "Overview").id;
    act(&mut ui, id, AccessAction::Click);
    assert_eq!(tab.get(), 0);
}

#[test]
fn headings_have_levels_and_decorations_are_hidden() {
    let tree = ui(VStack::new((
        Text::new("Settings").style(TextStyle::Title),
        Text::new("Plain"),
        Skeleton::line(12.0),
        Divider::horizontal(),
    )))
    .access_tree();
    assert_eq!(node(&tree, Role::Heading, "Settings").level, Some(2));
    node(&tree, Role::Label, "Plain");
    assert_eq!(tree.nodes.iter().filter(|n| n.role == Role::Separator).count(), 1);
    assert_eq!(tree.nodes.len(), 4, "window, heading, label and separator: {:#?}", summary(&tree));
}

#[test]
fn focus_follows_the_keyboard_and_ids_are_stable() {
    let mut ui = ui(VStack::new((Button::new("One"), Button::new("Two"))));
    let before = ui.access_tree();
    assert_eq!(before.focus, None);
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::Tab);
    let after = ui.access_tree();
    assert_eq!(after.focused().and_then(|n| n.name.as_deref()), Some("Two"));
    assert_eq!(node(&before, Role::Button, "One").id, node(&after, Role::Button, "One").id);
    let one = node(&after, Role::Button, "One").id;
    act(&mut ui, one, AccessAction::Focus);
    assert_eq!(ui.access_tree().focus, Some(one));
}

#[test]
fn data_grid_rows_cells_and_the_keyboard_row() {
    let data = GridData::new(
        (0..50).map(|i| vec![Value::from(format!("Customer {i}")), Value::from(i as f64)]).collect(),
    );
    let mut ui = ui(DataGrid::new(data, vec![GridColumn::text("Name"), GridColumn::number("Seats")])
        .toolbar(false)
        .height(400.0));
    let tree = ui.access_tree();
    let grid = tree.nodes.iter().find(|n| n.role == Role::Grid).expect("grid");
    assert!(grid.focusable);
    node(&tree, Role::ColumnHeader, "Name");
    node(&tree, Role::Cell, "Customer 0");
    let row = tree.nodes.iter().find(|n| n.role == Role::Row).expect("row");
    assert_eq!(row.children.iter().filter_map(|c| tree.get(*c)).filter(|c| c.role == Role::Cell).count(), 2);
    let grid = grid.id;
    act(&mut ui, grid, AccessAction::Focus);
    key(&mut ui, Key::ArrowDown);
    let tree = ui.access_tree();
    let active =
        tree.focused().and_then(|g| g.active_descendant).and_then(|id| tree.get(id)).expect("active row");
    assert_eq!(active.role, Role::Row);
    let first = active.children.first().and_then(|c| tree.get(*c)).and_then(|c| c.name.clone());
    assert!(first.is_some_and(|n| n.starts_with("Customer")), "the keyboard row is announced");
}

#[test]
fn toasts_are_status_messages() {
    let toasts = Toasts::new();
    let mut ui = ui(VStack::new((Toaster::new(toasts.clone()),)));
    toasts.show("Saved", "Your changes are live.", Tone::Success);
    ui.update();
    let tree = ui.access_tree();
    let status = tree.nodes.iter().find(|n| n.role == Role::Status).expect("status");
    let texts: Vec<_> =
        status.children.iter().filter_map(|c| tree.get(*c)).filter_map(|c| c.name.clone()).collect();
    assert!(texts.contains(&"Saved".to_owned()), "{texts:?}");
    node(&tree, Role::Button, "Dismiss");
}

#[test]
fn custom_components_describe_themselves() {
    struct Rating(State<u8>);
    impl View for Rating {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let stars = cx.child(&Text::new("★★★☆☆"));
            let node = cx.container(ravenrs_layout::LayoutStyle::leaf(), &[stars], Paint::None);
            cx.semantics(
                node,
                Semantics::new(Role::Slider).name("Rating").numeric(
                    f64::from(self.0.get()),
                    0.0,
                    5.0,
                    Some(1.0),
                ),
            );
            node
        }
    }
    let tree = ui(Rating(State::new(3))).access_tree();
    let rating = node(&tree, Role::Slider, "Rating");
    assert_eq!(rating.numeric.map(|v| v.value), Some(3.0));
}
