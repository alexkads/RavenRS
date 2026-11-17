//! The news of each version and the update's notice, drawn the same way in every app that
//! installs with this crate (feature `ui`): Raven Finanças, Raven Flows, Raven Test Studio and
//! any app made later.
//!
//! - [`release_note`] and [`release_notes`] turn a version's [`News`] into a
//!   [`ReleaseNote`]: what changed, why update (or why it mattered), who contributed, the
//!   issues it closed (each one opens), its date and, for a newer version, how to update, with
//!   the installer's command. The newer one is marked "Nova", the one running "Instalada".
//! - [`NewsDialog`] is the "Novidades" dialog over every version known, with "Atualizar"
//!   when there is a new one, and in its place "Detalhes da atualização" when one failed: why,
//!   where it stopped and the end of the installer's log.
//! - [`version_button`] is the version at the foot of the window, which opens the news of the
//!   version running; [`with_notice`] puts the updater's notice in the middle of a
//!   [`StatusBar`], each button doing what it says.
//!
//! The app keeps its own bar around them (Raven Finanças adds "Relatar problema", the Studio
//! the health of its self-test) and passes how to open a local file
//! ([`NewsDialog::on_open_file`]), which only the desktop can do.

use std::path::Path;
use std::rc::Rc;

use ravenrs_layout::Align;
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, TextStyle, Weight};
use ravenrs_widgets::{
    AnyView, BuildCx, Button, ButtonStyle, CodeBlock, Dialog, HStack, IconName, Links, NodeId, ReleaseNote,
    ReleaseNotes, Spinner, StatusBar, Text, Tone, VStack, View, ViewExt,
};

use crate::{News, NoticeAction, UpdateStatus, Updater, is_newer};

/// How many lines of the installer's log the details show: its end, where an error is, with
/// no scrolling to find it. The whole log opens with "Abrir o registro".
pub const LOG_LINES: usize = 12;

/// What a newer version says on how to update from the app.
const HOW_TO_UPDATE: &str = "1. Clique em Atualizar: o app baixa o código da versão nova e compila neste computador.\n\
     2. Leva alguns minutos. Você pode continuar usando o app enquanto isso.\n\
     3. No fim, clique em Reabrir agora. Seus dados continuam lá.";

/// A version's news as a [`ReleaseNote`], for the app `updater` updates: see the
/// [module](self). Each issue opens through `links`.
pub fn release_note(updater: &Updater, news: &News, links: &Links) -> ReleaseNote {
    let app = updater.app();
    let new = is_newer(&news.versao, app.version);
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
            let (links, url) = (links.clone(), app.issue_url(number));
            (format!("#{number}"), move || links.open(&url))
        }),
    );
    if let Some(day) = news.data.as_deref().and_then(day_label) {
        note = note.date(day);
    }
    if new {
        note = note
            .badge("Nova", Tone::Success)
            .section("Como atualizar", HOW_TO_UPDATE)
            .section("Se a atualização pelo app falhar", "Feche o app e rode no terminal:")
            .code(app.install_command());
    } else if news.versao == app.version {
        note = note.badge("Instalada", Tone::Neutral);
    }
    note
}

/// Every version `updater` knows, newest first, as [`release_note`] draws each.
pub fn release_notes(updater: &Updater, links: &Links) -> Vec<ReleaseNote> {
    updater.timeline().iter().map(|news| release_note(updater, news, links)).collect()
}

/// An ISO day (`2026-11-17`) as people read it here (`17/11/2026`).
pub fn day_label(iso: &str) -> Option<String> {
    let mut parts = iso.get(..10)?.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    let numbers = [year, month, day].iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    (numbers && year.len() == 4 && month.len() == 2 && day.len() == 2)
        .then(|| format!("{day}/{month}/{year}"))
}

/// The "Novidades" dialog and, when an update failed, its details: whether they are open and on
/// which version. Clone it freely; the copies are the same dialog.
#[derive(Clone)]
pub struct NewsDialog {
    open: State<bool>,
    at: State<usize>,
    details: State<bool>,
    open_file: Option<Rc<dyn Fn(&Path)>>,
}

impl std::fmt::Debug for NewsDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewsDialog")
            .field("open", &self.open.get())
            .field("at", &self.at.get())
            .field("details", &self.details.get())
            .finish_non_exhaustive()
    }
}

impl Default for NewsDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl NewsDialog {
    /// A closed dialog.
    pub fn new() -> Self {
        Self { open: State::new(false), at: State::new(0), details: State::new(false), open_file: None }
    }

    /// Opens a local file with the system, for "Abrir o registro" in the details; on the desktop,
    /// `ravenrs::native::open_with_system`. Without it, the details show the log's end only.
    #[must_use]
    pub fn on_open_file(mut self, open: impl Fn(&Path) + 'static) -> Self {
        self.open_file = Some(Rc::new(open));
        self
    }

    /// Whether the news are open.
    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    /// Whether the details of a failed update are open.
    pub fn details_open(&self) -> bool {
        self.details.get()
    }

