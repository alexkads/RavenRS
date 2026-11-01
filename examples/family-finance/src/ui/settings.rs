//! The database, backups, the look of the app, and updates.

use ravenrs::prelude::*;

use super::forms::Forms;
use super::store::{Store, heading};
use crate::paths;
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
    let folder = paths::exports();
    if let Err(e) = std::fs::create_dir_all(&folder) {
        store.toasts.show("Não foi possível criar a pasta", e.to_string(), Tone::Danger);
        return;
    }
    let target = folder.join(name);
    // A copy does not change what the screens show, so it stays out of `Store::write`.
    match store.db.backup_to(&target) {
        Ok(()) => store.toasts.show_for("Backup salvo", target.display().to_string(), Tone::Success, 8.0),
        Err(e) => store.toasts.show_for("Não foi possível fazer o backup", e.to_string(), Tone::Danger, 8.0),
    }
}

pub(crate) fn settings(store: &Store, forms: &Forms, news_open: &State<bool>) -> impl View + use<> {
    let path = store.path.as_ref().map_or_else(|| "Só na memória".to_owned(), |p| p.display().to_string());
    let version = store.read(|db| db.schema_version());
    let (backup_store, wipe_store, demo_store) = (store.clone(), store.clone(), store.clone());
    let (wipe_confirm, demo_confirm) = (forms.confirm.clone(), forms.confirm.clone());
    let news = news_open.clone();
    let updater = store.updater.clone();
    let update_text = match store.updater.status() {
        UpdateStatus::Quiet => "Você está na versão mais recente que o app conhece.".to_owned(),
        UpdateStatus::Available(n) | UpdateStatus::Development(n) => {
            format!("A versão {} está disponível.", n.versao)
        }
        UpdateStatus::Compiling(n) => format!("Compilando a versão {} em segundo plano.", n.versao),
        UpdateStatus::Ready(n) => format!("A versão {} está instalada; reabra o app para usar.", n.versao),
        UpdateStatus::Failed(n, reason) => format!("A versão {} não foi instalada: {reason}", n.versao),
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
                            .on_click(move || updater.check()),
                        Button::new("Ver novidades").style(ButtonStyle::Ghost).on_click(move || news.set(true)),
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
