//! The database, backups, the look of the app, and updates.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use ravenrs::native::{OpenDialog, SaveDialog, open_with_system};
use ravenrs::prelude::*;

use super::forms::{Confirm, Forms};
use super::store::{Store, heading};
use super::updates;
use crate::backup::{self, CopyReason};
use crate::db::{Database, DbResult};
use crate::model::{date_label, parse_date};
use crate::seed;
use crate::update::{self, UpdateStatus};

/// How many of the kept copies the page lists; the newest are the ones that matter.
const COPIES_SHOWN: usize = 5;

fn info_row(label: &str, value: String) -> impl View + use<> {
    HStack::new((
        Text::new(label.to_owned()).style(TextStyle::Callout).foreground(Semantic::InkMuted).width(150.0),
        Text::new(value).style(TextStyle::Callout),
    ))
    .spacing(8.0)
}

fn group<V: View + 'static>(title: &str, content: V) -> impl View + use<V> {
    VStack::new((Text::new(title.to_owned()).style(TextStyle::Headline), content))
        .spacing(10.0)
        .align(Align::Stretch)
}

fn make_backup(store: &Store) {
    let name = format!("raven-financas-{}.db", store.today);
    let Some(target) =
        SaveDialog::new(&name).title("Salvar backup").filter("Backup do Raven Finanças", &["db"]).ask()
    else {
        return;
    };
    match backup::save_backup(&store.db, store.path.as_deref(), &target, SystemTime::now()) {
        Ok(contents) => {
            let told = format!("{}. Guardado em {}", backup::contents_label(&contents), target.display());
            store.toasts.show_for("Backup salvo", told, Tone::Success, 8.0);
            store.refresh();
        }
        Err(e) => store.toasts.show_for("Não foi possível fazer o backup", e, Tone::Danger, 8.0),
    }
}

/// What the file holds and what would be lost, before anything changes. A file
/// that cannot be restored stops here, with the reason.
fn ask_to_restore(store: &Store, confirm: &Confirm, file: PathBuf, from: &str) {
    if store.path.as_deref() == Some(file.as_path()) {
        let why = "Esse é o próprio banco em uso. Escolha um arquivo de backup.";
        store.toasts.show_for("Não foi possível restaurar", why, Tone::Danger, 8.0);
        return;
    }
    let contents = match Database::inspect_backup(&file) {
        Ok(contents) => contents,
        Err(e) => {
            store.toasts.show_for("Não foi possível restaurar", e.to_string(), Tone::Danger, 8.0);
            return;
        }
    };
    let last = contents
        .last_date
        .as_deref()
        .and_then(parse_date)
        .map_or_else(String::new, |day| format!(" O último lançamento é de {}.", date_label(day)));
    let now = store.read(|db| db.contents().map(|c| backup::contents_label(&c)).map(Some));
    let keeps = match &store.path {
        Some(_) => {
            "Antes de trocar, o app guarda uma cópia dos dados de agora, e você pode voltar a ela nesta mesma tela."
        }
        None => "Os dados de agora serão perdidos.",
    };
    let message = format!(
        "{from} tem {}.{last}\n\nHoje o app tem {}. Esses dados serão trocados pelos do backup.\n\n{keeps}",
        backup::contents_label(&contents),
        now.unwrap_or_default(),
    );
    let store = store.clone();
    confirm.ask("Restaurar este backup?", &message, "Restaurar", move || {
        match backup::restore(&store.db, store.path.as_deref(), &file, store.today) {
            Ok((contents, _)) => {
                store.toasts.show_for(
                    "Backup restaurado",
                    backup::contents_label(&contents),
                    Tone::Success,
                    8.0,
                );
                store.refresh();
            }
            Err(e) => {
                let told = format!("Seus dados continuam como estavam. {e}");
                store.toasts.show_for("Não foi possível restaurar", told, Tone::Danger, 8.0);
            }
        }
    });
}

/// How the last backup stands: the first thing someone looking for it wants to know.
fn backup_status(store: &Store) -> Alert {
    let Some(last) = store.path.as_deref().and_then(backup::last_backup) else {
        return Alert::new(
            "Você ainda não fez nenhum backup",
            "Se este computador quebrar, seus dados vão junto. Faça um e guarde fora dele.",
        )
        .tone(Tone::Warning);
    };
    let moment = SystemTime::UNIX_EPOCH + Duration::from_secs(last.seconds);
    let days = backup::days_since(last.seconds, SystemTime::now());
    let place = format!("{}: {}", backup::moment_label(moment), last.path.display());
    if days > backup::BACKUP_IS_OLD_AFTER_DAYS {
        Alert::new(format!("Seu último backup foi há {days} dias"), place).tone(Tone::Warning)
    } else {
        Alert::new("Último backup", place).tone(Tone::Success)
    }
}

