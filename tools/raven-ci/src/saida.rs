//! How a command that speaks to a server ends (#941): its exit code ([`Exit`]) and the refusal it
//! says ([`Refused`]), in words or, with `--format json`, as `{"codigo", "saida", "mensagem"}`.
//! What a script or a session of AI reads to tell a build that failed from a server that did not
//! answer. A leaf: it uses no other module of the program.

use std::io::Write;

use raven_ci_plugin_api::CiError;
use serde_json::{Value, json};

fn failed(e: impl std::fmt::Display) -> CiError {
    CiError::failed(e.to_string())
}

/// Says `refused` to whoever ran the command: in words, or with `--format json` as
/// `{"codigo", "saida", "mensagem"}`; its exit code.
pub(crate) fn refuse(out: &mut dyn Write, refused: &Refused, json: bool) -> Result<i32, CiError> {
    if json {
        let said =
            json!({ "codigo": refused.exit.name(), "saida": refused.exit.code(), "mensagem": refused.why });
        writeln!(out, "{said}").map_err(failed)?;
    } else {
        writeln!(out, "{}", refused.why).map_err(failed)?;
    }
    Ok(refused.exit.code())
}

/// How a command ends, as its exit code says (#941): what a script or a session of AI reads to
/// tell a build that failed from a server that did not answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// 0: done, and the build passed.
    Done,
    /// 1: the build ended and did not pass.
    BuildFailed,
    /// 2: the words of the command are wrong.
    Usage,
    /// 3: the server did not answer.
    Unreachable,
    /// 4: no key, a key the server refused, or a key without the role.
    Denied,
    /// 5: the server answered with a refusal: a job that is not there, a wrong value.
    Refused,
    /// 6: something on this machine: a file, the input.
    Local,
    /// 7: the build ended unstable: it ran, with a warning (#947).
    Unstable,
}

impl Exit {
    /// Every one, in the order of their codes.
    pub const ALL: [Self; 8] = [
        Self::Done,
        Self::BuildFailed,
        Self::Usage,
        Self::Unreachable,
        Self::Denied,
        Self::Refused,
        Self::Local,
        Self::Unstable,
    ];

    /// The exit code.
    pub fn code(self) -> i32 {
        match self {
            Self::Done => 0,
            Self::BuildFailed => 1,
            Self::Usage => 2,
            Self::Unreachable => 3,
            Self::Denied => 4,
            Self::Refused => 5,
            Self::Local => 6,
            Self::Unstable => 7,
        }
    }

    /// Its name in the JSON of an error: `sem_servidor`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Done => "ok",
            Self::BuildFailed => "build_falhou",
            Self::Usage => "uso",
            Self::Unreachable => "sem_servidor",
            Self::Denied => "sem_permissao",
            Self::Refused => "recusado",
            Self::Local => "local",
            Self::Unstable => "instavel",
        }
    }
}

/// A refusal to tell whoever ran the command, in words, with how the command ends.
pub(crate) struct Refused {
    exit: Exit,
    why: String,
}

impl Refused {
    /// The refusal as JSON, as `--format json` writes it: `{codigo, saida, mensagem}`.
    pub(crate) fn to_json(&self) -> Value {
        json!({ "codigo": self.exit.name(), "saida": self.exit.code(), "mensagem": self.why })
    }

    /// The same, for what is not a refusal of the server.
    pub(crate) fn local_json(why: impl Into<String>) -> Value {
        Self::local(why).to_json()
    }

    /// What the server refused.
    pub(crate) fn new(why: impl Into<String>) -> Self {
        Self::with(Exit::Refused, why)
    }

    pub(crate) fn with(exit: Exit, why: impl Into<String>) -> Self {
        Self { exit, why: why.into() }
    }

    /// Words of the command that are wrong.
    pub(crate) fn usage(why: impl Into<String>) -> Self {
        Self::with(Exit::Usage, why)
    }

    /// Something on this machine.
    pub(crate) fn local(why: impl Into<String>) -> Self {
        Self::with(Exit::Local, why)
    }
}

impl From<CiError> for Refused {
    fn from(e: CiError) -> Self {
        Self::local(e.to_string())
    }
}

impl From<std::io::Error> for Refused {
    fn from(e: std::io::Error) -> Self {
        Self::local(e.to_string())
    }
}

pub(crate) type Said = Result<i32, Refused>;
