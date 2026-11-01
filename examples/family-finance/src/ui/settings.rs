//! The database, backups, the look of the app, and updates.

use std::path::Path;

use ravenrs::native::SaveDialog;
use ravenrs::prelude::*;

use super::forms::Forms;
use super::store::{Store, heading};
use super::updates;
use crate::seed;
use crate::update::{self, UpdateStatus};

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

fn backup(store: &Store) {
    let name = format!("financas-{}.db", store.today);
    let Some(target) = SaveDialog::new(&name).title("Salvar backup").filter("Banco SQLite", &["db"]).ask()
    else {
        return;
    };
    match write_backup(store, &target) {
        Ok(()) => store.toasts.show_for("Backup salvo", target.display().to_string(), Tone::Success, 8.0),
        Err(e) => store.toasts.show_for("Não foi possível fazer o backup", e, Tone::Danger, 8.0),
    }
}

/// The copy goes to a file beside the target and moves over it at the end, so a
/// failure leaves a backup that already exists there as it was.
fn write_backup(store: &Store, target: &Path) -> Result<(), String> {
    if store.path.as_deref() == Some(target) {
        return Err("Esse é o próprio banco em uso. Escolha outro arquivo.".to_owned());
    }
    let draft = target.with_extension("db.parcial");
    // Left by a backup that was interrupted; `backup_to` refuses to write over anything.
    let _ = std::fs::remove_file(&draft);
    store.db.backup_to(&draft).map_err(|e| e.to_string())?;
    std::fs::rename(&draft, target).map_err(|e| e.to_string())
}

pub(crate) fn settings(store: &Store, forms: &Forms, news: &updates::NewsDialog) -> impl View + use<> {
    let path = store.path.as_ref().map_or_else(|| "Só na memória".to_owned(), |p| p.display().to_string());
    let version = store.read(|db| db.schema_version());
    let (backup_store, wipe_store, demo_store) = (store.clone(), store.clone(), store.clone());
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
            "Banco de dados",
            Card::new(
                VStack::new((
                    info_row("Arquivo", path),
                    info_row("Versão do esquema", version.to_string()),
                    info_row("Motor", "SQLite, compilado dentro do app".to_owned()),
                    HStack::new((
                        Button::new("Fazer backup")
                            .icon(IconName::HardDrive)
                            .style(ButtonStyle::Secondary)
                            .on_click(move || backup(&backup_store)),
                        Button::new("Carregar dados de exemplo").style(ButtonStyle::Ghost).on_click(move || {
                            let store = demo_store.clone();
                            demo_confirm.ask(
                                "Trocar tudo pelos dados de exemplo?",
                                "Tudo o que está no banco será apagado e dará lugar a uma família de exemplo.",
                                "Trocar pelos dados de exemplo",
                                move || {
                                    let today = store.today;
                                    store.write("Dados de exemplo carregados", |db| {
                                        db.atomically(|db| {
                                            db.clear()?;
                                            seed::load_demo(db, today)
                                        })
                                    });
                                },
                            );
                        }),
                        Button::new("Começar do zero").icon(IconName::Trash2).style(ButtonStyle::Ghost).on_click(move || {
                            let store = wipe_store.clone();
                            wipe_confirm.ask(
                                "Apagar todos os dados?",
                                "Lançamentos, contas e cadastros serão apagados. Faça um backup antes, se quiser guardá-los.",
                                "Apagar tudo",
                                move || {
                                    store.write("Dados apagados", |db| {
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
