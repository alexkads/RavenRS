//! The database, backups, the look of the app, and updates.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use ravenrs::native::{OpenDialog, SaveDialog, open_with_system};
use ravenrs::prelude::*;

use super::forms::Forms;
use super::help;
use super::store::{PAGE_INSETS, Page, Store, content_width, titled_card};
use super::updates;
use crate::backup::{self, CopyReason};
use crate::db::{Database, DbResult};
use crate::model::{date_label, parse_date};
use crate::seed;
use crate::telegram;
use crate::update::{self, UpdateStatus};

/// Below this width the theme's three choices no longer fit in half of it.
const TWO_COLUMNS_FROM: f32 = 760.0;
/// How many of the kept copies the page lists; the newest are the ones that matter.
const COPIES_SHOWN: usize = 5;

enum TelegramVisual {
    Qr(String),
    Connected,
}

impl View for TelegramVisual {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (node, theme) = match self {
            Self::Qr(link) => (
                Barcode::qr(link.clone())
                    .size(180.0, 180.0)
                    .alt("QR para conectar o Telegram a este Raven")
                    .build(cx),
                true,
            ),
            Self::Connected => {
                (Icon::new(IconName::CircleCheck).size(72.0).color(Semantic::Success).build(cx), false)
            }
        };
        let motion = if theme {
            Motion::new()
                .enter(Effect::hidden(), cx.theme().motion.gentle)
                .exit(Effect::hidden(), cx.theme().motion.fast)
        } else {
            Motion::new()
                .enter(Effect::hidden().with_scale(0.7), cx.theme().motion.snappy)
                .exit(Effect::hidden().with_scale(0.7), cx.theme().motion.fast)
        };
        cx.motion(node, motion);
        node
    }
}