    /// Which version the news show, counted from the newest.
    pub fn at(&self) -> usize {
        self.at.get()
    }

    /// Opens the news on the newest version known, the one a notice talks about.
    pub fn show_latest(&self) {
        self.at.set(0);
        self.open.set(true);
    }

    /// Opens the news on the version running.
    pub fn show_installed(&self, updater: &Updater) {
        let at = updater.timeline().iter().position(|news| news.versao == updater.app().version);
        self.at.set(at.unwrap_or(0));
        self.open.set(true);
    }

    /// Opens the details of a failed update.
    pub fn show_details(&self) {
        self.details.set(true);
    }

    /// The dialog to place in the app's tree, over its screen: the news, or the details of a
    /// failed update in their place. The two are never open together, and the details read the
    /// installer's log only while they are shown.
    pub fn view(&self, updater: &Updater) -> impl View + use<> {
        Shown { dialog: self.clone(), updater: updater.clone() }
    }

    fn news_view(&self, updater: &Updater, links: &Links) -> Dialog {
        // Only when the dialog is on screen: this reads the whole history of versions.
        let notes = ReleaseNotes::new(release_notes(updater, links), self.at.binding());
        let close = self.open.clone();
        let mut dialog = Dialog::new(self.open.binding(), "Novidades")
            .width(780.0)
            .content(notes)
            .action(Button::new("Fechar").style(ButtonStyle::Secondary).on_click(move || close.set(false)));
        if let UpdateStatus::Available(news) = updater.status() {
            let (updater, open) = (updater.clone(), self.open.clone());
            dialog = dialog.action(Button::new("Atualizar").icon(IconName::Download).on_click(move || {
                updater.install(news.clone());
                open.set(false);
            }));
        }
        dialog
    }

    fn details_view(&self, updater: &Updater) -> Dialog {
        let reason = match updater.status() {
            UpdateStatus::Failed(_, reason) => reason,
            _ => String::new(),
        };
        let app = updater.app();
        let (tail, log) = (app.installer_log_tail(LOG_LINES), app.installer_log());
        let content = VStack::new((
            Text::new(if reason.is_empty() {
                "A atualização não deu certo.".to_owned()
            } else {
                capitalized(&reason)
            })
            .style(TextStyle::Body),
            updater.stage().map(|stage| {
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
                    .spacing(8.0)
                    .align(Align::Stretch),
                ) as AnyView,
                None => Box::new(
                    Text::new("O instalador não chegou a escrever um registro.")
                        .style(TextStyle::Callout)
                        .foreground(Semantic::InkMuted),
                ) as AnyView,
            },
        ))
        .spacing(12.0)
        .align(Align::Stretch);
        let (close, manual, news) = (self.details.clone(), self.details.clone(), self.clone());
        let mut dialog = Dialog::new(self.details.binding(), "Detalhes da atualização")
            .width(720.0)
            .content(content)
            .action(Button::new("Fechar").style(ButtonStyle::Secondary).on_click(move || close.set(false)))
            .action(Button::new("Ver como atualizar à mão").style(ButtonStyle::Secondary).on_click(
                move || {
                    manual.set(false);
                    news.show_latest();
                },
            ));
        if let Some(open_file) = self.open_file.clone() {
            dialog = dialog.action(
                Button::new("Abrir o registro")
                    .icon(IconName::FileText)
                    .style(ButtonStyle::Secondary)
                    .on_click(move || open_file(&log)),
            );
        }
        let (retry, details) = (updater.clone(), self.details.clone());
        dialog.action(Button::new("Tentar de novo").icon(IconName::RefreshCw).on_click(move || {
            if let Some(announced) = retry.status().news() {
                retry.install(announced.clone());
            }
            details.set(false);
        }))
    }
}

/// The dialog in the tree; it reads the app's links service when it builds.
struct Shown {
    dialog: NewsDialog,
    updater: Updater,
}

impl View for Shown {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        if self.dialog.details.get() {
            cx.child(&self.dialog.details_view(&self.updater))
        } else if self.dialog.open.get() {
            let links = cx.links();
            cx.child(&self.dialog.news_view(&self.updater, &links))
        } else {
            cx.empty()
        }
    }
}

/// The reason starts in lower case to follow a sentence; alone, it starts a new one.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

/// The version at the foot of the window, `label` (such as "Raven Finanças · versão 0.1.43"),
/// which opens the news on the version running.
pub fn version_button(label: impl Into<String>, updater: &Updater, dialog: &NewsDialog) -> Button {
    let (updater, dialog) = (updater.clone(), dialog.clone());
    StatusBar::action(label).on_click(move || dialog.show_installed(&updater))
}

