//! Speech from Kokoro 82M, following its published code: a small BERT reads the
//! phonemes, a predictor says how long each one lasts and how high and loud it is, and a
//! decoder turns that into the spectrum of 24 kHz sound.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use candle_core::pickle::PthTensors;
use candle_core::{D, Device, Module, Result, Tensor};
use candle_nn::Linear;
use candle_nn::ops::{layer_norm, sigmoid, softmax_last_dim};

use crate::spectrum::{hann, inverse_stft, stft};

/// How many phonemes fit at once, without the two that open and close them.
pub(crate) const MOST_PHONEMES: usize = 510;
const STYLE: usize = 128;
const BERT_LAYERS: usize = 12;
const BERT_HEADS: usize = 12;
const BERT_EPS: f32 = 1e-12;
const EPS: f32 = 1e-5;
const N_FFT: usize = 20;
const HOP: usize = 5;
/// Samples of sound for each frame the predictor writes.
const UPSAMPLE: usize = 300;
const HARMONICS: usize = 9;
const RATE: f32 = 24_000.0;
const SINE_AMP: f32 = 0.1;
const NOISE_STD: f32 = 0.003;
const VOICED: f32 = 10.0;

/// The weights as the file names them, without the `module.` that wraps each part.
struct Weights {
    tensors: HashMap<String, Tensor>,
}

impl Weights {
    fn load(path: &Path) -> Result<Self> {
        let mut tensors = HashMap::new();
        for part in ["bert", "bert_encoder", "predictor", "text_encoder", "decoder"] {
            let file = PthTensors::new(path, Some(part))?;
            for name in file.tensor_infos().keys() {
                let Some(tensor) = file.get(name)? else { continue };
                let short = name.strip_prefix("module.").unwrap_or(name);
                tensors.insert(format!("{part}.{short}"), tensor);
            }
        }
        Ok(Self { tensors })
    }

    fn get(&self, name: &str) -> Result<Tensor> {
        self.tensors.get(name).cloned().ok_or_else(|| candle_core::Error::Msg(format!("no {name}")))
    }

    /// A weight, or the one its weight norm stands for: each output's direction times
    /// its length.
    fn weight(&self, name: &str) -> Result<Tensor> {
        if let Some(weight) = self.tensors.get(&format!("{name}.weight")) {
            return Ok(weight.clone());
        }
        let length = self.get(&format!("{name}.weight_g"))?;
        let direction = self.get(&format!("{name}.weight_v"))?;
        let norm = direction.sqr()?.sum_keepdim(2)?.sum_keepdim(1)?.sqrt()?;
        length.broadcast_mul(&direction.broadcast_div(&norm)?)
    }

    fn bias(&self, name: &str) -> Option<Tensor> {
        self.tensors.get(&format!("{name}.bias")).cloned()
    }

    fn linear(&self, name: &str) -> Result<Linear> {
        Ok(Linear::new(self.get(&format!("{name}.weight"))?, self.bias(name)))
    }

    fn conv(&self, name: &str, padding: usize, stride: usize, dilation: usize) -> Result<Conv> {
        Ok(Conv { weight: self.weight(name)?, bias: self.bias(name), padding, stride, dilation, groups: 1 })
    }

    fn lstm(&self, name: &str) -> Result<Lstm> {
        Ok(Lstm {
            forward: Direction::load(self, name, "")?,
            backward: Direction::load(self, name, "_reverse")?,
        })
    }
}

fn leaky(x: &Tensor, slope: f64) -> Result<Tensor> {
    x.maximum(&x.affine(slope, 0.0)?)
}

/// Normalizes each channel over time, as `InstanceNorm1d` does.
fn instance_norm(x: &Tensor) -> Result<Tensor> {
    let centered = x.broadcast_sub(&x.mean_keepdim(D::Minus1)?)?;
    let variance = centered.sqr()?.mean_keepdim(D::Minus1)?;
    centered.broadcast_div(&(variance + f64::from(EPS))?.sqrt()?)
}

/// Normalizes the last dimension, with no weights of its own.
fn plain_layer_norm(x: &Tensor) -> Result<Tensor> {
    let size = x.dim(D::Minus1)?;
    let ones = Tensor::ones(size, x.dtype(), x.device())?;
    let zeros = Tensor::zeros(size, x.dtype(), x.device())?;
    layer_norm(x, &ones, &zeros, EPS)
}

struct Conv {
    weight: Tensor,
    bias: Option<Tensor>,
    padding: usize,
    stride: usize,
    dilation: usize,
    groups: usize,
}

impl Conv {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let y = x.conv1d(&self.weight, self.padding, self.stride, self.dilation, self.groups)?;
        match &self.bias {
            Some(bias) => y.broadcast_add(&bias.reshape((1, (), 1))?),
            None => Ok(y),
        }
    }
}

struct ConvTranspose {
    weight: Tensor,
    bias: Option<Tensor>,
    padding: usize,
    output_padding: usize,
    stride: usize,
    groups: usize,
}

impl ConvTranspose {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let y =
            x.conv_transpose1d(&self.weight, self.padding, self.output_padding, self.stride, 1, self.groups)?;
        match &self.bias {
            Some(bias) => y.broadcast_add(&bias.reshape((1, (), 1))?),
            None => Ok(y),
        }
    }
}

