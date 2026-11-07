//! A voice in use, and the screen for it. It speaks on its own thread, a sentence at a
//! time, and lets go of its memory when nobody has asked anything for a while.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ravenrs_app_i18n::{Language, Texts};
use ravenrs_layout::Align;
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, TextStyle};
use ravenrs_widgets::{
    BuildCx, Button, ButtonSize, ButtonStyle, Disclosure, HStack, IconName, NodeId, SegmentedControl, Select,
    Slider, Sound, SoundClip, Spacer, Text, Toggle, UiWaker, VStack, View,
};
use serde_json::{Value, json};

use crate::{
    LocalModel, Machine, ModelFolder, ModelManager, ModelManagerLabels, SPEECH_LANGUAGES, SPEECH_RATE,
    SpeechEngine, SpeechSettings, SpeechVoice,
};

const STAYS_LOADED: Duration = Duration::from_secs(120);
const SETTINGS_FILE: &str = "voice-settings.json";

struct Saying {
    text: String,
    settings: SpeechSettings,
    waker: UiWaker,
    turn: u64,
}

/// A model that speaks on its own thread. Dropping it ends the thread and frees the
/// memory.
#[derive(Clone)]
pub struct LocalVoice {
    jobs: Sender<Saying>,
    turn: Arc<AtomicU64>,
    ready: Arc<Mutex<VecDeque<SoundClip>>>,
}

impl std::fmt::Debug for LocalVoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalVoice").finish_non_exhaustive()
    }
}

