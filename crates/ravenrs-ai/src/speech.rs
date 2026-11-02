//! Speech from LFM3 Audio, following its model card: the model writes frames of codes,
//! a small decoder finds each frame's codes, and the codec turns them into 24 kHz sound.

use std::fs::File;

use candle_core::quantized::{QMatMul, gguf_file};
use candle_core::{Device, Module, Tensor};
use candle_nn::ops::rms_norm;
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::quantized_nn::RmsNorm;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex32;
use tokenizers::Tokenizer;

use crate::engine::EngineError;
use crate::lfm3::{self, Heads, LayerKind, LayerNames, Reach, Stack, mask, rotation};
use crate::{LocalModel, ModelFolder};

/// How many samples of sound a second has.
pub const SPEECH_RATE: u32 = 24_000;
/// The languages the model speaks, with the English name its prompt asks for.
pub const SPEECH_LANGUAGES: &[&str] = &[
    "Portuguese",
    "American English",
    "British English",
    "Spanish",
    "French",
    "German",
    "Italian",
    "Dutch",
    "Polish",
    "Swedish",
    "Danish",
    "Finnish",
    "Greek",
    "Turkish",
    "Arabic",
    "Hebrew",
    "Japanese",
    "Korean",
    "Chinese",
];

const CODEBOOKS: usize = 12;
const CODES: usize = 2017;
/// A code that says the speech is over.
const END_CODE: u32 = 2016;
/// The passes that reveal a frame's codes.
const PASSES: [usize; 3] = [4, 4, 4];
const DEPTH_SIZE: usize = 1024;
const DEPTH_HEAD_SIZE: usize = 32;
const STACKED_FRAMES: usize = 6;
const FFT_SIZE: usize = 1280;
const HOP: usize = 320;
const CODEC_WINDOW: usize = 30;
const CODEC_HEAD_SIZE: usize = 64;
const THETA: f32 = 1_000_000.0;
const SEED: u64 = 299_792_458;
/// How many frames a sentence may take at most: a minute.
const MOST_FRAMES: usize = 750;

/// Which of its voices the model speaks with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SpeechVoice {
    /// A female voice.
    #[default]
    Female,
    /// A male voice.
    Male,
}

/// How to speak. The default is what the model card recommends, in Portuguese.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeechSettings {
    /// One of [`SPEECH_LANGUAGES`].
    pub language: String,
    /// The voice.
    pub voice: SpeechVoice,
    /// How free each code is to vary; 0 always takes the likeliest.
    pub temperature: f32,
    /// How many codes it chooses among; 0 is all of them.
    pub top_k: usize,
    /// How much it avoids codes it has just used; 1 does not.
    pub repetition_penalty: f32,
    /// How many frames back it looks to avoid them.
    pub repetition_window: usize,
}

impl Default for SpeechSettings {
    fn default() -> Self {
        Self {
            language: "Portuguese".to_owned(),
            voice: SpeechVoice::Female,
            temperature: 1.0,
            top_k: 128,
            repetition_penalty: 1.1,
            repetition_window: 4096,
        }
    }
}

impl SpeechSettings {
    fn prompt(&self, text: &str) -> String {
        let voice = match self.voice {
            SpeechVoice::Female => "female",
            SpeechVoice::Male => "male",
        };
        format!(
            "<|startoftext|><|im_start|>system\nPerform TTS in {} using a {voice} voice.<|im_end|>\n<|im_start|>user\n{text}<|im_end|>\n<|im_start|>assistant\n<|audio_start|>",
            self.language
        )
    }

    fn sampling(&self) -> Sampling {
        let temperature = f64::from(self.temperature);
        if self.temperature <= 0.0 {
            Sampling::ArgMax
        } else if self.top_k == 0 {
            Sampling::All { temperature }
        } else {
            Sampling::TopK { k: self.top_k, temperature }
        }
    }
}

/// Finds a frame's codes from what the model holds.
struct DepthDecoder {
    into: QMatMul,
    into_bias: Tensor,
    mask_code: Tensor,
    stack: Stack,
    norm: RmsNorm,
    /// One table for each codebook, of `CODES` by `DEPTH_SIZE`.
    tables: Vec<Tensor>,
    /// One norm weight for each codebook.
    head_norms: Vec<Tensor>,
    frequencies: Vec<f32>,
    eps: f32,
    device: Device,
}

