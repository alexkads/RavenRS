//! What a film sounds like from start to end, as samples, for a video made from it.

use ravenrs_widgets::SoundClip;

use crate::movie::{FilmSound, Movie};
use crate::narration::Narration;
use crate::player::shown_at;

/// How often the player looks at what to say, in milliseconds: the voice starts on
/// the same beat in a video.
const BEAT_MS: u64 = 1000 / 30;

/// The sound of a whole film: its clicks and keys where they happen, and the
/// recording of each box from the moment it shows, as the player says them when it
/// does not stop at the steps. A box that starts while another is still being said
/// cuts it, like in the player.
#[derive(Clone, Debug)]
pub struct Soundtrack {
    samples: Vec<f32>,
    rate: u32,
    unrecorded: Vec<String>,
}

impl Soundtrack {
    /// The sound of `movie` at `rate` samples a second, with the voice of `narration`.
    pub fn of(movie: &Movie, narration: Option<&Narration>, rate: u32) -> Self {
        let rate = rate.max(1);
        let length = (movie.duration_ms() as usize * rate as usize).div_ceil(1000);
        let mut samples = vec![0.0_f32; length];
        let (click, key) = (resampled(&SoundClip::click(), rate), resampled(&SoundClip::key(), rate));
        for (at, sound) in movie.sounds_between(0, movie.duration_ms()) {
            let clip = match sound {
                FilmSound::Click => &click,
                FilmSound::Key => &key,
            };
            mix(&mut samples, sample_at(*at, rate), clip, usize::MAX);
        }
        let mut unrecorded = Vec::new();
        let mut said: Vec<(usize, Vec<f32>)> = Vec::new();
        let mut last = None;
        let mut at = 0;
        while at < movie.duration_ms() {
            if let Some(index) = shown_at(movie, at)
                && last != Some(index)
            {
                last = Some(index);
                let spot = &movie.hotspots()[index];
                let text = format!("{}. {}", spot.title, spot.text);
                match narration.and_then(|narration| narration.clip(&text)) {
                    Some(clip) => said.push((sample_at(at, rate), resampled(&clip, rate))),
                    None => unrecorded.push(text),
                }
            }
            at += BEAT_MS;
        }
        for (i, (start, voice)) in said.iter().enumerate() {
            let until = said.get(i + 1).map_or(usize::MAX, |(next, _)| *next);
            mix(&mut samples, *start, voice, until);
        }
        for sample in &mut samples {
            *sample = sample.clamp(-1.0, 1.0);
        }
        Self { samples, rate, unrecorded }
    }

    /// The samples, between -1 and 1, one channel.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// How many samples a second.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// What the boxes say that the narration has no recording of, so the video
    /// shows it without saying it.
    pub fn unrecorded(&self) -> &[String] {
        &self.unrecorded
    }
}

fn sample_at(ms: u64, rate: u32) -> usize {
    (ms as usize * rate as usize) / 1000
}

/// Adds `clip` from `start`, stopping before `until`.
fn mix(samples: &mut [f32], start: usize, clip: &[f32], until: usize) {
    let end = until.min(samples.len());
    for (sample, value) in samples.iter_mut().take(end).skip(start).zip(clip) {
        *sample += value;
    }
}

/// `clip` at `rate`, by straight lines between its samples.
fn resampled(clip: &SoundClip, rate: u32) -> Vec<f32> {
    let from = clip.samples();
    if clip.rate() == rate || from.is_empty() {
        return from.to_vec();
    }
    let step = f64::from(clip.rate()) / f64::from(rate);
    let length = (from.len() as f64 / step) as usize;
    (0..length)
        .map(|i| {
            let position = i as f64 * step;
            let left = position as usize;
            let right = (left + 1).min(from.len() - 1);
            let t = (position - left as f64) as f32;
            from[left] * (1.0 - t) + from[right] * t
        })
        .collect()
}
