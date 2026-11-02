//! LFM3 read from a GGUF file. candle stops at LFM2, and LFM3 changes the layers: some
//! attention looks only at a sliding window, attention heads have a gate, and each layer's
//! outputs pass through one more norm.

use candle_core::quantized::{QMatMul, QTensor, gguf_file};
use candle_core::{Device, Module, Result, Tensor, bail};
use candle_nn::Embedding;
use candle_nn::ops::{sigmoid, softmax_last_dim};
use candle_nn::rotary_emb::rope;
use candle_transformers::quantized_nn::RmsNorm;

/// How much a short convolution reads: the token and the six before it.
const CONV_WIDTH: usize = 7;

/// The cosines and sines of a kind of attention, at the positions being read.
struct Rotation {
    cos: Tensor,
    sin: Tensor,
}

/// Which tokens a kind of attention may read while going through the prompt.
#[derive(Clone, Copy)]
struct Reach<'a> {
    rotation: &'a Rotation,
    mask: Option<&'a Tensor>,
}

struct Feed {
    gate: QMatMul,
    up: QMatMul,
    down: QMatMul,
}

impl Feed {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        self.down.forward(&(self.gate.forward(xs)?.silu()? * self.up.forward(xs)?)?)
    }
}

struct Attention {
    query: QMatMul,
    key: QMatMul,
    value: QMatMul,
    out: QMatMul,
    gate: QMatMul,
    query_norm: RmsNorm,
    key_norm: RmsNorm,
    heads: usize,
    key_heads: usize,
    head_size: usize,
    /// The keys and values to keep, when this one looks only at a sliding window.
    keep: Option<usize>,
    cache: Option<(Tensor, Tensor)>,
}

impl Attention {
    fn forward(&mut self, xs: &Tensor, reach: Reach<'_>) -> Result<Tensor> {
        let (batch, tokens, size) = xs.dims3()?;
        let split = |tensor: Tensor, heads: usize| -> Result<Tensor> {
            tensor.reshape((batch, tokens, heads, self.head_size))?.transpose(1, 2)?.contiguous()
        };
        let query = self.query_norm.forward(&split(self.query.forward(xs)?, self.heads)?)?;
        let key = self.key_norm.forward(&split(self.key.forward(xs)?, self.key_heads)?)?;
        let value = split(self.value.forward(xs)?, self.key_heads)?;
        let query = rope(&query, &reach.rotation.cos, &reach.rotation.sin)?;
        let key = rope(&key, &reach.rotation.cos, &reach.rotation.sin)?;

        let (key, value) = match &self.cache {
            Some((old_key, old_value)) => {
                (Tensor::cat(&[old_key, &key], 2)?, Tensor::cat(&[old_value, &value], 2)?)
            }
            None => (key, value),
        };
        let seen = key.dim(2)?;
        self.cache = Some(match self.keep {
            Some(keep) if seen > keep => {
                (key.narrow(2, seen - keep, keep)?, value.narrow(2, seen - keep, keep)?)
            }
            _ => (key.clone(), value.clone()),
        });

        let groups = self.heads / self.key_heads;
        let key = candle_transformers::utils::repeat_kv(key, groups)?;
        let value = candle_transformers::utils::repeat_kv(value, groups)?.contiguous()?;
        let scores = (query.matmul(&key.t()?)? / (self.head_size as f64).sqrt())?;
        let scores = match reach.mask {
            Some(mask) => scores.broadcast_add(mask)?,
            None => scores,
        };
        let read = softmax_last_dim(&scores)?.matmul(&value)?.transpose(1, 2)?;
        let gate = sigmoid(&self.gate.forward(xs)?)?.unsqueeze(3)?;
        self.out.forward(&read.broadcast_mul(&gate)?.reshape((batch, tokens, size))?)
    }
}

struct ShortConv {
    into: QMatMul,
    out: QMatMul,
    /// One tensor for each place in the width, oldest token first.
    weights: Vec<Tensor>,
    cache: Option<Tensor>,
}

