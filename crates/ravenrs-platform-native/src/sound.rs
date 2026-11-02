//! Sound through `cpal`, and the voice through the system's own speech program.

use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use ravenrs_widgets::{SoundClip, SoundProvider};

/// How long the output stays open after the last sound. Keeping it open costs
/// CPU, so an app that went quiet closes it.
const QUIET_BEFORE_CLOSING: Duration = Duration::from_secs(2);

enum Order {
    Play(SoundClip),
    Speak { text: String, language: String },
    Stop,
}

/// The app's sound. Nothing starts until the first sound is asked for, so an app
/// that makes none has no extra thread and no open device.
#[derive(Default)]
pub(crate) struct SystemSound {
    orders: std::cell::OnceCell<Option<Sender<Order>>>,
}

impl SystemSound {
    fn send(&self, order: Order) {
        let orders = self.orders.get_or_init(|| {
            let (sender, receiver) = mpsc::channel();
            let worker = std::thread::Builder::new().name("ravenrs-sound".to_owned()).spawn(move || {
                let mut output = Output::default();
                let mut voice = Voice::default();
                loop {
                    let order = if output.is_open() {
                        match receiver.recv_timeout(QUIET_BEFORE_CLOSING) {
                            Ok(order) => order,
                            Err(RecvTimeoutError::Timeout) => {
                                output.close_if_quiet();
                                continue;
                            }
                            Err(RecvTimeoutError::Disconnected) => break,
                        }
                    } else {
                        match receiver.recv() {
                            Ok(order) => order,
                            Err(_) => break,
                        }
                    };
                    match order {
                        Order::Play(clip) => output.play(clip),
                        Order::Speak { text, language } => voice.speak(&text, &language),
                        Order::Stop => voice.stop(),
                    }
                }
                voice.stop();
            });
            match worker {
                Ok(_) => Some(sender),
                Err(e) => {
                    tracing::warn!(error = %e, "could not start the sound thread; the app stays silent");
                    None
                }
            }
        });
        if let Some(orders) = orders {
            // It only fails if the thread died, and then there is nothing to hear.
            let _ = orders.send(order);
        }
    }
}

impl SoundProvider for SystemSound {
    fn play(&self, clip: &SoundClip) {
        self.send(Order::Play(clip.clone()));
    }

    fn speak(&self, text: &str, language: &str) {
        self.send(Order::Speak { text: text.to_owned(), language: language.to_owned() });
    }

    fn stop_speaking(&self) {
        // With no thread yet, nothing was said, and there is no reason to start one.
        if self.orders.get().is_some() {
            self.send(Order::Stop);
        }
    }
}

/// A sound being played, and how far it got, in samples of the sound.
struct Playing {
    clip: SoundClip,
    at: f64,
}

/// The sounds playing now, mixed into whatever the device takes.
#[derive(Default)]
struct Output {
    stream: Option<cpal::Stream>,
    playing: Arc<Mutex<Vec<Playing>>>,
}

impl Output {
    fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    fn close_if_quiet(&mut self) {
        if self.playing.lock().map_or(true, |playing| playing.is_empty()) {
            self.stream = None;
        }
    }

    fn play(&mut self, clip: SoundClip) {
        if self.stream.is_none() {
            self.stream = open_stream(&self.playing);
        }
        if self.stream.is_some()
            && let Ok(mut playing) = self.playing.lock()
        {
            playing.push(Playing { clip, at: 0.0 });
        }
    }
}

fn open_stream(playing: &Arc<Mutex<Vec<Playing>>>) -> Option<cpal::Stream> {
    let device = cpal::default_host().default_output_device()?;
    let config = device
        .default_output_config()
        .inspect_err(|e| tracing::warn!(error = %e, "this device gives no sound format"))
        .ok()?;
    let format = config.sample_format();
    let config = cpal::StreamConfig::from(config);
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, &config, playing.clone()),
        SampleFormat::I16 => build::<i16>(&device, &config, playing.clone()),
        SampleFormat::U16 => build::<u16>(&device, &config, playing.clone()),
        SampleFormat::I32 => build::<i32>(&device, &config, playing.clone()),
        other => {
            tracing::warn!(format = ?other, "this sound format is not supported");
            return None;
        }
    }
    .inspect_err(|e| tracing::warn!(error = %e, "could not open the sound device"))
    .ok()?;
    stream.play().inspect_err(|e| tracing::warn!(error = %e, "could not start the sound device")).ok()?;
    Some(stream)
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    playing: Arc<Mutex<Vec<Playing>>>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let (channels, rate) = (usize::from(config.channels).max(1), config.sample_rate as f64);
    device.build_output_stream(
        config,
        move |out: &mut [T], _| {
            let Ok(mut playing) = playing.lock() else {
                return;
            };
            for frame in out.chunks_mut(channels) {
                let mixed: f32 = playing.iter_mut().map(|sound| sound.next(rate)).sum();
                frame.fill(T::from_sample(mixed.clamp(-1.0, 1.0)));
            }
            playing.retain(|sound| (sound.at as usize) < sound.clip.samples().len());
        },
        |e| tracing::warn!(error = %e, "the sound device failed"),
        None,
    )
}

