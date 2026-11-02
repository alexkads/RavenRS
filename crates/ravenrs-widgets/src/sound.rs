//! Sound and a spoken voice, as a service the platform layer provides.

use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

const RATE: u32 = 44_100;

/// A short sound in memory: one channel, ready to be played many times. Cloning
/// shares the samples.
///
/// The ready-made ones are computed, so they add nothing to an app's size.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundClip {
    samples: Arc<[f32]>,
    rate: u32,
}

impl SoundClip {
    /// A sound from its samples, between -1 and 1, at `rate` a second.
    pub fn new(samples: impl Into<Arc<[f32]>>, rate: u32) -> Self {
        Self { samples: samples.into(), rate: rate.max(1) }
    }

    /// The press and release of a mouse button.
    pub fn click() -> Self {
        let press = snap(0.030, 0.0016, 2_300.0, 0.55, 7);
        let release = snap(0.024, 0.0012, 3_100.0, 0.32, 23);
        let gap = (0.055 * RATE as f32) as usize;
        let mut samples = vec![0.0; gap + release.len()];
        for (sample, value) in samples.iter_mut().zip(&press) {
            *sample += value;
        }
        for (sample, value) in samples[gap..].iter_mut().zip(&release) {
            *sample += value;
        }
        Self::new(samples, RATE)
    }

    /// A key being pressed: softer than a click, since many come in a row.
    pub fn key() -> Self {
        Self::new(snap(0.022, 0.0014, 1_700.0, 0.20, 41), RATE)
    }

    /// Its samples.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// How many samples make a second.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// How long it lasts, in seconds.
    pub fn seconds(&self) -> f32 {
        self.samples.len() as f32 / self.rate as f32
    }
}

/// A burst of noise over a short tone, both dying out fast, the way plastic
/// sounds when struck.
fn snap(seconds: f32, fade: f32, tone: f32, volume: f32, seed: u32) -> Vec<f32> {
    let mut noise = seed.wrapping_mul(2_654_435_761) | 1;
    let length = (seconds * RATE as f32) as usize;
    (0..length)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            // Xorshift: a noise that is the same on every run and every machine.
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            let hiss = noise as f32 / u32::MAX as f32 * 2.0 - 1.0;
            let ring = (t * tone * std::f32::consts::TAU).sin();
            let thump = (t * 620.0 * std::f32::consts::TAU).sin();
            let sample = hiss * (-t / fade).exp() * 0.7
                + ring * (-t / (fade * 2.0)).exp() * 0.5
                + thump * (-t / (fade * 4.0)).exp() * 0.3;
            sample * volume
        })
        .collect()
}

/// Plays sounds and speaks. Platform layers install one with
/// [`crate::Ui::set_sound`]; until then, nothing is heard.
pub trait SoundProvider {
    /// Starts `clip`, over whatever is already playing.
    fn play(&self, clip: &SoundClip);
    /// Says `text` with the system's voice for `language`, such as `pt-BR`, in
    /// place of anything still being said.
    fn speak(&self, text: &str, language: &str);
    /// Stops what is being said.
    fn stop_speaking(&self);
}

/// A provider that makes no sound (tests, headless use).
#[derive(Debug, Default)]
pub struct Silence;

impl SoundProvider for Silence {
    fn play(&self, _clip: &SoundClip) {}

    fn speak(&self, _text: &str, _language: &str) {}

    fn stop_speaking(&self) {}
}

/// A shared handle to the sound system, from [`crate::BuildCx::sound`] or
/// [`crate::Ui::sound`].
///
/// ```
/// use ravenrs_widgets::{Silence, Sound, SoundClip};
///
/// let sound = Sound::new(Silence);
/// sound.play(&SoundClip::click());
/// sound.speak("Clique em Salvar.", "pt-BR");
/// ```
#[derive(Clone)]
pub struct Sound(Rc<dyn SoundProvider>);

impl Sound {
    /// Wraps a provider.
    pub fn new(provider: impl SoundProvider + 'static) -> Self {
        Self(Rc::new(provider))
    }

    /// Starts `clip`, over whatever is already playing.
    pub fn play(&self, clip: &SoundClip) {
        self.0.play(clip);
    }

    /// Says `text` with the system's voice for `language`, in place of anything
    /// still being said.
    pub fn speak(&self, text: &str, language: &str) {
        self.0.speak(text, language);
    }

    /// Stops what is being said.
    pub fn stop_speaking(&self) {
        self.0.stop_speaking();
    }
}

impl Default for Sound {
    fn default() -> Self {
        Self::new(Silence)
    }
}

impl fmt::Debug for Sound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Sound")
    }
}

#[cfg(test)]
mod tests {
    use super::SoundClip;

    fn peak(clip: &SoundClip) -> f32 {
        clip.samples().iter().fold(0.0, |peak: f32, sample| peak.max(sample.abs()))
    }

    #[test]
    fn a_click_is_short_audible_and_does_not_clip() {
        let click = SoundClip::click();
        assert!(click.seconds() > 0.05 && click.seconds() < 0.15, "{}", click.seconds());
        assert!(peak(&click) > 0.2 && peak(&click) <= 1.0, "{}", peak(&click));
        assert_eq!(click, SoundClip::click(), "the same on every run");
    }

    #[test]
    fn a_key_is_softer_than_a_click() {
        assert!(peak(&SoundClip::key()) < peak(&SoundClip::click()));
        assert!(peak(&SoundClip::key()) > 0.05);
    }
}