impl ShortConv {
    fn forward(&mut self, xs: &Tensor) -> Result<Tensor> {
        let (batch, tokens, size) = xs.dims3()?;
        let parts = self.into.forward(xs)?.transpose(1, 2)?;
        let mix = parts.narrow(1, size, size)?;
        let read = (parts.narrow(1, 0, size)? * parts.narrow(1, 2 * size, size)?)?;
        let before = match &self.cache {
            Some(cache) => cache.clone(),
            None => Tensor::zeros((batch, size, CONV_WIDTH - 1), read.dtype(), read.device())?,
        };
        let line = Tensor::cat(&[&before, &read], 2)?;
        self.cache = Some(line.narrow(2, tokens, CONV_WIDTH - 1)?);
        let mut sum = Tensor::zeros((batch, size, tokens), read.dtype(), read.device())?;
        for (place, weight) in self.weights.iter().enumerate() {
            sum = (sum + line.narrow(2, place, tokens)?.broadcast_mul(weight)?)?;
        }
        self.out.forward(&(mix * sum)?.transpose(1, 2)?.contiguous()?)
    }
}

enum Operator {
    Conv(ShortConv),
    Full(Attention),
    Window(Attention),
}

struct Layer {
    operator: Operator,
    feed: Feed,
    operator_norm: RmsNorm,
    after_operator_norm: RmsNorm,
    feed_norm: RmsNorm,
    after_feed_norm: RmsNorm,
}

/// An LFM3 model with its weights, reading one sequence at a time.
pub(crate) struct ModelWeights {
    embeddings: Embedding,
    layers: Vec<Layer>,
    norm: RmsNorm,
    output: QMatMul,
    full_frequencies: Vec<f32>,
    window_frequencies: Vec<f32>,
    window: usize,
    device: Device,
}