impl Playing {
    /// The next sample for a device running at `rate`, between two of the sound's.
    fn next(&mut self, rate: f64) -> f32 {
        let samples = self.clip.samples();
        let index = self.at as usize;
        let Some(&here) = samples.get(index) else {
            return 0.0;
        };
        let after = samples.get(index + 1).copied().unwrap_or(0.0);
        let between = (self.at - index as f64) as f32;
        self.at += f64::from(self.clip.rate()) / rate;
        here + (after - here) * between
    }
}

/// What the system's speech program is saying.
#[derive(Default)]
struct Voice {
    speaking: Option<Child>,
    #[cfg(target_os = "macos")]
    voices: Option<String>,
}

impl Voice {
    fn stop(&mut self) {
        if let Some(mut child) = self.speaking.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn speak(&mut self, text: &str, language: &str) {
        self.stop();
        let mut command = self.command(language);
        // The text goes through stdin, where no shell can take it for code.
        command.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
        match command.spawn() {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(text.as_bytes());
                }
                self.speaking = Some(child);
            }
            Err(e) => tracing::warn!(error = %e, "the system has no speech program"),
        }
    }

    /// `say` reads stdin when given no text. A voice in the language is picked
    /// from the system's list; with none, it speaks in the default voice.
    #[cfg(target_os = "macos")]
    fn command(&mut self, language: &str) -> Command {
        let voices = self.voices.get_or_insert_with(|| {
            Command::new("say")
                .args(["-v", "?"])
                .output()
                .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
                .unwrap_or_default()
        });
        let mut command = Command::new("say");
        if let Some(voice) = macos_voice(voices, language) {
            command.args(["-v", &voice]);
        }
        command
    }

    /// Windows has speech built in, reached through PowerShell. The language is
    /// checked first, since it goes into the script.
    #[cfg(target_os = "windows")]
    #[allow(clippy::unused_self, reason = "each system has its own way")]
    fn command(&mut self, language: &str) -> Command {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        let language: String = language.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        let script = format!(
            "[Console]::InputEncoding=[Text.Encoding]::UTF8;\
             Add-Type -AssemblyName System.Speech;\
             $s=New-Object System.Speech.Synthesis.SpeechSynthesizer;\
             try{{$s.SelectVoiceByHints(0,0,0,[Globalization.CultureInfo]'{language}')}}catch{{}};\
             $s.Speak([Console]::In.ReadToEnd())"
        );
        let mut command = Command::new("powershell");
        command.args(["-NoProfile", "-NonInteractive", "-Command", &script]).creation_flags(NO_WINDOW);
        command
    }

    /// eSpeak NG reads stdin and is what most Linux desktops have under Speech
    /// Dispatcher. Without it there is no voice, and the boxes still say it all.
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    #[allow(clippy::unused_self, reason = "each system has its own way")]
    fn command(&mut self, language: &str) -> Command {
        let mut command = Command::new("espeak-ng");
        command.args(["--stdin", "-v", &language.to_lowercase()]);
        command
    }
}

/// The voice for `language` in the list `say -v ?` prints, whose lines read
/// `Luciana (Português (Brasil)) pt_BR    # Olá`. The plain name, with no
/// parentheses, is the one that sounds natural when a system has several.
#[cfg(any(target_os = "macos", test))]
fn macos_voice(voices: &str, language: &str) -> Option<String> {
    let locale = language.replace('-', "_");
    let names: Vec<&str> = voices
        .lines()
        .filter_map(|line| {
            let (voice, _) = line.split_once('#')?;
            let (name, code) = voice.trim_end().rsplit_once(char::is_whitespace)?;
            (code == locale).then(|| name.trim_end())
        })
        .map(|name| name.split(" (").next().unwrap_or(name))
        .collect();
    const NATURAL: &[&str] = &["Luciana", "Joana", "Samantha", "Daniel"];
    NATURAL.iter().find(|voice| names.contains(voice)).or(names.first()).map(|name| (*name).to_owned())
}

#[cfg(test)]
mod tests {
    use super::{Output, Voice, macos_voice};
    use ravenrs_widgets::SoundClip;

    #[test]
    fn it_picks_a_natural_voice_for_the_language() {
        let list = "Albert              en_US    # Hello\n\
                    Eddy (Português (Brasil)) pt_BR    # Olá\n\
                    Luciana             pt_BR    # Olá, meu nome é Luciana.\n";
        assert_eq!(macos_voice(list, "pt-BR").as_deref(), Some("Luciana"));
        assert_eq!(macos_voice(list, "en-US").as_deref(), Some("Albert"));
        assert_eq!(macos_voice(list, "fr-FR"), None);
    }

    #[test]
    #[ignore = "it makes real sound; run it with --ignored"]
    fn the_device_takes_a_click_to_the_end_and_then_closes() {
        let mut output = Output::default();
        output.play(SoundClip::click());
        if !output.is_open() {
            eprintln!("no sound device: nothing played");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(600));
        assert!(output.playing.lock().is_ok_and(|playing| playing.is_empty()), "the device took it all");
        output.close_if_quiet();
        assert!(!output.is_open());
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "it speaks out loud; run it with --ignored"]
    fn the_voice_starts_and_stops_when_told() {
        let mut voice = Voice::default();
        voice.speak("Clique em Salvar lançamento para guardar a despesa.", "pt-BR");
        std::thread::sleep(std::time::Duration::from_millis(400));
        let child = voice.speaking.as_mut().map(|child| child.try_wait().map(|status| status.is_none()));
        assert!(matches!(child, Some(Ok(true))), "it is still talking");
        voice.stop();
        assert!(voice.speaking.is_none());
    }
}
