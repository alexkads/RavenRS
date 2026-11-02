//! `Chat` and `FloatingPanel`.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{AccessAction, Event, Key, Modifiers, Point, PointerButton, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn running(root: impl View + 'static) -> Ui {
    let mut ui = Ui::new(root, Theme::raven_light());
    ui.set_viewport(Size::new(900.0, 700.0), 1.0);
    ui.update();
    ui
}

fn says(ui: &mut Ui, wanted: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref().is_some_and(|name| name.starts_with(wanted)))
}

fn click(ui: &mut Ui, name: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some(name) && n.role == Role::Button)
        .unwrap_or_else(|| panic!("no button {name}"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
}

fn press(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    ui.update();
}

fn write(ui: &mut Ui, text: &str) {
    let tree = ui.access_tree();
    let field = tree.nodes.iter().find(|n| n.role == Role::TextInput).expect("the field").id;
    ui.perform_action(field, AccessAction::Focus);
    ui.update();
    for letter in text.chars() {
        press(ui, Key::Character(letter.to_string()));
    }
}

type Running = (Ui, State<Vec<ChatMessage>>, Rc<RefCell<Vec<String>>>);

fn chat() -> Running {
    let messages = State::new(Vec::<ChatMessage>::new());
    let sent = Rc::new(RefCell::new(Vec::new()));
    let (seen, answers) = (sent.clone(), messages.clone());
    let view =
        Chat::new(messages.binding()).intro("Ask me").examples(["How do I start?"]).on_send(move |text| {
            seen.borrow_mut().push(text);
            answers.update(|all| all.push(ChatMessage::assistant("An answer")));
        });
    (running(VStack::new((view,)).height(500.0)), messages, sent)
}

#[test]
fn it_offers_examples_until_the_first_message() {
    let (mut ui, _, sent) = chat();
    assert!(says(&mut ui, "Ask me"));
    click(&mut ui, "How do I start?");
    assert_eq!(*sent.borrow(), ["How do I start?"]);
    assert!(!says(&mut ui, "Ask me"), "the conversation takes its place");
    assert!(says(&mut ui, "An answer"));
}

#[test]
fn enter_sends_what_was_written_and_empties_the_field() {
    let (mut ui, messages, sent) = chat();
    write(&mut ui, " oi ");
    press(&mut ui, Key::Enter);
    assert_eq!(*sent.borrow(), ["oi"], "without the spaces around it");
    let all = messages.get();
    assert_eq!(all.len(), 2);
    assert_eq!((all[0].author(), all[0].text().as_str()), (ChatAuthor::Person, "oi"));
    assert_eq!((all[1].author(), all[1].text().as_str()), (ChatAuthor::Assistant, "An answer"));
    press(&mut ui, Key::Enter);
    assert_eq!(sent.borrow().len(), 1, "an empty field sends nothing");
}

#[test]
fn the_send_button_sends_too() {
    let (mut ui, _, sent) = chat();
    write(&mut ui, "oi");
    click(&mut ui, "Send");
    assert_eq!(*sent.borrow(), ["oi"]);
}

#[test]
fn a_message_shows_its_card() {
    let messages = State::new(vec![ChatMessage::assistant("Look").card(|| Box::new(Button::new("Open it")))]);
    let mut ui = running(VStack::new((Chat::new(messages.binding()),)).height(500.0));
    assert!(says(&mut ui, "Open it"));
}

#[test]
fn an_answer_is_announced() {
    let (mut ui, _, _) = chat();
    click(&mut ui, "How do I start?");
    assert!(ui.access_tree().nodes.iter().any(|n| n.role == Role::Status));
}

fn panel(open: &State<bool>) -> Ui {
    running(FloatingPanel::new(open.binding(), "Assistant", || Box::new(Text::new("Inside"))))
}

#[test]
fn the_panel_starts_closed_and_its_button_opens_and_closes_it() {
    let open = State::new(false);
    let mut ui = panel(&open);
    assert!(!says(&mut ui, "Inside"));
    click(&mut ui, "Assistant");
    assert!(open.get());
    assert!(says(&mut ui, "Inside"));
    click(&mut ui, "Close");
    assert!(!open.get());
    assert!(!says(&mut ui, "Inside"));
}

#[test]
fn escape_closes_the_panel() {
    let open = State::new(true);
    let mut ui = panel(&open);
    assert!(says(&mut ui, "Inside"));
    press(&mut ui, Key::Escape);
    assert!(!open.get());
}

#[test]
fn the_app_behind_the_panel_stays_usable() {
    let open = State::new(true);
    let clicks = State::new(0);
    let counted = clicks.clone();
    let view = VStack::new((
        Button::new("Behind").on_click(move || counted.update(|n| *n += 1)),
        FloatingPanel::new(open.binding(), "Assistant", || Box::new(Text::new("Inside"))),
    ));
    let mut ui = running(view);
    click(&mut ui, "Behind");
    assert_eq!(clicks.get(), 1);
    assert!(open.get());
}

/// Where the sphere is in each corner of the 900 by 700 window of these tests.
const BOTTOM_RIGHT: Point = Point::new(856.0, 656.0);
const TOP_LEFT: Point = Point::new(44.0, 44.0);

fn pointer(ui: &mut Ui, event: Event) {
    ui.handle_event(&event);
    ui.update();
}

fn drag(ui: &mut Ui, from: Point, to: Point) {
    pointer(ui, Event::PointerMove { position: from });
    pointer(ui, Event::PointerDown { position: from, button: PointerButton::Primary });
    let middle = Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
    pointer(ui, Event::PointerMove { position: middle });
    pointer(ui, Event::PointerMove { position: to });
    pointer(ui, Event::PointerUp { position: to, button: PointerButton::Primary });
}

fn click_at(ui: &mut Ui, at: Point) {
    pointer(ui, Event::PointerMove { position: at });
    pointer(ui, Event::PointerDown { position: at, button: PointerButton::Primary });
    pointer(ui, Event::PointerUp { position: at, button: PointerButton::Primary });
}

#[test]
fn the_sphere_opens_with_a_click_where_it_rests() {
    let open = State::new(false);
    let mut ui = panel(&open);
    click_at(&mut ui, BOTTOM_RIGHT);
    assert!(open.get());
}

#[test]
fn dragging_the_sphere_sends_it_to_the_nearest_corner_without_opening_it() {
    let open = State::new(false);
    let mut ui = panel(&open);
    drag(&mut ui, BOTTOM_RIGHT, Point::new(200.0, 150.0));
    assert!(!open.get(), "a drag is not a click");
    click_at(&mut ui, BOTTOM_RIGHT);
    assert!(!open.get(), "it left this corner");
    click_at(&mut ui, TOP_LEFT);
    assert!(open.get(), "and rests in this one");
}

#[test]
fn arrow_keys_send_the_focused_sphere_to_another_corner() {
    let open = State::new(false);
    let mut ui = panel(&open);
    let tree = ui.access_tree();
    let sphere = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Assistant")).expect("sphere").id;
    ui.perform_action(sphere, AccessAction::Focus);
    ui.update();
    press(&mut ui, Key::ArrowLeft);
    press(&mut ui, Key::ArrowUp);
    click_at(&mut ui, TOP_LEFT);
    assert!(open.get());
}

#[test]
fn the_panel_opens_toward_the_middle_from_any_corner() {
    let open = State::new(false);
    let mut ui = panel(&open);
    drag(&mut ui, BOTTOM_RIGHT, Point::new(200.0, 150.0));
    click_at(&mut ui, TOP_LEFT);
    assert!(says(&mut ui, "Inside"));
    click_at(&mut ui, TOP_LEFT);
    assert!(!open.get(), "the sphere stays where it was and closes it");
}

#[test]
fn the_sphere_asks_for_no_frames_until_the_pointer_is_on_it() {
    let open = State::new(false);
    let mut ui = panel(&open);
    ui.set_time(1.0);
    ui.update();
    assert_eq!(ui.next_wake(), None, "still, it sleeps");
    assert!(!ui.needs_redraw());
    pointer(&mut ui, Event::PointerMove { position: BOTTOM_RIGHT });
    ui.set_time(2.0);
    assert!(ui.needs_redraw(), "the pointer wakes it");
    pointer(&mut ui, Event::PointerMove { position: Point::new(100.0, 100.0) });
    for second in [3.0, 4.0] {
        ui.set_time(second);
        ui.update();
    }
    assert!(!ui.needs_redraw(), "and it sleeps again");
}

#[test]
fn an_answer_can_arrive_while_it_is_written() {
    let text = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let messages = State::new(vec![ChatMessage::assistant_writing(text.clone())]);
    let mut ui = running(VStack::new((Chat::new(messages.binding()),)).height(500.0));
    assert!(ui.access_tree().nodes.iter().any(|n| n.role == Role::ProgressIndicator), "on its way");
    text.lock().expect("text").push_str("Almost there");
    ui.ui_waker().wake();
    ui.update();
    assert!(says(&mut ui, "Almost there"));
}
