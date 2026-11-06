//! Speech to text with Whisper, on the CPU in plain Rust, the way OpenAI's reference
//! reads it: each 30 seconds padded with silence, the language held, the likeliest word
//! taken each step.

use std::fmt;

use candle_core::{D, Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self, Config, audio, model::Whisper};
use tokenizers::Tokenizer;

use crate::engine::EngineError;
use crate::{LocalModel, ModelFolder};

/// The rate a transcriber hears at: give it mono samples at 16 kHz.
pub const HEARING_RATE: usize = whisper::SAMPLE_RATE;

/// What a transcription found.
#[derive(Clone, Debug, PartialEq)]
pub struct Transcribed {
    /// The words, trimmed.
    pub text: String,
    /// How many tokens it wrote.
    pub tokens: usize,
    /// How long it took, in seconds.
    pub seconds: f64,
}

/// Whisper, loaded and ready to hear.
pub struct Transcriber {
    model: Whisper,
    tokenizer: Tokenizer,
    filters: Vec<f32>,
    suppressed: Tensor,
    device: Device,
    start: u32,
    transcribe: u32,
    no_timestamps: u32,
    end: u32,
}

impl fmt::Debug for Transcriber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transcriber").field("device", &self.device).finish_non_exhaustive()
    }
}

impl Transcriber {
    /// Loads `model` from `folder`.
    ///
    /// # Errors
    ///
    /// When its files are missing or are not a Whisper model.
    pub fn load(folder: &ModelFolder, model: &LocalModel) -> Result<Self, EngineError> {
        let load = |why: String| EngineError::Load(why);
        let named = |name: &str| {
            let file = model.files.iter().find(|file| file.name == name);
            file.map(|file| folder.path_of(model, file)).ok_or_else(|| load(format!("no {name}")))
        };
        let device = Device::Cpu;
        let config: Config =
            serde_json::from_slice(&std::fs::read(named("config.json")?).map_err(|e| load(e.to_string()))?)
                .map_err(|e| load(e.to_string()))?;
        let tensors = candle_core::safetensors::load(named("model.safetensors")?, &device)
            .map_err(|e| load(e.to_string()))?;
        let weights = VarBuilder::from_tensors(tensors, whisper::DTYPE, &device);
        let tokenizer = Tokenizer::from_file(named("tokenizer.json")?).map_err(|e| load(e.to_string()))?;
        let token = |text: &str| tokenizer.token_to_id(text).ok_or_else(|| load(format!("no token {text}")));
        let (start, transcribe) = (token(whisper::SOT_TOKEN)?, token(whisper::TRANSCRIBE_TOKEN)?);
        let (no_timestamps, end) = (token(whisper::NO_TIMESTAMPS_TOKEN)?, token(whisper::EOT_TOKEN)?);
        let mask: Vec<f32> = (0..config.vocab_size as u32)
            .map(|id| if config.suppress_tokens.contains(&id) { f32::NEG_INFINITY } else { 0.0 })
            .collect();
        let suppressed = Tensor::new(mask.as_slice(), &device).map_err(|e| load(e.to_string()))?;
        let filters = mel_filters(config.num_mel_bins);
        let model = Whisper::load(&weights, config).map_err(|e| load(e.to_string()))?;
        Ok(Self { model, tokenizer, filters, suppressed, device, start, transcribe, no_timestamps, end })
    }

