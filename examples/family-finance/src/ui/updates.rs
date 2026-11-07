//! What the update says on screen: the status bar and the news.

use ravenrs::movie::BugReporter;
use ravenrs::prelude::*;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use crate::model::parse_date;
use crate::update::{self, News, NoticeAction, UpdateStatus, Updater};

/// Connects the updater to the UI and takes in what its threads said. They only
/// wake the UI when there is something to read, so nothing runs while they work.
pub(crate) struct UpdatePump(pub Updater);

impl View for UpdatePump {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.0.connect(cx.ui_waker());
        self.0.poll();
        cx.empty()
    }
}

/// Runs the paired Telegram bot beside this Raven instance and refreshes the UI on incoming messages.
pub(crate) struct TelegramPump {
    path: Option<PathBuf>,
    paired: Arc<AtomicBool>,
}

impl TelegramPump {
    pub(crate) fn new(path: Option<PathBuf>, paired: Arc<AtomicBool>) -> Self {
        Self { path, paired }
    }
}

impl View for TelegramPump {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        static STARTED: OnceLock<AtomicBool> = OnceLock::new();
        if let Some(path) = self.path.clone()
            && crate::telegram::token_configured()
            && !STARTED.get_or_init(AtomicBool::default).swap(true, Ordering::AcqRel)
        {
            let waker = cx.ui_waker();
            let paired = self.paired.clone();
            thread::spawn(move || {
                loop {
                    if let Err(error) = crate::telegram::serve_with_waker(
                        path.clone(),
                        Some(waker.clone()),
                        Some(paired.clone()),
                    ) {
                        tracing::warn!(error = %error, "Telegram listener stopped; retrying");
                        thread::sleep(Duration::from_secs(5));
                    }
                }
            });
        }
        if self.paired.swap(false, Ordering::AcqRel) {
            cx.sound().play(&SoundClip::click());
        }
        cx.empty()
    }
}

/// The news dialog: whether it is open, and on which version.
#[derive(Clone)]
pub(crate) struct NewsDialog {
    open: State<bool>,
    at: State<usize>,
    /// The details of a failed update.
    details: State<bool>,
}

/// How many lines of the installer's log the details show: its end, where an error is,
/// with no scrolling to find it. The whole log opens with "Abrir o registro".
const LOG_LINES: usize = 12;

impl NewsDialog {
    pub(crate) fn new() -> Self {
        Self { open: State::new(false), at: State::new(0), details: State::new(false) }
    }

    /// Opens the details of a failed update.
    pub(crate) fn show_details(&self) {
        self.details.set(true);
    }

    /// Why the update failed, where it stopped and what the installer last wrote.
    fn details_view(&self, updater: &Updater) -> impl View + use<> {
        let reason = match updater.status() {
            UpdateStatus::Failed(_, reason) => reason,
            _ => String::new(),
        };
        let stage = updater.stage();
        let tail = update::installer_log_tail(LOG_LINES);
        let log = update::installer_log();
        let (retry, close, manual, news) =
            (updater.clone(), self.details.clone(), self.details.clone(), self.clone());
        let opened = self.details.clone();
        let content = VStack::new((
            Text::new(if reason.is_empty() {
                "A atualização não deu certo.".to_owned()
            } else {
                capitalized(&reason)
            })
            .style(TextStyle::Body),
            stage.map(|stage| {
                Text::new(format!("Parou em: {stage}."))
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted)
            }),
            Text::new("A versão em uso continua funcionando. Seus dados não foram tocados.")
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
            match tail {
                Some(tail) => Box::new(
                    VStack::new((
                        Text::new(format!("Fim do registro do instalador ({}):", log.display()))
                            .style(TextStyle::Callout)
                            .font_weight(Weight::SEMIBOLD),
                        CodeBlock::new(tail),
                    ))
                    .spacing(6.0)
                    .align(Align::Stretch),
                ) as AnyView,
                None => Box::new(
                    Text::new("O instalador não chegou a escrever um registro.")
                        .style(TextStyle::Callout)
                        .foreground(Semantic::InkMuted),
                ) as AnyView,
            },
        ))
        .spacing(10.0)
        .align(Align::Stretch);
        Dialog::new(opened.binding(), "Detalhes da atualização")
            .width(720.0)
            .content(content)
            .action(Button::new("Fechar").style(ButtonStyle::Secondary).on_click(move || close.set(false)))
            .action(Button::new("Como atualizar à mão").style(ButtonStyle::Secondary).on_click(move || {
                manual.set(false);
                news.show_latest();
            }))
            .action(
                Button::new("Abrir o registro")
                    .icon(IconName::FileText)
                    .style(ButtonStyle::Secondary)
                    .on_click(move || {
                        if let Err(error) = ravenrs::native::open_with_system(&log) {
                            tracing::warn!(%error, "could not open the installer's log");
                        }
                    }),
            )
            .action(Button::new("Tentar de novo").icon(IconName::RefreshCw).on_click({
                let details = self.details.clone();
                move || {
                    if let Some(announced) = retry.status().news() {
                        retry.install(announced.clone());
                    }
                    details.set(false);
                }
            }))
    }

    /// Opens it on the newest version known, the one a notice talks about.
    pub(crate) fn show_latest(&self) {
        self.at.set(0);
        self.open.set(true);
    }

    /// Opens it on the version in use.
    pub(crate) fn show_installed(&self, updater: &Updater) {
        let at = updater.timeline().iter().position(|news| news.versao == update::CURRENT);
        self.at.set(at.unwrap_or(0));
        self.open.set(true);
    }

    /// The news, or the details of a failed update in its place: the two are never open
    /// together, and the details read the installer's log only while they are shown.
    pub(crate) fn view(&self, updater: &Updater) -> AnyView {
        if self.details.get() {
            Box::new(self.details_view(updater))
        } else {
            Box::new(self.news_view(updater))
        }
    }

    fn news_view(&self, updater: &Updater) -> impl View + use<> {
        // Only when the dialog is on screen: this reads the whole history of versions.
        let (timeline, at) = (updater.clone(), self.at.binding());
        let notes = view_fn(move || {
            let notes: Vec<ReleaseNote> = timeline.timeline().iter().map(release_note).collect();
            ReleaseNotes::new(notes, at.clone())
        });
        let open = self.open.clone();
        let offer = match updater.status() {
            UpdateStatus::Available(news) => Some(news),
            _ => None,
        };
        let updater = updater.clone();
        let close = open.clone();
        let mut dialog = Dialog::new(self.open.binding(), "Novidades")
            .width(780.0)
            .content(notes)
            .action(Button::new("Fechar").style(ButtonStyle::Secondary).on_click(move || close.set(false)));
        if let Some(news) = offer {
            dialog = dialog.action(Button::new("Atualizar").icon(IconName::Download).on_click(move || {
                updater.install(news.clone());
                open.set(false);
            }));
        }
        dialog
    }
}