impl LocalVoice {
    /// A voice over `model`. Nothing is loaded until the first thing it says.
    pub fn new(folder: ModelFolder, model: LocalModel) -> Self {
        let (jobs, inbox) = mpsc::channel::<Saying>();
        let (turn, ready) = (Arc::new(AtomicU64::new(0)), Arc::new(Mutex::new(VecDeque::new())));
        let (now, out) = (Arc::clone(&turn), Arc::clone(&ready));
        std::thread::spawn(move || {
            let mut engine: Option<SpeechEngine> = None;
            loop {
                let job = if engine.is_some() {
                    match inbox.recv_timeout(STAYS_LOADED) {
                        Ok(job) => job,
                        Err(RecvTimeoutError::Timeout) => {
                            engine = None;
                            continue;
                        }
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                } else {
                    match inbox.recv() {
                        Ok(job) => job,
                        Err(_) => return,
                    }
                };
                if engine.is_none() && job.turn == now.load(Ordering::Acquire) {
                    engine = SpeechEngine::load(&folder, &model).ok();
                }
                let Some(speaking) = engine.as_mut() else { continue };
                let mut free_at = Instant::now();
                for sentence in sentences(&job.text) {
                    let current = || job.turn == now.load(Ordering::Acquire);
                    if !current() {
                        break;
                    }
                    let Ok(sound) = speaking.speak(&sentence, &job.settings, &mut |_| current()) else {
                        break;
                    };
                    if sound.is_empty() || !current() {
                        continue;
                    }
                    // One after the other: the sound system plays clips over each other.
                    std::thread::sleep(free_at.saturating_duration_since(Instant::now()));
                    if !current() {
                        break;
                    }
                    let clip = SoundClip::new(sound, SPEECH_RATE);
                    free_at = Instant::now() + Duration::from_secs_f32(clip.seconds());
                    if let Ok(mut ready) = out.lock() {
                        ready.push_back(clip);
                    }
                    job.waker.wake();
                }
            }
        });
        Self { jobs, turn, ready }
    }

    /// Says `text`, in place of anything still being said. `waker` asks for a rebuild
    /// when a sentence is ready, for [`LocalVoice::play_ready`] to start it.
    pub fn say(&self, text: String, settings: SpeechSettings, waker: UiWaker) {
        let turn = self.next_turn();
        // The thread ends only with this handle, so this does not fail.
        let _ = self.jobs.send(Saying { text, settings, waker, turn });
    }

    /// Stops saying anything still to come.
    pub fn stop(&self) {
        self.next_turn();
    }

    fn next_turn(&self) -> u64 {
        if let Ok(mut ready) = self.ready.lock() {
            ready.clear();
        }
        self.turn.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Plays what is ready. Call it when building a view that stays on the screen
    /// while it speaks.
    pub fn play_ready(&self, sound: &Sound) {
        let Ok(mut ready) = self.ready.lock() else { return };
        while let Some(clip) = ready.pop_front() {
            sound.play(&clip);
        }
    }
}

/// The sentences of `text`, which is how the model says long texts best.
pub fn sentences(text: &str) -> Vec<String> {
    let mut all = Vec::new();
    let mut sentence = String::new();
    let mut letters = text.chars().peekable();
    while let Some(letter) = letters.next() {
        let ends = matches!(letter, '.' | '!' | '?' | '\n')
            && letters.peek().is_none_or(|next| next.is_whitespace());
        if letter != '\n' {
            sentence.push(letter);
        }
        if ends {
            let done = sentence.trim();
            if done.chars().any(char::is_alphanumeric) {
                all.push(done.to_owned());
            }
            sentence.clear();
        }
    }
    let done = sentence.trim();
    if done.chars().any(char::is_alphanumeric) {
        all.push(done.to_owned());
    }
    all
}

impl SpeechSettings {
    /// What the model's makers measured it with: steadier, and less varied.
    pub fn steadier() -> Self {
        Self { temperature: 0.4, top_k: 0, repetition_penalty: 1.2, repetition_window: 64, ..Self::default() }
    }
}

fn to_json(settings: &SpeechSettings, read_aloud: bool) -> Value {
    json!({
        "language": settings.language,
        "voice": match settings.voice { SpeechVoice::Female => "female", SpeechVoice::Male => "male" },
        "temperature": settings.temperature,
        "top_k": settings.top_k,
        "repetition_penalty": settings.repetition_penalty,
        "repetition_window": settings.repetition_window,
        "read_aloud": read_aloud,
    })
}

/// Reads settings back. What is missing or strange stays as recommended.
fn from_json(value: &Value) -> (SpeechSettings, bool) {
    let mut settings = SpeechSettings::default();
    if let Some(language) = value.get("language").and_then(Value::as_str)
        && SPEECH_LANGUAGES.contains(&language)
    {
        language.clone_into(&mut settings.language);
    }
    if value.get("voice").and_then(Value::as_str) == Some("male") {
        settings.voice = SpeechVoice::Male;
    }
    let number = |name: &str| value.get(name).and_then(Value::as_f64).filter(|number| *number >= 0.0);
    if let Some(number) = number("temperature") {
        settings.temperature = number as f32;
    }
    if let Some(number) = number("top_k") {
        settings.top_k = number as usize;
    }
    if let Some(number) = number("repetition_penalty").filter(|number| *number >= 1.0) {
        settings.repetition_penalty = number as f32;
    }
    if let Some(number) = number("repetition_window") {
        settings.repetition_window = number as usize;
    }
    (settings, value.get("read_aloud").and_then(Value::as_bool).unwrap_or(true))
}

/// What the voice screen says.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct VoiceLabels {
    /// For the list of models.
    pub models: ModelManagerLabels,
    /// The language's field.
    pub language: String,
    /// The name of each of [`SPEECH_LANGUAGES`], in its order.
    pub languages: Vec<String>,
    /// The voice's field.
    pub voice: String,
    /// The female voice.
    pub female: String,
    /// The male voice.
    pub male: String,
    /// The switch that reads answers aloud.
    pub read_aloud: String,
    /// The button that plays a sample.
    pub listen: String,
    /// What the sample says.
    pub sample: String,
    /// What the fine settings are called.
    pub advanced: String,
    /// How free each code is to vary, and what that does.
    pub temperature: (String, String),
    /// How many codes it chooses among, and what that does.
    pub top_k: (String, String),
    /// How much it avoids repeating, and what that does.
    pub repetition_penalty: (String, String),
    /// How far back it looks, and what that does.
    pub repetition_window: (String, String),
    /// The button for the recommended settings.
    pub recommended: String,
    /// The button for the steadier settings.
    pub steadier: String,
    /// Whether numbers use a comma.
    pub decimal_comma: bool,
}

impl Default for VoiceLabels {
    fn default() -> Self {
        Self::from_texts(&Texts::new(Language::English))
    }
}

impl VoiceLabels {
    /// The texts of a catalog, in its language. What it does not have comes from English.
    pub fn from_texts(texts: &Texts) -> Self {
        Self {
            models: ModelManagerLabels::from_texts(texts),
            language: texts.get("ai.voice.language"),
            languages: texts.list("ai.voice.languages"),
            voice: texts.get("ai.voice.voice"),
            female: texts.get("ai.voice.female"),
            male: texts.get("ai.voice.male"),
            read_aloud: texts.get("ai.voice.read_aloud"),
            listen: texts.get("ai.voice.listen"),
            sample: texts.get("ai.voice.sample"),
            advanced: texts.get("ai.voice.advanced"),
            temperature: texts.pair("ai.voice.temperature"),
            top_k: texts.pair("ai.voice.top_k"),
            repetition_penalty: texts.pair("ai.voice.repetition_penalty"),
            repetition_window: texts.pair("ai.voice.repetition_window"),
            recommended: texts.get("ai.voice.recommended"),
            steadier: texts.get("ai.voice.steadier"),
            decimal_comma: texts.char("format.decimal", '.') == ',',
        }
    }