impl DepthDecoder {
    /// What each codebook's slot reads as, with `known` codes and the rest hidden.
    fn slots(
        &self,
        start: &Tensor,
        codes: &[u32; CODEBOOKS],
        known: &[bool; CODEBOOKS],
    ) -> candle_core::Result<Tensor> {
        let mut rows = Vec::with_capacity(CODEBOOKS);
        for index in 0..CODEBOOKS {
            let code = if known[index] {
                self.tables[index].narrow(0, codes[index] as usize, 1)?.squeeze(0)?
            } else {
                self.mask_code.clone()
            };
            rows.push(code);
        }
        let added = Tensor::stack(&rows, 0)?.unsqueeze(0)?;
        (start + added)?.contiguous()
    }

    fn frame(
        &mut self,
        hidden: &Tensor,
        sample: &mut Sampler,
        history: &mut [Vec<u32>],
    ) -> candle_core::Result<[u32; CODEBOOKS]> {
        let start = (self.into.forward(hidden)? + self.into_bias.unsqueeze(0)?)?
            .reshape((1, CODEBOOKS, DEPTH_SIZE))?;
        let rotation = rotation(&self.frequencies, 0, CODEBOOKS, &self.device)?;
        let reach = Reach { rotation: &rotation, mask: None };
        let order = sample.order();
        let mut codes = [0_u32; CODEBOOKS];
        let mut known = [false; CODEBOOKS];
        let mut taken = 0;
        for pass in PASSES {
            let slots = self.slots(&start, &codes, &known)?;
            let out = self.stack.forward(slots, reach, reach)?;
            let out = self.norm.forward(&out)?;
            for &index in &order[taken..taken + pass] {
                let row = out.narrow(1, index, 1)?.squeeze(1)?;
                let row = rms_norm(&row, &self.head_norms[index], self.eps)?;
                let logits = row.matmul(&self.tables[index].t()?)?.squeeze(0)?;
                codes[index] = sample.code(&logits, &history[index])?;
            }
            for &index in &order[taken..taken + pass] {
                known[index] = true;
            }
            taken += pass;
        }
        if !codes.contains(&END_CODE) {
            for (index, code) in codes.iter().enumerate() {
                history[index].push(*code);
            }
        }
        Ok(codes)
    }
}

/// Chooses codes as the settings ask, the same way each time.
struct Sampler {
    logits: LogitsProcessor,
    penalty: f32,
    window: usize,
    state: u64,
}

impl Sampler {
    fn new(settings: &SpeechSettings) -> Self {
        Self {
            logits: LogitsProcessor::from_sampling(SEED, settings.sampling()),
            penalty: settings.repetition_penalty,
            window: settings.repetition_window,
            state: SEED,
        }
    }

    /// The codebooks in a random order, for the passes to reveal.
    fn order(&mut self) -> [usize; CODEBOOKS] {
        let mut order: [usize; CODEBOOKS] = std::array::from_fn(|index| index);
        for last in (1..CODEBOOKS).rev() {
            self.state ^= self.state << 13;
            self.state ^= self.state >> 7;
            self.state ^= self.state << 17;
            order.swap(last, (self.state % (last as u64 + 1)) as usize);
        }
        order
    }

    fn code(&mut self, logits: &Tensor, history: &[u32]) -> candle_core::Result<u32> {
        let mut scores = logits.to_vec1::<f32>()?;
        if (self.penalty - 1.0).abs() > f32::EPSILON {
            let mut seen = [false; CODES];
            let start = history.len().saturating_sub(self.window);
            for &code in &history[start..] {
                seen[code as usize] = true;
            }
            for (score, seen) in scores.iter_mut().zip(seen) {
                if seen {
                    *score = if *score < 0.0 { *score * self.penalty } else { *score / self.penalty };
                }
            }
        }
        let scores = Tensor::new(scores.as_slice(), logits.device())?;
        self.logits.sample(&scores)
    }
}

/// Turns frames of codes into sound.
struct Codec {
    /// `2016` by 4: what each code means.
    values: Vec<f32>,
    from_values: QMatMul,
    from_values_bias: Tensor,
    into: QMatMul,
    into_bias: Tensor,
    stack: Stack,
    norm: RmsNorm,
    out: QMatMul,
    out_bias: Tensor,
    frequencies: Vec<f32>,
    device: Device,
}

