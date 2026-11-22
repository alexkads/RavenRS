//! Whether there is a newer Studio, for whoever asks with no window: a person at the command
//! line, or the pane beside a Claude conversation. The answer is the one the window's status bar
//! gives, from the same rule of `ravenrs-app-update`.

use ravenrs_app_update::{Decision, InstallKind, News, check_now};
use serde_json::{Value, json};

use crate::versao::APP;

/// What a check found: the version running, the latest one, and what to do about it.
#[derive(Clone, Debug)]
pub struct Answer {
    /// The version running now.
    pub installed: &'static str,
    /// The latest version the repository announces, when the network answered.
    pub latest: Option<News>,
    /// The latest one is newer than the one running.
    pub newer: bool,
    /// This copy was installed by the installer, which can update it; a checkout updates with
    /// `git pull`.
    pub updates_itself: bool,
    /// Why the network gave no answer.
    pub error: Option<String>,
}

impl Answer {
    /// What `check_now` said, as an answer.
    pub fn of(checked: Result<(News, Decision), String>, kind: InstallKind) -> Self {
        let updates_itself = kind == InstallKind::Installed;
        match checked {
            Ok((news, decision)) => Self {
                installed: APP.version,
                newer: matches!(decision, Decision::Announce { .. }),
                latest: Some(news),
                updates_itself,
                error: None,
            },
            Err(reason) => Self {
                installed: APP.version,
                latest: None,
                newer: false,
                updates_itself,
                error: Some(reason),
            },
        }
    }

    /// For a tool, such as the pane.
    pub fn json(&self) -> Value {
        json!({
            "installed": self.installed,
            "latest": self.latest.as_ref().map(|n| n.versao.clone()),
            "newer": self.newer,
            "updates_itself": self.updates_itself,
            "news": self.latest.as_ref().filter(|_| self.newer).map(|n| json!({
                "title": n.titulo,
                "items": n.novidades,
                "why": n.por_que_atualizar,
                "important": n.importante,
                "issues": n.issues,
            })),
            "error": self.error,
        })
    }

    /// For a person.
    pub fn text(&self) -> String {
        if let Some(reason) = &self.error {
            return format!(
                "Raven Test Studio {}: não deu para saber se há versão nova: {reason}",
                self.installed
            );
        }
        let Some(news) = self.latest.as_ref().filter(|_| self.newer) else {
            return format!("Raven Test Studio {}: é a versão mais nova", self.installed);
        };
        let mut out = format!("Raven Test Studio {} → {}: {}\n", self.installed, news.versao, news.titulo);
        for item in &news.novidades {
            out.push_str(&format!("  · {item}\n"));
        }
        out.push_str(if self.updates_itself {
            "Atualize com: raven-test-studio --atualizar"
        } else {
            "Esta cópia é de um checkout do RavenRS: atualize com git pull"
        });
        out
    }
}

/// Asks the network now.
pub fn ask() -> Answer {
    Answer::of(check_now(&APP), APP.install_kind())
}