    /// In Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self::from_texts(&Texts::new(Language::BrazilianPortuguese))
    }
}

/// The screen for the voice: which model speaks, and how. The values start as its model
/// card recommends, and are kept in the models' folder.
#[derive(Clone)]
pub struct VoiceManager {
    models: ModelManager,
    folder: ModelFolder,
    file: PathBuf,
    voice: Rc<RefCell<Option<(&'static str, LocalVoice)>>>,
    saved: Rc<RefCell<Value>>,
    language: State<usize>,
    gender: State<usize>,
    temperature: State<f32>,
    top_k: State<f32>,
    repetition_penalty: State<f32>,
    repetition_window: State<f32>,
    read_aloud: State<bool>,
    labels: VoiceLabels,
}

impl std::fmt::Debug for VoiceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoiceManager").finish_non_exhaustive()
    }
}

impl VoiceManager {
    /// The models of `catalog` in `root`, as many as fit this machine.
    pub fn new(catalog: &[LocalModel], root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self::for_machine(catalog, root.clone(), &Machine::detect(&root))
    }

    /// The same, for `machine`.
    pub fn for_machine(catalog: &[LocalModel], root: impl Into<PathBuf>, machine: &Machine) -> Self {
        let root = root.into();
        let file = root.join(SETTINGS_FILE);
        let found = fs::read(&file).ok().and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
        let (settings, read_aloud) =
            found.as_ref().map(from_json).unwrap_or((SpeechSettings::default(), true));
        let folder = ModelFolder::new(root);
        let language = SPEECH_LANGUAGES.iter().position(|name| *name == settings.language).unwrap_or(0);
        Self {
            models: ModelManager::for_machine(catalog, folder.clone(), machine),
            folder,
            file,
            voice: Rc::default(),
            saved: Rc::new(RefCell::new(to_json(&settings, read_aloud))),
            language: State::new(language),
            gender: State::new(usize::from(settings.voice == SpeechVoice::Male)),
            temperature: State::new(settings.temperature),
            top_k: State::new(settings.top_k as f32),
            repetition_penalty: State::new(settings.repetition_penalty),
            repetition_window: State::new(settings.repetition_window as f32),
            read_aloud: State::new(read_aloud),
            labels: VoiceLabels::default(),
        }
    }

    /// What it says.
    #[must_use]
    pub fn labels(mut self, labels: VoiceLabels) -> Self {
        self.models = self.models.labels(labels.models.clone());
        self.labels = labels;
        self
    }

    /// How to speak, as it is set now.
    pub fn settings(&self) -> SpeechSettings {
        let language = SPEECH_LANGUAGES.get(self.language.get()).copied().unwrap_or("Portuguese");
        SpeechSettings {
            language: language.to_owned(),
            voice: if self.gender.get() == 1 { SpeechVoice::Male } else { SpeechVoice::Female },
            temperature: self.temperature.get(),
            top_k: self.top_k.get().round() as usize,
            repetition_penalty: self.repetition_penalty.get(),
            repetition_window: self.repetition_window.get().round() as usize,
            ..SpeechSettings::default()
        }
    }

    /// The voice in use, if any.
    pub fn voice(&self) -> Option<LocalVoice> {
        let model = self.models.model_in_use()?;
        let mut kept = self.voice.borrow_mut();
        match kept.as_ref() {
            Some((id, voice)) if *id == model.id => Some(voice.clone()),
            _ => {
                let voice = LocalVoice::new(self.folder.clone(), model);
                *kept = Some((model.id, voice.clone()));
                Some(voice)
            }
        }
    }

    /// Says `text` with the voice in use and the current settings, if there is one.
    pub fn say(&self, text: String, waker: UiWaker) {
        if let Some(voice) = self.voice() {
            voice.say(text, self.settings(), waker);
        }
    }

    /// Whether answers should be read when they arrive.
    pub fn reads_aloud(&self) -> bool {
        self.read_aloud.get() && self.models.model_in_use().is_some()
    }

    /// Plays what is ready. Call it when building a view that stays on the screen
    /// while it speaks.
    pub fn play_ready(&self, sound: &Sound) {
        if let Some((_, voice)) = self.voice.borrow().as_ref() {
            voice.play_ready(sound);
        }
    }

    fn apply(&self, settings: &SpeechSettings) {
        self.temperature.set(settings.temperature);
        self.top_k.set(settings.top_k as f32);
        self.repetition_penalty.set(settings.repetition_penalty);
        self.repetition_window.set(settings.repetition_window as f32);
    }

