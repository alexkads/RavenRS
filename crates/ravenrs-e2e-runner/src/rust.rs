//! A scenario as a Rust integration test (T6 of #627), to read, or to keep in an app's
//! `tests/` when a team prefers Rust: it opens the app's binary with `--e2e-serve` and sends
//! one command of the test channel per block, as `cargo ravenrs e2e run` does.

use ravenrs_e2e_wire::{Command, Scenario};

use crate::session::Step;

/// `scenario` as a Rust test of the package whose binary is `binary`. The Open blocks open that
/// binary; a block that cannot run is refused with its title.
pub fn test(scenario: &Scenario, binary: &str, source: &str) -> Result<String, String> {
    let name = ident(&scenario.name);
    let mut steps = String::new();
    let mut opened = false;
    for block in &scenario.blocks {
        let step = Step::of(block).map_err(|why| format!("{}: {why}", block.title))?;
        let title = format!("{:?}", block.title);
        match step {
            Step::Open(app) => {
                // A second Open ends the app the first one started, as a run does.
                let bind = if opened { "app = " } else { "let mut app = " };
                opened = true;
                steps.push_str(&format!(
                    "    // {}\n    {bind}open({app:?});\n",
                    block.title.replace('\n', " ")
                ));
            }
            Step::Do(_) if !opened => {
                return Err(format!("{}: o cenário começa por um bloco abrir", block.title));
            }
            Step::Do(command) => {
                steps.push_str(&format!("    step(&mut app, {title}, &{});\n", command_code(&command)))
            }
        }
    }
    Ok(format!(
        r#"//! Gerado de {source} por `cargo ravenrs e2e rust`: só para leitura.
//! O cenário vale pelo arquivo .raventest; gere de novo depois de mudá-lo.
#![allow(clippy::expect_used, clippy::panic, reason = "a test fails loudly")]

use std::time::Duration;

use ravenrs_e2e_wire::{{Client, Command}};

/// Opens the app under test with no window, over the test channel.
fn open(_app: &str) -> Client {{
    let mut binary = std::process::Command::new(env!("CARGO_BIN_EXE_{binary}"));
    binary.arg(ravenrs_e2e_wire::SERVE_FLAG);
    Client::spawn(binary, Duration::from_secs(60)).expect("the app opens on the test channel")
}}

/// Runs one block, failing with its title, the stable code and the app's screen.
fn step(app: &mut Client, title: &str, command: &Command) {{
    if let Err(failure) = app.send(command) {{
        panic!("{{title}}: {{failure}}\n{{}}", failure.screen);
    }}
}}

#[test]
fn {name}() {{
{steps}}}
"#
    ))
}

/// The Rust that builds `command`.
fn command_code(command: &Command) -> String {
    let s = |text: &str| format!("{text:?}.into()");
    match command {
        Command::Click { target } => format!("Command::Click {{ target: {} }}", s(target)),
        Command::Type { target, text } => {
            format!("Command::Type {{ target: {}, text: {} }}", s(target), s(text))
        }
        Command::Check { target, text } => {
            format!("Command::Check {{ target: {}, text: {} }}", s(target), s(text))
        }
        Command::Mention { target, text } => {
            format!("Command::Mention {{ target: {}, text: {} }}", s(target), s(text))
        }
        Command::Fill { target, text } => {
            format!("Command::Fill {{ target: {}, text: {} }}", s(target), s(text))
        }
        Command::Select { target, text } => {
            format!("Command::Select {{ target: {}, text: {} }}", s(target), s(text))
        }
        Command::Expand { target } => format!("Command::Expand {{ target: {} }}", s(target)),
        Command::Collapse { target } => format!("Command::Collapse {{ target: {} }}", s(target)),
        Command::Scroll { target } => format!("Command::Scroll {{ target: {} }}", s(target)),
        Command::Drag { target, to } => format!("Command::Drag {{ target: {}, to: {} }}", s(target), s(to)),
        Command::Wait { target, text } => format!(
            "Command::Wait {{ target: {}, text: {} }}",
            s(target),
            text.as_deref().map_or_else(|| "None".to_owned(), |t| format!("Some({})", s(t)))
        ),
        Command::Snapshot => "Command::Snapshot".to_owned(),
        Command::Bye => "Command::Bye".to_owned(),
    }
}

/// A scenario's name as a test function's name: lower case ASCII letters, digits and `_`.
pub fn ident(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(fold) {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_owned();
    match out.chars().next() {
        None => "cenario".to_owned(),
        Some(c) if c.is_ascii_digit() => format!("cenario_{out}"),
        Some(_) => out,
    }
}

/// A letter with its accent taken off, for the Portuguese of scenario names.
fn fold(c: char) -> Option<char> {
    let plain = match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' => 'a',
        'é' | 'ê' | 'è' | 'É' | 'Ê' => 'e',
        'í' | 'Í' => 'i',
        'ó' | 'ô' | 'õ' | 'Ó' | 'Ô' | 'Õ' => 'o',
        'ú' | 'ü' | 'Ú' => 'u',
        'ç' | 'Ç' => 'c',
        c => c,
    };
    Some(plain)
}
