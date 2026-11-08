//! Small Telegram long-polling bridge: transactions said in a message, by text or by voice,
//! or sent as PDF receipts.

use ravenrs::prelude::UiWaker;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use ravenrs_ai::Transcriber;

use crate::db::Database;

mod api;
mod chat;
mod reading;
mod voice;

use api::{
    CallbackQuery, Message, TelegramBot, TelegramResponse, Update, send_message, telegram_agent,
    telegram_call,
};
use chat::{handle_message, tell};
use reading::Reply;
mod receipts;

pub use voice::hearing_folder;

const PENDING_TTL: Duration = Duration::from_secs(5 * 60);
/// A message older than this when the bot starts waited for it: the app was closed.
const LATE_AFTER: i64 = 60;
/// The most updates Telegram hands over at once; fewer means nothing more is waiting.
const UPDATES_AT_ONCE: usize = 100;
/// How often the bot looks for notices to send: bills, budgets, the weekly summary.
const NOTICE_LOOK: Duration = Duration::from_secs(600);
/// How long the last transaction a chat saved can be undone from the chat.
const UNDO_TTL: Duration = Duration::from_secs(5 * 60);

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
    let started = unix_seconds();
    // The messages that waited while the app was closed, per chat: how many, and the first.
    let mut late: HashMap<i64, (usize, i64)> = HashMap::new();
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
        let drained = updates.len() < UPDATES_AT_ONCE;
        for update in updates {
            offset = update.update_id + 1;
            if let Some(message) = &update.message
                && let Some(date) = message.date.filter(|date| *date < started - LATE_AFTER)
            {
                let (count, first) = late.entry(message.chat.id).or_insert((0, date));
                *count += 1;
                *first = (*first).min(date);
            }
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
        if drained && !late.is_empty() {
            tell_late(&service, std::mem::take(&mut late));
        }
    }
}

