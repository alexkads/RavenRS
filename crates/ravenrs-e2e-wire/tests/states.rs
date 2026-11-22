//! The expect block: the state of a part (checked, disabled, focused, visible, a count), as a
//! `.raventest` file writes it, in English or in Portuguese, and as a session checks it.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

use ravenrs_core::Size;
use ravenrs_e2e::E2e;
use ravenrs_e2e_wire::{Command, Kind, Scenario, State as Expected};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{BuildCx, Button, Checkbox, NodeId, Text, VStack, View, ViewExt};

/// A box that locks the button when checked, and three lines that say the same word.
struct Screen {
    locked: State<bool>,
}

impl View for Screen {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.child(&VStack::new((
            Checkbox::new("Travar", self.locked.binding()).test_id("travar"),
            Button::new("Enviar").disabled(self.locked.get()).test_id("enviar"),
            Text::new("Linha"),
            Text::new("Linha"),
            Text::new("Linha"),
        )))
    }
}

fn session() -> E2e {
    E2e::new(Screen { locked: State::new(false) }, Theme::raven_light())
        .viewport(Size::new(480.0, 320.0), 1.0)
}

const FILE: &str = "\
cenário: Travar o envio
abrir Tela | Abrir
estar #travar = desmarcado | Começa destravado
estar #enviar = habilitado | Dá para enviar
estar Linha = quantidade 3 | Três linhas
clicar #travar | Travar
estar #travar = marcado | Fica travado
estar #enviar = desabilitado | Não dá mais para enviar
";

#[test]
fn expect_reads_in_portuguese_and_writes_in_english() {
    let read = Scenario::parse(FILE).unwrap();
    assert_eq!(read.blocks[1].kind, Kind::Expect);
    let written = read.write();
    for line in
        ["expect #travar = desmarcado", "expect Linha = quantidade 3", "expect #enviar = desabilitado"]
    {
        assert!(written.contains(line), "{written}");
    }
    assert_eq!(Scenario::parse(&written).unwrap(), read);
    assert_eq!(Scenario::from_json(&read.to_json()).unwrap(), read);
    let words = ravenrs_e2e_wire::scenario_schema().to_string();
    assert!(words.contains("\"expect\""), "the schema lists the block");
}

#[test]
fn every_state_reads_in_both_languages() {
    for (english, portuguese, state) in [
        ("checked", "marcado", Expected::Checked),
        ("unchecked", "desmarcado", Expected::Unchecked),
        ("disabled", "desabilitado", Expected::Disabled),
        ("enabled", "habilitado", Expected::Enabled),
        ("focused", "com-foco", Expected::Focused),
        ("visible", "visivel", Expected::Visible),
        ("count 4", "quantidade 4", Expected::Count(4)),
    ] {
        assert_eq!(Expected::read(english), Ok(state));
        assert_eq!(Expected::read(&portuguese.to_uppercase()), Ok(state));
    }
}

#[test]
fn an_unknown_state_is_refused_when_the_scenario_is_read() {
    let wrong = Scenario::parse("scenario: x\nopen A\nexpect #a = azul\n").unwrap_err();
    assert_eq!(wrong.line, 3);
    assert!(wrong.message.contains("azul") && wrong.message.contains("checked"), "{wrong}");
    let count = Scenario::parse("scenario: x\nopen A\nexpect #a = count muitos\n").unwrap_err();
    assert!(count.message.contains("muitos"), "{count}");
    let bare = Scenario::parse("scenario: x\nopen A\nexpect #a\n").unwrap_err();
    assert!(bare.message.contains("precisa do texto"), "{bare}");
    let json = serde_json::json!({"scenario": "x", "blocks": [
        {"kind": "open", "target": "A"}, {"kind": "expect", "target": "#a", "text": "azul"}]});
    assert!(Scenario::from_json(&json).unwrap_err().message.contains("azul"));
}

#[test]
fn the_states_hold_as_the_screen_changes() {
    let mut e2e = session();
    Scenario::parse(FILE).unwrap().play(&mut e2e).unwrap();
}

#[test]
fn a_state_that_does_not_hold_fails_with_the_screen() {
    let mut e2e = session();
    let failed =
        Command::Expect { target: "#travar".into(), state: "checked".into() }.apply(&mut e2e).unwrap_err();
    assert_eq!(failed.code, "E2E_EXPECTATION", "{failed:?}");
    assert!(failed.screen.contains("travar"), "it shows the screen:\n{}", failed.screen);
    let many =
        Command::Expect { target: "Linha".into(), state: "count 2".into() }.apply(&mut e2e).unwrap_err();
    assert!(many.message.contains("3"), "{many:?}");
    let unknown =
        Command::Expect { target: "#travar".into(), state: "azul".into() }.apply(&mut e2e).unwrap_err();
    assert_eq!(unknown.code, "E2E_EXPECTATION");
    assert!(unknown.message.contains("checked"), "it says the states that exist: {unknown:?}");
}

#[test]
fn expect_goes_over_the_channel_as_one_line() {
    let command = Command::Expect { target: "#travar".into(), state: "checked".into() };
    let line = serde_json::to_string(&command).unwrap();
    assert_eq!(line, r##"{"op":"expect","target":"#travar","state":"checked"}"##);
    assert_eq!(serde_json::from_str::<Command>(&line).unwrap(), command);
}