/// One direction of an LSTM. What the input adds to the gates is found for all steps at
/// once; only what the last step adds goes one step at a time.
struct Direction {
    input: Tensor,
    bias: Tensor,
    /// For each of the last step's values, its weight on every gate.
    recurrent: Vec<f32>,
    hidden: usize,
}

impl Direction {
    fn load(weights: &Weights, name: &str, suffix: &str) -> Result<Self> {
        let input = weights.get(&format!("{name}.weight_ih_l0{suffix}"))?.t()?.contiguous()?;
        let recurrent = weights.get(&format!("{name}.weight_hh_l0{suffix}"))?;
        let hidden = recurrent.dim(1)?;
        let bias = (weights.get(&format!("{name}.bias_ih_l0{suffix}"))?
            + weights.get(&format!("{name}.bias_hh_l0{suffix}"))?)?;
        let recurrent = recurrent.t()?.contiguous()?.flatten_all()?.to_vec1::<f32>()?;
        Ok(Self { input, bias, recurrent, hidden })
    }

    fn run(&self, x: &Tensor, backward: bool) -> Result<Vec<Vec<f32>>> {
        let gates_in = x.matmul(&self.input)?.broadcast_add(&self.bias)?.to_vec2::<f32>()?;
        let (hidden, four) = (self.hidden, self.hidden * 4);
        let (mut h, mut c) = (vec![0.0_f32; hidden], vec![0.0_f32; hidden]);
        let mut out = vec![Vec::new(); gates_in.len()];
        let steps: Vec<usize> =
            if backward { (0..gates_in.len()).rev().collect() } else { (0..gates_in.len()).collect() };
        let logistic = |value: f32| 1.0 / (1.0 + (-value).exp());
        for step in steps {
            let mut gates = gates_in[step].clone();
            for (value, row) in h.iter().zip(self.recurrent.chunks_exact(four)) {
                for (gate, weight) in gates.iter_mut().zip(row) {
                    *gate += value * weight;
                }
            }
            for place in 0..hidden {
                let input = logistic(gates[place]);
                let forget = logistic(gates[hidden + place]);
                let cell = gates[2 * hidden + place].tanh();
                let output = logistic(gates[3 * hidden + place]);
                c[place] = forget * c[place] + input * cell;
                h[place] = output * c[place].tanh();
            }
            out[step] = h.clone();
        }
        Ok(out)
    }
}

/// A bidirectional LSTM over `(time, features)`.
struct Lstm {
    forward: Direction,
    backward: Direction,
}

impl Lstm {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let ahead = self.forward.run(x, false)?;
        let behind = self.backward.run(x, true)?;
        let steps = ahead.len();
        let size = self.forward.hidden * 2;
        let mut all = Vec::with_capacity(steps * size);
        for (ahead, behind) in ahead.iter().zip(&behind) {
            all.extend_from_slice(ahead);
            all.extend_from_slice(behind);
        }
        Tensor::from_vec(all, (steps, size), x.device())
    }
}

/// Instance norm whose scale and shift come from the style.
struct AdaIn {
    fc: Linear,
}

impl AdaIn {
    fn forward(&self, x: &Tensor, style: &Tensor) -> Result<Tensor> {
        let h = self.fc.forward(style)?;
        let channels = h.dim(1)? / 2;
        let gamma = h.narrow(1, 0, channels)?.unsqueeze(2)?;
        let beta = h.narrow(1, channels, channels)?.unsqueeze(2)?;
        instance_norm(x)?.broadcast_mul(&(gamma + 1.0)?)?.broadcast_add(&beta)
    }
}

/// The residual block of the predictor and the decoder, which may double the time.
struct AdaResBlock {
    conv1: Conv,
    conv2: Conv,
    norm1: AdaIn,
    norm2: AdaIn,
    shortcut: Option<Conv>,
    pool: Option<ConvTranspose>,
}

impl AdaResBlock {
    fn load(weights: &Weights, name: &str, upsample: bool) -> Result<Self> {
        let shortcut = weights
            .tensors
            .contains_key(&format!("{name}.conv1x1.weight_v"))
            .then(|| weights.conv(&format!("{name}.conv1x1"), 0, 1, 1))
            .transpose()?;
        let pool = if upsample {
            let weight = weights.weight(&format!("{name}.pool"))?;
            let groups = weight.dim(0)?;
            Some(ConvTranspose {
                weight,
                bias: weights.bias(&format!("{name}.pool")),
                padding: 1,
                output_padding: 1,
                stride: 2,
                groups,
            })
        } else {
            None
        };
        Ok(Self {
            conv1: weights.conv(&format!("{name}.conv1"), 1, 1, 1)?,
            conv2: weights.conv(&format!("{name}.conv2"), 1, 1, 1)?,
            norm1: AdaIn { fc: weights.linear(&format!("{name}.norm1.fc"))? },
            norm2: AdaIn { fc: weights.linear(&format!("{name}.norm2.fc"))? },
            shortcut,
            pool,
        })
    }

