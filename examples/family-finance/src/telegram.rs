//! Small Telegram long-polling bridge: transactions said in a message, by text or by voice,
//! or sent as PDF receipts.

use ravenrs::prelude::UiWaker;
use std::collections::HashMap;
use std::fs::{File, OpenOptions, remove_file};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use opus_decoder::OpusDecoder;
use ravenrs_ai::{HEARING_CATALOG, HEARING_RATE, LocalModel, ModelFolder, ModelState, Transcriber};

use crate::db::Database;
use crate::model::{Category, Flow, Member, TransactionInput};
use crate::reminders::{ReminderSettings, TelegramNotice, TelegramNotices};
use crate::today;

const PENDING_TTL: Duration = Duration::from_secs(5 * 60);
/// How often the bot looks for notices to send: bills, budgets, the weekly summary.
const NOTICE_LOOK: Duration = Duration::from_secs(600);
/// How long the last transaction a chat saved can be undone from the chat.
const UNDO_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_PDF_BYTES: u64 = 14 * 1024 * 1024;
/// A voice note is heard up to a minute: enough for "despesa, mercado, foi a Ana".
const MAX_VOICE_SECONDS: usize = 60;
const MAX_VOICE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone)]
struct Config {
    db_path: PathBuf,
    bot_token: String,
}

struct Service {
    config: Config,
    pending: Mutex<HashMap<i64, Pending>>,
    /// The model that hears, kept loaded between voice notes.
    transcriber: Mutex<Option<(&'static str, Transcriber)>>,
    /// The last transaction each chat saved, which `/desfazer` takes back for a while.
    saved: Mutex<HashMap<i64, Saved>>,
}

/// A transaction the bot saved, for `/desfazer`.
struct Saved {
    id: i64,
    summary: String,
    /// Whether its category was created with it, and goes when it goes, if nothing else uses it.
    new_category: bool,
    at: Instant,
}

/// A transaction on its way, one chat at a time.
#[derive(Debug)]
struct Pending {
    amount_cents: i64,
    /// Whether the amount came from a receipt's PDF rather than the message itself.
    from_pdf: bool,
    since: Instant,
    step: Step,
}

#[derive(Debug)]
enum Step {
    /// The amount, waiting for what it was.
    Amount,
    /// A category no one has yet, waiting for the person to pick one or create it.
    Category(Reply),
    /// Everything read, waiting for the person to confirm before anything is saved.
    Confirm(Reply),
}

#[derive(Deserialize)]
struct Update {
    update_id: i64,
    message: Option<Message>,
    callback_query: Option<CallbackQuery>,
}

/// A tap on one of the buttons under the bot's question.
#[derive(Deserialize)]
struct CallbackQuery {
    id: String,
    from: User,
    message: Option<CallbackMessage>,
    data: Option<String>,
}

#[derive(Deserialize)]
struct CallbackMessage {
    message_id: i64,
    chat: Chat,
}

#[derive(Deserialize)]
struct Message {
    chat: Chat,
    from: Option<User>,
    document: Option<Document>,
    text: Option<String>,
    voice: Option<Voice>,
}

#[derive(Deserialize)]
struct Chat {
    id: i64,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct User {
    id: i64,
}

/// A voice note: Opus in an Ogg file, as Telegram records it.
#[derive(Deserialize)]
struct Voice {
    file_id: String,
    duration: Option<u64>,
    file_size: Option<u64>,
}

#[derive(Deserialize)]
struct Document {
    file_id: String,
    mime_type: Option<String>,
    file_size: Option<u64>,
}

#[derive(Deserialize)]
struct TelegramResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct TelegramFile {
    file_path: String,
}

#[derive(Deserialize)]
struct TelegramBot {
    username: Option<String>,
}

/// A cryptographically random, short Telegram deep-link payload.
pub fn pairing_token() -> String {
    Uuid::new_v4().simple().to_string()
}

/// The value persisted for a pairing token; never persist the token itself.
pub fn pairing_hash(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Whether a BotFather token is saved in Ajustes or given in `TELEGRAM_BOT_TOKEN`.
pub fn token_configured() -> bool {
    bot_token().is_ok()
}

/// Saves the user's BotFather token privately beside Raven's local data.
pub fn save_bot_token(token: &str) -> Result<(), String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("cole o token recebido do BotFather".into());
    }
    let directory = crate::paths::home();
    std::fs::create_dir_all(&directory).map_err(|e| format!("não foi possível criar a pasta local: {e}"))?;
    let path = directory.join("telegram_bot_token");
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(0o600);
        let mut file = options.open(&path).map_err(|e| format!("não foi possível salvar o token: {e}"))?;
        file.write_all(token.as_bytes()).map_err(|e| format!("não foi possível salvar o token: {e}"))?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("não foi possível proteger o token: {e}"))?;
    }
    #[cfg(not(unix))]
    {
        let mut file = options.open(&path).map_err(|e| format!("não foi possível salvar o token: {e}"))?;
        file.write_all(token.as_bytes()).map_err(|e| format!("não foi possível salvar o token: {e}"))?;
    }
    Ok(())
}

/// Looks up the bot's public username without exposing its BotFather token to the UI.
pub fn pairing_link(token: &str) -> Result<String, String> {
    let bot_token = bot_token()?;
    let url = format!("{}/bot{bot_token}/getMe", api_base());
    let mut response = telegram_agent().get(&url).call().map_err(|_| "falha ao comunicar com Telegram")?;
    let parsed: TelegramResponse<TelegramBot> = serde_json::from_slice(
        &response.body_mut().read_to_vec().map_err(|_| "resposta inválida do Telegram")?,
    )
    .map_err(|_| "resposta inválida do Telegram")?;
    if !parsed.ok {
        return Err(parsed.description.unwrap_or_else(|| "Telegram API recusou a operação".into()));
    }
    let username = parsed
        .result
        .and_then(|bot| bot.username)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "esse bot do Telegram não tem username público".to_owned())?;
    Ok(format!("https://t.me/{username}?start=rf1_{token}"))
}

/// Long-polls Telegram, so Raven needs no public webhook port.
pub fn serve(db_path: PathBuf) -> Result<(), String> {
    serve_with_waker(db_path, None, None)
}

/// Long-polls Telegram beside the window: wakes the UI after each message and raises
/// `paired` when a chat claims a QR, so the window can confirm it.
pub fn serve_with_waker(
    db_path: PathBuf,
    waker: Option<UiWaker>,
    paired: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Result<(), String> {
    let config = Config { db_path: checked_db_path(db_path)?, bot_token: bot_token()? };
    let service = Service {
        config,
        pending: Mutex::new(HashMap::new()),
        transcriber: Mutex::new(None),
        saved: Mutex::new(HashMap::new()),
    };
    let mut offset = 0;
    let mut looked: Option<Instant> = None;
    loop {
        if looked.is_none_or(|at| at.elapsed() >= NOTICE_LOOK) {
            tell(&service);
            looked = Some(Instant::now());
        }
        if bot_token().is_ok_and(|token| token != service.config.bot_token) {
            return Err("token do Telegram alterado; reconectando".into());
        }
        let updates: Vec<Update> = telegram_call(
            &service,
            "getUpdates",
            json!({"offset":offset,"timeout":25,"allowed_updates":["message","callback_query"]}),
        )?;
        for update in updates {
            offset = update.update_id + 1;
            let message =
                update.message.or_else(|| update.callback_query.and_then(|tap| tapped(&service, tap)));
            if let Some(message) = message
                && message.chat.kind == "private"
                && message.from.is_some()
            {
                let chat = message.chat.id;
                let is_pairing = message.text.as_deref().and_then(start_token).is_some();
                let user = message.from.as_ref().map(|user| user.id).unwrap_or_default();
                if let Err(error) = handle_message(&service, message) {
                    let _ = send_message(&service, chat, &format!("Não consegui processar: {error}"));
                } else if is_pairing
                    && Database::open(&service.config.db_path)
                        .and_then(|db| db.telegram_chat_is_linked(chat, user))
                        .unwrap_or(false)
                    && let Some(paired) = &paired
                {
                    paired.store(true, std::sync::atomic::Ordering::Release);
                }
                if is_pairing {
                    // A chat just connected hears what is due today right away.
                    tell(&service);
                }
                if let Some(waker) = &waker {
                    waker.wake();
                }
            }
        }
    }
}

/// A button tap, as the command it stands for. The buttons go away once tapped, so the
/// same question cannot be answered twice.
fn tapped(service: &Service, tap: CallbackQuery) -> Option<Message> {
    let _: Result<Value, _> =
        telegram_call(service, "answerCallbackQuery", json!({"callback_query_id": tap.id}));
    let message = tap.message?;
    let _: Result<Value, _> = telegram_call(
        service,
        "editMessageReplyMarkup",
        json!({"chat_id": message.chat.id, "message_id": message.message_id, "reply_markup": {"inline_keyboard": []}}),
    );
    let command = match tap.data.as_deref()? {
        "rf:ok" => "/confirmar".to_owned(),
        "rf:fix" => "/corrigir".to_owned(),
        "rf:no" => "/cancelar".to_owned(),
        "rf:new" => "/criar".to_owned(),
        "rf:undo" => "/desfazer".to_owned(),
        data if data.starts_with("rf:aviso:") => {
            let (word, on) = data["rf:aviso:".len()..].split_once(':')?;
            format!("/avisos {word} {}", if on == "on" { "ligar" } else { "desligar" })
        }
        data => format!("/categoria {}", data.strip_prefix("rf:cat:")?.parse::<i64>().ok()?),
    };
    Some(Message {
        chat: message.chat,
        from: Some(tap.from),
        document: None,
        text: Some(command),
        voice: None,
    })
}

fn bot_token() -> Result<String, String> {
    if let Some(token) = std::fs::read_to_string(crate::paths::home().join("telegram_bot_token"))
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        return Ok(token.trim().to_owned());
    }
    if let Some(token) = std::env::var("TELEGRAM_BOT_TOKEN").ok().filter(|v| !v.trim().is_empty()) {
        return Ok(token.trim().to_owned());
    }
    Err("salve o token do bot em Ajustes, no cartão Telegram".to_owned())
}

