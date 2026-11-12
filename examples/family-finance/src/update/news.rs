//! What changed: the news of each version, and what the app says about it.

use serde::Deserialize;

/// What a version brings, written for the people using the app.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct News {
    /// The version.
    pub versao: String,
    /// The release day; only the history has it.
    #[serde(default)]
    pub data: Option<String>,
    /// One line: what people will notice first.
    pub titulo: String,
    /// An update that should not wait.
    #[serde(default)]
    pub importante: bool,
    /// What changed.
    pub novidades: Vec<String>,
    /// Why it is worth updating now.
    pub por_que_atualizar: String,
    /// The GitHub issues this version closes, by number. Older copies ignore it.
    #[serde(default)]
    pub issues: Vec<u32>,
    /// Who made this version, by name. Older copies ignore it.
    #[serde(default)]
    pub contribuidores: Vec<String>,
}

/// Reads `novidades.json`. A broken one is no news: better silent than a blank announcement.
pub fn parse_news(text: &str) -> Option<News> {
    let news: News = serde_json::from_str(text).ok()?;
    (version_numbers(&news.versao).is_some() && !news.titulo.trim().is_empty()).then_some(news)
}

/// Reads a history file, skipping an entry it cannot understand.
pub fn parse_history(text: &str) -> Vec<News> {
    let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap_or_default();
    items.into_iter().filter_map(|item| serde_json::from_value(item).ok()).collect()
}

/// Every version up to this one, newest first, compiled into the app.
pub fn history() -> Vec<News> {
    parse_history(include_str!("../../historico-de-novidades.json"))
}

/// One list of versions, newest first, out of several. When a version is in
/// more than one, the first list's account of it is the one that stays, with
/// the date of whichever list has one.
pub fn join_history(lists: impl IntoIterator<Item = Vec<News>>) -> Vec<News> {
    let mut all: Vec<News> = Vec::new();
    for news in lists.into_iter().flatten() {
        match all.iter_mut().find(|known| known.versao == news.versao) {
            // The announcement has no date; the history does.
            Some(known) => known.data = known.data.take().or(news.data),
            None => all.push(news),
        }
    }
    all.sort_by_key(|news| std::cmp::Reverse(version_numbers(&news.versao)));
    all
}

pub(super) fn version_numbers(version: &str) -> Option<Vec<u64>> {
    version.trim().split('.').map(|part| part.parse().ok()).collect()
}

/// `candidate` is a later version than `current`, comparing numbers (0.1.10 is after 0.1.9).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (version_numbers(candidate), version_numbers(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// Where an update stands, for the status bar and the settings page.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum UpdateStatus {
    /// Nothing to say yet, or up to date.
    #[default]
    Quiet,
    /// A new version exists and is waiting for a click.
    Available(News),
    /// Compiling it now.
    Compiling(News),
    /// It is installed; reopening uses it.
    Ready(News),
    /// It failed, and the app that is open keeps working.
    Failed(News, String),
    /// A new version exists, but this copy updates with `git pull`.
    Development(News),
    /// Someone asked, and the answer has not come yet.
    Checking,
    /// Someone asked, and there is nothing newer.
    UpToDate,
    /// Someone asked, and the network did not answer.
    NoAnswer(String),
}

impl UpdateStatus {
    /// The version it is about, if any.
    pub fn news(&self) -> Option<&News> {
        match self {
            Self::Available(news)
            | Self::Compiling(news)
            | Self::Ready(news)
            | Self::Failed(news, _)
            | Self::Development(news) => Some(news),
            Self::Quiet | Self::Checking | Self::UpToDate | Self::NoAnswer(_) => None,
        }
    }
}

/// What a notice in the status bar offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeAction {
    /// Opens the news on the announced version.
    News,
    /// Installs the announced version.
    Update,
    /// Installs it again, after a failure.
    Retry,
    /// The news again, where the steps for doing it by hand are.
    HowToUpdate,
    /// Why it failed, the step it stopped at and the end of the installer's log.
    Details,
    /// Opens the installed version.
    Reopen,
    /// Asks the network again.
    CheckAgain,
    /// Hides the notice for this session.
    Later,
    /// Hides a notice that has been read.
    Close,
}