impl Codec {
    fn sound(&mut self, frames: &[[u32; CODEBOOKS]]) -> candle_core::Result<Vec<f32>> {
        let mut values = Vec::with_capacity(frames.len() * CODEBOOKS * 4);
        for frame in frames {
            for code in frame {
                let start = *code as usize * 4;
                values.extend_from_slice(&self.values[start..start + 4]);
            }
        }
        let values = Tensor::from_vec(values, (1, frames.len(), CODEBOOKS * 4), &self.device)?;
        let latents = (self.from_values.forward(&values)?.broadcast_add(&self.from_values_bias))?;
        let embeds = self.into.forward(&latents)?.broadcast_add(&self.into_bias)?;
        // Each frame of codes stands for six of the spectrum.
        let (batch, count, size) = embeds.dims3()?;
        let stacked = embeds.unsqueeze(2)?.repeat((1, 1, STACKED_FRAMES, 1))?.reshape((
            batch,
            count * STACKED_FRAMES,
            size,
        ))?;
        let tokens = count * STACKED_FRAMES;
        let rotation = rotation(&self.frequencies, 0, tokens, &self.device)?;
        let window = mask(tokens, CODEC_WINDOW, &self.device)?;
        let reach = Reach { rotation: &rotation, mask: Some(&window) };
        self.stack.clear();
        let hidden = self.stack.forward(stacked, reach, reach)?;
        let hidden = self.norm.forward(&hidden)?;
        let spectrum = self.out.forward(&hidden)?.broadcast_add(&self.out_bias)?;
        let spectrum = spectrum.squeeze(0)?.to_vec2::<f32>()?;
        Ok(from_spectrum(&spectrum))
    }
}

/// Sound from frames of half log magnitudes and half angles, as the codec writes them.
fn from_spectrum(frames: &[Vec<f32>]) -> Vec<f32> {
    let bins = FFT_SIZE / 2 + 1;
    let fft = FftPlanner::new().plan_fft_inverse(FFT_SIZE);
    let window: Vec<f32> = (0..FFT_SIZE)
        .map(|place| {
            let turn = 2.0 * std::f32::consts::PI * place as f32 / FFT_SIZE as f32;
            0.5 - 0.5 * turn.cos()
        })
        .collect();
    let size = (frames.len() - 1) * HOP + FFT_SIZE;
    let (mut sound, mut envelope) = (vec![0.0_f32; size], vec![0.0_f32; size]);
    let mut buffer = vec![Complex32::new(0.0, 0.0); FFT_SIZE];
    for (index, frame) in frames.iter().enumerate() {
        for bin in 0..bins {
            let value = Complex32::from_polar(frame[bin].exp(), frame[bin + bins]);
            buffer[bin] = value;
            if bin > 0 && bin < FFT_SIZE / 2 {
                buffer[FFT_SIZE - bin] = value.conj();
            }
        }
        fft.process(&mut buffer);
        let start = index * HOP;
        for place in 0..FFT_SIZE {
            sound[start + place] += buffer[place].re / FFT_SIZE as f32 * window[place];
            envelope[start + place] += window[place] * window[place];
        }
    }
    let trim = (FFT_SIZE - HOP) / 2;
    sound[trim..size - trim]
        .iter()
        .zip(&envelope[trim..size - trim])
        .map(|(sound, envelope)| sound / envelope)
        .collect()
}

/// LFM3 Audio in memory. Loading takes a while and speaking blocks, so keep it away
/// from the UI thread.
pub struct SpeechEngine {
    model: lfm3::ModelWeights,
    depth: DepthDecoder,
    /// One row for each code of each codebook, for feeding a frame back.
    frame_embeds: Tensor,
    codec: Codec,
    tokenizer: Tokenizer,
    device: Device,
}

impl std::fmt::Debug for SpeechEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpeechEngine").finish_non_exhaustive()
    }
}