/// Telegram's Bot API, or the stand-in a test points `TELEGRAM_API_BASE` at.
fn api_base() -> String {
    std::env::var("TELEGRAM_API_BASE")
        .ok()
        .filter(|base| !base.trim().is_empty())
        .map_or_else(|| "https://api.telegram.org".to_owned(), |base| base.trim_end_matches('/').to_owned())
}

/// The file beside the database that keeps the account receipts go to.
fn account_file(db_path: &Path) -> PathBuf {
    db_path.with_extension("telegram-conta")
}

/// The account chosen in Ajustes for this database, if any.
pub fn chosen_account(db_path: &Path) -> Option<i64> {
    std::fs::read_to_string(account_file(db_path)).ok()?.trim().parse().ok()
}

/// Remembers the account the bot's transactions go to.
pub fn choose_account(db_path: &Path, account_id: i64) {
    if let Err(error) = std::fs::write(account_file(db_path), account_id.to_string()) {
        tracing::warn!(%error, "could not save the Telegram account");
    }
}

fn checked_db_path(path: PathBuf) -> Result<PathBuf, String> {
    path.canonicalize().map_err(|e| format!("não foi possível validar o banco: {e}"))
}

fn handle_message(service: &Service, message: Message) -> Result<(), String> {
    let chat = message.chat.id;
    let user = message.from.as_ref().map(|user| user.id).ok_or_else(|| "remetente ausente".to_owned())?;
    if let Some(token) = message.text.as_deref().and_then(start_token) {
        let db = Database::open(&service.config.db_path).map_err(|e| e.to_string())?;
        if db
            .claim_telegram_pairing(&pairing_hash(token), chat, user, unix_seconds())
            .map_err(|e| e.to_string())?
        {
            service.pending.lock().map_err(|_| "estado do bot indisponível")?.remove(&chat);
            send_message(service, chat, "Telegram conectado a este Raven. Agora envie um PDF.")?;
        } else {
            send_message(service, chat, "Esse QR expirou ou já foi usado. Gere outro no Raven.")?;
        }
        return Ok(());
    }
    let linked = Database::open(&service.config.db_path)
        .and_then(|db| db.telegram_chat_is_linked(chat, user))
        .map_err(|e| e.to_string())?;
    if !linked {
        service.pending.lock().map_err(|_| "estado do bot indisponível")?.remove(&chat);
        send_message(service, chat, "Escaneie o QR em Ajustes no Raven para conectar este chat.")?;
        return Ok(());
    }
    if let Some(document) = message.document {
        if document.mime_type.as_deref() != Some("application/pdf")
            || document.file_size.is_some_and(|n| n > MAX_PDF_BYTES)
        {
            send_message(service, chat, "Envie um PDF de até 14 MB.")?;
            return Ok(());
        }
        let bytes = download_pdf(service, &document.file_id)?;
        let amount_cents = extract_total_from_pdf(&bytes)?;
        service.pending.lock().map_err(|_| "estado do bot indisponível")?.insert(
            chat,
            Pending { amount_cents, from_pdf: true, since: Instant::now(), step: Step::Amount },
        );
        send_message(
            service,
            chat,
            &format!(
                "Valor encontrado: R$ {}. Envie `despesa Categoria` ou `receita Categoria`, e quem gastou se quiser, em até 5 minutos.",
                format_brl(amount_cents)
            ),
        )?;
        return Ok(());
    }
    let text = match message.voice {
        Some(voice) => match hear(service, chat, &voice)? {
            Some(text) => Some(text),
            None => return Ok(()),
        },
        None => message.text,
    };
    let Some(text) = text.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    if !text.starts_with('/') {
        let db = Database::open(&service.config.db_path).map_err(|e| e.to_string())?;
        if let Some(answer) = crate::questions::answer(&db, today(), &text) {
            // A question leaves whatever is pending as it was.
            return send_message(service, chat, &answer.map_err(|e| e.to_string())?);
        }
    }
    let said = folded(&text);
    let pending = service.pending.lock().map_err(|_| "estado do bot indisponível")?.remove(&chat);
    let keep = |pending: Pending| -> Result<(), String> {
        service.pending.lock().map_err(|_| "estado do bot indisponível")?.insert(chat, pending);
        Ok(())
    };
    if text == "/avisos" || text.starts_with("/avisos ") {
        if let Some(pending) = pending {
            keep(pending)?;
        }
        return notices_command(service, chat, &text);
    }
    if text == "/desfazer" || said == "desfazer" {
        if let Some(pending) = pending {
            keep(pending)?;
        }
        return undo(service, chat);
    }
    let asking = pending.as_ref().is_some_and(|p| matches!(p.step, Step::Category(_) | Step::Confirm(_)));
    if text == "/cancelar" || (asking && NO_WORDS.contains(&said.as_str())) {
        send_message(service, chat, "Lançamento cancelado.")?;
        return Ok(());
    }
    let expired = pending.as_ref().is_some_and(|p| p.since.elapsed() > PENDING_TTL);
    let pending = pending.filter(|_| !expired);
    let confirmed = text == "/confirmar" || YES_WORDS.contains(&said.as_str());
    let command = confirmed || text.starts_with('/');
    match pending {
        Some(Pending { step: Step::Confirm(reply), amount_cents, from_pdf, since }) if confirmed => {
            let id = match save_transaction(&service.config.db_path, amount_cents, from_pdf, &reply) {
                Ok(id) => id,
                Err(error) => {
                    keep(Pending { amount_cents, from_pdf, since, step: Step::Confirm(reply) })?;
                    return Err(error);
                }
            };
            let new_category = !reply.known;
            let summary = summary(amount_cents, &Reply { known: true, ..reply });
            let rows = [vec![("Desfazer".to_owned(), "rf:undo".to_owned())]];
            send_with_buttons(service, chat, &format!("{summary} salva."), &rows)?;
            let saved = Saved { id, summary, new_category, at: Instant::now() };
            service.saved.lock().map_err(|_| "estado do bot indisponível")?.insert(chat, saved);
        }
        Some(Pending { step: Step::Confirm(_), amount_cents, from_pdf, .. }) if text == "/corrigir" => {
            keep(Pending { amount_cents, from_pdf, since: Instant::now(), step: Step::Amount })?;
            send_message(service, chat, "Envie de novo, por exemplo: `despesa Mercado Ana`.")?;
        }
        Some(Pending { step: Step::Category(reply), amount_cents, from_pdf, since }) if text == "/criar" => {
            ask(service, chat, Pending { amount_cents, from_pdf, since, step: Step::Confirm(reply) })?;
        }
        Some(Pending { step: Step::Category(mut reply), amount_cents, from_pdf, since })
            if text.starts_with("/categoria ") =>
        {
            let id = text["/categoria ".len()..].parse::<i64>().ok();
            let chosen = Database::open(&service.config.db_path)
                .and_then(|db| db.categories())
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|category| Some(category.id) == id && category.flow == reply.flow);
            let step = match chosen {
                Some(category) => {
                    (reply.category, reply.known) = (category.name, true);
                    Step::Confirm(reply)
                }
                None => Step::Category(reply),
            };
            ask(service, chat, Pending { amount_cents, from_pdf, since, step })?;
        }
        pending if !command => match take_amount(&text) {
            Err(question) => {
                if let Some(pending) = pending {
                    keep(pending)?;
                }
                send_message(service, chat, &question)?;
            }
            Ok(Some((amount_cents, rest))) => {
                let said =
                    Pending { amount_cents, from_pdf: false, since: Instant::now(), step: Step::Amount };
                read(service, chat, said, &rest)?;
            }
            Ok(None) => match pending {
                Some(pending) => read(service, chat, pending, &text)?,
                None if expired => {
                    send_message(service, chat, "O prazo desse lançamento terminou. Envie de novo.")?;
                }
                None => send_message(service, chat, NOTHING_WAITING)?,
            },
        },
        Some(pending) => {
            keep(pending)?;
            send_message(service, chat, "Responda com os botões, ou envie `despesa Categoria` de novo.")?;
        }
        None if expired => send_message(service, chat, "O prazo desse lançamento terminou. Envie de novo.")?,
        None => send_message(service, chat, NOTHING_WAITING)?,
    }
    Ok(())
}