impl NoticeAction {
    /// The button's words.
    pub fn label(self) -> &'static str {
        match self {
            Self::News => "Ver novidades",
            Self::Update => "Atualizar",
            Self::Retry | Self::CheckAgain => "Tentar de novo",
            Self::HowToUpdate => "Como atualizar",
            Self::Details => "Ver detalhes",
            Self::Reopen => "Reabrir agora",
            Self::Later => "Depois",
            Self::Close => "Fechar",
        }
    }

    /// The button that moves things forward, drawn as the main one.
    pub fn is_primary(self) -> bool {
        matches!(self, Self::Update | Self::Reopen | Self::Retry | Self::CheckAgain)
    }
}

/// What the status bar says about updates, in one line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// The sentence.
    pub text: String,
    /// Something is under way.
    pub busy: bool,
    /// Tint this one: an important version that is waiting for a click.
    pub important: bool,
    /// Its buttons, in order.
    pub actions: Vec<NoticeAction>,
}

/// The whole rule of what the status bar says, with nothing on screen, as
/// VintageLightbox words it. `dismissed` is the version hidden for this session;
/// what is under way and what waits for reopening cannot be hidden.
pub fn notice(
    status: &UpdateStatus,
    current: &str,
    stage: Option<&str>,
    dismissed: Option<&str>,
) -> Option<Notice> {
    let said = |text: String, busy, important, actions: &[NoticeAction]| {
        Some(Notice { text, busy, important, actions: actions.to_vec() })
    };
    use NoticeAction::{CheckAgain, Close, Details, Later, News, Reopen, Retry, Update};
    match status {
        UpdateStatus::Quiet => None,
        UpdateStatus::Checking => said("Procurando versão nova…".to_owned(), true, false, &[]),
        UpdateStatus::UpToDate => {
            said(format!("Você está na versão mais recente ({current})."), false, false, &[Close])
        }
        UpdateStatus::NoAnswer(reason) => said(
            format!("Não consegui verificar se há versão nova: {reason}"),
            false,
            false,
            &[CheckAgain, Close],
        ),
        UpdateStatus::Available(news) | UpdateStatus::Development(news) | UpdateStatus::Failed(news, _)
            if dismissed == Some(news.versao.as_str()) =>
        {
            None
        }
        UpdateStatus::Available(news) => {
            let opening = if news.importante {
                format!("Atualização importante: versão {}", news.versao)
            } else {
                format!("Versão {} disponível", news.versao)
            };
            said(
                format!("{opening} — {}", news.titulo.trim()),
                false,
                news.importante,
                &[News, Update, Later],
            )
        }
        UpdateStatus::Development(news) => said(
            format!(
                "Versão {} no repositório. Esta cópia foi compilada à mão: atualize com git pull.",
                news.versao
            ),
            false,
            false,
            &[News, Later],
        ),
        UpdateStatus::Compiling(news) => said(
            format!(
                "Atualizando para a versão {} em segundo plano — {}. O app continua funcionando.",
                news.versao,
                stage.unwrap_or("começando"),
            ),
            true,
            false,
            &[News],
        ),
        UpdateStatus::Ready(news) => said(
            format!(
                "Versão {} instalada. Ela entra na próxima vez que o app abrir — ou reabra agora.",
                news.versao
            ),
            false,
            news.importante,
            &[Reopen],
        ),
        // The reason goes in the details: here it would be cut, and the end is what matters.
        UpdateStatus::Failed(news, _) => said(
            format!(
                "A atualização para a versão {} não deu certo; a {current} continua funcionando.",
                news.versao
            ),
            false,
            false,
            &[Retry, Details, Close],
        ),
    }
}