/// The reason starts in lower case to follow a sentence; alone, it starts a new one.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

fn release_note(news: &News) -> ReleaseNote {
    let new = update::is_newer(&news.versao, update::CURRENT);
    let mut note = ReleaseNote::new(format!("Versão {}", news.versao), news.titulo.clone())
        .changes(news.novidades.iter().cloned())
        .section(
            if new { "Por que atualizar" } else { "Por que ela importa" },
            news.por_que_atualizar.clone(),
        );
    if !news.contribuidores.is_empty() {
        note = note.section("Quem contribuiu", news.contribuidores.join(", "));
    }
    note = note.links(
        "Issues resolvidas",
        news.issues.iter().map(|&number| {
            (format!("#{number}"), move || {
                if let Err(error) = ravenrs::native::open_url(&update::issue_url(number)) {
                    tracing::warn!(%error, number, "could not open the issue");
                }
            })
        }),
    );
    if let Some(day) = news.data.as_deref().and_then(parse_date) {
        note = note.date(crate::model::date_label(day));
    }
    if new {
        note = note
            .badge("Nova", Tone::Success)
            .section(
                "Como atualizar",
                "1. Clique em Atualizar: o app baixa o código da versão nova e compila neste computador.\n\
                 2. Leva alguns minutos. Você pode continuar usando o app enquanto isso.\n\
                 3. No fim, clique em Reabrir agora. Seus dados continuam lá.",
            )
            .section("Se a atualização pelo app falhar", "Feche o app e rode no terminal:")
            .code(update::INSTALL_COMMAND);
    } else if news.versao == update::CURRENT {
        note = note.badge("Instalada", Tone::Neutral);
    }
    note
}

fn notice_button(action: NoticeAction, updater: &Updater, news: &NewsDialog) -> Button {
    let (updater, news) = (updater.clone(), news.clone());
    let button = StatusBar::action(action.label());
    let button = if action.is_primary() { button.style(ButtonStyle::Primary) } else { button };
    button.on_click(move || match action {
        NoticeAction::News | NoticeAction::HowToUpdate => news.show_latest(),
        NoticeAction::Details => news.show_details(),
        NoticeAction::Update | NoticeAction::Retry => {
            if let Some(announced) = updater.status().news() {
                updater.install(announced.clone());
            }
        }
        NoticeAction::Reopen => updater.reopen(),
        NoticeAction::CheckAgain => updater.ask(),
        NoticeAction::Later | NoticeAction::Close => updater.dismiss(),
    })
}

/// The line at the foot of the window: the version, always, and a notice in the
/// middle when there is one. Clicking the version shows what it brought.
pub(crate) fn status_bar(updater: &Updater, news: &NewsDialog, reporter: &BugReporter) -> impl View + use<> {
    let (installed_news, installed_updater) = (news.clone(), updater.clone());
    let report = reporter.clone();
    let bar = StatusBar::new()
        .right((StatusBar::action("Relatar problema")
            .icon(IconName::CircleAlert)
            .on_click(move || report.open()),))
        .left((StatusBar::action(format!("Raven Finanças · versão {}", update::CURRENT))
            .on_click(move || installed_news.show_installed(&installed_updater)),));
    let Some(notice) = updater.notice() else {
        return bar;
    };
    let buttons: Vec<Button> =
        notice.actions.iter().map(|action| notice_button(*action, updater, news)).collect();
    let bar = bar.center((
        notice.busy.then(|| Spinner::new().size(12.0)),
        StatusBar::text(notice.text).foreground(Semantic::Ink).truncate().grow(1.0),
        HStack::new(buttons).spacing(2.0),
    ));
    if notice.important { bar.center_tone(Tone::Warning) } else { bar }
}