/// Takes back the last transaction this chat saved, while it is recent: only that one, and
/// only from the chat that saved it. A category created with it goes too, if unused.
fn undo(service: &Service, chat: i64) -> Result<(), String> {
    let saved = service.saved.lock().map_err(|_| "estado do bot indisponível")?.remove(&chat);
    let Some(saved) = saved else {
        return send_message(service, chat, "Não há lançamento recente deste chat para desfazer.");
    };
    if saved.at.elapsed() > UNDO_TTL {
        return send_message(
            service,
            chat,
            "O prazo para desfazer terminou. Apague o lançamento no Raven, em Lançamentos.",
        );
    }
    let db = Database::open(&service.config.db_path).map_err(|e| e.to_string())?;
    let Some(transaction) = db.transaction(saved.id).map_err(|e| e.to_string())? else {
        return send_message(service, chat, "Esse lançamento já foi apagado no Raven.");
    };
    db.delete_transaction(saved.id).map_err(|e| e.to_string())?;
    if saved.new_category {
        // Refused while another transaction or a bill uses it, and then it stays.
        let _ = db.delete_category(transaction.input.category_id);
    }
    send_message(service, chat, &format!("Desfeito: {} apagada.", saved.summary))
}

/// Sends every paired chat what is due, each notice with a button that turns its kind off.
/// A failure is logged, never fatal: notices are not worth stopping the bot for.
fn tell(service: &Service) {
    let path = &service.config.db_path;
    let file = TelegramNotices::file(path);
    let mut notices = TelegramNotices::load(&file);
    let chosen = ReminderSettings::load(&crate::reminders::settings_file(path));
    let hour = u8::try_from(jiff::Zoned::now().hour()).unwrap_or_default();
    let due = Database::open(path)
        .and_then(|db| Ok((db.telegram_chats()?, notices.due(&db, today(), hour, &chosen)?)));
    let (chats, due) = match due {
        Ok((chats, due)) if !chats.is_empty() => (chats, due),
        Ok(_) => return,
        Err(error) => return tracing::warn!(%error, "could not look for Telegram notices"),
    };
    for (kind, text) in &due {
        let off = [vec![(format!("Desligar: {}", kind.label()), format!("rf:aviso:{}:off", kind.word()))]];
        for chat in &chats {
            if let Err(error) = send_with_buttons(service, *chat, text, &off) {
                tracing::warn!(%error, "could not send a Telegram notice");
            }
        }
    }
    if let Err(error) = notices.save(&file) {
        tracing::warn!(%error, "could not save what the Telegram notices told");
    }
}

/// `/avisos` lists the notices with a button to turn each on or off; `/avisos contas
/// desligar` does it.
fn notices_command(service: &Service, chat: i64, text: &str) -> Result<(), String> {
    let file = TelegramNotices::file(&service.config.db_path);
    let mut notices = TelegramNotices::load(&file);
    let mut words = text.split_whitespace().skip(1);
    if let (Some(word), Some(state)) = (words.next(), words.next()) {
        let kind = TelegramNotice::ALL.into_iter().find(|kind| kind.word() == folded(word));
        let on = match state {
            "ligar" => true,
            "desligar" => false,
            _ => {
                return send_message(
                    service,
                    chat,
                    "Use `/avisos contas ligar` ou `/avisos contas desligar`.",
                );
            }
        };
        let Some(kind) = kind else {
            return send_message(service, chat, "Os avisos são: contas, orcamento e resumo.");
        };
        notices.set(kind, on);
        notices.save(&file)?;
        let done = if on { "ligado" } else { "desligado" };
        return send_message(
            service,
            chat,
            &format!("Aviso {done}: {}. Veja todos em /avisos.", kind.label()),
        );
    }
    let rows: Vec<Vec<(String, String)>> = TelegramNotice::ALL
        .iter()
        .map(|kind| {
            let (label, state) = if notices.is_on(*kind) { ("Desligar", "off") } else { ("Ligar", "on") };
            vec![(format!("{label}: {}", kind.label()), format!("rf:aviso:{}:{state}", kind.word()))]
        })
        .collect();
    let lines: Vec<String> = TelegramNotice::ALL
        .iter()
        .map(|kind| {
            format!("{}: {}", kind.label(), if notices.is_on(*kind) { "ligado" } else { "desligado" })
        })
        .collect();
    let text = format!("Avisos do Raven, a partir da hora escolhida em Ajustes:\n{}", lines.join("\n"));
    send_with_buttons(service, chat, &text, &rows)
}

const NOTHING_WAITING: &str = "Nada aguardando. Para lançar, diga o valor: `gastei 50 no mercado, foi a Ana`. Ou envie o PDF do comprovante.";

/// Reads what `text` says about the pending amount and asks the next question: which
/// category, whether to save, or what is missing, the amount kept.
fn read(service: &Service, chat: i64, pending: Pending, text: &str) -> Result<(), String> {
    let Pending { amount_cents, from_pdf, since, .. } = pending;
    let read = Database::open(&service.config.db_path)
        .and_then(|db| Ok((db.members()?, db.categories()?)))
        .map_err(|e| e.to_string())
        .map(|(members, categories)| read_reply(text, &members, &categories));
    let waiting = Pending { amount_cents, from_pdf, since, step: Step::Amount };
    match read {
        Ok(Ok(reply)) => {
            let step = if reply.known { Step::Confirm(reply) } else { Step::Category(reply) };
            ask(service, chat, Pending { step, ..waiting })
        }
        Ok(Err(question)) => {
            let question = if from_pdf {
                question
            } else {
                format!("Valor: R$ {}. {question}", format_brl(amount_cents))
            };
            service.pending.lock().map_err(|_| "estado do bot indisponível")?.insert(chat, waiting);
            send_message(service, chat, &question)
        }
        Err(error) => {
            service.pending.lock().map_err(|_| "estado do bot indisponível")?.insert(chat, waiting);
            Err(error)
        }
    }
}

const YES_WORDS: &[&str] = &["sim", "confirmar", "confirmo", "confirma", "ok"];
const NO_WORDS: &[&str] = &["nao", "cancelar", "cancela"];
const CONFIRM_BUTTONS: [(&str, &str); 3] =
    [("Confirmar", "rf:ok"), ("Corrigir", "rf:fix"), ("Cancelar", "rf:no")];
/// How many of the most used categories are offered when the one said does not exist.
const CATEGORY_CHOICES: usize = 6;