    fn save_if_changed(&self) {
        let now = to_json(&self.settings(), self.read_aloud.get());
        if *self.saved.borrow() != now {
            let bytes = serde_json::to_vec_pretty(&now).unwrap_or_default();
            // Not saving only means going back to these values next time.
            if let Some(parent) = self.file.parent() {
                let _ = fs::create_dir_all(parent).and_then(|()| fs::write(&self.file, bytes));
            }
            *self.saved.borrow_mut() = now;
        }
    }

    fn number(&self, value: f32) -> String {
        let text = if value.fract().abs() < 1e-3 { format!("{value:.0}") } else { format!("{value:.2}") };
        if self.labels.decimal_comma { text.replace('.', ",") } else { text }
    }

    fn slider(
        &self,
        names: &(String, String),
        state: &State<f32>,
        range: (f32, f32, f32),
    ) -> impl View + 'static {
        VStack::new((
            HStack::new((
                Text::new(names.0.clone()).style(TextStyle::Callout),
                Spacer::new(),
                Text::new(self.number(state.get())).style(TextStyle::Callout).foreground(Semantic::InkMuted),
            ))
            .align(Align::Center),
            Slider::new(state.binding()).range(range.0, range.1).step(range.2).label(names.0.clone()),
            Text::new(names.1.clone()).style(TextStyle::Caption).foreground(Semantic::InkMuted),
        ))
        .spacing(4.0)
        .align(Align::Stretch)
    }
}

impl View for VoiceManager {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        self.save_if_changed();
        let sound = cx.sound();
        self.play_ready(&sound);
        let models = self.models.clone();
        if self.models.model_in_use().is_none() {
            return cx.child(&models);
        }
        let labels = &self.labels;
        let waker = cx.ui_waker();
        let (speaking, sample) = (self.clone(), labels.sample.clone());
        let (recommended, steadier) = (self.clone(), self.clone());
        let fine = VStack::new((
            self.slider(&labels.temperature, &self.temperature, (0.0, 1.5, 0.05)),
            self.slider(&labels.top_k, &self.top_k, (0.0, 512.0, 1.0)),
            self.slider(&labels.repetition_penalty, &self.repetition_penalty, (1.0, 1.5, 0.01)),
            self.slider(&labels.repetition_window, &self.repetition_window, (0.0, 4096.0, 64.0)),
            HStack::new((
                Button::new(labels.recommended.clone())
                    .size(ButtonSize::Small)
                    .style(ButtonStyle::Secondary)
                    .on_click(move || recommended.apply(&SpeechSettings::default())),
                Button::new(labels.steadier.clone())
                    .size(ButtonSize::Small)
                    .style(ButtonStyle::Secondary)
                    .on_click(move || steadier.apply(&SpeechSettings::steadier())),
            ))
            .spacing(theme.spacing.s2)
            .wrap(),
        ))
        .spacing(theme.spacing.s3)
        .align(Align::Stretch);
        let controls = VStack::new((
            HStack::new((
                Select::new(self.language.binding(), labels.languages.clone()).label(labels.language.clone()),
                SegmentedControl::new(self.gender.binding(), [labels.female.clone(), labels.male.clone()])
                    .label(labels.voice.clone()),
            ))
            .spacing(theme.spacing.s3)
            .align(Align::Center)
            .wrap(),
            Toggle::new(self.read_aloud.binding()).label(labels.read_aloud.clone()),
            HStack::new((Button::new(labels.listen.clone())
                .icon(IconName::Volume2)
                .size(ButtonSize::Small)
                .on_click(move || speaking.say(sample.clone(), waker.clone())),))
            .align(Align::Center),
            Disclosure::new(labels.advanced.clone(), fine),
        ))
        .spacing(theme.spacing.s3)
        .align(Align::Stretch);
        cx.child(&VStack::new((models, controls)).spacing(theme.spacing.s4).align(Align::Stretch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_said_a_sentence_at_a_time() {
        assert_eq!(
            sentences("Você tem R$ 1.234,56. Quer ver mais?\nOk"),
            ["Você tem R$ 1.234,56.", "Quer ver mais?", "Ok"]
        );
        assert!(sentences(" ... ").is_empty());
    }

    #[test]
    fn the_settings_come_back_as_they_were_saved() {
        let settings = SpeechSettings {
            language: "Spanish".to_owned(),
            voice: SpeechVoice::Male,
            temperature: 0.5,
            top_k: 64,
            repetition_penalty: 1.2,
            repetition_window: 128,
            ..SpeechSettings::default()
        };
        assert_eq!(from_json(&to_json(&settings, false)), (settings, false));
    }

    #[test]
    fn what_is_strange_stays_as_recommended() {
        let strange = json!({"language": "Klingon", "temperature": -1, "repetition_penalty": 0.1});
        assert_eq!(from_json(&strange), (SpeechSettings::default(), true));
    }
}
