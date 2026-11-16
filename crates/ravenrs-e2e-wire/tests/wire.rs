//! The channel end to end: the protocol, the app's side in process, and a real child process.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use std::io::Cursor;
use std::process::Command as Process;
use std::time::Duration;

use ravenrs_e2e::E2e;
use ravenrs_e2e_wire::{Client, Command, Failure, Hello, Reply, codes, serve, serve_on};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{BuildCx, Button, NodeId, Text, TextField, VStack, View, ViewExt};

/// The variable that turns this test binary into the app under test.
const CHILD: &str = "RAVENRS_E2E_WIRE_CHILD";

/// A small form: a field, a button that saves and a line that says what was saved.
struct Form {
    name: State<String>,
    saved: State<String>,
}

impl View for Form {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (n, s) = (self.name.clone(), self.saved.clone());
        cx.child(&VStack::new((
            TextField::new(self.name.binding()).placeholder("Nome").test_id("nome"),
            Button::new("Salvar").on_click(move || s.set(format!("Salvo: {}", n.get()))).test_id("salvar"),
            Text::new(if self.saved.get().is_empty() { "Nada salvo".to_owned() } else { self.saved.get() })
                .test_id("resultado"),
        )))
    }
}

fn session() -> E2e {
    E2e::new(Form { name: State::new(String::new()), saved: State::new(String::new()) }, Theme::raven_light())
}

#[test]
fn commands_and_replies_read_back_as_written() {
    for command in [
        Command::Click { target: "#salvar".into() },
        Command::Type { target: "Nome".into(), text: "Ana".into() },
        Command::Check { target: "#resultado".into(), text: "Salvo: Ana".into() },
        Command::Mention { target: "#resultado".into(), text: "Ana".into() },
        Command::Snapshot,
        Command::Bye,
    ] {
        let line = serde_json::to_string(&command).unwrap();
        assert!(!line.contains('\n'), "one line per command: {line}");
        assert_eq!(serde_json::from_str::<Command>(&line).unwrap(), command);
    }
    assert_eq!(serde_json::to_string(&Command::Snapshot).unwrap(), r#"{"op":"snapshot"}"#);
    let failed = Reply::of(Err(Failure::of(codes::TIMEOUT, "slow")));
    let back: Reply = serde_json::from_str(&serde_json::to_string(&failed).unwrap()).unwrap();
    assert_eq!(back.into_result().unwrap_err().code, "E2E_TIMEOUT");
}

#[test]
fn a_command_runs_on_a_session_in_this_process_too() {
    let mut e2e = session();
    Command::Type { target: "#nome".into(), text: "Ana".into() }.apply(&mut e2e).unwrap();
    Command::Click { target: "Salvar".into() }.apply(&mut e2e).unwrap();
    Command::Check { target: "#resultado".into(), text: "Salvo: Ana".into() }.apply(&mut e2e).unwrap();
    let missing = Command::Click { target: "#apagar".into() }.apply(&mut e2e).unwrap_err();
    assert_eq!(missing.code, "E2E_NOT_FOUND");
    assert!(missing.screen.contains("salvar"), "the failure brings the screen: {}", missing.screen);
}

#[test]
fn the_app_says_hello_answers_in_order_and_leaves_on_bye() {
    let input = [
        r##"{"op":"type","target":"#nome","text":"Bia"}"##,
        "",
        r##"{"op":"click","target":"#salvar"}"##,
        r##"{"op":"check","target":"#resultado","text":"Salvo: Bia"}"##,
        "isto não é json",
        r##"{"op":"bye"}"##,
        r##"{"op":"click","target":"#salvar"}"##,
    ]
    .join("\n");
    let mut output = Vec::new();
    serve_on("Formulário", session(), Cursor::new(input), &mut output).unwrap();
    let lines: Vec<&str> = std::str::from_utf8(&output).unwrap().lines().collect();
    let hello: Hello = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(hello, Hello::of("Formulário"));
    let replies: Vec<Reply> = lines[1..].iter().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(replies.len(), 5, "nothing is answered after bye: {lines:?}");
    assert!(replies[..3].iter().all(|r| r.ok));
    assert_eq!(replies[3].failure.as_ref().unwrap().code, codes::WIRE);
    assert!(replies[4].ok, "bye is answered");
}

/// The app's side when this binary runs as the child: it serves the form and exits.
#[test]
fn child_app() {
    let Some(kind) = std::env::var_os(CHILD) else {
        return;
    };
    if kind == "stuck" {
        // Says hello, then never answers.
        println!("\n{}", serde_json::to_string(&Hello::of("Parado")).unwrap());
        std::thread::sleep(Duration::from_secs(60));
        std::process::exit(0);
    }
    // The test runner leaves "test child_app ... " on the line: the channel starts on a new one.
    println!();
    serve("Formulário", session()).unwrap();
    std::process::exit(0);
}

fn child_as(kind: &str) -> Process {
    let mut process = Process::new(std::env::current_exe().unwrap());
    process.args(["--exact", "child_app", "--nocapture", "--test-threads", "1"]).env(CHILD, kind);
    process
}

fn child() -> Process {
    child_as("form")
}

#[test]
fn a_tool_drives_the_app_in_a_child_process() {
    let mut app = Client::spawn(child(), Duration::from_secs(60)).unwrap();
    assert_eq!(app.app(), "Formulário");
    app.send(&Command::Type { target: "#nome".into(), text: "Caio".into() }).unwrap();
    app.send(&Command::Click { target: "#salvar".into() }).unwrap();
    app.send(&Command::Check { target: "#resultado".into(), text: "Salvo: Caio".into() }).unwrap();
    let screen = app.send(&Command::Snapshot).unwrap().unwrap();
    assert!(screen.contains("Salvo: Caio"), "{screen}");
    let wrong =
        app.send(&Command::Check { target: "#resultado".into(), text: "Salvo: Ana".into() }).unwrap_err();
    assert_eq!(wrong.code, "E2E_EXPECTATION");
    assert!(wrong.screen.contains("Salvo: Caio"), "the child's screen crossed the channel: {wrong:?}");
}

#[test]
fn an_app_that_never_says_hello_fails_to_start_and_is_ended() {
    // A child that runs no test and so says nothing on the channel, then exits.
    let mut quiet = Process::new(std::env::current_exe().unwrap());
    quiet.args(["--exact", "no_such_test"]);
    let failure = Client::spawn(quiet, Duration::from_secs(60)).unwrap_err();
    assert_eq!(failure.code, codes::APP_START, "{failure:?}");

    let missing = Client::spawn(Process::new("/no/such/app"), Duration::from_secs(1)).unwrap_err();
    assert_eq!(missing.code, codes::APP_START);
    assert!(missing.message.contains("could not start"), "{missing:?}");
}

#[test]
fn a_stuck_app_is_ended_after_its_time() {
    let mut app = Client::spawn(child_as("stuck"), Duration::from_secs(60))
        .unwrap()
        .step_time(Duration::from_millis(300));
    assert_eq!(app.app(), "Parado");
    let started = std::time::Instant::now();
    let late = app.send(&Command::Snapshot).unwrap_err();
    assert_eq!(late.code, codes::TIMEOUT, "{late:?}");
    assert!(started.elapsed() < Duration::from_secs(10), "it did not wait for the child's 60 s");
    // Ended: the next step finds no app.
    assert_eq!(app.send(&Command::Snapshot).unwrap_err().code, codes::APP_START);
}
