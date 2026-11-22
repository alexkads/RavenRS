//! Imaging and chat in the testability contract (#741): a color and a crop are set by value, as
//! a screen reader does, and the messages of a conversation are found by their number.

use ravenrs_core::{Rect, Size};
use ravenrs_e2e::{E2e, ErrorCode};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{
    Chat, ChatMessage, ColorWheel, CropArea, CropOverlay, HueSaturation, VStack, View, ViewExt,
};

fn session(view: impl View + 'static) -> E2e {
    E2e::new(view, Theme::raven_dark()).viewport(Size::new(1000.0, 760.0), 1.0)
}

#[test]
fn a_color_wheel_takes_a_color_as_hex_and_says_it() {
    let color = State::new(HueSaturation::new(200.0, 0.2));
    let mut e2e = session(VStack::new((ColorWheel::new(color.binding()).test_id("cor"),)));
    e2e.by_test_id("cor").set_value("#ff0000").unwrap();
    let now = color.get();
    assert!(now.hue.abs() < 0.5 && (now.saturation - 1.0).abs() < 0.01, "{now:?}");
    let said = e2e.by_test_id("cor").node().unwrap().value.unwrap();
    assert!(said.contains("0°") && said.contains("100%"), "{said}");
    assert_eq!(e2e.by_test_id("cor").set_value("vermelho").unwrap_err().code(), ErrorCode::Expectation);
}

#[test]
fn a_disabled_color_wheel_refuses_the_value() {
    let color = State::new(HueSaturation::new(10.0, 0.5));
    let mut e2e = session(VStack::new((ColorWheel::new(color.binding()).disabled(true).test_id("cor"),)));
    assert_eq!(e2e.by_test_id("cor").set_value("#00ff00").unwrap_err().code(), ErrorCode::Disabled);
    assert_eq!(color.get(), HueSaturation::new(10.0, 0.5));
}

#[test]
fn the_driver_sets_the_crop_by_value_and_reads_it_back() {
    use ravenrs_widgets::testing::CropDriver;
    let area = State::new(CropArea { rect: Rect::new(0.0, 0.0, 400.0, 300.0), angle: 0.0 });
    let mut e2e =
        session(VStack::new((CropOverlay::new(area.binding()).test_id("corte"),)).width(400.0).height(300.0));
    let mut crop = CropDriver::new(&mut e2e, "corte");
    crop.set(Rect::new(10.0, 20.0, 200.0, 100.0), 5.0).unwrap();
    let said = crop.value().unwrap();
    assert_eq!(area.get().rect, Rect::new(10.0, 20.0, 200.0, 100.0));
    assert!((area.get().angle - 5.0).abs() < 0.01);
    assert!(said.contains("200 × 100"), "{said}");
}

#[test]
fn a_chat_is_busy_until_the_answer_and_its_messages_are_found_by_number() {
    let messages = State::new(vec![ChatMessage::assistant("Olá! Em que posso ajudar?")]);
    let waiting = State::new(false);
    let (m, w) = (messages.clone(), waiting.clone());
    let (sent, wait) = (messages.clone(), waiting.clone());
    let view = ravenrs_widgets::view_fn(move || {
        let (sent, wait) = (sent.clone(), wait.clone());
        Chat::new(m.binding()).busy(w.get()).on_send(move |_| wait.set(true)).test_id("chat")
    });
    let _ = sent;
    let mut e2e = session(view);
    assert_eq!(e2e.by_test_id("message:1").count().unwrap(), 1);
    // Sending makes it wait; the answer arrives later and the wait ends.
    waiting.set(true);
    e2e.advance(0.1);
    assert!(e2e.by_test_id("chat").node().unwrap().busy);
    messages.update(|all| all.push(ChatMessage::assistant("Pronto, está feito.")));
    waiting.set(false);
    e2e.advance(0.1);
    let answer = e2e.by_test_id("message:2").node().unwrap();
    assert!(!e2e.by_test_id("chat").node().unwrap().busy);
    assert!(e2e.snapshot().contains("Pronto, está feito."), "{answer:?}");
    let _ = wait;
}
