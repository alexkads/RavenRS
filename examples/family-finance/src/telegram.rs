//! Small Telegram long-polling bridge for PDF receipts.

use std::collections::HashMap;
use std::fs::{File, remove_file};
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
use crate::model::{Flow, TransactionInput};
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

/// Looks up the bot's public username without exposing its BotFather token to the UI.
pub fn pairing_link(token: &str) -> Result<String, String> {
    let bot_token = required_env("TELEGRAM_BOT_TOKEN")?;
    let url = format!("https://api.telegram.org/bot{bot_token}/getMe");
    let mut response = telegram_agent().get(&url).call().map_err(|_| "falha ao comunicar com Telegram")?;
    let parsed: TelegramResponse<TelegramBot> =
        response.body_mut().read_json().map_err(|_| "resposta inválida do Telegram")?;
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
    let config =
        Config { db_path: checked_db_path(db_path)?, bot_token: required_env("TELEGRAM_BOT_TOKEN")? };
    let service = Service { config, pending: Mutex::new(HashMap::new()) };
    let mut offset = 0;
    loop {
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
                if let Err(error) = handle_message(&service, message) {
                    let _ = send_message(&service, chat, &format!("Não consegui processar: {error}"));
                }
            }
        }
    }
}

fn required_env(key: &str) -> Result<String, String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty()).ok_or_else(|| format!("configure {key}"))
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
            let Some((flow, category)) = parse_category_command(&text) else {
                service
                    .pending
                    .lock()
                    .map_err(|_| "estado do bot indisponível")?
                    .insert(chat, Pending::Amount { amount_cents, since });
                send_message(service, chat, "Use `despesa Categoria` ou `receita Categoria`.")?;
                return Ok(());
            };
            if let Err(error) = save_transaction(&service.config.db_path, amount_cents, flow, &category) {
                service
                    .pending
                    .lock()
                    .map_err(|_| "estado do bot indisponível")?
                    .insert(chat, Pending::Amount { amount_cents, since });
                return Err(error);
            }
            send_message(
                service,
                chat,
                &format!("{} de R$ {} salva em {}.", flow.label(), format_brl(amount_cents), category),
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

fn parse_category_command(text: &str) -> Option<(Flow, String)> {
    let (kind, category) = text.trim().split_once(char::is_whitespace)?;
    let category = category.split_whitespace().collect::<Vec<_>>().join(" ");
    if category.is_empty() {
        return None;
    }
    match kind.to_lowercase().as_str() {
        "despesa" => Some((Flow::Expense, category)),
        "receita" => Some((Flow::Income, category)),
        _ => None,
    }
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
    let url = format!("https://api.telegram.org/bot{}/{method}", service.config.bot_token);
    let mut response = agent.post(&url).send_json(body).map_err(|_| "falha ao comunicar com Telegram")?;
    let parsed: TelegramResponse<T> =
        response.body_mut().read_json().map_err(|_| "resposta inválida do Telegram")?;
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
    let url = format!("https://api.telegram.org/file/bot{}/{}", service.config.bot_token, file.file_path);
    let mut response = agent.get(&url).call().map_err(|_| "falha ao baixar PDF do Telegram")?;
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

fn extract_total_from_pdf(pdf: &[u8]) -> Result<i64, String> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "não foi possível preparar o PDF")?
        .as_nanos();
    let path = std::env::temp_dir().join(format!("raven-financas-{}-{nonce}.pdf", std::process::id()));
    let mut file = File::options()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|_| "não foi possível preparar o PDF")?;
    if file.write_all(pdf).is_err() {
        let _ = remove_file(&path);
        return Err("não foi possível preparar o PDF".into());
    }
    let output = Command::new("pdftotext").arg("-layout").arg(&path).arg("-").output();
    let _ = remove_file(&path);
    let output = output.map_err(|_| "não foi possível ler o PDF neste computador")?;
    if !output.status.success() {
        return Err("o PDF não tem texto extraível. Envie outro comprovante.".into());
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| "o PDF não tem texto extraível. Envie outro comprovante.")?;
    extract_total_from_text(&text)
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

fn save_transaction(path: &Path, amount_cents: i64, flow: Flow, category: &str) -> Result<i64, String> {
    let db = Database::open(path).map_err(|e| e.to_string())?;
    let account_id: i64 = std::env::var("RAVEN_ACCOUNT_ID")
        .map_err(|_| "configure RAVEN_ACCOUNT_ID")?
        .parse()
        .map_err(|_| "RAVEN_ACCOUNT_ID inválido")?;
    if !db.accounts().map_err(|e| e.to_string())?.iter().any(|a| a.id == account_id) {
        return Err("a conta configurada não existe no Raven".into());
    }
    db.atomically(|db| {
        let category_id = db.find_or_create_category(category, flow)?;
        db.add_transaction(&TransactionInput {
            date: today(),
            description: format!("Comprovante PDF — {category}"),
            amount_cents,
            flow,
            account_id,
            category_id,
            member_id: None,
        })
    })
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_category_command() {
        assert_eq!(
            parse_category_command(" despesa   Café da Casa "),
            Some((Flow::Expense, "Café da Casa".into()))
        );
        assert_eq!(parse_category_command("receita Salário"), Some((Flow::Income, "Salário".into())));
        assert_eq!(parse_category_command("Mercado"), None);
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
