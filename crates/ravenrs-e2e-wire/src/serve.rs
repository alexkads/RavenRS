//! The app's side of the channel.

use std::io::{self, BufRead, Write};

use ravenrs_e2e::E2e;

use crate::{Command, Failure, Hello, Reply, codes};

/// Answers a tool over this process's standard input and output until it says
/// [`Command::Bye`] or closes the input, starting with `hello`: the app's name and the scenarios
/// it brings. Call it instead of opening the window, when the app is opened with
/// [`crate::SERVE_FLAG`]; nothing else may write to standard output meanwhile.
pub fn serve(hello: Hello, session: E2e) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve_on(hello, session, stdin.lock(), stdout.lock())
}

/// [`serve`] over any input and output, for tests and other transports.
pub fn serve_on(
    hello: Hello,
    mut session: E2e,
    input: impl BufRead,
    mut output: impl Write,
) -> io::Result<()> {
    write_line(&mut output, &hello)?;
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Command>(&line) {
            Ok(Command::Bye) => {
                write_line(&mut output, &Reply::of(Ok(None)))?;
                return Ok(());
            }
            Ok(command) => Reply::of(command.apply(&mut session)),
            Err(e) => Reply::of(Err(Failure::of(codes::WIRE, format!("not a command of this channel: {e}")))),
        };
        write_line(&mut output, &reply)?;
    }
    Ok(())
}

fn write_line(output: &mut impl Write, value: &impl serde::Serialize) -> io::Result<()> {
    let text = serde_json::to_string(value).map_err(io::Error::other)?;
    writeln!(output, "{text}")?;
    output.flush()
}