    /// The words in `samples`, mono at [`HEARING_RATE`], spoken in `language` (its
    /// two-letter code, such as `pt`).
    ///
    /// # Errors
    ///
    /// When the language is not one Whisper knows, or it stops in the middle.
    pub fn transcribe(&mut self, samples: &[f32], language: &str) -> Result<Transcribed, EngineError> {
        let started = std::time::Instant::now();
        let run = |why: candle_core::Error| EngineError::Run(why.to_string());
        let spoken = self
            .tokenizer
            .token_to_id(&format!("<|{language}|>"))
            .ok_or_else(|| EngineError::Run(format!("Whisper does not know the language {language}")))?;
        let prompt = [self.start, spoken, self.transcribe, self.no_timestamps];
        let bins = self.model.config.num_mel_bins;
        let most = self.model.config.max_target_positions / 2;
        let (mut text, mut written) = (String::new(), 0);
        for chunk in samples.chunks(whisper::N_SAMPLES) {
            let mut padded = chunk.to_vec();
            padded.resize(whisper::N_SAMPLES, 0.0);
            let mel = audio::pcm_to_mel(&self.model.config, &padded, &self.filters);
            let frames = mel.len() / bins;
            let mel = Tensor::from_vec(mel, (1, bins, frames), &self.device)
                .and_then(|mel| mel.narrow(2, 0, frames.min(whisper::N_FRAMES)))
                .map_err(run)?;
            let heard = self.model.encoder.forward(&mel, true).map_err(run)?;
            let mut tokens = prompt.to_vec();
            for step in 0..most {
                let input =
                    Tensor::new(tokens.as_slice(), &self.device).and_then(|t| t.unsqueeze(0)).map_err(run)?;
                let output = self.model.decoder.forward(&input, &heard, step == 0).map_err(run)?;
                let last = output.dim(1).map_err(run)? - 1;
                let next = output
                    .i((..1, last..))
                    .and_then(|last| self.model.decoder.final_linear(&last))
                    .and_then(|logits| logits.i(0)?.i(0)?.broadcast_add(&self.suppressed))
                    .and_then(|logits| logits.argmax(D::Minus1)?.to_scalar::<u32>())
                    .map_err(run)?;
                if next == self.end {
                    break;
                }
                tokens.push(next);
            }
            written += tokens.len() - prompt.len();
            let words = self
                .tokenizer
                .decode(&tokens[prompt.len()..], true)
                .map_err(|e| EngineError::Run(e.to_string()))?;
            text.push_str(words.trim());
            text.push(' ');
        }
        Ok(Transcribed {
            text: text.trim().to_owned(),
            tokens: written,
            seconds: started.elapsed().as_secs_f64(),
        })
    }
}

/// The mel filters Whisper was trained with: librosa's, with Slaney's scale and area
/// normalization, for its 400-point FFT at 16 kHz. Worked out here rather than shipped.
fn mel_filters(bins: usize) -> Vec<f32> {
    const MIN_LOG_HZ: f64 = 1000.0;
    const HZ_PER_MEL: f64 = 200.0 / 3.0;
    let min_log_mel = MIN_LOG_HZ / HZ_PER_MEL;
    let log_step = 6.4_f64.ln() / 27.0;
    let to_mel = |hz: f64| {
        if hz >= MIN_LOG_HZ { min_log_mel + (hz / MIN_LOG_HZ).ln() / log_step } else { hz / HZ_PER_MEL }
    };
    let to_hz = |mel: f64| {
        if mel >= min_log_mel {
            MIN_LOG_HZ * (log_step * (mel - min_log_mel)).exp()
        } else {
            HZ_PER_MEL * mel
        }
    };
    let rate = HEARING_RATE as f64;
    let columns = whisper::N_FFT / 2 + 1;
    let top = to_mel(rate / 2.0);
    let edges: Vec<f64> = (0..bins + 2).map(|i| to_hz(top * i as f64 / (bins + 1) as f64)).collect();
    let mut filters = Vec::with_capacity(bins * columns);
    for band in 0..bins {
        let (low, middle, high) = (edges[band], edges[band + 1], edges[band + 2]);
        let area = 2.0 / (high - low);
        for column in 0..columns {
            let hz = column as f64 * rate / whisper::N_FFT as f64;
            let rising = (hz - low) / (middle - low);
            let falling = (high - hz) / (high - middle);
            filters.push((rising.min(falling).max(0.0) * area) as f32);
        }
    }
    filters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mel_filters_are_triangles_over_the_spectrum() {
        let filters = mel_filters(80);
        assert_eq!(filters.len(), 80 * 201);
        for band in filters.chunks(201) {
            assert!(band.iter().all(|weight| *weight >= 0.0));
            assert!(band.iter().any(|weight| *weight > 0.0), "every band hears something");
        }
        // The first band starts at 0 Hz, on the second FFT bin, as in librosa.
        assert_eq!(filters[0], 0.0);
        assert!(filters[1] > 0.0);
    }

    /// The same numbers as the filters candle's example ships, when that file is given.
    #[test]
    fn mel_filters_match_the_published_ones() {
        let Some(path) = std::env::var_os("WHISPER_MEL_FILTERS") else { return };
        let bytes = std::fs::read(path).unwrap_or_default();
        let published: Vec<f32> =
            bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        let ours = mel_filters(80);
        assert_eq!(published.len(), ours.len());
        let worst = published.iter().zip(&ours).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
        assert!(worst < 1e-6, "differs by {worst}");
    }
}