    fn forward(&self, x: &Tensor, style: &Tensor) -> Result<Tensor> {
        let mut residual = leaky(&self.norm1.forward(x, style)?, 0.2)?;
        if let Some(pool) = &self.pool {
            residual = pool.forward(&residual)?;
        }
        let residual = self.conv1.forward(&residual)?;
        let residual = leaky(&self.norm2.forward(&residual, style)?, 0.2)?;
        let residual = self.conv2.forward(&residual)?;
        let mut shortcut = x.clone();
        if self.pool.is_some() {
            let time = shortcut.dim(2)?;
            shortcut = shortcut.upsample_nearest1d(time * 2)?;
        }
        if let Some(conv) = &self.shortcut {
            shortcut = conv.forward(&shortcut)?;
        }
        (residual + shortcut)? * std::f64::consts::FRAC_1_SQRT_2
    }
}

/// The generator's residual block, with snake activations.
struct SnakeBlock {
    first: Vec<(AdaIn, Tensor, Conv)>,
    second: Vec<(AdaIn, Tensor, Conv)>,
}

impl SnakeBlock {
    fn load(weights: &Weights, name: &str, kernel: usize) -> Result<Self> {
        let mut first = Vec::new();
        let mut second = Vec::new();
        for (index, dilation) in [1, 3, 5].into_iter().enumerate() {
            first.push((
                AdaIn { fc: weights.linear(&format!("{name}.adain1.{index}.fc"))? },
                weights.get(&format!("{name}.alpha1.{index}"))?,
                weights.conv(
                    &format!("{name}.convs1.{index}"),
                    (kernel * dilation - dilation) / 2,
                    1,
                    dilation,
                )?,
            ));
            second.push((
                AdaIn { fc: weights.linear(&format!("{name}.adain2.{index}.fc"))? },
                weights.get(&format!("{name}.alpha2.{index}"))?,
                weights.conv(&format!("{name}.convs2.{index}"), (kernel - 1) / 2, 1, 1)?,
            ));
        }
        Ok(Self { first, second })
    }

    fn forward(&self, x: &Tensor, style: &Tensor) -> Result<Tensor> {
        let snake = |x: &Tensor, alpha: &Tensor| -> Result<Tensor> {
            x + x.broadcast_mul(alpha)?.sin()?.sqr()?.broadcast_div(alpha)?
        };
        let mut x = x.clone();
        for ((norm1, alpha1, conv1), (norm2, alpha2, conv2)) in self.first.iter().zip(&self.second) {
            let xt = conv1.forward(&snake(&norm1.forward(&x, style)?, alpha1)?)?;
            let xt = conv2.forward(&snake(&norm2.forward(&xt, style)?, alpha2)?)?;
            x = (xt + x)?;
        }
        Ok(x)
    }
}

/// The BERT that reads the phonemes: one layer, used twelve times.
struct Albert {
    words: Tensor,
    positions: Tensor,
    kind: Tensor,
    norm: (Tensor, Tensor),
    into: Linear,
    query: Linear,
    key: Linear,
    value: Linear,
    dense: Linear,
    attention_norm: (Tensor, Tensor),
    ffn: Linear,
    ffn_output: Linear,
    full_norm: (Tensor, Tensor),
}

impl Albert {
    fn load(weights: &Weights) -> Result<Self> {
        let layer = "bert.encoder.albert_layer_groups.0.albert_layers.0";
        let norm = |name: &str| -> Result<(Tensor, Tensor)> {
            Ok((weights.get(&format!("{name}.weight"))?, weights.get(&format!("{name}.bias"))?))
        };
        Ok(Self {
            words: weights.get("bert.embeddings.word_embeddings.weight")?,
            positions: weights.get("bert.embeddings.position_embeddings.weight")?,
            kind: weights.get("bert.embeddings.token_type_embeddings.weight")?.narrow(0, 0, 1)?,
            norm: norm("bert.embeddings.LayerNorm")?,
            into: weights.linear("bert.encoder.embedding_hidden_mapping_in")?,
            query: weights.linear(&format!("{layer}.attention.query"))?,
            key: weights.linear(&format!("{layer}.attention.key"))?,
            value: weights.linear(&format!("{layer}.attention.value"))?,
            dense: weights.linear(&format!("{layer}.attention.dense"))?,
            attention_norm: norm(&format!("{layer}.attention.LayerNorm"))?,
            ffn: weights.linear(&format!("{layer}.ffn"))?,
            ffn_output: weights.linear(&format!("{layer}.ffn_output"))?,
            full_norm: norm(&format!("{layer}.full_layer_layer_norm"))?,
        })
    }

    fn forward(&self, ids: &Tensor) -> Result<Tensor> {
        let count = ids.dim(0)?;
        let x = self
            .words
            .index_select(ids, 0)?
            .add(&self.positions.narrow(0, 0, count)?)?
            .broadcast_add(&self.kind)?;
        let x = layer_norm(&x, &self.norm.0, &self.norm.1, BERT_EPS)?;
        let mut x = self.into.forward(&x)?;
        let size = x.dim(1)?;
        let head = size / BERT_HEADS;
        let scale = 1.0 / (head as f64).sqrt();
        for _ in 0..BERT_LAYERS {
            let heads = |linear: &Linear| -> Result<Tensor> {
                linear.forward(&x)?.reshape((count, BERT_HEADS, head))?.transpose(0, 1)?.contiguous()
            };
            let (query, key, value) = (heads(&self.query)?, heads(&self.key)?, heads(&self.value)?);
            let scores = (query.matmul(&key.t()?)? * scale)?;
            let context =
                softmax_last_dim(&scores)?.matmul(&value)?.transpose(0, 1)?.reshape((count, size))?;
            let attended = (&x + self.dense.forward(&context)?)?;
            let attended = layer_norm(&attended, &self.attention_norm.0, &self.attention_norm.1, BERT_EPS)?;
            let fed = self.ffn_output.forward(&self.ffn.forward(&attended)?.gelu()?)?;
            x = layer_norm(&(fed + &attended)?, &self.full_norm.0, &self.full_norm.1, BERT_EPS)?;
        }
        Ok(x)
    }
}