/// A button of the updater's notice, doing what it says: the news, the details of a failure,
/// install, reopen, look again or put it away.
pub fn notice_button(action: NoticeAction, updater: &Updater, dialog: &NewsDialog) -> Button {
    let (updater, dialog) = (updater.clone(), dialog.clone());
    let button = StatusBar::action(action.label());
    let button = if action.is_primary() { button.style(ButtonStyle::Primary) } else { button };
    button.on_click(move || match action {
        NoticeAction::News | NoticeAction::HowToUpdate => dialog.show_latest(),
        NoticeAction::Details => dialog.show_details(),
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

/// `bar` with the updater's notice in the middle, when there is one: a spinner while it works,
/// the text, and its buttons ([`notice_button`]); an important one in the warning tone.
pub fn with_notice(bar: StatusBar, updater: &Updater, dialog: &NewsDialog) -> StatusBar {
    let Some(notice) = updater.notice() else { return bar };
    let buttons: Vec<Button> =
        notice.actions.iter().map(|action| notice_button(*action, updater, dialog)).collect();
    let bar = bar.center((
        notice.busy.then(|| Spinner::new().size(12.0)),
        StatusBar::text(notice.text).foreground(Semantic::Ink).truncate().grow(1.0),
        HStack::new(buttons).spacing(4.0),
    ));
    if notice.important { bar.center_tone(Tone::Warning) } else { bar }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InstalledApp;

    static APP: InstalledApp = InstalledApp {
        name: "Raven Teste",
        package: "raven-teste",
        home_dir: ".raven-teste-das-novidades",
        bundle_id: "dev.ravenrs.raven-teste",
        icons: "examples/raven-teste/assets",
        mac_category: "public.app-category.productivity",
        linux_categories: "Office;",
        source: "https://example.invalid/raven-teste/master.tar.gz",
        env_prefix: "RT",
        installed_env: "RAVEN_TESTE_INSTALLED",
        news_urls: &["https://example.invalid/raven-teste/novidades.json"],
        history_urls: &["https://example.invalid/raven-teste/historico-de-novidades.json"],
        installer_url: "https://example.invalid/raven-teste/install.cmd",
        issues_url: "https://example.invalid/raven-teste/issues/",
        version: "0.2.0",
        installed: true,
        history: r#"[
            {"versao":"0.2.0","data":"2026-11-17","titulo":"A de agora","novidades":["Uma coisa"],"por_que_atualizar":"Porque sim","contribuidores":["Alex"],"issues":[744]},
            {"versao":"0.1.0","data":"2026-11-01","titulo":"A primeira","novidades":[],"por_que_atualizar":"p"}
        ]"#,
    };

    fn news(versao: &str) -> News {
        News {
            versao: versao.to_owned(),
            titulo: format!("Versão {versao}"),
            novidades: vec!["Algo novo".to_owned()],
            por_que_atualizar: "Para ter".to_owned(),
            contribuidores: vec!["Alex".to_owned()],
            issues: vec![744],
            data: Some("2026-11-18".to_owned()),
            importante: false,
        }
    }

    #[test]
    fn a_newer_version_says_why_and_how_to_update() {
        let updater = Updater::idle(&APP);
        let note = format!("{:?}", release_note(&updater, &news("0.3.0"), &Links::default()));
        for said in ["Nova", "Por que atualizar", "Quem contribuiu", "#744", "Como atualizar", "18/11/2026"] {
            assert!(note.contains(said), "{said}: {note}");
        }
        assert!(note.contains(&APP.install_command()), "the installer's command: {note}");
    }

    #[test]
    fn the_running_version_is_marked_installed_and_an_older_one_says_why_it_mattered() {
        let updater = Updater::idle(&APP);
        let installed = format!("{:?}", release_note(&updater, &news("0.2.0"), &Links::default()));
        assert!(installed.contains("Instalada") && !installed.contains("Como atualizar"), "{installed}");
        let older = format!("{:?}", release_note(&updater, &news("0.1.0"), &Links::default()));
        assert!(older.contains("Por que ela importa"), "{older}");
        assert!(!older.contains("Instalada") && !older.contains("\"Nova\""), "{older}");
    }

    #[test]
    fn an_issue_opens_its_page() {
        let (updater, links) = (Updater::idle(&APP), Links::default());
        let notes = release_notes(&updater, &links);
        assert_eq!(notes.len(), 2, "every version the app knows");
        let mut e2e =
            ravenrs_e2e::E2e::new(ReleaseNotes::new(notes, State::new(0).binding()), Default::default());
        e2e.by_text("#744").click().expect("the issue's link");
        assert_eq!(links.opened(), [APP.issue_url(744)]);
    }

    #[test]
    fn days_read_as_people_write_them() {
        assert_eq!(day_label("2026-11-17").as_deref(), Some("17/11/2026"));
        assert_eq!(day_label("2026-11-17T10:00:00Z").as_deref(), Some("17/11/2026"));
        assert_eq!(day_label("17/11/2026"), None);
        assert_eq!(day_label(""), None);
    }

    #[test]
    fn the_dialog_opens_on_the_version_running() {
        let (updater, dialog) = (Updater::idle(&APP), NewsDialog::new());
        dialog.show_installed(&updater);
        assert!(dialog.is_open());
        let at = dialog.at();
        assert_eq!(updater.timeline()[at].versao, APP.version);
        dialog.show_latest();
        assert_eq!(dialog.at(), 0);
        dialog.show_details();
        assert!(dialog.details_open());
    }
}
