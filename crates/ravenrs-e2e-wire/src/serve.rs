//! The app's side of the channel.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

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

/// What films the app's session when a tool asks (`ravenrs_movie::ChannelFilm` does), for
/// [`serve_filmed`]: the channel only says when to start and stop.
pub trait Filming {
    /// Starts filming `e2e`, to keep the film at `path`.
    fn start(&mut self, e2e: &mut E2e, path: &Path);
    /// Stops filming `e2e`, keeps the film and says where.
    fn stop(&mut self, e2e: &mut E2e) -> Result<PathBuf, String>;
}

/// [`serve`] that also films the session when a tool asks, with `filming`.
pub fn serve_filmed(hello: Hello, session: E2e, mut filming: impl Filming) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve_with(hello, session, stdin.lock(), stdout.lock(), Some(&mut filming))
}

/// [`serve`] over any input and output, for tests and other transports.
pub fn serve_on(hello: Hello, session: E2e, input: impl BufRead, output: impl Write) -> io::Result<()> {
    serve_with(hello, session, input, output, None)
}

/// [`serve_on`] with filming, when given.
pub fn serve_with(
    hello: Hello,
    mut session: E2e,
    input: impl BufRead,
    mut output: impl Write,
    mut filming: Option<&mut dyn Filming>,
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
            Ok(Command::Film { path }) if filming.is_some() => {
                if let Some(film) = filming.as_deref_mut() {
                    film.start(&mut session, Path::new(&path));
                }
                Reply::of(Ok(None))
            }
            Ok(Command::FilmStop) if filming.is_some() => {
                let kept = filming.as_deref_mut().map(|film| film.stop(&mut session));
                Reply::of(match kept {
                    Some(Ok(path)) => Ok(Some(path.display().to_string())),
                    Some(Err(why)) => Err(Failure::of(codes::NO_FILM, why)),
                    None => Ok(None),
                })
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
