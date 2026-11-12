//! Voice notes: decoding Telegram's Opus and hearing it with the local model.

use std::path::Path;

use opus_decoder::OpusDecoder;
use ravenrs_ai::{HEARING_CATALOG, HEARING_RATE, LocalModel, ModelFolder, ModelState, Transcriber};

use super::api::*;
use super::*;

/// A voice note is heard up to a minute: enough for "despesa, mercado, foi a Ana".
pub(super) const MAX_VOICE_SECONDS: usize = 60;

pub(super) const MAX_VOICE_BYTES: u64 = 2 * 1024 * 1024;

/// The words of a voice note, said back so the person sees what was understood. `None`
/// when there was nothing to hear it with, or nothing heard; the person is told.
pub(super) fn hear(service: &Service, chat: i64, voice: &Voice) -> Result<Option<String>, String> {
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
pub(super) fn model_that_hears(folder: &ModelFolder) -> Option<LocalModel> {
    let id = std::fs::read_to_string(folder.root().join("in-use")).ok()?;
    let model = HEARING_CATALOG.iter().find(|model| model.id == id.trim())?;
    (folder.state(model) == ModelState::Installed).then_some(*model)
}

/// A voice note as mono samples at the rate Whisper hears: Telegram records Opus in an
/// Ogg file, which is decoded here in Rust, the same on every system.
pub(super) fn voice_samples(ogg: &[u8]) -> Result<Vec<f32>, String> {
    let unreadable = || "não consegui ler o áudio. Envie de novo ou escreva a resposta.".to_owned();
    std::panic::catch_unwind(|| decode_voice(ogg)).map_err(|_| unreadable())?.ok_or_else(unreadable)
}

pub(super) fn decode_voice(ogg: &[u8]) -> Option<Vec<f32>> {
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