/// Keeps `pending` and asks its question: confirm what will be saved, or which category,
/// since a category is never created without the person saying so.
fn ask(service: &Service, chat: i64, pending: Pending) -> Result<(), String> {
    let (rows, question) = match &pending.step {
        Step::Category(reply) => {
            let used = Database::open(&service.config.db_path)
                .and_then(|db| db.categories_by_use(reply.flow))
                .map_err(|e| e.to_string())?;
            let mut rows: Vec<Vec<(String, String)>> = used
                .iter()
                .take(CATEGORY_CHOICES)
                .map(|category| (category.name.clone(), format!("rf:cat:{}", category.id)))
                .collect::<Vec<_>>()
                .chunks(2)
                .map(<[_]>::to_vec)
                .collect();
            rows.push(vec![
                (format!("Criar “{}”", reply.category), "rf:new".to_owned()),
                ("Cancelar".to_owned(), "rf:no".to_owned()),
            ]);
            let kind = if reply.flow == Flow::Income { "receitas" } else { "despesas" };
            let pick = if used.is_empty() { "Crie" } else { "Escolha uma das mais usadas ou crie" };
            (rows, format!("Não encontrei a categoria “{}” em {kind}. {pick}:", reply.category))
        }
        Step::Confirm(reply) => {
            let row = CONFIRM_BUTTONS.iter().map(|(label, data)| ((*label).to_owned(), (*data).to_owned()));
            (vec![row.collect()], format!("{}\nConfirma?", summary(pending.amount_cents, reply)))
        }
        Step::Amount => return Err("nada a perguntar".into()),
    };
    service.pending.lock().map_err(|_| "estado do bot indisponível")?.insert(chat, pending);
    send_with_buttons(service, chat, &question, &rows)
}

/// One line with everything that will be saved: "Despesa · Mercado · Ana · R$ 87,50".
fn summary(amount_cents: i64, reply: &Reply) -> String {
    let who = reply.member.as_ref().map(|member| format!(" · {}", member.name)).unwrap_or_default();
    let new = if reply.known { "" } else { " (nova)" };
    format!("{} · {}{new}{who} · R$ {}", reply.flow.label(), reply.category, format_brl(amount_cents))
}

fn start_token(text: &str) -> Option<&str> {
    let token = text.strip_prefix("/start rf1_")?;
    (token.len() == 32 && token.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(token)
}

fn unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

/// What a reply says about a receipt: typed (`despesa Mercado Ana`) or spoken
/// ("despesa de mercado, quem gastou foi a Ana").
#[derive(Debug, PartialEq, Eq)]
struct Reply {
    flow: Flow,
    category: String,
    /// Whether the category already exists; one that does not is asked about first.
    known: bool,
    member: Option<Member>,
}

const EXPENSE_WORDS: &[&str] =
    &["despesa", "gasto", "gastei", "gastou", "paguei", "pagou", "comprei", "comprou"];
const INCOME_WORDS: &[&str] = &["receita", "recebi", "recebeu", "ganhei", "ganhou", "entrada"];
/// Words that only join the others; they are dropped from the ends of a category.
const JOINING_WORDS: &[&str] = &[
    "a",
    "o",
    "as",
    "os",
    "de",
    "da",
    "do",
    "das",
    "dos",
    "com",
    "em",
    "na",
    "no",
    "e",
    "foi",
    "quem",
    "gastou",
    "recebeu",
    "pagou",
    "por",
    "pela",
    "pelo",
    "para",
    "pra",
    "categoria",
    "reais",
    "real",
];
/// What comes before a name nobody registered: "por Pedro", "foi o Pedro", "quem gastou Pedro",
/// "para o Pedro".
const NAME_MARKERS: &[&str] = &["por", "foi", "quem", "para", "pra"];

/// Reads a reply against the people and categories this Raven already has. A name
/// that is not a member is asked about, never created.
fn read_reply(text: &str, members: &[Member], categories: &[Category]) -> Result<Reply, String> {
    let mut words: Vec<String> = text
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()).to_owned())
        .filter(|word| !word.is_empty())
        .collect();
    let keys = |words: &[String]| words.iter().map(|word| folded(word)).collect::<Vec<_>>();

    let found = keys(&words).iter().enumerate().find_map(|(i, key)| {
        if EXPENSE_WORDS.iter().any(|word| sounds_like(key, word, 5)) {
            Some((i, Flow::Expense))
        } else {
            INCOME_WORDS.iter().any(|word| sounds_like(key, word, 5)).then_some((i, Flow::Income))
        }
    });
    let Some((at, flow)) = found else {
        return Err(
            "Diga se é despesa ou receita, por exemplo: `despesa Mercado` ou `despesa Mercado Ana`.".into()
        );
    };
    words.remove(at);

    let member = members
        .iter()
        .filter_map(|member| {
            let name = member.name.split_whitespace().map(folded).collect::<Vec<_>>();
            let said = keys(&words);
            let exact = find_run(&said, &name).map(|at| (at, true));
            let heard = || {
                said.windows(name.len().max(1))
                    .position(|window| window.iter().zip(&name).all(|(a, b)| sounds_like(a, b, 6)))
                    .map(|at| (at, false))
            };
            exact.or_else(heard).map(|(at, exact)| (at, name.len(), exact, member))
        })
        .max_by_key(|(_, len, exact, _)| (*len, *exact));
    let member = match member {
        Some((at, len, _, member)) => {
            words.drain(at..at + len);
            Some(member.clone())
        }
        None => {
            if let Some(name) = unknown_name(&words, categories) {
                return Err(format!(
                    "Não encontrei “{name}” entre os membros do Raven. Cadastre em Membros ou responda sem o nome."
                ));
            }
            None
        }
    };

    words.retain(|word| !word.chars().any(|c| c.is_ascii_digit()) && folded(word) != "r");
    while words.first().is_some_and(|word| JOINING_WORDS.contains(&folded(word).as_str())) {
        words.remove(0);
    }
    while words.last().is_some_and(|word| JOINING_WORDS.contains(&folded(word).as_str())) {
        words.pop();
    }
    if words.is_empty() {
        return Err("Faltou a categoria, por exemplo: `despesa Mercado`.".into());
    }
    let said = keys(&words);
    let known = categories
        .iter()
        .filter(|category| category.flow == flow)
        .filter(|category| {
            let name = category.name.split_whitespace().map(folded).collect::<Vec<_>>();
            find_run(&said, &name).is_some()
        })
        .max_by_key(|category| category.name.len());
    let (category, known) = match known {
        Some(category) => (category.name.clone(), true),
        None => (capitalized(&words.join(" ")), false),
    };
    Ok(Reply { flow, category, known, member })
}

/// The same word, or one of 6 letters or more heard with one letter wrong, missing or
/// extra: Whisper writes "despeza", and "astei" for "gastei". Neither may be shorter than
/// `shortest`: 5 for the keywords, 6 for names, so "Marina" is not taken for Maria.
fn sounds_like(heard: &str, word: &str, shortest: usize) -> bool {
    if heard == word {
        return true;
    }
    let (a, b): (Vec<char>, Vec<char>) = (heard.chars().collect(), word.chars().collect());
    if a.len().max(b.len()) < 6 || a.len().min(b.len()) < shortest || a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let same_start = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let same_end = a.iter().rev().zip(b.iter().rev()).take_while(|(x, y)| x == y).count();
    same_start + same_end + 1 >= a.len().max(b.len())
}

/// Where `run` appears whole in `words`.
fn find_run(words: &[String], run: &[String]) -> Option<usize> {
    if run.is_empty() || run.len() > words.len() {
        return None;
    }
    words.windows(run.len()).position(|window| window == run)
}

/// A name after "por", "foi", "quem", "para" or "pra", at the end of a reply, that no member has.
/// "para" also comes before a place ("para o mercado"), so after it only a capitalized word
/// that names no category is taken for a name.
fn unknown_name(words: &[String], categories: &[Category]) -> Option<String> {
    let keys = words.iter().map(|word| folded(word)).collect::<Vec<_>>();
    let marker = keys.iter().rposition(|key| NAME_MARKERS.contains(&key.as_str()))?;
    let (name, name_keys): (Vec<&str>, Vec<String>) = words[marker + 1..]
        .iter()
        .zip(&keys[marker + 1..])
        .skip_while(|(_, key)| JOINING_WORDS.contains(&key.as_str()))
        .map(|(word, key)| (word.as_str(), key.clone()))
        .unzip();
    if matches!(keys[marker].as_str(), "para" | "pra") {
        let capitalized = name.first().and_then(|word| word.chars().next()).is_some_and(char::is_uppercase);
        let a_category = categories
            .iter()
            .any(|category| category.name.split_whitespace().map(folded).collect::<Vec<_>>() == name_keys);
        if !capitalized || a_category {
            return None;
        }
    }
    (!name.is_empty() && name.len() <= 3).then(|| name.join(" "))
}