/// Keeps a copy before `work` replaces the data. Without one, nothing is replaced.
fn replace_keeping_a_copy(
    store: &Store,
    reason: CopyReason,
    done: &str,
    work: impl FnOnce(&Database) -> DbResult<()>,
) {
    if let Some(path) = &store.path
        && let Err(e) = backup::keep_copy(&store.db, &backup::copies_folder(path), store.today, reason)
    {
        let told = format!("Seus dados continuam como estavam. {e}");
        store.toasts.show_for("Não foi possível guardar uma cópia", told, Tone::Danger, 8.0);
        return;
    }
    store.write(done, work);
}

fn copy_rows(store: &Store, confirm: &Confirm) -> Vec<ListItem> {
    let Some(path) = &store.path else {
        return Vec::new();
    };
    backup::copies(&backup::copies_folder(path))
        .into_iter()
        .take(COPIES_SHOWN)
        .map(|copy| {
            let (store, confirm) = (store.clone(), confirm.clone());
            let from = format!("A cópia de {}", copy.when);
            ListItem::new(copy.when.clone())
                .subtitle(copy.reason.label())
                .trailing("Restaurar")
                .on_click(move || ask_to_restore(&store, &confirm, copy.path.clone(), &from))
        })
        .collect()
}

pub(crate) fn settings(
    store: &Store,
    forms: &Forms,
    news: &updates::NewsDialog,
    page: &State<usize>,
) -> impl View + use<> {
    let (diagnostics, painel) = (store.diagnostics.clone(), page.clone());
    let path = store.path.as_ref().map_or_else(|| "Só na memória".to_owned(), |p| p.display().to_string());
    let version = store.read(|db| db.schema_version());
    let (backup_store, restore_store, wipe_store, demo_store) =
        (store.clone(), store.clone(), store.clone(), store.clone());
    let restore_confirm = forms.confirm.clone();
    let folder = store.path.as_ref().and_then(|p| p.parent().map(PathBuf::from));
    let rows = copy_rows(store, &forms.confirm);
    let (wipe_confirm, demo_confirm) = (forms.confirm.clone(), forms.confirm.clone());
    let news = news.clone();
    let updater = store.updater.clone();
    let update_text = match store.updater.status() {
        UpdateStatus::Quiet => "Você está na versão mais recente que o app conhece.".to_owned(),
        UpdateStatus::Available(n) | UpdateStatus::Development(n) => {
            format!("A versão {} está disponível.", n.versao)
        }
        UpdateStatus::Compiling(n) => format!("Compilando a versão {} em segundo plano.", n.versao),
        UpdateStatus::Ready(n) => format!("A versão {} está instalada; reabra o app para usar.", n.versao),
        UpdateStatus::Failed(n, reason) => format!("A versão {} não foi instalada: {reason}", n.versao),
        UpdateStatus::Checking => "Procurando versão nova…".to_owned(),
        UpdateStatus::UpToDate => format!("Você está na versão mais recente ({}).", update::CURRENT),
        UpdateStatus::NoAnswer(reason) => format!("Não consegui verificar se há versão nova: {reason}"),
    };

    ScrollView::new((VStack::new((
        heading("Ajustes", "Onde seus dados ficam e como o app se atualiza."),
        group("Aparência", ThemeSwitcher),
        group(
            "Desempenho",
            Card::new(
                VStack::new((
                    Text::new(
                        "Está achando o app lento? O teste abre o Painel, redesenha a janela por 5 segundos e diz o resultado.",
                    )
                    .style(TextStyle::Callout),
                    Button::new("Testar desempenho").icon(IconName::Activity).style(ButtonStyle::Secondary).on_click(
                        move || {
                            painel.set(0);
                            diagnostics.start_test();
                        },
                    ),
                ))
                .spacing(10.0)
                .align(Align::Start),
            ),
        ),
        group(
            "Backup",
            Card::new(
                VStack::new((
                    backup_status(store),
                    Text::new(
                        "Um backup é um arquivo com todos os seus dados. Guarde-o fora deste computador, em um pen drive ou na nuvem. Para trazer os dados de volta, aqui ou em outro computador, escolha esse arquivo em Restaurar backup.",
                    )
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
                    HStack::new((
                        Button::new("Fazer backup")
                            .icon(IconName::Download)
                            .on_click(move || make_backup(&backup_store)),
                        Button::new("Restaurar backup").icon(IconName::Upload).style(ButtonStyle::Secondary).on_click(
                            move || {
                                let chosen = OpenDialog::new()
                                    .title("Escolha o arquivo de backup")
                                    .filter("Backup do Raven Finanças", &["db"])
                                    .ask();
                                if let Some(file) = chosen {
                                    ask_to_restore(&restore_store, &restore_confirm, file, "Esse backup");
                                }
                            },
                        ),
                    ))
                    .spacing(8.0)
                    .wrap(),
                ))
                .spacing(14.0)
                .align(Align::Stretch),
            ),
        ),
        (!rows.is_empty()).then(|| {
            group(
                "Cópias automáticas",
                VStack::new((
                    Text::new(
                        "O app guarda sozinho uma cópia por dia e outra antes de trocar seus dados. Elas ficam neste computador, então não substituem um backup. Clique em uma para voltar a ela.",
                    )
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
                    List::new(rows),
                ))
                .spacing(10.0)
                .align(Align::Stretch),
            )
        }),
        group(
            "Onde seus dados ficam",
            Card::new(
                VStack::new((
                    info_row("Arquivo", path),
                    info_row("Versão do esquema", version.to_string()),
                    info_row("Motor", "SQLite, compilado dentro do app".to_owned()),
                    folder.map(|folder| {
                        Button::new("Abrir a pasta").icon(IconName::FolderOpen).style(ButtonStyle::Ghost).on_click(
                            move || {
                                if let Err(e) = open_with_system(&folder) {
                                    tracing::warn!(error = %e, "could not open the folder");
                                }
                            },
                        )
                    }),
                ))
                .spacing(10.0)
                .align(Align::Stretch),
            ),
        ),
        group(
            "Recomeçar",
            Card::new(
                VStack::new((
                    Text::new("Trocam tudo o que está no app. Antes, ele guarda uma cópia, que aparece em Cópias automáticas.")
                        .style(TextStyle::Callout)
                        .foreground(Semantic::InkMuted),
                    HStack::new((
                        Button::new("Carregar dados de exemplo").style(ButtonStyle::Secondary).on_click(move || {
                            let store = demo_store.clone();
                            demo_confirm.ask(
                                "Trocar tudo pelos dados de exemplo?",
                                "Tudo o que está no app dará lugar a uma família de exemplo. Uma cópia dos dados de agora fica guardada.",
                                "Trocar pelos dados de exemplo",
                                move || {
                                    let today = store.today;
                                    replace_keeping_a_copy(&store, CopyReason::BeforeSample, "Dados de exemplo carregados", |db| {
                                        db.atomically(|db| {
                                            db.clear()?;
                                            seed::load_demo(db, today)
                                        })
                                    });
                                },
                            );
                        }),
                        Button::new("Começar do zero").icon(IconName::Trash2).style(ButtonStyle::Secondary).on_click(move || {
                            let store = wipe_store.clone();
                            wipe_confirm.ask(
                                "Apagar todos os dados?",
                                "Lançamentos, contas e cadastros serão apagados. Uma cópia dos dados de agora fica guardada.",
                                "Apagar tudo",
                                move || {
                                    replace_keeping_a_copy(&store, CopyReason::BeforeErase, "Dados apagados", |db| {
                                        db.atomically(|db| {
                                            db.clear()?;
                                            seed::load_defaults(db)
                                        })
                                    });
                                },
                            );
                        }),
                    ))
                    .spacing(8.0)
                    .wrap(),
                ))
                .spacing(10.0)
                .align(Align::Stretch),
            ),
        ),
        group(
            "Atualização",
            Card::new(
                VStack::new((
                    info_row("Versão instalada", update::CURRENT.to_owned()),
                    Text::new(update_text).style(TextStyle::Callout),
                    Text::new(
                        "O app baixa o código e compila neste computador. A versão atual só é trocada depois de a nova provar que abre.",
                    )
                    .style(TextStyle::Caption)
                    .foreground(Semantic::InkMuted),
                    HStack::new((
                        Button::new("Procurar atualização")
                            .icon(IconName::RefreshCw)
                            .style(ButtonStyle::Secondary)
                            .on_click(move || updater.ask()),
                        Button::new("Ver novidades").style(ButtonStyle::Ghost).on_click(move || news.show_latest()),
                    ))
                    .spacing(8.0),
                ))
                .spacing(10.0)
                .align(Align::Stretch),
            ),
        ),
    ))
    .spacing(24.0)
    .align(Align::Stretch),))
    .padding(24.0)
}
