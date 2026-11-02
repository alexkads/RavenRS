//! Runs a model on this machine, on the CPU.

use std::fmt;
use std::fs::File;
use std::time::Instant;

use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::{
    quantized_gemma3, quantized_llama, quantized_phi3, quantized_qwen2, quantized_qwen3,
};
use tokenizers::Tokenizer;

use crate::{LocalModel, ModelFamily, ModelFolder};

/// How sure it answers: low, because it should stick to what it is given.
const TEMPERATURE: f64 = 0.2;
const SEED: u64 = 42;

/// The weights of each kind of model.
enum Weights {
    Qwen3(quantized_qwen3::ModelWeights),
    Qwen2(quantized_qwen2::ModelWeights),
    Gemma3(quantized_gemma3::ModelWeights),
    Llama(quantized_llama::ModelWeights),
    Phi3(quantized_phi3::ModelWeights),
}

impl Weights {
    /// The logits for the next token. At offset 0 each kind forgets what it read before.
    fn forward(&mut self, input: &Tensor, offset: usize) -> candle_core::Result<Tensor> {
        let logits = match self {
            Self::Qwen3(model) => model.forward(input, offset),
            Self::Qwen2(model) => model.forward(input, offset),
            Self::Gemma3(model) => model.forward(input, offset),
            Self::Llama(model) => model.forward(input, offset),
            Self::Phi3(model) => model.forward(input, offset),
        }?;
        // Some give one row per token read, others only the last.
        let logits = logits.squeeze(0)?;
        if logits.rank() == 2 {
            let last = logits.dim(0)? - 1;
            logits.get(last)
        } else {
            Ok(logits)
        }
    }

    fn clear(&mut self) {
        match self {
            Self::Qwen3(model) => model.clear_kv_cache(),
            Self::Qwen2(model) => model.clear_kv_cache(),
            Self::Llama(model) => model.clear_kv_cache(),
            Self::Gemma3(_) | Self::Phi3(_) => {}
        }
    }
}

/// A family's way of writing a conversation, and the tokens that end its answer.
pub(crate) fn prompt_of(family: ModelFamily, system: &str, user: &str) -> (String, &'static [&'static str]) {
    match family {
        ModelFamily::Qwen3 => (
            format!(
                "<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{user} /no_think<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
            ),
            &["<|im_end|>", "<|endoftext|>"],
        ),
        ModelFamily::Qwen2 => (
            format!(
                "<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n"
            ),
            &["<|im_end|>", "<|endoftext|>"],
        ),
        ModelFamily::Gemma3 => (
            format!("<bos><start_of_turn>user\n{system}\n\n{user}<end_of_turn>\n<start_of_turn>model\n"),
            &["<end_of_turn>", "<eos>"],
        ),
        ModelFamily::Llama3 => (
            format!(
                "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n{system}<|eot_id|><|start_header_id|>user<|end_header_id|>\n\n{user}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n"
            ),
            &["<|eot_id|>", "<|end_of_text|>"],
        ),
        ModelFamily::Phi3 => (
            format!("<|system|>\n{system}<|end|>\n<|user|>\n{user}<|end|>\n<|assistant|>\n"),
            &["<|end|>", "<|endoftext|>"],
        ),
        ModelFamily::Mistral => (format!("<s>[INST] {system}\n\n{user} [/INST]"), &["</s>"]),
    }
}

/// Why a model did not run.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EngineError {
    /// Its files are missing or are not what this engine reads.
    Load(String),
    /// It stopped in the middle.
    Run(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(why) => write!(f, "the model could not be loaded: {why}"),
            Self::Run(why) => write!(f, "the model stopped: {why}"),
        }
    }
}

impl std::error::Error for EngineError {}

#[cfg(test)]
mod tests {
    use super::prompt_of;
    use crate::ModelFamily;

    #[test]
    fn each_family_writes_the_conversation_its_way() {
        let families = [
            ModelFamily::Qwen3,
            ModelFamily::Qwen2,
            ModelFamily::Gemma3,
            ModelFamily::Llama3,
            ModelFamily::Phi3,
            ModelFamily::Mistral,
        ];
        for family in families {
            let (prompt, ends) = prompt_of(family, "RULES", "QUESTION");
            assert!(prompt.find("RULES") < prompt.find("QUESTION"), "{family:?}");
            assert!(!ends.is_empty(), "{family:?}");
        }
        assert!(prompt_of(ModelFamily::Qwen3, "", "").0.contains("/no_think"));
        assert!(!prompt_of(ModelFamily::Qwen2, "", "").0.contains("/no_think"));
    }
}

/// What a generation cost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Generated {
    /// Tokens it wrote.
    pub tokens: usize,
    /// Seconds until the first of them.
    pub first_token: f64,
    /// Seconds in all.
    pub seconds: f64,
}