/// The amount a message says, in cents, and the message without it: "50", "50,90",
/// "R$ 1.234,56", "2 mil reais", "cinquenta reais e noventa centavos". Spelled-out numbers
/// count only before "reais" or "centavos", since "um" is also "a". Two amounts are asked
/// about, unless only one is marked as money (R$, reais, centavos or a comma).
fn take_amount(text: &str) -> Result<Option<(i64, String)>, String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let keys: Vec<String> = words
        .iter()
        .map(|word| {
            let key = folded(word);
            let key = key.strip_prefix("r$").unwrap_or(&key);
            key.trim_matches(|c: char| !c.is_alphanumeric()).to_owned()
        })
        .collect();
    let mut found = Vec::new();
    let mut at = 0;
    while at < keys.len() {
        match amount_at(&keys, at) {
            Some((cents, end, marked)) => {
                let marked =
                    marked || folded(words[at]).starts_with("r$") || at > 0 && keys[at - 1].is_empty();
                found.push((at, end, cents, marked));
                at = end;
            }
            None => at += 1,
        }
    }
    let marked: Vec<_> = found.iter().filter(|amount| amount.3).collect();
    let (start, end, cents) = match (found.as_slice(), marked.as_slice()) {
        ([], _) => return Ok(None),
        ([(start, end, cents, _)], _) | (_, [(start, end, cents, _)]) => (*start, *end, *cents),
        _ => {
            return Err(
                "Encontrei mais de um valor na mensagem. Diga só um, por exemplo: `gastei 50 no mercado`."
                    .into(),
            );
        }
    };
    let rest = words.iter().enumerate().filter(|(i, _)| !(start..end).contains(i)).map(|(_, word)| *word);
    Ok(Some((cents, rest.collect::<Vec<_>>().join(" "))))
}

/// An amount starting at `keys[at]`: its cents, where it ends, and whether a currency
/// word or a comma marks it as money.
fn amount_at(keys: &[String], at: usize) -> Option<(i64, usize, bool)> {
    let word = |i: usize| keys.get(i).map_or("", String::as_str);
    let (mut cents, mut end, spelled, comma) = match digits_in_cents(word(at)) {
        Some(cents) => (cents, at + 1, false, word(at).contains(',')),
        None => {
            let (units, end) = spelled_number(keys, at)?;
            (units.checked_mul(100)?, end, true, false)
        }
    };
    if !spelled && word(end) == "mil" {
        cents = cents.checked_mul(1000)?;
        end += 1;
    }
    if word(end) == "centavos" {
        return (cents < 10_000).then_some((cents / 100, end + 1, true));
    }
    let currency = matches!(word(end), "reais" | "real" | "conto" | "contos");
    if currency {
        end += 1;
        let small = digits_in_cents(word(end + 1))
            .filter(|_| !word(end + 1).contains(','))
            .map(|cents| (cents / 100, end + 2))
            .or_else(|| spelled_number(keys, end + 1));
        if word(end) == "e"
            && let Some((extra, after)) = small
            && extra < 100
            && word(after) == "centavos"
        {
            cents += extra;
            end = after + 1;
        }
    }
    if spelled && !currency {
        return None;
    }
    (cents > 0 && cents <= 100_000_000).then_some((cents, end, currency || comma))
}

/// "50" → 5000, "50,9" → 5090, "1.234,56" → 123456, "1.234" → 123400, "50.90" → 5090.
fn digits_in_cents(key: &str) -> Option<i64> {
    if !key.starts_with(|c: char| c.is_ascii_digit())
        || !key.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',')
    {
        return None;
    }
    let (whole, fraction) = match key.rsplit_once(',') {
        Some((whole, fraction)) => (whole, fraction),
        None => match key.rsplit_once('.') {
            Some((whole, fraction)) if fraction.len() <= 2 && !whole.contains('.') => (whole, fraction),
            _ => (key, ""),
        },
    };
    if fraction.len() > 2 || fraction.contains('.') || whole.contains(',') {
        return None;
    }
    let whole: i64 = whole.replace('.', "").parse().ok()?;
    let fraction: i64 = if fraction.is_empty() { 0 } else { format!("{fraction:0<2}").parse().ok()? };
    whole.checked_mul(100)?.checked_add(fraction)
}

/// A number spelled out from `keys[at]`, "mil duzentos e trinta e cinco", and where it ends.
fn spelled_number(keys: &[String], at: usize) -> Option<(i64, usize)> {
    let value = |key: &str| -> Option<i64> {
        Some(match key {
            "um" | "uma" => 1,
            "dois" | "duas" => 2,
            "tres" => 3,
            "quatro" => 4,
            "cinco" => 5,
            "seis" => 6,
            "sete" => 7,
            "oito" => 8,
            "nove" => 9,
            "dez" => 10,
            "onze" => 11,
            "doze" => 12,
            "treze" => 13,
            "catorze" | "quatorze" => 14,
            "quinze" => 15,
            "dezesseis" => 16,
            "dezessete" => 17,
            "dezoito" => 18,
            "dezenove" => 19,
            "vinte" => 20,
            "trinta" => 30,
            "quarenta" => 40,
            "cinquenta" => 50,
            "sessenta" => 60,
            "setenta" => 70,
            "oitenta" => 80,
            "noventa" => 90,
            "cem" | "cento" => 100,
            "duzentos" | "duzentas" => 200,
            "trezentos" | "trezentas" => 300,
            "quatrocentos" | "quatrocentas" => 400,
            "quinhentos" | "quinhentas" => 500,
            "seiscentos" | "seiscentas" => 600,
            "setecentos" | "setecentas" => 700,
            "oitocentos" | "oitocentas" => 800,
            "novecentos" | "novecentas" => 900,
            _ => return None,
        })
    };
    let is_number = |i: usize| keys.get(i).is_some_and(|key| key == "mil" || value(key).is_some());
    let (mut total, mut group, mut end) = (0, 0, at);
    while let Some(key) = keys.get(end) {
        if key == "mil" {
            total += group.max(1) * 1000;
            group = 0;
        } else if let Some(units) = value(key) {
            group += units;
        } else if !(key == "e" && end > at && is_number(end + 1)) {
            break;
        }
        end += 1;
    }
    (end > at).then_some((total + group, end))
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

fn format_brl(cents: i64) -> String {
    format!("{:.2}", cents as f64 / 100.0).replace('.', ",")
}

fn telegram_agent() -> ureq::Agent {
    let cfg = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(45))).build();
    cfg.into()
}

fn telegram_call<T: for<'de> Deserialize<'de>>(
    service: &Service,
    method: &str,
    body: Value,
) -> Result<T, String> {
    let agent = telegram_agent();
    let url = format!("{}/bot{}/{method}", api_base(), service.config.bot_token);
    let mut response = agent
        .post(&url)
        .header("content-type", "application/json")
        .send(serde_json::to_vec(&body).map_err(|_| "requisição inválida")?)
        .map_err(|_| "falha ao comunicar com Telegram")?;
    let parsed: TelegramResponse<T> = serde_json::from_slice(
        &response.body_mut().read_to_vec().map_err(|_| "resposta inválida do Telegram")?,
    )
    .map_err(|_| "resposta inválida do Telegram")?;
    if !parsed.ok {
        return Err(parsed.description.unwrap_or_else(|| "Telegram API recusou a operação".into()));
    }
    parsed.result.ok_or_else(|| "Telegram não retornou resultado".into())
}

fn send_message(service: &Service, chat: i64, text: &str) -> Result<(), String> {
    let _: Value = telegram_call(service, "sendMessage", json!({"chat_id":chat,"text":text}))?;
    Ok(())
}

/// A message with rows of buttons, each a label and the `data` it sends back as a callback.
fn send_with_buttons(
    service: &Service,
    chat: i64,
    text: &str,
    rows: &[Vec<(String, String)>],
) -> Result<(), String> {
    let rows: Vec<Vec<Value>> = rows
        .iter()
        .map(|row| row.iter().map(|(label, data)| json!({"text": label, "callback_data": data})).collect())
        .collect();
    let markup = json!({ "inline_keyboard": rows });
    let _: Value =
        telegram_call(service, "sendMessage", json!({"chat_id":chat,"text":text,"reply_markup":markup}))?;
    Ok(())
}

