//! Long text, numbers, masks and times, driven through the public `Ui` API.
#![allow(clippy::expect_used)]

use ravenrs_core::{Event, Key, Modifiers, Point, PointerButton, Role, Size};
use ravenrs_layout::Align;
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

fn key(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    ui.update();
}

fn typed(ui: &mut Ui, text: &str) {
    for c in text.chars() {
        key(ui, if c == ' ' { Key::Space } else { Key::Character(c.to_string()) });
    }
}

fn field_value(ui: &mut Ui) -> String {
    let tree = ui.access_tree();
    let node = tree.nodes.iter().find(|n| n.role == Role::TextInput).expect("a text input");
    node.value.clone().unwrap_or_default()
}

fn focus_field(ui: &mut Ui) {
    let spot = {
        let tree = ui.access_tree();
        tree.nodes.iter().find(|n| n.role == Role::TextInput).expect("a text input").bounds.center()
    };
    click(ui, spot);
}

#[test]
fn text_area_starts_new_lines_and_moves_between_them() {
    let text = State::new(String::new());
    let mut ui = ui(TextArea::new(text.binding()).width(400.0));
    focus_field(&mut ui);
    typed(&mut ui, "one");
    key(&mut ui, Key::Enter);
    typed(&mut ui, "two");
    assert_eq!(text.get(), "one\ntwo");
    key(&mut ui, Key::ArrowUp);
    typed(&mut ui, "X");
    assert_eq!(text.get(), "oneX\ntwo", "up keeps the column");
    key(&mut ui, Key::ArrowDown);
    key(&mut ui, Key::Home);
    typed(&mut ui, "Y");
    assert_eq!(text.get(), "oneX\nYtwo", "home goes to the start of the line");
    key(&mut ui, Key::End);
    typed(&mut ui, "Z");
    assert_eq!(text.get(), "oneX\nYtwoZ");
}

#[test]
fn text_area_keeps_line_breaks_when_pasting_and_reports_its_text() {
    let text = State::new(String::new());
    let mut ui = ui(TextArea::new(text.binding()).width(400.0));
    focus_field(&mut ui);
    key(&mut ui, Key::Character("a\r\nb".to_owned()));
    assert_eq!(text.get(), "a\nb");
    assert_eq!(field_value(&mut ui), "a\nb");
}

#[test]
fn text_area_read_only_and_disabled_do_not_change() {
    for (disabled, read_only) in [(true, false), (false, true)] {
        let text = State::new("kept".to_owned());
        let mut ui = ui(TextArea::new(text.binding()).width(400.0).disabled(disabled).read_only(read_only));
        focus_field(&mut ui);
        key(&mut ui, Key::Enter);
        typed(&mut ui, "x");
        key(&mut ui, Key::Backspace);
        assert_eq!(text.get(), "kept");
    }
}

#[test]
fn a_long_text_area_builds_only_what_is_in_view_and_follows_the_caret() {
    let text = State::new((0..40).map(|n| n.to_string()).collect::<Vec<_>>().join("\n"));
    let mut ui = ui(TextArea::new(text.binding()).width(400.0).rows(3));
    let glyph_runs = |ui: &Ui| {
        ui.scene().primitives().iter().filter(|p| matches!(p, ravenrs_core::Primitive::Glyphs(_))).count()
    };
    assert!(glyph_runs(&ui) <= 8, "40 lines, {} built", glyph_runs(&ui));
    focus_field(&mut ui);
    for _ in 0..45 {
        key(&mut ui, Key::ArrowUp);
    }
    key(&mut ui, Key::Home);
    typed(&mut ui, "!");
    assert!(text.get().starts_with("!0\n"), "up reaches the first line");
    for _ in 0..30 {
        key(&mut ui, Key::ArrowDown);
    }
    key(&mut ui, Key::Home);
    typed(&mut ui, "?");
    assert!(text.get().contains("\n?30\n"), "down reaches line 30");
    assert!(glyph_runs(&ui) <= 8, "still only what is in view");
}

#[test]
fn a_stretching_text_area_wraps_to_its_real_width() {
    // Too long for the first guess at the width, and short for the 800 points it gets.
    let line = "word ".repeat(16);
    let text = State::new(line.trim_end().to_owned());
    let mut ui = ui(VStack::new((TextArea::new(text.binding()),)).align(Align::Stretch));
    ui.update();
    focus_field(&mut ui);
    key(&mut ui, Key::Home);
    key(&mut ui, Key::ArrowDown);
    typed(&mut ui, "!");
    assert!(text.get().ends_with('!'), "one line, so down goes to its end: {}", text.get());
}

