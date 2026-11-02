//! A film's narration recorded ahead of time: nobody who watches needs a voice model, or
//! a good system voice. Each text is kept once, compressed to a quarter of its size.

use std::collections::HashMap;

use ravenrs_widgets::SoundClip;

const MAGIC: &[u8; 4] = b"RVN1";
const HEADER: usize = 12;
const ENTRY: usize = 16;
/// The rate it is kept at: enough for a voice.
const RATE: u32 = 16_000;
/// How loud every recording ends up, so that a film does not change volume.
const LEVEL: f32 = 0.12;
const LOUDEST: f32 = 0.95;

const STEPS: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88,
    97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658,
    724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660,
    4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818,
    18500, 20350, 22385, 24623, 27086, 29794, 32767,
];
const STEP_CHANGE: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

/// Where the ADPCM decoder stands. The encoder follows the same path.
#[derive(Default)]
struct Adpcm {
    sample: i32,
    step: i32,
}

impl Adpcm {
    fn take(&mut self, code: u8) -> i32 {
        let step = STEPS[self.step as usize];
        let mut change = step >> 3;
        if code & 4 != 0 {
            change += step;
        }
        if code & 2 != 0 {
            change += step >> 1;
        }
        if code & 1 != 0 {
            change += step >> 2;
        }
        self.sample += if code & 8 != 0 { -change } else { change };
        self.sample = self.sample.clamp(-32768, 32767);
        self.step = (self.step + STEP_CHANGE[code as usize]).clamp(0, 88);
        self.sample
    }

    fn code_for(&self, sample: i32) -> u8 {
        let mut step = STEPS[self.step as usize];
        let difference = sample - self.sample;
        let mut code = if difference < 0 { 8 } else { 0 };
        let mut left = difference.abs();
        for bit in [4, 2, 1] {
            if left >= step {
                code |= bit;
                left -= step;
            }
            step >>= 1;
        }
        code
    }
}

fn encode(samples: &[f32]) -> Vec<u8> {
    let mut state = Adpcm::default();
    let mut out = vec![0_u8; samples.len().div_ceil(2)];
    for (index, sample) in samples.iter().enumerate() {
        let wanted = (sample.clamp(-1.0, 1.0) * 32767.0) as i32;
        let code = state.code_for(wanted);
        state.take(code);
        out[index / 2] |= if index % 2 == 0 { code } else { code << 4 };
    }
    out
}

fn decode(bytes: &[u8], samples: usize) -> Vec<f32> {
    let mut state = Adpcm::default();
    (0..samples)
        .map(|index| {
            let byte = bytes.get(index / 2).copied().unwrap_or(0);
            let code = if index % 2 == 0 { byte & 15 } else { byte >> 4 };
            state.take(code) as f32 / 32768.0
        })
        .collect()
}

/// `samples` at the rate kept, filtered first so that nothing above it folds back.
fn resampled(samples: &[f32], rate: u32) -> Vec<f32> {
    const REACH: isize = 12;
    if rate == RATE || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = f64::from(rate) / f64::from(RATE);
    let cutoff = (f64::from(RATE) / f64::from(rate)).min(1.0) * 0.95;
    let length = (samples.len() as f64 / ratio) as usize;
    (0..length)
        .map(|index| {
            let center = index as f64 * ratio;
            let nearest = center.round() as isize;
            let mut sum = 0.0;
            let mut weights = 0.0;
            for offset in -REACH..=REACH {
                let place = nearest + offset;
                let Some(sample) = usize::try_from(place).ok().and_then(|place| samples.get(place)) else {
                    continue;
                };
                let distance = place as f64 - center;
                let angle = std::f64::consts::PI * distance * cutoff;
                let sinc = if angle.abs() < 1e-9 { 1.0 } else { angle.sin() / angle };
                let window = 0.5 + 0.5 * (std::f64::consts::PI * distance / (REACH as f64 + 1.0)).cos();
                sum += f64::from(*sample) * sinc * window;
                weights += sinc * window;
            }
            if weights.abs() < 1e-9 { 0.0 } else { (sum / weights) as f32 }
        })
        .collect()
}

/// `samples` at the level kept, without clipping.
fn leveled(samples: &[f32]) -> Vec<f32> {
    let energy: f32 = samples.iter().map(|sample| sample * sample).sum();
    let level = (energy / samples.len().max(1) as f32).sqrt();
    let peak = samples.iter().fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    if level <= f32::EPSILON || peak <= f32::EPSILON {
        return samples.to_vec();
    }
    let gain = (LEVEL / level).min(LOUDEST / peak);
    samples.iter().map(|sample| sample * gain).collect()
}

/// The same number on every run and every machine, for finding a text.
fn key_of(text: &str) -> u64 {
    text.bytes()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3))
}

/// Recordings of what films say, each found by its text. It shares the bytes, so
/// cloning it is cheap. Give it to [`MoviePlayer::narration`](crate::MoviePlayer::narration).
#[derive(Clone, Debug)]
pub struct Narration {
    bytes: &'static [u8],
    entries: std::rc::Rc<HashMap<u64, (usize, usize)>>,
}