fn download_pdf(service: &Service, file_id: &str) -> Result<Vec<u8>, String> {
    download(service, file_id, MAX_PDF_BYTES)
        .map_err(|why| why.unwrap_or_else(|| "PDF vazio ou maior que 14 MB".into()))
}

/// A file someone sent, up to `limit` bytes. `Err(None)` when it is empty or too big.
fn download(service: &Service, file_id: &str, limit: u64) -> Result<Vec<u8>, Option<String>> {
    let file: TelegramFile = telegram_call(service, "getFile", json!({"file_id":file_id})).map_err(Some)?;
    let agent = telegram_agent();
    let url = format!("{}/file/bot{}/{}", api_base(), service.config.bot_token, file.file_path);
    let mut response = agent.get(&url).call().map_err(|error| {
        Some(format!(
            "falha ao baixar o arquivo do Telegram: {}",
            error.to_string().replace(&service.config.bot_token, "[oculto]")
        ))
    })?;
    let bytes =
        response.body_mut().with_config().limit(limit + 1).read_to_vec().map_err(|e| Some(e.to_string()))?;
    if bytes.is_empty() || bytes.len() as u64 > limit {
        return Err(None);
    }
    Ok(bytes)
}

/// The words of a voice note, said back so the person sees what was understood. `None`
/// when there was nothing to hear it with, or nothing heard; the person is told.
fn hear(service: &Service, chat: i64, voice: &Voice) -> Result<Option<String>, String> {
    if voice.duration.is_some_and(|seconds| seconds > MAX_VOICE_SECONDS as u64)
        || voice.file_size.is_some_and(|bytes| bytes > MAX_VOICE_BYTES)
    {
        send_message(service, chat, "Envie um áudio de até 1 minuto.")?;
        return Ok(None);
    }
    let folder = hearing_folder(&service.config.db_path);
    let Some(model) = model_that_hears(&folder) else {
        send_message(
            service,
            chat,
            "Para entender áudios, baixe e use um modelo em Ajustes, no cartão Telegram. Por enquanto, escreva a resposta.",
        )?;
        return Ok(None);
    };
    send_message(service, chat, "Ouvindo…")?;
    let bytes = download(service, &voice.file_id, MAX_VOICE_BYTES)
        .map_err(|why| why.unwrap_or_else(|| "áudio vazio ou maior que 2 MB".into()))?;
    let samples = voice_samples(&bytes)?;
    let mut kept = service.transcriber.lock().map_err(|_| "estado do bot indisponível")?;
    if kept.as_ref().is_none_or(|(id, _)| *id != model.id) {
        let transcriber = Transcriber::load(&folder, &model).map_err(|e| e.to_string())?;
        *kept = Some((model.id, transcriber));
    }
    let (_, transcriber) = kept.as_mut().ok_or("modelo de áudio indisponível")?;
    let heard = transcriber.transcribe(&samples, "pt").map_err(|e| e.to_string())?;
    if heard.text.is_empty() {
        send_message(service, chat, "Não entendi o áudio. Tente de novo ou escreva a resposta.")?;
        return Ok(None);
    }
    send_message(service, chat, &format!("Entendi: “{}”", heard.text))?;
    Ok(Some(heard.text))
}

/// Where the models that hear live: beside the database, with the other models.
pub fn hearing_folder(db_path: &Path) -> ModelFolder {
    ModelFolder::new(db_path.parent().unwrap_or(Path::new(".")).join("modelos").join("ouvido"))
}

/// The model chosen in Ajustes to hear voice notes, if it is all there.
fn model_that_hears(folder: &ModelFolder) -> Option<LocalModel> {
    let id = std::fs::read_to_string(folder.root().join("in-use")).ok()?;
    let model = HEARING_CATALOG.iter().find(|model| model.id == id.trim())?;
    (folder.state(model) == ModelState::Installed).then_some(*model)
}

/// A voice note as mono samples at the rate Whisper hears: Telegram records Opus in an
/// Ogg file, which is decoded here in Rust, the same on every system.
fn voice_samples(ogg: &[u8]) -> Result<Vec<f32>, String> {
    let unreadable = || "não consegui ler o áudio. Envie de novo ou escreva a resposta.".to_owned();
    std::panic::catch_unwind(|| decode_voice(ogg)).map_err(|_| unreadable())?.ok_or_else(unreadable)
}

fn decode_voice(ogg: &[u8]) -> Option<Vec<f32>> {
    let mut reader = ogg::PacketReader::new(std::io::Cursor::new(ogg));
    let head = reader.read_packet().ok()??;
    if !head.data.starts_with(b"OpusHead") {
        return None;
    }
    let channels = usize::from(*head.data.get(9)?).clamp(1, 2);
    // The samples an encoder put before the sound, counted at 48 kHz.
    let skip =
        usize::from(u16::from_le_bytes([*head.data.get(10)?, *head.data.get(11)?])) * HEARING_RATE / 48_000;
    let mut decoder = OpusDecoder::new(HEARING_RATE as u32, channels).ok()?;
    let mut frame = vec![0.0_f32; OpusDecoder::MAX_FRAME_SIZE_48K * channels];
    let mut samples = Vec::new();
    while let Some(packet) = reader.read_packet().ok()? {
        if packet.data.starts_with(b"OpusTags") {
            continue;
        }
        let decoded = decoder.decode_float(&packet.data, &mut frame, false).ok()?;
        samples.extend(
            frame
                .get(..decoded * channels)?
                .chunks(channels)
                .map(|s| s.iter().sum::<f32>() / channels as f32),
        );
        if samples.len() > (MAX_VOICE_SECONDS + 1) * HEARING_RATE {
            return None;
        }
    }
    Some(samples.split_off(skip.min(samples.len())))
}

/// Finds the one total in a receipt. The text is read in Rust, the same on every system;
/// when that text has no single total and `pdftotext` happens to be installed, its
/// reading gets a second chance.
fn extract_total_from_pdf(pdf: &[u8]) -> Result<i64, String> {
    let ours = text_in_pdf(pdf)
        .ok_or_else(|| "o PDF não tem texto extraível. Envie outro comprovante.".to_owned())
        .and_then(|text| extract_total_from_text(&text));
    match ours {
        Ok(total) => Ok(total),
        Err(reason) => match total_with_pdftotext(pdf) {
            Some(Ok(total)) => Ok(total),
            _ => Err(reason),
        },
    }
}

/// The PDF's text. A malformed file can make the reader panic; that is no text.
fn text_in_pdf(pdf: &[u8]) -> Option<String> {
    std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(pdf).ok()).ok().flatten()
}

/// The total as `pdftotext` reads it, or `None` when it is not installed or fails.
fn total_with_pdftotext(pdf: &[u8]) -> Option<Result<i64, String>> {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    let path = std::env::temp_dir().join(format!("raven-financas-{}-{nonce}.pdf", std::process::id()));
    let written =
        File::options().create_new(true).write(true).open(&path).and_then(|mut file| file.write_all(pdf));
    let output =
        written.is_ok().then(|| Command::new("pdftotext").arg("-layout").arg(&path).arg("-").output());
    let _ = remove_file(&path);
    let output = output?.ok().filter(|output| output.status.success())?;
    Some(extract_total_from_text(&String::from_utf8(output.stdout).ok()?))
}

fn extract_total_from_text(text: &str) -> Result<i64, String> {
    let mut totals = text.lines().filter(|line| is_total_line(line)).flat_map(amounts_in).collect::<Vec<_>>();
    if totals.is_empty() {
        totals = text.lines().flat_map(amounts_after_currency).collect();
    }
    totals.sort_unstable();
    match totals.as_slice() {
        [amount] => Ok(*amount),
        [] => Err("Não encontrei um valor total legível no PDF. Envie outro comprovante.".into()),
        _ => Err("Encontrei mais de um valor total no PDF. Envie outro comprovante.".into()),
    }
}

fn is_total_line(line: &str) -> bool {
    let line = folded(line);
    !line.contains("tribut")
        && ["valor total", "valor a pagar", "total a pagar", "valor pago"]
            .iter()
            .any(|label| line.contains(label))
}

fn amounts_in(text: &str) -> Vec<i64> {
    let mut values = Vec::new();
    let mut token = String::new();
    for character in text.chars().chain(std::iter::once(' ')) {
        if character.is_ascii_digit() || character == '.' || character == ',' {
            token.push(character);
        } else {
            if let Some(value) = cents_in(&token) {
                values.push(value);
            }
            token.clear();
        }
    }
    values
}

