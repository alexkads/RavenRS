//! The journal of a build's log on the agent's disk (#966): `<number>.saida`, beside the folder
//! and its note `<number>.estado`.
//!
//! # Its role
//!
//! The [`Outbox`](crate::site) keeps the lines on their way to the server in memory, so a process
//! killed (`kill -9`, out of memory, the machine restarted) lost the last ones, the very lines
//! that say why it fell. Each line is written here before it waits, and an agent that takes the
//! build up again sends what never arrived, under the session and at the place it had then: the
//! server drops what it already has, so nothing repeats.
//!
//! # How it works
//!
//! - One JSON per line: first the session (`{"session": …}`), then each line of the log
//!   (`{"i": index, "step": …, "text": …}`, its text already hidden), and a mark each time the
//!   server took lines (`{"ack": index, "at": place}`: the lines before `index` arrived, and the
//!   next one goes to `place`).
//! - Each line is one `write`, with no `fsync`: a step never waits for the disk.
//! - When every line written has arrived, the file starts over with the session and the last
//!   mark, so it stays small; it is removed when the build ends.
//! - A file cut in the middle of a line (the process died writing it) is read up to its last
//!   whole line.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use raven_ci_core::agents::AgentLine;
use serde_json::{Value, json};

/// What a journal left by a life before still has to send.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Pending {
    /// The session the lines were written under.
    pub session: String,
    /// The place of the first of them on the server.
    pub from: u64,
    /// The lines, in order.
    pub lines: Vec<AgentLine>,
    /// Whether the file ended in the middle of a line.
    pub cut: bool,
}

struct Open {
    file: File,
    session: String,
    /// The index the next line gets.
    next: u64,
    /// The last mark: lines before `acked` arrived, and the next goes to `at`.
    acked: u64,
    at: u64,
}

/// The journal of one build. See the [module](self).
pub(crate) struct Journal {
    path: PathBuf,
    open: Mutex<Option<Open>>,
}

impl std::fmt::Debug for Journal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Journal").field("path", &self.path).finish_non_exhaustive()
    }
}

fn write_line(file: &mut File, value: &Value) -> bool {
    let mut text = value.to_string();
    text.push('\n');
    file.write_all(text.as_bytes()).is_ok()
}

impl Journal {
    /// A journal at `path`, started over for `session`. One that cannot be written is no journal:
    /// the log goes on as before, only from memory.
    pub(crate) fn create(path: &Path, session: &str) -> Self {
        let open = path.parent().and_then(|parent| std::fs::create_dir_all(parent).ok()).and_then(|()| {
            let mut file = File::create(path).ok()?;
            write_line(&mut file, &json!({ "session": session })).then_some(Open {
                file,
                session: session.to_owned(),
                next: 0,
                acked: 0,
                at: 0,
            })
        });
        Self { path: path.to_owned(), open: Mutex::new(open) }
    }

    /// Writes `line` before it waits for the server; its index, to [`ack`](Self::ack) it later.
    pub(crate) fn line(&self, line: &AgentLine) -> Option<u64> {
        let mut guard = self.open.lock().unwrap_or_else(PoisonError::into_inner);
        let open = guard.as_mut()?;
        let index = open.next;
        if !write_line(&mut open.file, &json!({ "i": index, "step": line.step, "text": line.text })) {
            return None;
        }
        open.next += 1;
        Some(index)
    }

    /// The server took every line before `upto`, and the next one goes to `place`.
    pub(crate) fn ack(&self, upto: u64, place: u64) {
        let mut guard = self.open.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(open) = guard.as_mut() else { return };
        if upto < open.acked {
            return;
        }
        (open.acked, open.at) = (upto, place);
        if upto >= open.next {
            // Everything arrived: the file starts over, with the session and this mark.
            let started = File::create(&self.path).ok().and_then(|mut file| {
                (write_line(&mut file, &json!({ "session": open.session }))
                    && write_line(&mut file, &json!({ "ack": upto, "at": place })))
                .then_some(file)
            });
            if let Some(file) = started {
                open.file = file;
                return;
            }
        }
        write_line(&mut open.file, &json!({ "ack": upto, "at": place }));
    }