impl SpeechEngine {
    /// Loads `model` from `folder`, which needs `model.gguf`, `projector.gguf` and
    /// `tokenizer.json`.
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
        let weights =
            lfm3::ModelWeights::from_gguf(&content, &mut file, &device).map_err(|e| load(e.to_string()))?;
        let mut file = File::open(named("projector.gguf")?).map_err(|e| load(e.to_string()))?;
        let content = gguf_file::Content::read(&mut file).map_err(|e| load(e.to_string()))?;
        let (depth, frame_embeds, codec) =
            parts_of(&content, &mut file, &device).map_err(|e| load(e.to_string()))?;
        let tokenizer = Tokenizer::from_file(named("tokenizer.json")?).map_err(|e| load(e.to_string()))?;
        Ok(Self { model: weights, depth, frame_embeds, codec, tokenizer, device })
    }

    /// Says `text` as the settings ask. `go_on` hears how many frames are ready, each
    /// of 80 ms, and returning false stops it with what it has.
    ///
    /// # Errors
    ///
    /// When it stops in the middle.
    pub fn speak(
        &mut self,
        text: &str,
        settings: &SpeechSettings,
        go_on: &mut dyn FnMut(usize) -> bool,
    ) -> Result<Vec<f32>, EngineError> {
        let run = |why: String| EngineError::Run(why);
        let encoding = self.tokenizer.encode(settings.prompt(text), false).map_err(|e| run(e.to_string()))?;
        let prompt = encoding.get_ids();
        self.model.clear_kv_cache();
        let frames = self.frames(prompt, text, settings, go_on).map_err(|e| run(e.to_string()))?;
        if frames.is_empty() {
            return Ok(Vec::new());
        }
        self.codec.sound(&frames).map_err(|e| run(e.to_string()))
    }

    fn frames(
        &mut self,
        prompt: &[u32],
        text: &str,
        settings: &SpeechSettings,
        go_on: &mut dyn FnMut(usize) -> bool,
    ) -> candle_core::Result<Vec<[u32; CODEBOOKS]>> {
        let input = Tensor::new(prompt, &self.device)?.unsqueeze(0)?;
        let embeds = self.model.embed(&input)?;
        let mut hidden = self.model.last_hidden(&embeds, 0)?;
        let mut offset = prompt.len();
        let most = (30 + text.chars().count() * 2).min(MOST_FRAMES);
        let mut sampler = Sampler::new(settings);
        let mut history = vec![Vec::new(); CODEBOOKS];
        let mut frames = Vec::new();
        while frames.len() < most {
            let codes = self.depth.frame(&hidden, &mut sampler, &mut history)?;
            if codes.contains(&END_CODE) {
                break;
            }
            frames.push(codes);
            if !go_on(frames.len()) {
                break;
            }
            let rows: Vec<u32> =
                codes.iter().enumerate().map(|(index, code)| *code + (index * CODES) as u32).collect();
            let rows = Tensor::new(rows.as_slice(), &self.device)?;
            let sum = self.frame_embeds.index_select(&rows, 0)?.sum_keepdim(0)?.unsqueeze(0)?;
            hidden = self.model.last_hidden(&sum, offset)?;
            offset += 1;
        }
        Ok(frames)
    }
}