impl ModelWeights {
    /// Reads the weights in `content` from `reader`.
    pub(crate) fn from_gguf<R: std::io::Seek + std::io::Read>(
        content: &gguf_file::Content,
        reader: &mut R,
        device: &Device,
    ) -> Result<Self> {
        let value = |name: &str| match content.metadata.get(name) {
            Some(value) => Ok(value),
            None => bail!("no {name} in the model"),
        };
        let number = |name: &str| value(name)?.to_u32().map(|number| number as usize);
        let eps = f64::from(value("lfm3.attention.layer_norm_rms_epsilon")?.to_f32()?);
        let heads = number("lfm3.attention.head_count")?;
        let head_size = number("lfm3.attention.key_length")?;
        let window = number("lfm3.attention.sliding_window")?;
        let full_base = value("lfm3.rope.freq_base")?.to_f32()?;
        let window_base = value("lfm3.rope.freq_base_swa")?.to_f32()?;
        let key_heads = value("lfm3.attention.head_count_kv")?
            .to_vec()?
            .iter()
            .map(|heads| heads.to_i32().map(|heads| heads.unsigned_abs() as usize))
            .collect::<Result<Vec<_>>>()?;
        let windowed = value("lfm3.attention.sliding_window_pattern")?
            .to_vec()?
            .iter()
            .map(gguf_file::Value::to_bool)
            .collect::<Result<Vec<_>>>()?;

        let mut tensor = |name: &str| -> Result<QTensor> { content.tensor(reader, name, device) };
        let token_weights = tensor("token_embd.weight")?;
        let (_, size) = token_weights.shape().dims2()?;
        let embeddings = Embedding::new(token_weights.dequantize(device)?, size);
        let output = QMatMul::from_qtensor(token_weights)?;
        let norm = RmsNorm::from_qtensor(tensor("output_norm.weight")?, eps)?;
        let frequencies = |base: f32| -> Vec<f32> {
            (0..head_size).step_by(2).map(|place| 1.0 / base.powf(place as f32 / head_size as f32)).collect()
        };
        // The file divides the frequencies by these: so much that the second half does not rotate.
        let full_frequencies = frequencies(full_base)
            .into_iter()
            .zip(tensor("rope_freqs.weight")?.dequantize(device)?.to_vec1::<f32>()?)
            .map(|(frequency, divisor)| frequency / divisor)
            .collect();
        let window_frequencies = frequencies(window_base);

        let mut layers = Vec::with_capacity(key_heads.len());
        for (layer, key_heads) in key_heads.into_iter().enumerate() {
            let mut matrix = |name: &str| -> Result<QMatMul> {
                QMatMul::from_qtensor(tensor(&format!("blk.{layer}.{name}.weight"))?)
            };
            let feed = Feed { gate: matrix("ffn_gate")?, up: matrix("ffn_up")?, down: matrix("ffn_down")? };
            let operator = if key_heads == 0 {
                let into = matrix("shortconv.in_proj")?;
                let out = matrix("shortconv.out_proj")?;
                let weights = tensor(&format!("blk.{layer}.shortconv.conv.weight"))?.dequantize(device)?;
                if weights.dims2()? != (size, CONV_WIDTH) {
                    bail!("unexpected convolution of {:?}", weights.dims());
                }
                let weights = (0..CONV_WIDTH)
                    .map(|place| weights.narrow(1, place, 1)?.contiguous())
                    .collect::<Result<Vec<_>>>()?;
                Operator::Conv(ShortConv { into, out, weights, cache: None })
            } else {
                let in_window = windowed.get(layer).copied().unwrap_or(false);
                let query = matrix("attn_q")?;
                let key = matrix("attn_k")?;
                let value = matrix("attn_v")?;
                let out = matrix("attn_output")?;
                let gate = matrix("attn_gate")?;
                let attention = Attention {
                    query,
                    key,
                    value,
                    out,
                    gate,
                    query_norm: RmsNorm::from_qtensor(
                        tensor(&format!("blk.{layer}.attn_q_norm.weight"))?,
                        eps,
                    )?,
                    key_norm: RmsNorm::from_qtensor(
                        tensor(&format!("blk.{layer}.attn_k_norm.weight"))?,
                        eps,
                    )?,
                    heads,
                    key_heads,
                    head_size,
                    keep: in_window.then(|| window - 1),
                    cache: None,
                };
                if in_window { Operator::Window(attention) } else { Operator::Full(attention) }
            };
            let mut layer_norm = |name: &str| -> Result<RmsNorm> {
                RmsNorm::from_qtensor(tensor(&format!("blk.{layer}.{name}.weight"))?, eps)
            };
            layers.push(Layer {
                operator,
                feed,
                operator_norm: layer_norm("attn_norm")?,
                after_operator_norm: layer_norm("post_attention_norm")?,
                feed_norm: layer_norm("ffn_norm")?,
                after_feed_norm: layer_norm("post_ffw_norm")?,
            });
        }
        Ok(Self {
            embeddings,
            layers,
            norm,
            output,
            full_frequencies,
            window_frequencies,
            window,
            device: device.clone(),
        })
    }

    /// The logits for the token after `input`, which starts at position `offset`.
    pub(crate) fn forward(&mut self, input: &Tensor, offset: usize) -> Result<Tensor> {
        let (_, tokens) = input.dims2()?;
        let full = rotation(&self.full_frequencies, offset, tokens, &self.device)?;
        let in_window = rotation(&self.window_frequencies, offset, tokens, &self.device)?;
        // Only the prompt comes in more than one token at a time.
        let (full_mask, window_mask) = if tokens > 1 {
            (Some(mask(tokens, tokens, &self.device)?), Some(mask(tokens, self.window, &self.device)?))
        } else {
            (None, None)
        };
        let full = Reach { rotation: &full, mask: full_mask.as_ref() };
        let in_window = Reach { rotation: &in_window, mask: window_mask.as_ref() };

        let mut hidden = self.embeddings.forward(input)?;
        for layer in &mut self.layers {
            let normed = layer.operator_norm.forward(&hidden)?;
            let done = match &mut layer.operator {
                Operator::Conv(conv) => conv.forward(&normed)?,
                Operator::Full(attention) => attention.forward(&normed, full)?,
                Operator::Window(attention) => attention.forward(&normed, in_window)?,
            };
            hidden = (layer.after_operator_norm.forward(&done)? + hidden)?;
            let fed = layer.feed.forward(&layer.feed_norm.forward(&hidden)?)?;
            hidden = (layer.after_feed_norm.forward(&fed)? + hidden)?;
        }
        let last = self.norm.forward(&hidden)?.narrow(1, tokens - 1, 1)?.squeeze(1)?;
        self.output.forward(&last)
    }