fn amounts_after_currency(line: &str) -> Vec<i64> {
    line.split("R$").flat_map(|part| amounts_in(part).into_iter().take(1)).collect()
}

fn cents_in(token: &str) -> Option<i64> {
    let token = token.trim_matches(|c| c == '.' || c == ',');
    let (whole, cents) = token.rsplit_once(',')?;
    if cents.len() != 2 || !cents.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let whole = whole.chars().filter(char::is_ascii_digit).collect::<String>();
    let whole = whole.parse::<i64>().ok()?;
    let cents = cents.parse::<i64>().ok()?;
    let value = whole.checked_mul(100)?.checked_add(cents)?;
    (value > 0 && value <= 100_000_000).then_some(value)
}

fn folded(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

fn save_transaction(path: &Path, amount_cents: i64, from_pdf: bool, reply: &Reply) -> Result<i64, String> {
    let db = Database::open(path).map_err(|e| e.to_string())?;
    let accounts = db.accounts().map_err(|e| e.to_string())?;
    let account_id = match std::env::var("RAVEN_ACCOUNT_ID") {
        Ok(id) => id.parse().map_err(|_| "RAVEN_ACCOUNT_ID inválido")?,
        Err(_) => match (chosen_account(path), accounts.as_slice()) {
            (Some(id), _) => id,
            (None, [only]) => only.id,
            (None, []) => return Err("cadastre uma conta no Raven antes de lançar".into()),
            (None, _) => return Err("escolha em Ajustes, no cartão Telegram, a conta dos lançamentos".into()),
        },
    };
    if !accounts.iter().any(|a| a.id == account_id) {
        return Err("a conta configurada não existe no Raven".into());
    }
    db.atomically(|db| {
        let category_id = db.find_or_create_category(&reply.category, reply.flow)?;
        db.add_transaction(&TransactionInput {
            date: today(),
            description: format!(
                "{} — {}",
                if from_pdf { "Comprovante PDF" } else { "Telegram" },
                reply.category
            ),
            amount_cents,
            flow: reply.flow,
            account_id,
            category_id,
            member_id: reply.member.as_ref().map(|member| member.id),
        })
    })
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(id: i64, name: &str) -> Member {
        Member { id, name: name.into() }
    }

    fn category(id: i64, name: &str, flow: Flow) -> Category {
        Category { id, name: name.into(), flow, budget_cents: 0 }
    }

    #[test]
    fn reads_typed_replies() {
        let read = |text| read_reply(text, &[], &[]);
        let reply = read(" despesa   Café da Casa ").expect("typed");
        assert_eq!(
            (reply.flow, reply.category.as_str(), reply.member),
            (Flow::Expense, "Café da Casa", None)
        );
        assert_eq!(read("receita Salário").expect("income").flow, Flow::Income);
        assert!(read("Mercado").is_err(), "no flow, no guess");
        assert!(read("despesa").is_err(), "no category");
    }

    #[test]
    fn finds_who_spent_among_the_members() {
        let members = [member(1, "Ana"), member(2, "Ana Paula"), member(3, "João")];
        let categories = [category(7, "Mercado", Flow::Expense), category(8, "Mercado", Flow::Income)];
        let read = |text| read_reply(text, &members, &categories).expect(text);

        let typed = read("despesa Mercado Ana");
        assert_eq!((typed.category.as_str(), typed.member), ("Mercado", Some(member(1, "Ana"))));
        assert_eq!(
            read("despesa Mercado Ana Paula").member,
            Some(member(2, "Ana Paula")),
            "the longest name"
        );
        assert_eq!(read("despesa Farmácia joao").member, Some(member(3, "João")), "without the accent");

        let spoken = read("Despesa de mercado, quem gastou foi a Ana.");
        assert_eq!(
            (spoken.flow, spoken.category.as_str()),
            (Flow::Expense, "Mercado"),
            "the existing category"
        );
        assert_eq!(spoken.member, Some(member(1, "Ana")));
        let spoken = read("Gastei 50 reais na farmácia, foi o João");
        assert_eq!((spoken.category.as_str(), spoken.member), ("Farmácia", Some(member(3, "João"))));
        assert_eq!(read("Recebi salário").category, "Salário");
    }

    /// What Whisper wrote in the first real test: "Mateus" for Matheus, after "para".
    #[test]
    fn hears_a_member_with_one_letter_wrong() {
        let members = [member(1, "Matheus"), member(2, "Ana")];
        let categories = [category(7, "Mercado", Flow::Expense)];
        let heard = read_reply("Despeza de mercado para o Mateus.", &members, &categories).expect("heard");
        assert_eq!((heard.category.as_str(), heard.member), ("Mercado", Some(member(1, "Matheus"))));
        let new = read_reply("Despeza de mercado para o Mateus.", &members, &[]).expect("heard");
        assert_eq!(new.category, "Mercado", "a new category without the name");

        let both = [member(1, "Matheus"), member(2, "Mateus")];
        assert_eq!(
            read_reply("despesa Mercado Mateus", &both, &[]).expect("exact").member,
            Some(member(2, "Mateus")),
            "the exact name wins"
        );
        assert!(
            read_reply("despesa Mercado Ane", &members, &[]).is_ok_and(|r| r.member.is_none()),
            "short names must be exact"
        );
    }

    #[test]
    fn hears_a_keyword_with_one_letter_wrong() {
        let reply = read_reply("Despeza de padaria", &[], &[]).expect("heard");
        assert_eq!((reply.flow, reply.category.as_str()), (Flow::Expense, "Padaria"));
        assert_eq!(read_reply("Receitta Salário", &[], &[]).expect("heard").flow, Flow::Income);
        assert!(read_reply("gasta padaria", &[], &[]).is_err(), "short words must be exact");
        assert_eq!(
            read_reply("Astei no mercado", &[], &[]).expect("heard").flow,
            Flow::Expense,
            "a letter missing"
        );
        assert!(read_reply("dispensa padaria", &[], &[]).is_err(), "two letters off is another word");
        assert!(
            sounds_like("mateus", "matheus", 6) && !sounds_like("marina", "maria", 6),
            "names: 6 letters"
        );
    }

    #[test]
    fn asks_about_a_name_nobody_registered() {
        let members = [member(1, "Ana")];
        let question = read_reply("despesa de mercado, foi o Pedro", &members, &[]).expect_err("unknown");
        assert!(question.contains("Pedro"), "{question}");
        let question = read_reply("despesa de mercado para o Pedro", &members, &[]).expect_err("unknown");
        assert!(question.contains("Pedro"), "{question}");
        let question = read_reply("despesa de farmácia pra Pedro", &members, &[]).expect_err("unknown");
        assert!(question.contains("Pedro"), "{question}");
        assert_eq!(
            read_reply("despesa Mercado Pedro", &members, &[]).expect("typed").category,
            "Mercado Pedro"
        );
    }

    #[test]
    fn says_when_a_category_is_new() {
        let categories = [category(7, "Mercado", Flow::Expense), category(8, "Salário", Flow::Income)];
        let read = |text| read_reply(text, &[], &categories).expect(text);
        assert!(read("despesa de mercado").known);
        assert!(!read("despesa Feira").known, "asked about before it is created");
        assert!(!read("despesa Salário").known, "an income category is not an expense one");
        let new = read("despesa Feira");
        assert_eq!(summary(1_000, &new), "Despesa · Feira (nova) · R$ 10,00");
    }

    /// "para" comes before a person and before a place: the effect #84 left behind.
    #[test]
    fn para_before_a_place_is_a_category() {
        let members = [member(1, "Ana")];
        let place = read_reply("despesa para o mercado", &members, &[]).expect("a place");
        assert_eq!((place.category.as_str(), place.known, place.member), ("Mercado", false, None));
        let known = [category(7, "Mercado", Flow::Expense)];
        let spoken = read_reply("Despesa para o Mercado", &members, &known).expect("a category");
        assert_eq!(
            (spoken.category.as_str(), spoken.known),
            ("Mercado", true),
            "capitalized, but a category"
        );
        assert!(read_reply("despesa para o Pedro", &members, &known).is_err(), "a name is still asked about");
        assert_eq!(
            read_reply("despesa de mercado para a Ana", &members, &known).expect("member").member,
            Some(member(1, "Ana"))
        );
    }

    #[test]
    fn reads_the_amount_said_in_a_message() {
        let amount = |text: &str| take_amount(text).expect(text).map(|(cents, _)| cents);
        assert_eq!(amount("gastei 50 no mercado, foi a Ana"), Some(5_000));
        assert_eq!(amount("gastei 50,90 no mercado"), Some(5_090));
        assert_eq!(amount("paguei R$ 1.234,56 de aluguel"), Some(123_456));
        assert_eq!(amount("paguei R$1.234,56 de aluguel"), Some(123_456));
        assert_eq!(amount("despesa 1.234 aluguel"), Some(123_400), "a dot before three digits is thousands");
        assert_eq!(amount("despesa 50.9 padaria"), Some(5_090), "Whisper may write a dot for the comma");
        assert_eq!(amount("recebi 2 mil reais de salário"), Some(200_000));
        assert_eq!(amount("gastei cinquenta reais no mercado"), Some(5_000));
        assert_eq!(amount("gastei cento e vinte e cinco reais e cinquenta centavos"), Some(12_550));
        assert_eq!(amount("paguei mil e duzentos reais de aluguel"), Some(120_000));
        assert_eq!(amount("gastei 50 reais e 90 centavos"), Some(5_090));
        assert_eq!(amount("noventa centavos de bala"), Some(90));
        assert_eq!(amount("Gastei um real de bala."), Some(100));
        assert_eq!(amount("despesa de uma pizza"), None, "\"uma\" is an article without reais");
        assert_eq!(amount("despesa Mercado Ana"), None);
        assert_eq!(amount("comprei 2 pães por R$ 9,80"), Some(980), "only one is money");
        assert!(take_amount("gastei 50 no mercado e 30 na farmácia").is_err(), "two amounts are asked about");
        assert_eq!(amount("despesa 0 mercado"), None);

        let (_, rest) = take_amount("Gastei R$ 50,90 no mercado, foi a Ana").expect("read").expect("amount");
        assert_eq!(rest, "Gastei R$ no mercado, foi a Ana");
        let members = [member(1, "Ana")];
        let reply = read_reply(&rest, &members, &[category(7, "Mercado", Flow::Expense)]).expect("read");
        assert_eq!(
            (reply.flow, reply.category.as_str(), reply.member),
            (Flow::Expense, "Mercado", Some(member(1, "Ana")))
        );
        let (_, rest) = take_amount("gastei cinquenta reais na farmácia").expect("read").expect("amount");
        assert_eq!(read_reply(&rest, &[], &[]).expect("read").category, "Farmácia");
    }

    const VOICE_NOTE: &[u8] = include_bytes!("../tests/fixtures/voz-despesa.ogg");
    const SPENT_NOTE: &[u8] = include_bytes!("../tests/fixtures/voz-gasto.ogg");

    #[test]
    fn decodes_a_telegram_voice_note() {
        let samples = voice_samples(VOICE_NOTE).expect("decoded");
        let seconds = samples.len() as f32 / HEARING_RATE as f32;
        assert!((3.5..4.0).contains(&seconds), "{seconds} s, as the file says 3.8 s");
        let loudness = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        assert!(loudness > 0.01, "someone speaks: {loudness}");
        assert!(voice_samples(b"OggS not really").is_err());
        assert!(voice_samples(&[]).is_err());
    }

    /// Hears the voice note with a real model, when its folder is given: the folder of
    /// Ajustes, with the model in use (`modelos/ouvido`). Whisper Small passes; Base
    /// mishears this synthetic voice.
    #[test]
    fn hears_a_voice_note_and_reads_it() {
        let Some(root) = std::env::var_os("RAVEN_WHISPER_FOLDER") else {
            return;
        };
        let folder = ModelFolder::new(root);
        let model = model_that_hears(&folder).expect("a model in use");
        let mut transcriber = Transcriber::load(&folder, &model).expect("loaded");
        let heard =
            transcriber.transcribe(&voice_samples(VOICE_NOTE).expect("decoded"), "pt").expect("heard");
        println!("{}: “{}” in {:.1} s", model.name, heard.text, heard.seconds);
        let members = [member(1, "Ana")];
        let categories = [category(7, "Mercado", Flow::Expense)];
        let reply = read_reply(&heard.text, &members, &categories).expect("read");
        assert_eq!((reply.flow, reply.category.as_str()), (Flow::Expense, "Mercado"), "{}", heard.text);
        assert_eq!(reply.member, Some(member(1, "Ana")), "{}", heard.text);

        // "Gastei cinquenta reais no mercado, foi a Ana.", with no receipt: Base writes
        // "Astei 50 reais no mercado, foi a...", and the amount, the flow and the category hold.
        let heard =
            transcriber.transcribe(&voice_samples(SPENT_NOTE).expect("decoded"), "pt").expect("heard");
        println!("{}: “{}” in {:.1} s", model.name, heard.text, heard.seconds);
        let (amount, rest) = take_amount(&heard.text).expect("one amount").expect("an amount");
        let reply = read_reply(&rest, &members, &categories).expect("read");
        assert_eq!(
            (amount, reply.flow, reply.category.as_str()),
            (5_000, Flow::Expense, "Mercado"),
            "{}",
            heard.text
        );
    }

    #[test]
    fn extracts_total_from_text() {
        assert_eq!(extract_total_from_text("Comprovante de Pix\nR$ 3.000,00").unwrap(), 300_000);
        assert!(extract_total_from_text("R$ 10,00\nR$ 20,00").is_err());
        assert!(extract_total_from_text("R$ 10,00\nR$ 10,00").is_err());
    }

    #[test]
    fn extracts_supplied_pdf_receipts() {
        let Some(folder) = std::env::var_os("RAVEN_TELEGRAM_ACCEPTANCE_PDFS") else {
            return;
        };
        for (name, expected) in [
            ("shareFile.pdf", 3_999),
            ("shareFile (1).pdf", 10_000),
            ("shareFile (2).pdf", 54_358),
            ("shareFile (3).pdf", 300_000),
        ] {
            let pdf = std::fs::read(Path::new(&folder).join(name)).expect("supplied PDF");
            assert_eq!(extract_total_from_pdf(&pdf).expect("total"), expected, "{name}");
        }
    }

    #[test]
    fn creates_or_reuses_normalized_category() {
        let db = Database::in_memory().expect("database");
        let first = db.find_or_create_category("  Café   da Casa ", Flow::Expense).expect("create");
        let same = db.find_or_create_category("cafe da casa", Flow::Expense).expect("reuse");
        let income = db.find_or_create_category("CAFÉ DA CASA", Flow::Income).expect("other flow");
        assert_eq!(first, same);
        assert_ne!(first, income);
        assert_eq!(db.categories().expect("categories").len(), 2);
    }

    #[test]
    fn pairing_expires_after_ten_minutes() {
        let db = Database::in_memory().expect("database");
        let hash = pairing_hash("expired");
        db.begin_telegram_pairing(&hash, 1_000).expect("pairing");
        assert!(!db.claim_telegram_pairing(&hash, 10, 10, 1_600).expect("expired"));
        assert!(!db.telegram_chat_is_linked(10, 10).expect("not linked"));
    }

    #[test]
    fn pairing_token_is_single_use() {
        let db = Database::in_memory().expect("database");
        let hash = pairing_hash("one-use");
        db.begin_telegram_pairing(&hash, 1_000).expect("pairing");
        assert!(db.claim_telegram_pairing(&hash, 10, 10, 1_001).expect("first use"));
        assert!(!db.claim_telegram_pairing(&hash, 20, 20, 1_002).expect("second use"));
        assert!(db.telegram_chat_is_linked(10, 10).expect("linked"));
    }

    #[test]
    fn revoking_pairing_disconnects_the_chat() {
        let db = Database::in_memory().expect("database");
        let hash = pairing_hash("revoke");
        db.begin_telegram_pairing(&hash, 1_000).expect("pairing");
        assert!(db.claim_telegram_pairing(&hash, 10, 10, 1_001).expect("link"));
        db.revoke_telegram_pairings(1_002).expect("revoke");
        assert!(!db.telegram_chat_is_linked(10, 10).expect("revoked"));
    }

    #[test]
    fn unlinked_chat_is_rejected() {
        let db = Database::in_memory().expect("database");
        let hash = pairing_hash("linked");
        db.begin_telegram_pairing(&hash, 1_000).expect("pairing");
        assert!(db.claim_telegram_pairing(&hash, 10, 10, 1_001).expect("link"));
        assert!(!db.telegram_chat_is_linked(20, 20).expect("unlinked"));
    }
}