fn parts_of<R: std::io::Seek + std::io::Read>(
    content: &gguf_file::Content,
    reader: &mut R,
    device: &Device,
) -> candle_core::Result<(DepthDecoder, Tensor, Codec)> {
    const EPS: f64 = 1e-5;
    let frequencies = |head_size: usize| -> Vec<f32> {
        (0..head_size).step_by(2).map(|place| 1.0 / THETA.powf(place as f32 / head_size as f32)).collect()
    };
    let depth_names = LayerNames {
        attention_out: "attn_out",
        operator_norm: "ln1",
        after_operator_norm: None,
        feed_norm: "ln2",
        after_feed_norm: None,
        conv_in: "",
        conv_out: "",
        conv: "",
    };
    let depth_sizes =
        Heads { heads: 32, key_heads: 8, head_size: DEPTH_HEAD_SIZE, keep: 0, remembers: false, eps: EPS };
    let depth_stack = Stack::from_gguf(
        content,
        reader,
        device,
        "a.gen.code.blk",
        &depth_names,
        depth_sizes,
        &[LayerKind::Full; 3],
    )?;
    let codec_names = LayerNames {
        attention_out: "attn_out",
        operator_norm: "ln1",
        after_operator_norm: Some("attn_post_norm"),
        feed_norm: "ln2",
        after_feed_norm: Some("ffn_post_norm"),
        conv_in: "conv_pw1",
        conv_out: "conv_pw2",
        conv: "conv_dw",
    };
    let codec_sizes =
        Heads { heads: 8, key_heads: 2, head_size: CODEC_HEAD_SIZE, keep: 0, remembers: false, eps: EPS };
    let kinds: Vec<LayerKind> = (0..8)
        .map(|layer| if matches!(layer, 2 | 4 | 6) { LayerKind::Window } else { LayerKind::Conv })
        .collect();
    let codec_stack =
        Stack::from_gguf(content, reader, device, "a.gen.wav.tfm.blk", &codec_names, codec_sizes, &kinds)?;

    let mut tensor = |name: &str| content.tensor(reader, name, device);
    let mut plain = |name: &str| tensor(name)?.dequantize(device);
    let tables = plain("a.gen.code.embd.weight")?;
    let tables = (0..CODEBOOKS)
        .map(|index| tables.narrow(0, index, 1)?.squeeze(0)?.contiguous())
        .collect::<candle_core::Result<Vec<_>>>()?;
    let head_norms = plain("a.gen.code.head_norm.weight")?;
    let head_norms = (0..CODEBOOKS)
        .map(|index| head_norms.narrow(0, index, 1)?.squeeze(0)?.contiguous())
        .collect::<candle_core::Result<Vec<_>>>()?;
    let mask_code = plain("a.gen.code.mask_embd")?;
    let frame_embeds = plain("a.gen.code.out_embd.weight")?;
    let into_bias = plain("a.gen.code.proj_in.bias")?;
    let values = plain("a.gen.wav.quant.first.codebook.weight")?.flatten_all()?.to_vec1::<f32>()?;
    let from_values_bias = plain("a.gen.wav.quant.first.out_proj.bias")?;
    let codec_into_bias = plain("a.gen.wav.tfm.in_proj.bias")?;
    let out_bias = plain("a.gen.wav.tfm.out_proj.bias")?;
    let mut tensor = |name: &str| content.tensor(reader, name, device);
    let depth = DepthDecoder {
        into: QMatMul::from_qtensor(tensor("a.gen.code.proj_in.weight")?)?,
        into_bias,
        mask_code,
        stack: depth_stack,
        norm: RmsNorm::from_qtensor(tensor("a.gen.code.output_norm.weight")?, EPS)?,
        tables,
        head_norms,
        frequencies: frequencies(DEPTH_HEAD_SIZE),
        eps: EPS as f32,
        device: device.clone(),
    };
    let codec = Codec {
        values,
        from_values: QMatMul::from_qtensor(tensor("a.gen.wav.quant.first.out_proj.weight")?)?,
        from_values_bias,
        into: QMatMul::from_qtensor(tensor("a.gen.wav.tfm.in_proj.weight")?)?,
        into_bias: codec_into_bias,
        stack: codec_stack,
        norm: RmsNorm::from_qtensor(tensor("a.gen.wav.tfm.output_norm.weight")?, EPS)?,
        out: QMatMul::from_qtensor(tensor("a.gen.wav.tfm.out_proj.weight")?)?,
        out_bias,
        frequencies: frequencies(CODEC_HEAD_SIZE),
        device: device.clone(),
    };
    Ok((depth, frame_embeds, codec))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_is_the_one_on_the_model_card() {
        let prompt = SpeechSettings::default().prompt("Oi");
        assert!(prompt.contains("system\nPerform TTS in Portuguese using a female voice.<|im_end|>"));
        assert!(prompt.ends_with("<|im_start|>assistant\n<|audio_start|>"));
        let male = SpeechSettings { voice: SpeechVoice::Male, ..SpeechSettings::default() };
        assert!(male.prompt("Oi").contains("using a male voice"));
    }

    #[test]
    fn the_settings_start_as_the_model_card_says() {
        let settings = SpeechSettings::default();
        assert_eq!(settings.temperature, 1.0);
        assert_eq!(settings.top_k, 128);
        assert_eq!(settings.repetition_penalty, 1.1);
        assert_eq!(settings.repetition_window, 4096);
    }

    #[test]
    fn the_order_has_each_codebook_once() {
        let mut sampler = Sampler::new(&SpeechSettings::default());
        let mut order = sampler.order();
        order.sort_unstable();
        assert_eq!(order, std::array::from_fn(|index| index));
    }

    #[test]
    fn a_silent_spectrum_is_silence_of_the_right_length() {
        let bins = FFT_SIZE / 2 + 1;
        let frames = vec![vec![-40.0_f32; bins * 2]; 12];
        let sound = from_spectrum(&frames);
        // Two frames of codes, of 1920 samples each.
        assert_eq!(sound.len(), 3840);
        assert!(sound.iter().all(|sample| sample.abs() < 1e-3));
    }

    #[test]
    fn repeated_codes_lose_points() {
        let settings = SpeechSettings { temperature: 0.0, ..SpeechSettings::default() };
        let mut sampler = Sampler::new(&settings);
        let logits = Tensor::new(&[1.05_f32, 1.0, 0.0], &Device::Cpu).expect("logits");
        assert_eq!(sampler.code(&logits_padded(&logits), &[0]).expect("code"), 1);
    }

    fn logits_padded(short: &Tensor) -> Tensor {
        let mut all = short.to_vec1::<f32>().expect("values");
        all.resize(CODES, -10.0);
        Tensor::new(all.as_slice(), &Device::Cpu).expect("logits")
    }
}