impl Narration {
    /// Reads what [`Narration::pack`] wrote. Bytes it does not understand hold no
    /// recordings, and the films use the system's voice.
    pub fn new(bytes: &'static [u8]) -> Self {
        Self { bytes, entries: std::rc::Rc::new(entries_of(bytes).unwrap_or_default()) }
    }

    /// The recording of `text`, if there is one.
    pub fn clip(&self, text: &str) -> Option<SoundClip> {
        let (samples, start) = *self.entries.get(&key_of(text))?;
        let body = self.bytes.get(start..start + samples.div_ceil(2))?;
        Some(SoundClip::new(decode(body, samples), RATE))
    }

    /// How many recordings it has.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it has none.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Packs recordings: each text with its samples, at `rate` a second.
    pub fn pack(recordings: &[(String, Vec<f32>)], rate: u32) -> Vec<u8> {
        let mut entries: Vec<(u64, usize, Vec<u8>)> = recordings
            .iter()
            .map(|(text, samples)| {
                let samples = leveled(&resampled(samples, rate));
                (key_of(text), samples.len(), encode(&samples))
            })
            .collect();
        // In a fixed order, so that a text that did not change stays the same in the file.
        entries.sort_by_key(|entry| entry.0);
        entries.dedup_by_key(|entry| entry.0);
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&RATE.to_le_bytes());
        out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        let mut start = HEADER + entries.len() * ENTRY;
        for (key, samples, body) in &entries {
            out.extend_from_slice(&key.to_le_bytes());
            out.extend_from_slice(&(*samples as u32).to_le_bytes());
            out.extend_from_slice(&(start as u32).to_le_bytes());
            start += body.len();
        }
        for (_, _, body) in &entries {
            out.extend_from_slice(body);
        }
        out
    }
}

fn entries_of(bytes: &[u8]) -> Option<HashMap<u64, (usize, usize)>> {
    if bytes.get(..4)? != MAGIC || u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?) != RATE {
        return None;
    }
    let count = u32::from_le_bytes(bytes.get(8..12)?.try_into().ok()?) as usize;
    let mut entries = HashMap::with_capacity(count);
    for index in 0..count {
        let at = HEADER + index * ENTRY;
        let entry = bytes.get(at..at + ENTRY)?;
        let key = u64::from_le_bytes(entry[..8].try_into().ok()?);
        let samples = u32::from_le_bytes(entry[8..12].try_into().ok()?) as usize;
        let start = u32::from_le_bytes(entry[12..16].try_into().ok()?) as usize;
        entries.insert(key, (samples, start));
    }
    Some(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A voice-like tone: a low note with a higher one on it.
    fn tone(rate: u32, seconds: f32) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize)
            .map(|index| {
                let time = index as f32 / rate as f32;
                let turn = std::f32::consts::TAU * time;
                0.3 * (turn * 180.0).sin() + 0.1 * (turn * 1200.0).sin()
            })
            .collect()
    }

    #[test]
    fn a_recording_comes_back_close_to_what_went_in() {
        let bytes = Narration::pack(&[("Oi".to_owned(), tone(24_000, 1.0))], 24_000);
        let narration = Narration::new(Box::leak(bytes.into_boxed_slice()));
        let clip = narration.clip("Oi").expect("the recording");
        assert_eq!(clip.rate(), RATE);
        assert_eq!(clip.samples().len(), RATE as usize);
        let wanted = leveled(&tone(RATE, 1.0));
        let noise: f32 = clip.samples().iter().zip(&wanted).skip(800).map(|(a, b)| (a - b).powi(2)).sum();
        let signal: f32 = wanted.iter().skip(800).map(|a| a * a).sum();
        assert!(noise / signal < 0.02, "{}", noise / signal);
    }

    #[test]
    fn it_takes_a_quarter_of_the_space_of_plain_samples() {
        let bytes = Narration::pack(&[("Oi".to_owned(), tone(RATE, 1.0))], RATE);
        assert!(bytes.len() <= (RATE as usize) / 2 + HEADER + ENTRY + 1);
    }

    #[test]
    fn a_text_with_no_recording_has_none() {
        let bytes = Narration::pack(&[("Oi".to_owned(), tone(RATE, 0.1))], RATE);
        let narration = Narration::new(Box::leak(bytes.into_boxed_slice()));
        assert_eq!(narration.len(), 1);
        assert!(narration.clip("Tchau").is_none());
    }

    #[test]
    fn bytes_it_does_not_understand_hold_nothing() {
        assert!(Narration::new(b"not a narration").is_empty());
        assert!(Narration::new(b"").is_empty());
    }

    #[test]
    fn the_same_recordings_make_the_same_file() {
        let list = |first: &str, second: &str| {
            Narration::pack(
                &[(first.to_owned(), tone(RATE, 0.1)), (second.to_owned(), tone(RATE, 0.1))],
                RATE,
            )
        };
        assert_eq!(list("a", "b"), list("b", "a"));
    }
}