impl Generated {
    /// Tokens per second after the first one.
    pub fn tokens_per_second(&self) -> f64 {
        let writing = self.seconds - self.first_token;
        if self.tokens > 1 && writing > 0.0 { (self.tokens - 1) as f64 / writing } else { 0.0 }
    }
}

/// A model loaded in memory. Loading takes a while and generating blocks, so keep it
/// away from the UI thread.
pub struct LocalEngine {
    family: ModelFamily,
    model: Weights,
    tokenizer: Tokenizer,
    device: Device,
    ends: Vec<u32>,
}

impl fmt::Debug for LocalEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalEngine").field("device", &self.device).finish_non_exhaustive()
    }
}

impl LocalEngine {
    /// Loads `model` from `folder`.
    ///
    /// # Errors
    ///
    /// When its files are missing or cannot be read.
    pub fn load(folder: &ModelFolder, model: &LocalModel) -> Result<Self, EngineError> {
        let load = |why: String| EngineError::Load(why);
        let named = |name: &str| {
            let file = model.files.iter().find(|file| file.name == name);
            file.map(|file| folder.path_of(model, file)).ok_or_else(|| load(format!("no {name}")))
        };
        let device = Device::Cpu;
        let mut file = File::open(named("model.gguf")?).map_err(|e| load(e.to_string()))?;
        let content = gguf_file::Content::read(&mut file).map_err(|e| load(e.to_string()))?;
        let family = model.family;
        let weights = match family {
            ModelFamily::Qwen3 => {
                quantized_qwen3::ModelWeights::from_gguf(content, &mut file, &device).map(Weights::Qwen3)
            }
            ModelFamily::Qwen2 => {
                quantized_qwen2::ModelWeights::from_gguf(content, &mut file, &device).map(Weights::Qwen2)
            }
            ModelFamily::Gemma3 => {
                quantized_gemma3::ModelWeights::from_gguf(content, &mut file, &device).map(Weights::Gemma3)
            }
            ModelFamily::Llama3 | ModelFamily::Mistral => {
                quantized_llama::ModelWeights::from_gguf(content, &mut file, &device).map(Weights::Llama)
            }
            ModelFamily::Phi3 => {
                quantized_phi3::ModelWeights::from_gguf(false, content, &mut file, &device).map(Weights::Phi3)
            }
        }
        .map_err(|e| load(e.to_string()))?;
        let tokenizer = Tokenizer::from_file(named("tokenizer.json")?).map_err(|e| load(e.to_string()))?;
        let ends = prompt_of(family, "", "").1.iter().filter_map(|end| tokenizer.token_to_id(end)).collect();
        Ok(Self { family, model: weights, tokenizer, device, ends })
    }

    /// Answers `user`, following `system`, in at most `max_tokens`. `on_text` gets the
    /// text as it comes, and returning false stops it.
    ///
    /// # Errors
    ///
    /// When it stops in the middle.
    pub fn generate(
        &mut self,
        system: &str,
        user: &str,
        max_tokens: usize,
        on_text: &mut dyn FnMut(&str) -> bool,
    ) -> Result<Generated, EngineError> {
        let run = |why: String| EngineError::Run(why);
        let (prompt, _) = prompt_of(self.family, system, user);
        let started = Instant::now();
        self.model.clear();
        let encoding = self.tokenizer.encode(prompt, false).map_err(|e| run(e.to_string()))?;
        let prompt_tokens = encoding.get_ids();
        let mut sampler = LogitsProcessor::from_sampling(SEED, Sampling::All { temperature: TEMPERATURE });
        let mut written: Vec<u32> = Vec::new();
        let mut shown = 0;
        let mut first_token = 0.0;
        let mut input = prompt_tokens.to_vec();
        let mut offset = 0;
        while written.len() < max_tokens {
            let tensor = Tensor::new(input.as_slice(), &self.device)
                .and_then(|t| t.unsqueeze(0))
                .map_err(|e| run(e.to_string()))?;
            let logits = self.model.forward(&tensor, offset).map_err(|e| run(e.to_string()))?;
            let next = sampler.sample(&logits).map_err(|e| run(e.to_string()))?;
            if written.is_empty() {
                first_token = started.elapsed().as_secs_f64();
            }
            offset += input.len();
            input = vec![next];
            if self.ends.contains(&next) {
                break;
            }
            written.push(next);
            let text = self.tokenizer.decode(&written, true).map_err(|e| run(e.to_string()))?;
            // A letter split between tokens shows up only when it is whole.
            if !text.ends_with('\u{fffd}') && text.len() > shown {
                let keep_going = on_text(text.get(shown..).unwrap_or_default());
                shown = text.len();
                if !keep_going {
                    break;
                }
            }
        }
        Ok(Generated { tokens: written.len(), first_token, seconds: started.elapsed().as_secs_f64() })
    }
}
