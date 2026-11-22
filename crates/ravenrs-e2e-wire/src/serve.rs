//! The app's side of the channel.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use ravenrs_e2e::E2e;

use crate::{Command, Failure, Hello, Reply, Source, codes, target};

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
    /// The current display list as one `.ravenmovie` frame. Capturing must not advance the
    /// session or replace its observer. Existing film-only adapters may leave this unsupported.
    fn frame(&mut self, e2e: &E2e) -> Result<Vec<u8>, String> {
        let _ = e2e;
        Err("this app provides no vector frames".to_owned())
    }
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
        let Some((reply, end)) = answer(&line, &mut session, filming.as_deref_mut()) else {
            continue;
        };
        write_line(&mut output, &reply)?;
        if end.is_some() {
            return Ok(());
        }
    }
    Ok(())
}

/// How a session ends, when a command ends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum End {
    /// [`Command::Bye`]: the app exits.
    Bye,
    /// [`Command::Release`]: the window, if any, is left to the person.
    Release,
}

/// The reply to one line of the channel, run on `session`, and whether it ends the session;
/// `None` for a blank line. Shared by the session with no window and the one with it, so a
/// command answers the same either way.
pub(crate) fn answer<'f>(
    line: &str,
    session: &mut E2e,
    filming: Option<&mut (dyn Filming + 'f)>,
) -> Option<(Reply, Option<End>)> {
    if line.trim().is_empty() {
        return None;
    }
    let reply = match serde_json::from_str::<Command>(line) {
        Ok(Command::Bye) => return Some((Reply::of(Ok(None)), Some(End::Bye))),
        Ok(Command::Release) => return Some((Reply::of(Ok(None)), Some(End::Release))),
        Ok(Command::Source { target: query }) => match session.source(&target(&query)) {
            Ok(location) => Reply::source(Source::from(location)),
            Err(error) => Reply::of(Err(error.into())),
        },
        Ok(Command::Film { path }) if filming.is_some() => {
            if let Some(film) = filming {
                film.start(session, Path::new(&path));
            }
            Reply::of(Ok(None))
        }
        Ok(Command::FilmStop) if filming.is_some() => {
            let kept = filming.map(|film| film.stop(session));
            Reply::of(match kept {
                Some(Ok(path)) => Ok(Some(path.display().to_string())),
                Some(Err(why)) => Err(Failure::of(codes::NO_FILM, why)),
                None => Ok(None),
            })
        }
        Ok(Command::Frame) if filming.is_some() => match filming.map(|film| film.frame(session)) {
            Some(Ok(bytes)) => Reply::frame(bytes),
            Some(Err(why)) => Reply::of(Err(Failure::of(codes::NO_FRAME, why))),
            None => Reply::of(Err(Failure::of(codes::NO_FRAME, "no vector-frame adapter"))),
        },
        Ok(command) => Reply::of(command.apply(session)),
        Err(e) => Reply::of(Err(Failure::of(codes::WIRE, format!("not a command of this channel: {e}")))),
    };
    Some((reply, None))
}

pub(crate) fn write_line(output: &mut impl Write, value: &impl serde::Serialize) -> io::Result<()> {
    let text = serde_json::to_string(value).map_err(io::Error::other)?;
    writeln!(output, "{text}")?;
    output.flush()
}