    /// Removes the file: the build ended.
    pub(crate) fn remove(&self) {
        self.open.lock().unwrap_or_else(PoisonError::into_inner).take();
        let _ = std::fs::remove_file(&self.path);
    }

    /// What the journal at `path`, left by a life before, still has to send; `None` when there
    /// is no journal, or nothing waits.
    pub(crate) fn pending(path: &Path) -> Option<Pending> {
        let text = std::fs::read_to_string(path).ok()?;
        let mut pending = Pending::default();
        let (mut acked, mut lines): (u64, Vec<(u64, AgentLine)>) = (0, Vec::new());
        for (n, raw) in text.split_inclusive('\n').enumerate() {
            let Ok(value) = serde_json::from_str::<Value>(raw.trim_end()) else {
                pending.cut = true;
                break;
            };
            if !raw.ends_with('\n') {
                pending.cut = true;
                break;
            }
            if let Some(session) = value.get("session").and_then(Value::as_str) {
                if n == 0 {
                    pending.session = session.to_owned();
                }
            } else if let (Some(upto), Some(at)) =
                (value.get("ack").and_then(Value::as_u64), value.get("at").and_then(Value::as_u64))
            {
                (acked, pending.from) = (upto, at);
            } else if let (Some(i), Some(step), Some(text)) = (
                value.get("i").and_then(Value::as_u64),
                value.get("step").and_then(Value::as_str),
                value.get("text").and_then(Value::as_str),
            ) {
                lines.push((i, AgentLine { step: step.to_owned(), text: text.to_owned() }));
            }
        }
        pending.lines = lines.into_iter().filter(|(i, _)| *i >= acked).map(|(_, line)| line).collect();
        (!pending.session.is_empty() && (!pending.lines.is_empty() || pending.cut)).then_some(pending)
    }
}

#[cfg(test)]
mod tests {
    use super::{Journal, Pending};
    use raven_ci_core::agents::AgentLine;

    fn line(text: &str) -> AgentLine {
        AgentLine { step: "build".into(), text: text.into() }
    }

    fn folder(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("raven-ci-diario-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn what_the_server_never_took_is_read_back_at_its_place_and_nothing_else() {
        let dir = folder("pendente");
        let path = dir.join("7.saida");
        let journal = Journal::create(&path, "sessao-a");
        for n in 0..5 {
            assert_eq!(journal.line(&line(&format!("linha {n}"))), Some(n));
        }
        // The server took the first three, which went to its places 10 to 12.
        journal.ack(3, 13);
        // The process dies here: the journal stays.
        let pending = Journal::pending(&path).expect("two lines wait");
        assert_eq!(
            pending,
            Pending {
                session: "sessao-a".into(),
                from: 13,
                lines: vec![line("linha 3"), line("linha 4")],
                cut: false
            }
        );
        // Everything taken: the file starts over and nothing waits.
        journal.ack(5, 15);
        assert_eq!(Journal::pending(&path), None);
        assert!(std::fs::metadata(&path).expect("the file").len() < 100, "it starts over small");
        journal.remove();
        assert!(!path.exists(), "removed when the build ends");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_journal_cut_in_the_middle_of_a_line_is_read_up_to_its_last_whole_line() {
        let dir = folder("cortado");
        let path = dir.join("8.saida");
        let journal = Journal::create(&path, "sessao-b");
        journal.line(&line("inteira"));
        drop(journal);
        let mut text = std::fs::read_to_string(&path).expect("the file");
        text.push_str(r#"{"i":1,"step":"build","te"#);
        std::fs::write(&path, text).expect("cut");
        let pending = Journal::pending(&path).expect("one line and the cut");
        assert_eq!(pending.lines, vec![line("inteira")]);
        assert!(pending.cut);
        assert_eq!(pending.from, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_journal_or_nothing_waiting_is_none() {
        let dir = folder("vazio");
        assert_eq!(Journal::pending(&dir.join("9.saida")), None);
        let path = dir.join("9.saida");
        let journal = Journal::create(&path, "sessao-c");
        assert_eq!(Journal::pending(&path), None, "only the session");
        journal.remove();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