/// Tells each paired chat how many of its messages waited while the app was closed and
/// were answered just now, so a late answer is not taken for a new one.
fn tell_late(service: &Service, late: HashMap<i64, (usize, i64)>) {
    let chats = match Database::open(&service.config.db_path).and_then(|db| db.telegram_chats()) {
        Ok(chats) => chats,
        Err(error) => return tracing::warn!(%error, "could not read the Telegram chats"),
    };
    for (chat, (count, first)) in late.into_iter().filter(|(chat, _)| chats.contains(chat)) {
        let when = jiff::Timestamp::from_second(first)
            .map(|at| at.to_zoned(jiff::tz::TimeZone::system()).strftime("%d/%m às %H:%M").to_string())
            .unwrap_or_default();
        let messages = if count == 1 { "1 mensagem".to_owned() } else { format!("{count} mensagens") };
        let text = format!(
            "O Raven estava fechado: respondi agora {messages} que chegaram enquanto isso, desde {when}. Confira as respostas acima."
        );
        if let Err(error) = send_message(service, chat, &text) {
            tracing::warn!(%error, "could not tell about the late messages");
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
        date: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::{chat::*, reading::*, receipts::*, voice::*};
    use crate::model::{Category, Flow, Member};
    use ravenrs_ai::{HEARING_RATE, ModelFolder};

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

    const VOICE_NOTE: &[u8] = include_bytes!("../../tests/fixtures/voz-despesa.ogg");
    const SPENT_NOTE: &[u8] = include_bytes!("../../tests/fixtures/voz-gasto.ogg");

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
        db.begin_telegram_pairing(&hash, None, 1_000).expect("pairing");
        assert!(!db.claim_telegram_pairing(&hash, 10, 10, 1_600).expect("expired"));
        assert!(!db.telegram_chat_is_linked(10, 10).expect("not linked"));
    }

    #[test]
    fn pairing_token_is_single_use() {
        let db = Database::in_memory().expect("database");
        let hash = pairing_hash("one-use");
        db.begin_telegram_pairing(&hash, None, 1_000).expect("pairing");
        assert!(db.claim_telegram_pairing(&hash, 10, 10, 1_001).expect("first use"));
        assert!(!db.claim_telegram_pairing(&hash, 20, 20, 1_002).expect("second use"));
        assert!(db.telegram_chat_is_linked(10, 10).expect("linked"));
    }

    #[test]
    fn revoking_pairing_disconnects_the_chat() {
        let db = Database::in_memory().expect("database");
        let hash = pairing_hash("revoke");
        db.begin_telegram_pairing(&hash, None, 1_000).expect("pairing");
        assert!(db.claim_telegram_pairing(&hash, 10, 10, 1_001).expect("link"));
        db.revoke_telegram_chat(10, 1_002).expect("revoke");
        assert!(!db.telegram_chat_is_linked(10, 10).expect("revoked"));
    }

    /// One chat per person: each QR is for a member, and a new one leaves the others linked.
    #[test]
    fn several_chats_stay_linked_each_with_its_member() {
        let db = Database::in_memory().expect("database");
        let ana = db.add_member("Ana").expect("member");
        let bruno = db.add_member("Bruno").expect("member");
        db.begin_telegram_pairing(&pairing_hash("ana"), Some(ana), 1_000).expect("pairing");
        assert!(db.claim_telegram_pairing(&pairing_hash("ana"), 10, 10, 1_001).expect("Ana's chat"));
        db.begin_telegram_pairing(&pairing_hash("bruno"), Some(bruno), 1_002).expect("pairing");
        assert!(db.claim_telegram_pairing(&pairing_hash("bruno"), 20, 20, 1_003).expect("Bruno's chat"));
        assert!(db.telegram_chat_is_linked(10, 10).expect("Ana still linked"), "a new QR leaves the others");
        assert!(db.telegram_chat_is_linked(20, 20).expect("Bruno linked"));
        assert_eq!(db.telegram_chat_member(10).expect("read").map(|m| m.name), Some("Ana".to_owned()));
        assert_eq!(db.telegram_chat_member(20).expect("read").map(|m| m.name), Some("Bruno".to_owned()));
        assert_eq!(db.telegram_chats().expect("chats"), [10, 20]);

        // Still no access without a QR, nor from another account in a linked chat's place.
        assert!(!db.telegram_chat_is_linked(30, 30).expect("unlinked"));
        assert!(!db.telegram_chat_is_linked(10, 99).expect("another user"));
        // An unclaimed QR is replaced by the next one, as before.
        db.begin_telegram_pairing(&pairing_hash("old"), None, 1_004).expect("pairing");
        db.begin_telegram_pairing(&pairing_hash("new"), None, 1_005).expect("pairing");
        assert!(!db.claim_telegram_pairing(&pairing_hash("old"), 30, 30, 1_006).expect("replaced"));
        assert!(db.telegram_qr_waiting(1_006).expect("read"));
        assert!(!db.telegram_qr_waiting(1_005 + 600).expect("read"), "ten minutes");
    }

    #[test]
    fn unlinking_one_chat_leaves_the_others() {
        let db = Database::in_memory().expect("database");
        let ana = db.add_member("Ana").expect("member");
        for (code, chat) in [("a", 10), ("b", 20)] {
            db.begin_telegram_pairing(&pairing_hash(code), Some(ana), 1_000).expect("pairing");
            assert!(db.claim_telegram_pairing(&pairing_hash(code), chat, chat, 1_001).expect("link"));
        }
        db.begin_telegram_pairing(&pairing_hash("waiting"), None, 1_002).expect("pairing");
        db.revoke_telegram_chat(10, 1_003).expect("revoke");
        assert!(!db.telegram_chat_is_linked(10, 10).expect("read"), "the one unlinked");
        assert!(db.telegram_chat_is_linked(20, 20).expect("read"), "the other stays");
        assert!(
            !db.claim_telegram_pairing(&pairing_hash("waiting"), 30, 30, 1_004).expect("read"),
            "QRs expire too"
        );

        // A chat that pairs again keeps one link, for its new member.
        let bruno = db.add_member("Bruno").expect("member");
        db.begin_telegram_pairing(&pairing_hash("again"), Some(bruno), 1_005).expect("pairing");
        assert!(db.claim_telegram_pairing(&pairing_hash("again"), 20, 20, 1_006).expect("link"));
        let links = db.telegram_links().expect("links");
        assert_eq!(links.len(), 1, "{links:?}");
        assert_eq!(links[0].member.as_ref().map(|m| m.name.as_str()), Some("Bruno"));
        // A member removed leaves the chat linked, for no one in particular.
        db.delete_member(bruno).expect("delete");
        assert!(db.telegram_chat_is_linked(20, 20).expect("read"));
        assert_eq!(db.telegram_chat_member(20).expect("read"), None);
    }

    #[test]
    fn unlinked_chat_is_rejected() {
        let db = Database::in_memory().expect("database");
        let hash = pairing_hash("linked");
        db.begin_telegram_pairing(&hash, None, 1_000).expect("pairing");
        assert!(db.claim_telegram_pairing(&hash, 10, 10, 1_001).expect("link"));
        assert!(!db.telegram_chat_is_linked(20, 20).expect("unlinked"));
    }
}