/// What reads the phonemes for the sound itself.
struct TextEncoder {
    embedding: Tensor,
    convs: Vec<(Conv, Tensor, Tensor)>,
    lstm: Lstm,
}

impl TextEncoder {
    fn load(weights: &Weights) -> Result<Self> {
        let convs = (0..3)
            .map(|index| {
                Ok((
                    weights.conv(&format!("text_encoder.cnn.{index}.0"), 2, 1, 1)?,
                    weights.get(&format!("text_encoder.cnn.{index}.1.gamma"))?,
                    weights.get(&format!("text_encoder.cnn.{index}.1.beta"))?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            embedding: weights.get("text_encoder.embedding.weight")?,
            convs,
            lstm: weights.lstm("text_encoder.lstm")?,
        })
    }

    /// `(1, channels, phonemes)`.
    fn forward(&self, ids: &Tensor) -> Result<Tensor> {
        let mut x = self.embedding.index_select(ids, 0)?.t()?.unsqueeze(0)?.contiguous()?;
        for (conv, gamma, beta) in &self.convs {
            let y = conv.forward(&x)?.transpose(1, 2)?;
            let y = layer_norm(&y.contiguous()?, gamma, beta, EPS)?.transpose(1, 2)?;
            x = leaky(&y, 0.2)?;
        }
        let x = self.lstm.forward(&x.squeeze(0)?.t()?.contiguous()?)?;
        x.t()?.unsqueeze(0)?.contiguous()
    }
}

/// How long each phoneme lasts, and the pitch and loudness of each frame.
struct Predictor {
    lstms: Vec<(Lstm, Linear)>,
    lstm: Lstm,
    duration: Linear,
    shared: Lstm,
    pitch: Vec<AdaResBlock>,
    loudness: Vec<AdaResBlock>,
    pitch_out: Conv,
    loudness_out: Conv,
}

impl Predictor {
    fn load(weights: &Weights) -> Result<Self> {
        let lstms = (0..3)
            .map(|index| {
                Ok((
                    weights.lstm(&format!("predictor.text_encoder.lstms.{}", index * 2))?,
                    weights.linear(&format!("predictor.text_encoder.lstms.{}.fc", index * 2 + 1))?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let blocks = |name: &str| -> Result<Vec<AdaResBlock>> {
            (0..3)
                .map(|index| AdaResBlock::load(weights, &format!("predictor.{name}.{index}"), index == 1))
                .collect()
        };
        Ok(Self {
            lstms,
            lstm: weights.lstm("predictor.lstm")?,
            duration: weights.linear("predictor.duration_proj.linear_layer")?,
            shared: weights.lstm("predictor.shared")?,
            pitch: blocks("F0")?,
            loudness: blocks("N")?,
            pitch_out: weights.conv("predictor.F0_proj", 0, 1, 1)?,
            loudness_out: weights.conv("predictor.N_proj", 0, 1, 1)?,
        })
    }

    /// `(phonemes, features)` from the BERT's reading, `(1, 512, phonemes)`.
    fn encode(&self, reading: &Tensor, style: &Tensor) -> Result<Tensor> {
        let count = reading.dim(2)?;
        let styles = style.broadcast_as((count, STYLE))?;
        let mut x = Tensor::cat(&[&reading.squeeze(0)?.t()?, &styles], 1)?;
        for (lstm, fc) in &self.lstms {
            let y = lstm.forward(&x)?;
            let h = fc.forward(style)?;
            let channels = h.dim(1)? / 2;
            let gamma = h.narrow(1, 0, channels)?;
            let beta = h.narrow(1, channels, channels)?;
            let y = plain_layer_norm(&y)?.broadcast_mul(&(gamma + 1.0)?)?.broadcast_add(&beta)?;
            x = Tensor::cat(&[&y, &styles], 1)?;
        }
        Ok(x)
    }

    /// Frames for each phoneme, rounded as PyTorch rounds: halves to even.
    fn durations(&self, encoded: &Tensor, speed: f32) -> Result<Vec<usize>> {
        let x = self.lstm.forward(encoded)?;
        let sums = sigmoid(&self.duration.forward(&x)?)?.sum(1)?.to_vec1::<f32>()?;
        Ok(sums.iter().map(|sum| ((sum / speed).round_ties_even().max(1.0)) as usize).collect())
    }

    /// Pitch and loudness for frames `(frames, features)`, each `(1, 1, 2 * frames)`.
    fn pitch_and_loudness(&self, frames: &Tensor, style: &Tensor) -> Result<(Tensor, Tensor)> {
        let x = self.shared.forward(frames)?.t()?.unsqueeze(0)?.contiguous()?;
        let mut pitch = x.clone();
        for block in &self.pitch {
            pitch = block.forward(&pitch, style)?;
        }
        let mut loudness = x;
        for block in &self.loudness {
            loudness = block.forward(&loudness, style)?;
        }
        Ok((self.pitch_out.forward(&pitch)?, self.loudness_out.forward(&loudness)?))
    }
}

/// A small random source, the same for the same seed on every system.
pub(crate) struct Noise {
    state: u64,
    /// Silence in place of noise, for comparing with the published code.
    quiet: bool,
}

impl Noise {
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed ^ 0x9E37_79B9_7F4A_7C15, quiet: false }
    }

    #[cfg(test)]
    pub(crate) fn quiet() -> Self {
        Self { state: 0, quiet: true }
    }

    fn uniform(&mut self) -> f32 {
        // SplitMix64.
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        ((z >> 40) as f32 + 0.5) / (1u64 << 24) as f32
    }

    fn normal(&mut self) -> f32 {
        if self.quiet {
            return 0.0;
        }
        let (a, b) = (self.uniform(), self.uniform());
        (-2.0 * a.ln()).sqrt() * (std::f32::consts::TAU * b).cos()
    }
}

/// Linear interpolation as PyTorch does it with `align_corners=False`.
fn interpolate(values: &[f32], size: usize, scale: f32) -> Vec<f32> {
    let last = values.len() - 1;
    let step = 1.0 / scale;
    (0..size)
        .map(|place| {
            let source = (step * (place as f32 + 0.5) - 0.5).max(0.0);
            let low = (source as usize).min(last);
            let high = (low + 1).min(last);
            let weight = source - low as f32;
            values[low] * (1.0 - weight) + values[high] * weight
        })
        .collect()
}

/// Sees the stages as they are made, for comparing them with the published code.
type Trace<'a> = dyn FnMut(&'static str, &Tensor) + 'a;

/// The decoder, and the generator that writes the spectrum.
struct Decoder {
    pitch_conv: Conv,
    loudness_conv: Conv,
    encode: AdaResBlock,
    asr_res: Conv,
    decode: Vec<AdaResBlock>,
    source: Linear,
    noise_convs: Vec<Conv>,
    noise_res: Vec<SnakeBlock>,
    ups: Vec<ConvTranspose>,
    resblocks: Vec<SnakeBlock>,
    post: Conv,
    window: Vec<f32>,
}

impl Decoder {
    fn load(weights: &Weights) -> Result<Self> {
        let decode = (0..4)
            .map(|index| AdaResBlock::load(weights, &format!("decoder.decode.{index}"), index == 3))
            .collect::<Result<Vec<_>>>()?;
        let ups = [(5, 10), (3, 6)]
            .into_iter()
            .enumerate()
            .map(|(index, (padding, stride))| {
                let name = format!("decoder.generator.ups.{index}");
                Ok(ConvTranspose {
                    weight: weights.weight(&name)?,
                    bias: weights.bias(&name),
                    padding,
                    output_padding: 0,
                    stride,
                    groups: 1,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut resblocks = Vec::new();
        for index in 0..6 {
            let kernel = [3, 7, 11][index % 3];
            resblocks.push(SnakeBlock::load(
                weights,
                &format!("decoder.generator.resblocks.{index}"),
                kernel,
            )?);
        }
        Ok(Self {
            pitch_conv: weights.conv("decoder.F0_conv", 1, 2, 1)?,
            loudness_conv: weights.conv("decoder.N_conv", 1, 2, 1)?,
            encode: AdaResBlock::load(weights, "decoder.encode", false)?,
            asr_res: weights.conv("decoder.asr_res.0", 0, 1, 1)?,
            decode,
            source: weights.linear("decoder.generator.m_source.l_linear")?,
            noise_convs: vec![
                weights.conv("decoder.generator.noise_convs.0", 3, 6, 1)?,
                weights.conv("decoder.generator.noise_convs.1", 0, 1, 1)?,
            ],
            noise_res: vec![
                SnakeBlock::load(weights, "decoder.generator.noise_res.0", 7)?,
                SnakeBlock::load(weights, "decoder.generator.noise_res.1", 11)?,
            ],
            ups,
            resblocks,
            post: weights.conv("decoder.generator.conv_post", 3, 1, 1)?,
            window: hann(N_FFT),
        })
    }

    /// The source the generator shapes: sines at the pitch and its harmonics where the
    /// voice sounds, noise where it does not. Its spectrum, `(1, 22, frames)`.
    #[allow(clippy::needless_range_loop)]
    fn harmonics(
        &self,
        pitch: &[f32],
        noise: &mut Noise,
        device: &Device,
        trace: &mut Trace<'_>,
    ) -> Result<Tensor> {
        let frames = pitch.len();
        let samples = frames * UPSAMPLE;
        // The phase moves once per frame, then is spread over the frame's samples. The
        // published code also starts each harmonic at a random phase, on the first
        // sample only, which its averaging over the frame then leaves out.
        let mut sines = vec![[0.0_f32; HARMONICS]; samples];
        for harmonic in 0..HARMONICS {
            // In f32 and in the published code's order, which keeps the phase close to
            // its own over a long sentence.
            let mut phase = 0.0_f32;
            let phases: Vec<f32> = pitch
                .iter()
                .map(|f0| {
                    phase += (f0 * (harmonic + 1) as f32 / RATE).rem_euclid(1.0);
                    phase * 2.0 * std::f32::consts::PI * UPSAMPLE as f32
                })
                .collect();
            for (place, value) in interpolate(&phases, samples, UPSAMPLE as f32).into_iter().enumerate() {
                sines[place][harmonic] = value.sin() * SINE_AMP;
            }
        }
        let weights = self.source.weight().flatten_all()?.to_vec1::<f32>()?;
        let bias =
            self.source.bias().map_or(Ok(0.0), |bias| bias.flatten_all()?.to_vec1::<f32>().map(|b| b[0]))?;
        let mut source = Vec::with_capacity(samples);
        for (place, sine) in sines.iter().enumerate() {
            let voiced = pitch[place / UPSAMPLE] > VOICED;
            let amplitude = if voiced { NOISE_STD } else { SINE_AMP / 3.0 };
            let mut sum = bias;
            for (value, weight) in sine.iter().zip(&weights) {
                let value = if voiced { *value } else { 0.0 } + amplitude * noise.normal();
                sum += value * weight;
            }
            source.push(sum.tanh());
        }
        trace("har_source", &Tensor::new(source.as_slice(), device)?);
        let (magnitudes, phases) = stft(&source, N_FFT, HOP, &self.window);
        let count = magnitudes.len();
        let bins = N_FFT / 2 + 1;
        let mut all = vec![0.0_f32; 2 * bins * count];
        for (frame, (magnitude, phase)) in magnitudes.iter().zip(&phases).enumerate() {
            for bin in 0..bins {
                all[bin * count + frame] = magnitude[bin];
                all[(bins + bin) * count + frame] = phase[bin];
            }
        }
        Tensor::from_vec(all, (1, 2 * bins, count), device)
    }

    fn forward(
        &self,
        asr: &Tensor,
        pitch: &Tensor,
        loudness: &Tensor,
        style: &Tensor,
        noise: &mut Noise,
        trace: &mut Trace<'_>,
    ) -> Result<Vec<f32>> {
        let small_pitch = self.pitch_conv.forward(pitch)?;
        let small_loudness = self.loudness_conv.forward(loudness)?;
        let mut x = self.encode.forward(&Tensor::cat(&[asr, &small_pitch, &small_loudness], 1)?, style)?;
        let asr_res = self.asr_res.forward(asr)?;
        let mut joined = true;
        for block in &self.decode {
            if joined {
                x = Tensor::cat(&[&x, &asr_res, &small_pitch, &small_loudness], 1)?;
            }
            x = block.forward(&x, style)?;
            joined = block.pool.is_none();
        }
        trace("decoder_decode3", &x);
        let pitch = pitch.flatten_all()?.to_vec1::<f32>()?;
        let harmonics = self.harmonics(&pitch, noise, x.device(), trace)?;
        for (index, up) in self.ups.iter().enumerate() {
            x = leaky(&x, 0.1)?;
            let from_source =
                self.noise_res[index].forward(&self.noise_convs[index].forward(&harmonics)?, style)?;
            x = up.forward(&x)?;
            trace(["ups0", "ups1"][index], &x);
            if index == self.ups.len() - 1 {
                // Reflects one sample in front.
                x = Tensor::cat(&[&x.narrow(2, 1, 1)?, &x], 2)?;
            }
            x = (x + from_source)?;
            let blocks = &self.resblocks[index * 3..index * 3 + 3];
            let mut sum = blocks[0].forward(&x, style)?;
            for block in &blocks[1..] {
                sum = (sum + block.forward(&x, style)?)?;
            }
            x = (sum / 3.0)?;
        }
        let x = self.post.forward(&leaky(&x, 0.01)?)?;
        trace("conv_post", &x);
        let x = x.squeeze(0)?;
        let bins = N_FFT / 2 + 1;
        let magnitudes = x.narrow(0, 0, bins)?.exp()?.t()?.to_vec2::<f32>()?;
        let phases = x.narrow(0, bins, bins)?.sin()?.t()?.to_vec2::<f32>()?;
        let sound = inverse_stft(&magnitudes, &phases, N_FFT, HOP, &self.window);
        let length = HOP * (magnitudes.len() - 1);
        Ok(sound[N_FFT / 2..N_FFT / 2 + length].to_vec())
    }
}

/// A voice: one style for each length of text, as many phonemes as its row.
pub(crate) struct Voice {
    styles: Vec<f32>,
}

impl Voice {
    /// Reads a voice file as the model's authors publish it: a PyTorch file holding
    /// `(510, 1, 256)` numbers.
    pub(crate) fn load(path: &Path) -> std::result::Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
        let name = zip
            .file_names()
            .filter_map(|name| name.ok().map(|name| name.into_owned()))
            .find(|name| name.ends_with("/data/0"))
            .ok_or_else(|| "no data in the voice".to_owned())?;
        let mut bytes = Vec::new();
        zip.by_name(&name).map_err(|e| e.to_string())?.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        if bytes.len() != MOST_PHONEMES * 2 * STYLE * 4 {
            return Err(format!("a voice of {} bytes", bytes.len()));
        }
        let styles = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
        Ok(Self { styles })
    }

    fn style(&self, phonemes: usize, device: &Device) -> Result<Tensor> {
        let row = phonemes.clamp(1, MOST_PHONEMES) - 1;
        let size = 2 * STYLE;
        Tensor::from_slice(&self.styles[row * size..(row + 1) * size], (1, size), device)
    }
}

/// Kokoro in memory.
pub(crate) struct Kokoro {
    bert: Albert,
    bert_encoder: Linear,
    predictor: Predictor,
    text_encoder: TextEncoder,
    decoder: Decoder,
    vocabulary: HashMap<char, u32>,
    device: Device,
}

impl Kokoro {
    /// Loads the weights and the phoneme vocabulary of `config.json`.
    pub(crate) fn load(weights: &Path, config: &Path) -> std::result::Result<Self, String> {
        let text = std::fs::read(config).map_err(|e| e.to_string())?;
        let config: serde_json::Value = serde_json::from_slice(&text).map_err(|e| e.to_string())?;
        let vocabulary = config["vocab"]
            .as_object()
            .ok_or_else(|| "no vocabulary".to_owned())?
            .iter()
            .filter_map(|(phoneme, id)| Some((phoneme.chars().next()?, u32::try_from(id.as_u64()?).ok()?)))
            .collect();
        let weights = Weights::load(weights).map_err(|e| e.to_string())?;
        let made = || -> Result<Self> {
            Ok(Self {
                bert: Albert::load(&weights)?,
                bert_encoder: weights.linear("bert_encoder")?,
                predictor: Predictor::load(&weights)?,
                text_encoder: TextEncoder::load(&weights)?,
                decoder: Decoder::load(&weights)?,
                vocabulary,
                device: Device::Cpu,
            })
        };
        made().map_err(|e| e.to_string())
    }

    /// The model's numbers for `phonemes`, opened and closed by the pause: what it does
    /// not know is left out.
    pub(crate) fn ids(&self, phonemes: &str) -> Vec<u32> {
        let mut ids = vec![0];
        ids.extend(phonemes.chars().filter_map(|phoneme| self.vocabulary.get(&phoneme)).take(MOST_PHONEMES));
        ids.push(0);
        ids
    }

    /// Sound for `phonemes`, at most [`MOST_PHONEMES`] of them, in `voice`.
    pub(crate) fn speak(
        &self,
        phonemes: &str,
        voice: &Voice,
        speed: f32,
        noise: &mut Noise,
    ) -> Result<Vec<f32>> {
        let ids = self.ids(phonemes);
        let style = voice.style(phonemes.chars().count(), &self.device)?;
        self.sound(&ids, &style, speed, noise, &mut |_, _| {}).map(|(sound, _)| sound)
    }

    /// Sound and the frames of each phoneme, for `ids` and a style of 256 numbers.
    pub(crate) fn sound(
        &self,
        ids: &[u32],
        style: &Tensor,
        speed: f32,
        noise: &mut Noise,
        trace: &mut Trace<'_>,
    ) -> Result<(Vec<f32>, Vec<usize>)> {
        let ids = Tensor::new(ids, &self.device)?;
        let prosody = style.narrow(1, STYLE, STYLE)?;
        let timbre = style.narrow(1, 0, STYLE)?;
        let reading =
            self.bert_encoder.forward(&self.bert.forward(&ids)?)?.t()?.unsqueeze(0)?.contiguous()?;
        let encoded = self.predictor.encode(&reading, &prosody)?;
        let durations = self.predictor.durations(&encoded, speed)?;
        let spread: Vec<u32> = durations
            .iter()
            .enumerate()
            .flat_map(|(index, frames)| std::iter::repeat_n(index as u32, *frames))
            .collect();
        let spread = Tensor::new(spread.as_slice(), &self.device)?;
        let frames = encoded.index_select(&spread, 0)?;
        let (pitch, loudness) = self.predictor.pitch_and_loudness(&frames, &prosody)?;
        trace("f0", &pitch);
        trace("n", &loudness);
        let asr = self.text_encoder.forward(&ids)?.index_select(&spread, 2)?;
        let sound = self.decoder.forward(&asr, &pitch, &loudness, &timbre, noise, trace)?;
        Ok((sound, durations))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_matches_pytorch_at_the_edges() {
        // F.interpolate([0, 10], scale_factor=2, mode="linear") == [0, 2.5, 7.5, 10].
        let values = interpolate(&[0.0, 10.0], 4, 2.0);
        assert_eq!(values, vec![0.0, 2.5, 7.5, 10.0]);
    }

    #[test]
    fn noise_is_the_same_for_the_same_seed() {
        let draws = |seed| {
            let mut noise = Noise::new(seed);
            (0..4).map(|_| noise.normal()).collect::<Vec<_>>()
        };
        assert_eq!(draws(7), draws(7));
        assert_ne!(draws(7), draws(8));
        assert!(Noise::quiet().normal() == 0.0);
    }

    /// Says a sentence into a wave file, for listening:
    /// `RAVENRS_KOKORO=folder RAVENRS_SAY="texto" RAVENRS_VOICE=pf_dora RAVENRS_SAY_OUT=file.wav cargo test --release -p ravenrs-ai kokoro_says -- --ignored`.
    #[test]
    #[ignore = "needs the real model"]
    fn kokoro_says() {
        let folder = std::path::PathBuf::from(std::env::var("RAVENRS_KOKORO").expect("RAVENRS_KOKORO"));
        let text = std::env::var("RAVENRS_SAY").expect("RAVENRS_SAY");
        let voice = std::env::var("RAVENRS_VOICE").unwrap_or_else(|_| "pf_dora".to_owned());
        let out = std::env::var("RAVENRS_SAY_OUT").expect("RAVENRS_SAY_OUT");
        let model =
            Kokoro::load(&folder.join("kokoro-v1_0.pth"), &folder.join("config.json")).expect("the model");
        let voice = Voice::load(&folder.join("voices").join(format!("{voice}.pt"))).expect("the voice");
        // Phonemes given as they are, to hear the model apart from the rules.
        let phonemes =
            std::env::var("RAVENRS_SAY_PHONEMES").unwrap_or_else(|_| crate::phonemes::phonemes_pt_br(&text));
        eprintln!("{phonemes}");
        let started = std::time::Instant::now();
        let sound = model.speak(&phonemes, &voice, 1.0, &mut Noise::new(1)).expect("sound");
        eprintln!("{:.1}s of sound in {:.1}s", sound.len() as f32 / RATE, started.elapsed().as_secs_f32());
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + sound.len() as u32 * 2).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&24_000_u32.to_le_bytes());
        wav.extend_from_slice(&48_000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(sound.len() as u32 * 2).to_le_bytes());
        for sample in &sound {
            wav.extend_from_slice(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
        std::fs::write(out, wav).expect("wav");
    }

    /// Compares each stage with the published code, from references it wrote:
    /// `RAVENRS_KOKORO=folder RAVENRS_KOKORO_REF=folder cargo test --release -p ravenrs-ai kokoro_matches -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the real model and references written by the published code"]
    fn kokoro_matches_the_published_code() {
        let folder = std::path::PathBuf::from(std::env::var("RAVENRS_KOKORO").expect("RAVENRS_KOKORO"));
        let references =
            std::path::PathBuf::from(std::env::var("RAVENRS_KOKORO_REF").expect("RAVENRS_KOKORO_REF"));
        let started = std::time::Instant::now();
        let model =
            Kokoro::load(&folder.join("kokoro-v1_0.pth"), &folder.join("config.json")).expect("the model");
        eprintln!("loaded in {:.1}s", started.elapsed().as_secs_f32());
        for name in ["curta", "longa"] {
            let reference =
                candle_core::safetensors::load(references.join(format!("{name}.safetensors")), &Device::Cpu)
                    .expect("references");
            let ids: Vec<u32> =
                reference["ids"].to_vec1::<i64>().expect("ids").iter().map(|id| *id as u32).collect();
            let style = &reference["ref_s"];
            let started = std::time::Instant::now();
            let mut stages = Vec::new();
            let (sound, durations) = model
                .sound(&ids, style, 1.0, &mut Noise::quiet(), &mut |name, tensor| {
                    stages.push((name, tensor.clone()));
                })
                .expect("sound");
            for (stage, tensor) in &stages {
                let ours = tensor.flatten_all().expect("ours").to_vec1::<f32>().expect("ours");
                let theirs =
                    reference[*stage].flatten_all().expect("theirs").to_vec1::<f32>().expect("theirs");
                let worst = ours.iter().zip(&theirs).map(|(a, b)| (a - b).abs()).fold(0.0_f32, f32::max);
                let scale = theirs.iter().map(|b| b.abs()).fold(0.0_f32, f32::max);
                eprintln!("  {stage}: worst {worst:.2e} of {scale:.2e}");
                assert_eq!(ours.len(), theirs.len(), "{name}: {stage}");
                // Up to the harmonics, the numbers are the same but for rounding. After
                // them, see below.
                if ["f0", "n", "decoder_decode3", "ups0"].contains(stage) {
                    assert!(worst < scale * 1e-4, "{name}: {stage} differs by {worst}");
                }
            }
            let took = started.elapsed().as_secs_f32();
            let expected: Vec<usize> = reference["pred_dur"]
                .to_vec1::<i64>()
                .expect("durations")
                .iter()
                .map(|d| *d as usize)
                .collect();
            assert_eq!(durations, expected, "{name}: durations");
            let wanted = reference["audio"].to_vec1::<f32>().expect("audio");
            assert_eq!(sound.len(), wanted.len());
            let error: f32 = sound.iter().zip(&wanted).map(|(a, b)| (a - b).powi(2)).sum::<f32>();
            let energy: f32 = wanted.iter().map(|b| b * b).sum::<f32>();
            let ratio = 10.0 * (energy / error.max(1e-12)).log10();
            eprintln!(
                "{name}: {:.1}s of sound in {took:.1}s, {ratio:.1} dB above the difference",
                sound.len() as f32 / RATE
            );
            // The angles of the near-silent bins of the harmonics change by up to a turn
            // when the source moves by a millionth, so no two runs agree to the sample:
            // the published code with its own noise is 15 dB from itself without it, and
            // 16 to 17 dB between two seeds. Here it is measured with no noise on either side.
            assert!(ratio > 12.0, "{name}: only {ratio:.1} dB");
        }
    }
}