    /// Forgets what it has read.
    pub(crate) fn clear_kv_cache(&mut self) {
        for layer in &mut self.layers {
            match &mut layer.operator {
                Operator::Conv(conv) => conv.cache = None,
                Operator::Full(attention) | Operator::Window(attention) => attention.cache = None,
            }
        }
    }
}

fn rotation(frequencies: &[f32], offset: usize, tokens: usize, device: &Device) -> Result<Rotation> {
    let angles: Vec<f32> = (offset..offset + tokens)
        .flat_map(|position| frequencies.iter().map(move |frequency| position as f32 * frequency))
        .collect();
    let angles = Tensor::from_vec(angles, (tokens, frequencies.len()), device)?;
    Ok(Rotation { cos: angles.cos()?, sin: angles.sin()? })
}

/// What to add to the scores of `tokens` tokens, so that each reads only itself and the
/// ones before it, `reach` tokens at most.
fn mask(tokens: usize, reach: usize, device: &Device) -> Result<Tensor> {
    let values: Vec<f32> = (0..tokens)
        .flat_map(|row| {
            (0..tokens).map(
                move |column| {
                    if column <= row && row - column < reach { 0.0 } else { f32::NEG_INFINITY }
                },
            )
        })
        .collect();
    Tensor::from_vec(values, (tokens, tokens), device)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_reads_only_what_came_before_and_within_reach() {
        let values = mask(4, 2, &Device::Cpu).and_then(|mask| mask.to_vec2::<f32>()).expect("mask");
        let read: Vec<Vec<bool>> =
            values.iter().map(|row| row.iter().map(|value| value.is_finite()).collect()).collect();
        assert_eq!(read[0], [true, false, false, false]);
        assert_eq!(read[1], [true, true, false, false]);
        assert_eq!(read[3], [false, false, true, true]);
    }

    #[test]
    fn the_convolution_reads_the_same_token_by_token_or_all_at_once() {
        let device = Device::Cpu;
        let identity = Tensor::eye(2, candle_core::DType::F32, &device).expect("eye");
        let into = Tensor::cat(&[identity.clone(), identity.clone(), identity.clone()], 0).expect("into");
        let weights: Vec<Tensor> = (0..CONV_WIDTH)
            .map(|place| Tensor::new(&[[place as f32 + 1.0], [1.0]], &device).expect("weight"))
            .collect();
        let conv = || ShortConv {
            into: QMatMul::Tensor(into.clone()),
            out: QMatMul::Tensor(identity.clone()),
            weights: weights.clone(),
            cache: None,
        };
        let line: Vec<f32> = (0..18).map(|value| value as f32 / 10.0).collect();
        let input = Tensor::from_vec(line, (1, 9, 2), &device).expect("input");
        let together = conv().forward(&input).expect("together").to_vec3::<f32>().expect("values");
        let mut parts = conv();
        for (token, row) in together[0].iter().enumerate() {
            let one = parts.forward(&input.narrow(1, token, 1).expect("one")).expect("forward");
            let one = one.to_vec3::<f32>().expect("values");
            for (written, expected) in one[0][0].iter().zip(row) {
                assert!((written - expected).abs() < 1e-4, "{written} and {expected}");
            }
        }
    }
}
