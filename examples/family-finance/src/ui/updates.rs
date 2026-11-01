//! What the update says on screen: the status bar and the news.

use ravenrs::movie::BugReporter;
use ravenrs::prelude::*;

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

/// The news dialog: whether it is open, and on which version.
#[derive(Clone)]
pub(crate) struct NewsDialog {
    open: State<bool>,
    at: State<usize>,
}

impl NewsDialog {
    pub(crate) fn new() -> Self {
        Self { open: State::new(false), at: State::new(0) }
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

    pub(crate) fn view(&self, updater: &Updater) -> impl View + use<> {
        let notes: Vec<ReleaseNote> = updater.timeline().iter().map(release_note).collect();
        let open = self.open.clone();
        let offer = match updater.status() {
            UpdateStatus::Available(news) => Some(news),
            _ => None,
        };
        let updater = updater.clone();
        let close = open.clone();
        let mut dialog = Dialog::new(self.open.binding(), "Novidades")
            .width(780.0)
            .content(ReleaseNotes::new(notes, self.at.binding()))
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

fn release_note(news: &News) -> ReleaseNote {
    let new = update::is_newer(&news.versao, update::CURRENT);
    let mut note = ReleaseNote::new(format!("Versão {}", news.versao), news.titulo.clone())
        .changes(news.novidades.iter().cloned())
        .section(
            if new { "Por que atualizar" } else { "Por que ela importa" },
            news.por_que_atualizar.clone(),
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
