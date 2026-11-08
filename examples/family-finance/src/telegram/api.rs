//! The Telegram Bot API: what it sends, and the calls that answer, send and download.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use super::receipts::*;
use super::*;

#[derive(Deserialize)]
pub(super) struct Update {
    pub(super) update_id: i64,
    pub(super) message: Option<Message>,
    pub(super) callback_query: Option<CallbackQuery>,
}

/// A tap on one of the buttons under the bot's question.
#[derive(Deserialize)]
pub(super) struct CallbackQuery {
    pub(super) id: String,
    pub(super) from: User,
    pub(super) message: Option<CallbackMessage>,
    pub(super) data: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct CallbackMessage {
    pub(super) message_id: i64,
    pub(super) chat: Chat,
}

#[derive(Deserialize)]
pub(super) struct Message {
    pub(super) chat: Chat,
    /// When it was sent, in Unix seconds.
    pub(super) date: Option<i64>,
    pub(super) from: Option<User>,
    pub(super) document: Option<Document>,
    pub(super) text: Option<String>,
    pub(super) voice: Option<Voice>,
}

#[derive(Deserialize)]
pub(super) struct Chat {
    pub(super) id: i64,
    #[serde(rename = "type")]
    pub(super) kind: String,
}

#[derive(Deserialize)]
pub(super) struct User {
    pub(super) id: i64,
}

/// A voice note: Opus in an Ogg file, as Telegram records it.
#[derive(Deserialize)]
pub(super) struct Voice {
    pub(super) file_id: String,
    pub(super) duration: Option<u64>,
    pub(super) file_size: Option<u64>,
}

#[derive(Deserialize)]
pub(super) struct Document {
    pub(super) file_id: String,
    pub(super) mime_type: Option<String>,
    pub(super) file_size: Option<u64>,
}

#[derive(Deserialize)]
pub(super) struct TelegramResponse<T> {
    pub(super) ok: bool,
    pub(super) result: Option<T>,
    pub(super) description: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct TelegramFile {
    pub(super) file_path: String,
}

#[derive(Deserialize)]
pub(super) struct TelegramBot {
    pub(super) username: Option<String>,
}

pub(super) fn telegram_agent() -> ureq::Agent {
    let cfg = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(45))).build();
    cfg.into()
}

pub(super) fn telegram_call<T: for<'de> Deserialize<'de>>(
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

pub(super) fn send_message(service: &Service, chat: i64, text: &str) -> Result<(), String> {
    let _: Value = telegram_call(service, "sendMessage", json!({"chat_id":chat,"text":text}))?;
    Ok(())
}

/// A message with rows of buttons, each a label and the `data` it sends back as a callback.
pub(super) fn send_with_buttons(
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

pub(super) fn download_pdf(service: &Service, file_id: &str) -> Result<Vec<u8>, String> {
    download(service, file_id, MAX_PDF_BYTES)
        .map_err(|why| why.unwrap_or_else(|| "PDF vazio ou maior que 14 MB".into()))
}

/// A file someone sent, up to `limit` bytes. `Err(None)` when it is empty or too big.
pub(super) fn download(service: &Service, file_id: &str, limit: u64) -> Result<Vec<u8>, Option<String>> {
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