#[test]
fn number_field_keeps_what_is_typed_and_formats_without_focus() {
    let n = State::new(None::<f64>);
    let other = State::new(String::new());
    let mut ui = ui(VStack::new((
        Localized::new(Labels::pt_br(), NumberField::new(n.binding()).decimals(2).prefix("R$ ").width(300.0)),
        TextField::new(other.binding()).placeholder("other"),
    )));
    focus_field(&mut ui);
    typed(&mut ui, "1234,");
    assert_eq!(n.get(), Some(1234.0));
    assert_eq!(field_value(&mut ui), "1234,", "the comma is not lost while typing");
    typed(&mut ui, "5");
    assert_eq!(n.get(), Some(1234.5));
    key(&mut ui, Key::Tab);
    assert_eq!(field_value(&mut ui), "R$ 1.234,50", "formatted once it loses focus");
    key(&mut ui, Key::Tab);
}

#[test]
fn number_field_steps_stays_in_range_and_leaves_the_value_on_bad_text() {
    let n = State::new(Some(5.0));
    let mut ui = ui(NumberField::new(n.binding()).range(0.0, 6.0));
    focus_field(&mut ui);
    key(&mut ui, Key::ArrowUp);
    key(&mut ui, Key::ArrowUp);
    assert_eq!(n.get(), Some(6.0), "up stops at the top");
    key(&mut ui, Key::End);
    typed(&mut ui, "x");
    assert_eq!(n.get(), Some(6.0), "text that is not a number changes nothing");
    assert_eq!(field_value(&mut ui), "6x", "and stays there to fix");
    key(&mut ui, Key::Backspace);
    key(&mut ui, Key::Backspace);
    assert_eq!(n.get(), None, "empty means no value");
}

#[test]
fn a_value_changed_from_outside_replaces_the_draft() {
    let n = State::new(Some(1.0));
    let mut ui = ui(NumberField::new(n.binding()));
    focus_field(&mut ui);
    key(&mut ui, Key::End);
    typed(&mut ui, "2");
    assert_eq!(n.get(), Some(12.0));
    n.set(Some(99.0));
    ui.update();
    assert_eq!(field_value(&mut ui), "99");
}

#[test]
fn money_is_held_in_whole_cents() {
    let cents = State::new(Some(150_i64));
    let mut ui = ui(NumberField::money(cents.binding(), "R$ "));
    assert_eq!(field_value(&mut ui), "R$ 1.50");
    focus_field(&mut ui);
    key(&mut ui, Key::End);
    key(&mut ui, Key::Backspace);
    typed(&mut ui, "29");
    assert_eq!(cents.get(), Some(129), "no rounding error in 1.29");
}

fn field_description(ui: &mut Ui) -> Option<String> {
    let tree = ui.access_tree();
    tree.nodes.iter().find(|n| n.role == Role::TextInput).and_then(|n| n.description.clone())
}

#[test]
fn money_reads_as_the_framework_does_and_says_what_to_fix() {
    let cents = State::new(None::<i64>);
    let mut ui = ui(Localized::new(Labels::pt_br(), NumberField::money(cents.binding(), "R$ ")));
    focus_field(&mut ui);
    typed(&mut ui, "1.000");
    assert_eq!(cents.get(), Some(100_000), "a thousand, as in an action's parameter and a report's");
    assert_eq!(field_description(&mut ui), None);
    for _ in 0..5 {
        key(&mut ui, Key::Backspace);
    }
    typed(&mut ui, "1,2x");
    assert_eq!(cents.get(), Some(120), "the x leaves alone what 1,2 gave");
    assert_eq!(
        field_description(&mut ui).as_deref(),
        Some("\"1,2x\" não é um valor válido. Use, por exemplo, 1.234,56."),
    );
    for _ in 0..4 {
        key(&mut ui, Key::Backspace);
    }
    typed(&mut ui, "99999999999999999999");
    assert!(
        cents.get().is_some_and(|c| c > 0 && c < 1 << 50),
        "too large is refused, not cut: {:?}",
        cents.get()
    );
    assert_eq!(field_description(&mut ui).as_deref(), Some("O valor é grande demais."));
}

#[test]
fn masked_field_follows_the_pattern_while_typing() {
    let phone = State::new(String::new());
    let mut ui = ui(MaskedField::new(phone.binding(), "(00) 00000-0000"));
    focus_field(&mut ui);
    typed(&mut ui, "11a98765");
    assert_eq!(phone.get(), "1198765", "letters do not fit a digit slot");
    assert_eq!(field_value(&mut ui), "(11) 98765");
    typed(&mut ui, "432199");
    assert_eq!(phone.get(), "11987654321", "no more than the pattern holds");
    assert_eq!(field_value(&mut ui), "(11) 98765-4321");
    key(&mut ui, Key::Backspace);
    assert_eq!(phone.get(), "1198765432");
}

