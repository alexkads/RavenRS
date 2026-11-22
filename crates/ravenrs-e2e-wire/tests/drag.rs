//! The `drag` block (#663): read in English or Portuguese, written back in English, and sent as
//! `Command::Drag`.

use ravenrs_e2e_wire::{Command, Kind, Scenario};

#[test]
fn drag_is_read_in_either_language_and_written_in_english() {
    let text = "scenario: Ligar\nopen App\narrastar #port:a:out = #port:b:in | Ligar a ao b\n";
    let scenario = Scenario::parse(text).unwrap();
    let block = &scenario.blocks[1];
    assert_eq!(block.kind, Kind::Drag);
    assert_eq!(
        block.command(),
        Some(Command::Drag { target: "#port:a:out".into(), to: "#port:b:in".into() })
    );
    let written = scenario.write();
    assert!(written.contains("drag #port:a:out = #port:b:in"), "{written}");
    assert_eq!(Scenario::parse(&written).unwrap(), scenario);
}

#[test]
fn drag_without_where_to_is_refused() {
    let error = Scenario::parse("scenario: x\nopen App\ndrag #a\n").unwrap_err();
    assert_eq!(error.line, 3);
}

#[test]
fn drag_goes_over_the_channel_as_json() {
    let command = Command::Drag { target: "#a".into(), to: "#b".into() };
    let json = serde_json::to_string(&command).unwrap();
    assert_eq!(json, r##"{"op":"drag","target":"#a","to":"#b"}"##);
    assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), command);
}
