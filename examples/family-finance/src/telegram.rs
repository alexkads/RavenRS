//! Small Telegram long-polling bridge for PDF receipts.

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

use crate::db::Database;
use crate::model::{Category, Flow, Member, TransactionInput};
use crate::today;

const PENDING_TTL: Duration = Duration::from_secs(60);
const MAX_PDF_BYTES: u64 = 14 * 1024 * 1024;

#[derive(Clone)]
struct Config {
    db_path: PathBuf,
    bot_token: String,
}

struct Service {
    config: Config,
    pending: Mutex<HashMap<i64, Pending>>,
}

#[derive(Debug)]
enum Pending {
    Amount { amount_cents: i64, since: Instant },
}

#[derive(Deserialize)]
struct Update {
    update_id: i64,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    chat: Chat,
    from: Option<User>,
    document: Option<Document>,
    text: Option<String>,
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
    let service = Service { config, pending: Mutex::new(HashMap::new()) };
    let mut offset = 0;
    loop {
        if bot_token().is_ok_and(|token| token != service.config.bot_token) {
            return Err("token do Telegram alterado; reconectando".into());
        }
        let updates: Vec<Update> = telegram_call(
            &service,
            "getUpdates",
            json!({"offset":offset,"timeout":25,"allowed_updates":["message"]}),
        )?;
        for update in updates {
            offset = update.update_id + 1;
            if let Some(message) = update.message
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
                if let Some(waker) = &waker {
                    waker.wake();
                }
            }
        }
    }
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
        service
            .pending
            .lock()
            .map_err(|_| "estado do bot indisponível")?
            .insert(chat, Pending::Amount { amount_cents, since: Instant::now() });
        send_message(
            service,
            chat,
            &format!(
                "Valor encontrado: R$ {}. Envie `despesa Categoria` ou `receita Categoria` em até 60 segundos.",
                format_brl(amount_cents)
            ),
        )?;
        return Ok(());
    }
    let Some(text) = message.text.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    if text == "/cancelar" {
        service.pending.lock().map_err(|_| "estado do bot indisponível")?.remove(&chat);
        send_message(service, chat, "Lançamento cancelado.")?;
        return Ok(());
    }
    let pending = service.pending.lock().map_err(|_| "estado do bot indisponível")?.remove(&chat);
    match pending {
        Some(Pending::Amount { amount_cents, since }) if since.elapsed() <= PENDING_TTL => {
            let keep = || -> Result<(), String> {
                service
                    .pending
                    .lock()
                    .map_err(|_| "estado do bot indisponível")?
                    .insert(chat, Pending::Amount { amount_cents, since });
                Ok(())
            };
            let reply = Database::open(&service.config.db_path)
                .and_then(|db| Ok((db.members()?, db.categories()?)))
                .map_err(|e| e.to_string())
                .and_then(|(members, categories)| Ok(read_reply(&text, &members, &categories)));
            let reply = match reply {
                Ok(Ok(reply)) => reply,
                Ok(Err(question)) => {
                    keep()?;
                    send_message(service, chat, &question)?;
                    return Ok(());
                }
                Err(error) => {
                    keep()?;
                    return Err(error);
                }
            };
            if let Err(error) = save_transaction(&service.config.db_path, amount_cents, &reply) {
                keep()?;
                return Err(error);
            }
            let who = reply.member.as_ref().map(|member| format!(" ({})", member.name)).unwrap_or_default();
            send_message(
                service,
                chat,
                &format!(
                    "{} de R$ {} salva em {}{who}.",
                    reply.flow.label(),
                    format_brl(amount_cents),
                    reply.category
                ),
            )?;
        }
        Some(Pending::Amount { .. }) => {
            send_message(service, chat, "O prazo para esse PDF terminou. Envie-o novamente.")?;
        }
        _ => send_message(service, chat, "Não há PDF aguardando. Envie o comprovante novamente.")?,
    }
    Ok(())
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
/// What comes before a name nobody registered: "por Pedro", "foi o Pedro", "quem gastou Pedro".
const NAME_MARKERS: &[&str] = &["por", "foi", "quem"];

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
        if EXPENSE_WORDS.contains(&key.as_str()) {
            Some((i, Flow::Expense))
        } else {
            INCOME_WORDS.contains(&key.as_str()).then_some((i, Flow::Income))
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
            find_run(&keys(&words), &name).map(|at| (at, name.len(), member))
        })
        .max_by_key(|(_, len, _)| *len);
    let member = match member {
        Some((at, len, member)) => {
            words.drain(at..at + len);
            Some(member.clone())
        }
        None => {
            if let Some(name) = unknown_name(&words) {
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
    let category = match known {
        Some(category) => category.name.clone(),
        None => capitalized(&words.join(" ")),
    };
    Ok(Reply { flow, category, member })
}

/// Where `run` appears whole in `words`.
fn find_run(words: &[String], run: &[String]) -> Option<usize> {
    if run.is_empty() || run.len() > words.len() {
        return None;
    }
    words.windows(run.len()).position(|window| window == run)
}

/// A name after "por", "foi" or "quem", at the end of a reply, that no member has.
fn unknown_name(words: &[String]) -> Option<String> {
    let keys = words.iter().map(|word| folded(word)).collect::<Vec<_>>();
    let marker = keys.iter().rposition(|key| NAME_MARKERS.contains(&key.as_str()))?;
    let name = words[marker + 1..]
        .iter()
        .zip(&keys[marker + 1..])
        .skip_while(|(_, key)| JOINING_WORDS.contains(&key.as_str()))
        .map(|(word, _)| word.as_str())
        .collect::<Vec<_>>();
    (!name.is_empty() && name.len() <= 3).then(|| name.join(" "))
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

fn download_pdf(service: &Service, file_id: &str) -> Result<Vec<u8>, String> {
    let file: TelegramFile = telegram_call(service, "getFile", json!({"file_id":file_id}))?;
    let agent = telegram_agent();
    let url = format!("{}/file/bot{}/{}", api_base(), service.config.bot_token, file.file_path);
    let mut response = agent.get(&url).call().map_err(|error| {
        format!(
            "falha ao baixar PDF do Telegram: {}",
            error.to_string().replace(&service.config.bot_token, "[oculto]")
        )
    })?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_PDF_BYTES + 1)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_PDF_BYTES {
        return Err("PDF vazio ou maior que 14 MB".into());
    }
    Ok(bytes)
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

fn save_transaction(path: &Path, amount_cents: i64, reply: &Reply) -> Result<i64, String> {
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
            description: format!("Comprovante PDF — {}", reply.category),
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

    #[test]
    fn asks_about_a_name_nobody_registered() {
        let members = [member(1, "Ana")];
        let question = read_reply("despesa de mercado, foi o Pedro", &members, &[]).expect_err("unknown");
        assert!(question.contains("Pedro"), "{question}");
        assert_eq!(
            read_reply("despesa Mercado Pedro", &members, &[]).expect("typed").category,
            "Mercado Pedro"
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