#[test]
fn masked_field_can_keep_the_pattern_in_the_value() {
    let cpf = State::new(String::new());
    let mut ui = ui(MaskedField::new(cpf.binding(), "000.000.000-00").keep_literals());
    focus_field(&mut ui);
    typed(&mut ui, "12345678901");
    assert_eq!(cpf.get(), "123.456.789-01");
}

#[test]
fn time_picker_takes_typed_digits_and_the_list() {
    let at = State::new(None::<Time>);
    let mut ui = ui(TimePicker::new(at.binding()).step_minutes(60));
    focus_field(&mut ui);
    typed(&mut ui, "09");
    assert_eq!(at.get(), None, "not a time yet");
    assert_eq!(field_value(&mut ui), "09");
    typed(&mut ui, "3");
    assert_eq!(field_value(&mut ui), "09:3");
    typed(&mut ui, "0");
    assert_eq!(at.get(), Time::new(9, 30));

    let spot = {
        let tree = ui.access_tree();
        tree.nodes.iter().find(|n| n.role == Role::Button).expect("the list's button").bounds.center()
    };
    click(&mut ui, spot);
    let option = {
        let tree = ui.access_tree();
        let options: Vec<_> = tree.nodes.iter().filter(|n| n.role == Role::ListBoxOption).collect();
        assert_eq!(options.len(), 24, "one per hour");
        options[14].bounds.center()
    };
    click(&mut ui, option);
    assert_eq!(at.get(), Time::new(14, 0));
    assert_eq!(field_value(&mut ui), "14:00");
}

#[test]
fn a_time_picker_offers_and_takes_only_the_times_inside_its_limits() {
    let at = State::new(None::<Time>);
    let mut picker =
        ui(TimePicker::new(at.binding()).step_minutes(60).range(Time::new(9, 0), Time::new(17, 0)));
    focus_field(&mut picker);
    typed(&mut picker, "0730");
    assert_eq!(at.get(), None, "07:30 is before the first time: not taken");
    assert_eq!(field_value(&mut picker), "07:30", "and it stays as typed");
    let spot = {
        let tree = picker.access_tree();
        tree.nodes.iter().find(|n| n.role == Role::Button).expect("the list's button").bounds.center()
    };
    click(&mut picker, spot);
    let names: Vec<String> = picker
        .access_tree()
        .nodes
        .iter()
        .filter(|n| n.role == Role::ListBoxOption)
        .filter_map(|n| n.name.clone())
        .collect();
    assert_eq!(names.first().map(String::as_str), Some("09:00"), "{names:?}");
    assert_eq!(names.last().map(String::as_str), Some("17:00"), "both limits included");
    assert_eq!(names.len(), 9);

    let late = State::new(Time::new(20, 0));
    let mut shown = ui(TimePicker::new(late.binding()).range(None, Time::new(17, 0)).label("Fim"));
    let said = shown
        .access_tree()
        .nodes
        .iter()
        .any(|n| n.description.as_deref() == Some("Outside the times allowed"));
    assert!(said, "a time the app set past the limit is marked");
    assert_eq!(late.get(), Time::new(20, 0), "and never corrected");
}

#[test]
fn a_date_time_picker_starts_the_times_of_its_first_day_at_the_limit() {
    let first = Date::new(2026, 11, 10).expect("a real date");
    let min = DateTime { date: first, time: Time::new(14, 0).expect("a real time") };
    let at = State::new(Some(DateTime { date: first, time: Time::new(15, 0).expect("a real time") }));
    let mut ui = ui(DateTimePicker::new(at.binding(), first).step_minutes(60).range(Some(min), None));
    let spot = {
        let tree = ui.access_tree();
        tree.nodes
            .iter()
            .filter(|n| n.role == Role::Button)
            .find(|n| n.name.as_deref() == Some(&*Labels::default().pick_time))
            .expect("the list's button")
            .bounds
            .center()
    };
    click(&mut ui, spot);
    let first_time =
        ui.access_tree().nodes.iter().find(|n| n.role == Role::ListBoxOption).and_then(|n| n.name.clone());
    assert_eq!(first_time.as_deref(), Some("14:00"), "on the first day, from the limit");
}

#[test]
fn date_time_picker_keeps_both_parts() {
    let month = Date::new(2026, 11, 1).expect("a real date");
    let at = State::new(Some(DateTime { date: month, time: Time::new(8, 15).expect("a real time") }));
    let mut ui = ui(DateTimePicker::new(at.binding(), month));
    assert_eq!(field_value(&mut ui), "08:15");
    focus_field(&mut ui);
    key(&mut ui, Key::End);
    key(&mut ui, Key::Backspace);
    key(&mut ui, Key::Backspace);
    typed(&mut ui, "45");
    assert_eq!(at.get().map(|v| (v.date, v.time)), Some((month, Time::new(8, 45).expect("a real time"))));
}