fn info_row(label: &str, value: String) -> impl View + use<> {
    HStack::new((
        Text::new(label.to_owned()).style(TextStyle::Callout).foreground(Semantic::InkMuted).width(150.0),
        Text::new(value).style(TextStyle::Callout),
    ))
    .spacing(8.0)
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
fn ask_to_restore(store: &Store, confirm: &ConfirmDialog, file: PathBuf, from: &str) {
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

fn copy_rows(store: &Store, confirm: &ConfirmDialog) -> Vec<ListItem> {
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
    telegram_link: &State<Option<String>>,
    telegram_token: &State<String>,
    telegram_token_saved: &State<bool>,
    models: Option<ravenrs_ai::ModelManager>,
    voices: Option<ravenrs_ai::VoiceManager>,
    window: Size,
) -> Page {
    let columns = if content_width(window) < TWO_COLUMNS_FROM { 1 } else { 2 };
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

    let appearance = titled_card("Aparência", help::APARENCIA, ThemeSwitcher);
    let performance = titled_card(
        "Desempenho",
        help::DESEMPENHO,
        VStack::new((
            Text::new(
                "Está achando o app lento? O teste abre o Painel, redesenha a janela por 5 segundos e diz o resultado.",
            )
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted),
            Button::new("Testar desempenho")
                .icon(IconName::Activity)
                .size(ButtonSize::Small)
                .style(ButtonStyle::Secondary)
                .on_click(move || {
                    painel.set(0);
                    diagnostics.start_test();
                }),
        ))
        .spacing(10.0)
        .align(Align::Start),
    );
    let assistant = models.map(|models| {
        titled_card(
            "Assistente com IA",
            help::ASSISTENTE,
            VStack::new((
                Text::new(
                    "Opcional. Sem modelo, o assistente acha os momentos dos filmes. Com um modelo, ele também responde sobre saldo, gastos e contas, abre telas e prepara lançamentos, que só são gravados quando você confere e salva. Ele roda neste computador, nada sai daqui, e os números vêm direto do seu banco de dados. Aparece só o que cabe nesta máquina: baixe, teste e remova se ficar lento. Acerto é quantos pedidos o modelo entendeu nos nossos testes; quando ele não entende, o assistente procura nos filmes.",
                )
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
                models,
            ))
            .spacing(10.0)
            .align(Align::Stretch),
        )
    });
    let voice = voices.map(|voices| {
        titled_card(
            "Voz do assistente",
            help::VOZ,
            VStack::new((
                Text::new(
                    "Opcional. Com o LFM3 Audio, o assistente lê as respostas em voz alta, gerando a fala neste computador. Nada sai daqui.",
                )
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
                voices,
            ))
            .spacing(10.0)
            .align(Align::Stretch),
        )
    });
    let backups = titled_card(
        "Backup",
        help::BACKUP,
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
                    .size(ButtonSize::Small)
                    .on_click(move || make_backup(&backup_store)),
                Button::new("Restaurar backup")
                    .icon(IconName::Upload)
                    .size(ButtonSize::Small)
                    .style(ButtonStyle::Secondary)
                    .on_click(move || {
                        let chosen = OpenDialog::new()
                            .title("Escolha o arquivo de backup")
                            .filter("Backup do Raven Finanças", &["db"])
                            .ask();
                        if let Some(file) = chosen {
                            ask_to_restore(&restore_store, &restore_confirm, file, "Esse backup");
                        }
                    }),
            ))
            .spacing(8.0)
            .wrap(),
        ))
        .spacing(12.0)
        .align(Align::Stretch),
    );
    let copies = (!rows.is_empty()).then(|| {
        titled_card(
            "Cópias automáticas",
            help::COPIAS,
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
    });
    let data = titled_card(
        "Onde seus dados ficam",
        help::DADOS,
        VStack::new((
            info_row("Arquivo", path),
            Disclosure::new(
                "Detalhes técnicos",
                VStack::new((
                    info_row("Versão do esquema", version.to_string()),
                    info_row("Motor", "SQLite, compilado dentro do app".to_owned()),
                ))
                .spacing(10.0),
            ),
            folder.map(|folder| {
                Button::new("Abrir a pasta")
                    .icon(IconName::FolderOpen)
                    .size(ButtonSize::Small)
                    .style(ButtonStyle::Ghost)
                    .on_click(move || {
                        if let Err(e) = open_with_system(&folder) {
                            tracing::warn!(error = %e, "could not open the folder");
                        }
                    })
            }),
        ))
        .spacing(10.0)
        .align(Align::Start),
    );
    let updates_card = titled_card(
        "Atualização",
        help::ATUALIZACAO,
        VStack::new((
            info_row("Versão instalada", update::CURRENT.to_owned()),
            Text::new(update_text).style(TextStyle::Callout),
            Text::new(
                "O app baixa o código e compila neste computador. A versão atual só é trocada depois de a nova provar que abre.",
            )
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted),
            HStack::new((
                Button::new("Procurar atualização")
                    .icon(IconName::RefreshCw)
                    .size(ButtonSize::Small)
                    .style(ButtonStyle::Secondary)
                    .on_click(move || updater.ask()),
                Button::new("Ver novidades")
                    .size(ButtonSize::Small)
                    .style(ButtonStyle::Ghost)
                    .on_click(move || news.show_latest()),
            ))
            .spacing(8.0),
        ))
        .spacing(10.0)
        .align(Align::Stretch),
    );
    let _revision = store.version();
    let linked = store.read(|db| db.has_telegram_pairing());
    let (pair_store, revoke_store, pairing, revoke_link) =
        (store.clone(), store.clone(), telegram_link.clone(), telegram_link.clone());
    let (token_input, token_typed, save_token, token_store) =
        (telegram_token.clone(), telegram_token.clone(), telegram_token_saved.clone(), store.clone());
    let saved = telegram_token_saved.get();
    let shown_link = telegram_link.get();
    let telegram = titled_card(
        "Telegram",
        help::TELEGRAM,
        VStack::new((
            Text::new(
                "Envie seus comprovantes em PDF pelo Telegram. Depois, responda se é despesa ou receita e informe a categoria.",
            )
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted),
            Text::new(
                "No Telegram, abra @BotFather, envie /newbot e siga as instruções para criar seu bot. Cole aqui o token recebido; ele fica salvo somente neste computador. Depois, conecte e escaneie o QR code.",
            )
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted),
            HStack::new((
                TextField::new(token_input.binding())
                    .secure()
                    .placeholder("Token do bot recebido do BotFather")
                    .stretch()
                    .grow(1.0),
                Button::new("Salvar token")
                    .icon(IconName::Check)
                    .size(ButtonSize::Small)
                    .style(ButtonStyle::Secondary)
                    .on_click(move || match telegram::save_bot_token(&token_typed.get()) {
                        Ok(()) => {
                            token_typed.set(String::new());
                            save_token.set(true);
                        }
                        Err(error) => {
                            token_store.toasts.show_for(
                                "Não foi possível salvar o token",
                                error,
                                Tone::Danger,
                                8.0,
                            );
                        }
                    }),
            ))
            .spacing(8.0)
            .align(Align::Center),
            Text::new(if saved { "Token salvo neste computador." } else { "Informe e salve o token antes de conectar." })
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
            Text::new(if linked { "Um chat está conectado." } else { "Nenhum chat conectado." })
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
            if linked {
                Some(Box::new(TelegramVisual::Connected) as AnyView)
            } else {
                shown_link.clone().map(|link| Box::new(TelegramVisual::Qr(link)) as AnyView)
            },
            HStack::new((
                Button::new(if shown_link.is_some() { "Gerar novo QR" } else { "Conectar Telegram" })
                    .icon(IconName::Link)
                    .size(ButtonSize::Small)
                    .disabled(!saved)
                    .on_click(move || {
                        let token = telegram::pairing_token();
                        let link = match telegram::pairing_link(&token) {
                            Ok(link) => link,
                            Err(error) => {
                                pair_store.toasts.show_for(
                                    "Não foi possível conectar Telegram",
                                    error,
                                    Tone::Danger,
                                    8.0,
                                );
                                return;
                            }
                        };
                        let now = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map(|duration| duration.as_secs() as i64)
                            .unwrap_or_default();
                        if pair_store
                            .write("Escaneie o QR com o Telegram.", |db| {
                                db.begin_telegram_pairing(&telegram::pairing_hash(&token), now)
                            })
                            .is_some()
                        {
                            pairing.set(Some(link));
                        }
                    }),
                linked.then(|| {
                    Button::new("Desvincular")
                        .icon(IconName::X)
                        .size(ButtonSize::Small)
                        .style(ButtonStyle::Secondary)
                        .on_click(move || {
                            let now = SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .map(|duration| duration.as_secs() as i64)
                                .unwrap_or_default();
                            if revoke_store
                                .write("Telegram desvinculado.", |db| db.revoke_telegram_pairings(now))
                                .is_some()
                            {
                                revoke_link.set(None);
                            }
                        })
                }),
            ))
            .spacing(8.0)
            .wrap(),
        ))
        .spacing(12.0)
        .align(Align::Start),
    );
    let start_over = titled_card(
        "Recomeçar",
        help::RECOMECAR,
        VStack::new((
            Text::new("Trocam tudo o que está no app. Antes, ele guarda uma cópia, que aparece em Cópias automáticas.")
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
            HStack::new((
                Button::new("Carregar dados de exemplo").size(ButtonSize::Small).style(ButtonStyle::Secondary).on_click(move || {
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
                Button::new("Começar do zero").icon(IconName::Trash2).size(ButtonSize::Small).style(ButtonStyle::Secondary).on_click(move || {
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
    );

    Page::new(
        help::AJUSTES,
        "Onde seus dados ficam e como o app se atualiza.",
        ScrollView::new((VStack::new((
            backups,
            Grid::new(columns, (appearance, performance)).spacing(16.0),
            assistant,
            voice,
            copies,
            Grid::new(columns, (data, updates_card)).spacing(16.0),
            telegram,
            start_over,
        ))
        .spacing(16.0)
        .align(Align::Stretch),))
        .padding(PAGE_INSETS),
    )
}