#[test]
fn a_text_field_undoes_and_redoes_with_the_keyboard() {
    let value = State::new(String::new());
    let mut ui = ui(TextField::new(value.binding()));
    focus_field(&mut ui);
    typed(&mut ui, "olá mundo");
    let ctrl = Modifiers { ctrl: true, ..Modifiers::default() };
    let press = |ui: &mut Ui, c: &str, modifiers: Modifiers| {
        ui.handle_event(&Event::KeyDown { key: Key::Character(c.into()), modifiers });
        ui.update();
    };
    press(&mut ui, "z", ctrl);
    assert_eq!(value.get(), "olá ");
    press(&mut ui, "z", ctrl);
    press(&mut ui, "z", ctrl);
    assert_eq!(value.get(), "");
    press(&mut ui, "y", ctrl);
    assert_eq!(value.get(), "olá");
    assert_eq!(field_value(&mut ui), "olá");
}

#[test]
fn a_field_recalls_what_was_sent_with_the_arrows() {
    let (value, sent) = (State::new(String::new()), State::new(Vec::<String>::new()));
    let mut ui = ui(TextField::new(value.binding()).recall(sent.clone()));
    focus_field(&mut ui);
    for line in ["one", "two", "two"] {
        typed(&mut ui, line);
        key(&mut ui, Key::Enter);
        value.set(String::new());
        ui.update();
    }
    assert_eq!(sent.get(), ["one", "two"], "twice in a row is kept once");
    key(&mut ui, Key::ArrowUp);
    assert_eq!(value.get(), "two");
    key(&mut ui, Key::ArrowUp);
    key(&mut ui, Key::ArrowUp);
    assert_eq!(value.get(), "one", "it stops at the first");
    key(&mut ui, Key::ArrowDown);
    assert_eq!(value.get(), "two");
    key(&mut ui, Key::ArrowDown);
    assert_eq!(value.get(), "", "past the last, empty again");
}

fn field_id(ui: &mut Ui) -> u64 {
    ui.access_tree().nodes.iter().find(|n| n.role == Role::TextInput).expect("a text input").id
}

#[test]
fn masked_field_takes_an_optional_slot_and_says_when_it_is_complete() {
    let phone = State::new(String::new());
    let done = State::new(false);
    let field = MaskedField::new(phone.binding(), "(00) 90000-0000")
        .optional_rule('9', |c| c.is_ascii_digit())
        .complete(done.binding());
    let mut ui = ui(field);
    focus_field(&mut ui);
    typed(&mut ui, "119876543");
    assert!(!done.get(), "not done while typing, and not wrong either");
    typed(&mut ui, "2");
    assert!(done.get(), "ten digits fill every slot that must be");
    assert_eq!(field_value(&mut ui), "(11) 9876-5432", "the optional slot stays out");
    typed(&mut ui, "1");
    assert_eq!(field_value(&mut ui), "(11) 98765-4321", "and takes the eleventh digit");
    key(&mut ui, Key::Backspace);
    key(&mut ui, Key::Backspace);
    assert_eq!(phone.get(), "119876543");
    assert!(!done.get(), "Backspace makes it incomplete again");
}

#[test]
fn masked_field_takes_pasted_text_with_or_without_the_pattern() {
    let phone = State::new(String::new());
    let mut ui =
        ui(MaskedField::new(phone.binding(), "(00) 90000-0000").optional_rule('9', |c| c.is_ascii_digit()));
    let id = field_id(&mut ui);
    assert!(ui.set_access_value(id, "(11) 98765-4321"));
    ui.update();
    assert_eq!(phone.get(), "11987654321", "the pattern's own characters are left out");
    assert!(ui.set_access_value(id, "11 9876 5432"));
    ui.update();
    assert_eq!(phone.get(), "1198765432", "plain digits with spaces give the same");
    assert_eq!(field_value(&mut ui), "(11) 9876-5432");
}

#[test]
fn masked_field_rules_take_unicode_and_follow_a_new_mask() {
    let code = State::new(String::new());
    let mask = State::new("v-v".to_owned());
    let (c, m) = (code.clone(), mask.clone());
    let mut ui =
        ui(view_fn(move || MaskedField::new(c.binding(), m.get()).rule('v', |ch| "aeiouáéíóú".contains(ch))));
    focus_field(&mut ui);
    typed(&mut ui, "xáé");
    assert_eq!(code.get(), "áé", "the rule takes accented vowels and nothing else");
    assert_eq!(field_value(&mut ui), "á-é");
    mask.set("v/v/v".to_owned());
    ui.update();
    assert_eq!(field_value(&mut ui), "á/é", "the same value follows the new mask");
}
