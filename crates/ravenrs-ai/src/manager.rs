//! The screen for choosing models: offers what fits, and does the work away from
//! the UI thread.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ravenrs_app_i18n::{Language, Texts};
use ravenrs_reactive::State;
use ravenrs_widgets::{
    BuildCx, ModelChoice, ModelPicker, ModelPickerLabels, ModelStatus, NodeId, UiWaker, View,
};

use crate::{
    HEARING_RATE, LocalAssistant, LocalEngine, LocalModel, Machine, ModelError, ModelFamily, ModelFolder,
    ModelState, Transcriber,
};

/// How a model that hears does on this machine, on a quiet tone it has to listen to whole.
fn hearing_test(folder: &ModelFolder, model: &LocalModel, started: Instant) -> Option<Tested> {
    let mut transcriber = Transcriber::load(folder, model).ok()?;
    let tone: Vec<f32> = (0..TEST_HEARING_SECONDS * HEARING_RATE)
        .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / HEARING_RATE as f32).sin() * 0.01)
        .collect();
    transcriber.transcribe(&tone, "pt").ok()?;
    let seconds = started.elapsed().as_secs_f64();
    let heard = TEST_HEARING_SECONDS as f64;
    let tokens_per_second = if seconds <= heard / 5.0 {
        FAST
    } else if seconds <= heard {
        USABLE
    } else {
        0.0
    };
    Some(Tested { seconds, tokens_per_second })
}

/// How many times a download tells the UI it moved: one per cent is plenty.
const STEPS: u64 = 100;
const GIGABYTE: f32 = 1_000_000_000.0;
/// The file in the folder that says which model is in use.
const IN_USE_FILE: &str = "in-use";
/// What the speed test asks, and how long an answer it wants.
const TEST_SYSTEM: &str = "Answer in one short sentence.";
const TEST_USER: &str = "Why keep a copy of important files?";
const TEST_TOKENS: usize = 40;
/// Tokens per second from which an answer feels instant, and from which it can be
/// read as it arrives.
const FAST: f64 = 15.0;
const USABLE: f64 = 5.0;
/// What a model that hears is tested with: ten seconds, heard in two feels instant, and
/// in ten it keeps up with whoever speaks.
const TEST_HEARING_SECONDS: usize = 10;

/// The words a [`ModelManager`] shows. English by default.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ModelManagerLabels {
    /// The picker's own words.
    pub picker: ModelPickerLabels,
    /// Before how many requests a model understood.
    pub understood: String,
    /// When that was not measured.
    pub not_measured: String,
    /// Before a model's license.
    pub license: String,
    /// Before how much memory a model takes.
    pub memory: String,
    /// Writes numbers with a decimal comma.
    pub decimal_comma: bool,
    /// When the connection failed.
    pub network_error: String,
    /// When the disk refused.
    pub disk_error: String,
    /// When what arrived is not the published file.
    pub wrong_file: String,
    /// Before how long the speed test took, in seconds.
    pub answered_in: String,
    /// A fast result.
    pub fast: String,
    /// A result that can be lived with.
    pub usable: String,
    /// A slow result, suggesting what to do.
    pub slow: String,
    /// When the model did not run.
    pub did_not_run: String,
}

impl Default for ModelManagerLabels {
    fn default() -> Self {
        Self::from_texts(&Texts::new(Language::English))
    }
}

impl ModelManagerLabels {
    /// The texts of a catalog, in its language. What it does not have comes from English.
    pub fn from_texts(texts: &Texts) -> Self {
        Self {
            picker: ModelPickerLabels::from_texts(texts),
            understood: texts.get("ai.models.understood"),
            not_measured: texts.get("ai.models.not_measured"),
            license: texts.get("ai.models.license"),
            memory: texts.get("ai.models.memory"),
            decimal_comma: texts.char("format.decimal", '.') == ',',
            network_error: texts.get("ai.models.network_error"),
            disk_error: texts.get("ai.models.disk_error"),
            wrong_file: texts.get("ai.models.wrong_file"),
            answered_in: texts.get("ai.models.answered_in"),
            fast: texts.get("ai.models.fast"),
            usable: texts.get("ai.models.usable"),
            slow: texts.get("ai.models.slow"),
            did_not_run: texts.get("ai.models.did_not_run"),
        }
    }

    /// The words in Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self::from_texts(&Texts::new(Language::BrazilianPortuguese))
    }

    fn details(&self, model: &LocalModel) -> String {
        let gigabytes = |bytes: u64| {
            let size = format!("{:.1} GB", bytes as f32 / GIGABYTE);
            if self.decimal_comma { size.replace('.', ",") } else { size }
        };
        let understood =
            model.understood.map_or_else(|| self.not_measured.clone(), |percent| format!("{percent}%"));
        format!(
            "{} · {}: {} · {}: {understood} · {}: {}",
            gigabytes(model.bytes()),
            self.memory,
            gigabytes(model.memory),
            self.understood,
            self.license,
            model.license
        )
    }

    fn result(&self, outcome: Option<&Tested>) -> Option<String> {
        let tested = outcome?;
        let verdict = if tested.tokens_per_second >= FAST {
            &self.fast
        } else if tested.tokens_per_second >= USABLE {
            &self.usable
        } else {
            &self.slow
        };
        let seconds = format!("{:.1} s", tested.seconds);
        let seconds = if self.decimal_comma { seconds.replace('.', ",") } else { seconds };
        Some(format!("{} {seconds}. {verdict}", self.answered_in))
    }

    fn error(&self, error: &ModelError) -> Option<String> {
        match error {
            ModelError::Cancelled => None,
            ModelError::Network(_) => Some(self.network_error.clone()),
            ModelError::WrongFile => Some(self.wrong_file.clone()),
            _ => Some(self.disk_error.clone()),
        }
    }
}

/// A download in progress, shared with its thread.
#[derive(Debug, Default)]
struct Download {
    received: AtomicU64,
    cancelled: AtomicBool,
    finished: Mutex<Option<Result<(), ModelError>>>,
}

/// How a speed test went.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Tested {
    seconds: f64,
    tokens_per_second: f64,
}

/// A speed test in progress: what its thread leaves when it ends.
type TestRun = Arc<Mutex<Option<Option<Tested>>>>;

/// One model on offer.
#[derive(Debug)]
struct Entry {
    model: LocalModel,
    installed: bool,
    download: Option<Arc<Download>>,
    test: Option<TestRun>,
    tested: Option<Tested>,
    did_not_run: bool,
    error: Option<ModelError>,
}

/// The models of a list that fit this machine, to download, try, use and remove. A
/// model that does not fit is not offered, unless it is already installed and still
/// fits in memory. Build it once and keep it: downloads belong to it.
///
/// ```no_run
/// use ravenrs_ai::{CATALOG, ModelFolder, ModelManager};
///
/// let manager = ModelManager::new(CATALOG, ModelFolder::new("models"));
/// ```
#[derive(Clone, Debug)]
pub struct ModelManager {
    folder: ModelFolder,
    entries: Rc<RefCell<Vec<Entry>>>,
    in_use: Rc<RefCell<Option<(LocalModel, LocalAssistant)>>>,
    changes: State<u64>,
    labels: ModelManagerLabels,
}

impl ModelManager {
    /// The models of `catalog` that fit this machine, kept in `folder`.
    pub fn new(catalog: &[LocalModel], folder: ModelFolder) -> Self {
        let machine = Machine::detect(folder.root());
        Self::for_machine(catalog, folder, &machine)
    }

    /// The models of `catalog` that fit `machine`.
    pub fn for_machine(catalog: &[LocalModel], folder: ModelFolder, machine: &Machine) -> Self {
        let entries = catalog
            .iter()
            .filter_map(|model| {
                let installed = folder.state(model) == ModelState::Installed;
                model.fits(machine, installed).then_some(Entry {
                    model: *model,
                    installed,
                    download: None,
                    test: None,
                    tested: None,
                    did_not_run: false,
                    error: None,
                })
            })
            .collect::<Vec<Entry>>();
        // What was in use last time, if it is still here and still fits.
        let saved = std::fs::read_to_string(folder.root().join(IN_USE_FILE)).unwrap_or_default();
        let in_use = entries
            .iter()
            .find(|entry| entry.installed && entry.model.id == saved.trim())
            .map(|entry| (entry.model, LocalAssistant::new(folder.clone(), entry.model)));
        Self {
            folder,
            entries: Rc::new(RefCell::new(entries)),
            in_use: Rc::new(RefCell::new(in_use)),
            changes: State::new(0),
            labels: ModelManagerLabels::default(),
        }
    }

    /// The words it shows.
    #[must_use]
    pub fn labels(mut self, labels: ModelManagerLabels) -> Self {
        self.labels = labels;
        self
    }

    /// The model in use, ready to be asked, if there is one.
    pub fn assistant(&self) -> Option<LocalAssistant> {
        self.in_use.borrow().as_ref().map(|(_, assistant)| assistant.clone())
    }

    /// The model in use, if any.
    pub fn model_in_use(&self) -> Option<LocalModel> {
        self.in_use.borrow().as_ref().map(|(model, _)| *model)
    }

    fn is_in_use(&self, model: &LocalModel) -> bool {
        self.in_use.borrow().as_ref().is_some_and(|(used, _)| used.id == model.id)
    }

    /// Starts or stops using a model, and remembers it. Letting go of the assistant
    /// frees its memory.
    fn toggle_use(&self, index: usize) {
        let Some(model) = self.entries.borrow().get(index).map(|entry| entry.model) else { return };
        let next =
            (!self.is_in_use(&model)).then(|| (model, LocalAssistant::new(self.folder.clone(), model)));
        self.set_in_use(next);
    }

    fn set_in_use(&self, next: Option<(LocalModel, LocalAssistant)>) {
        let saved = next.as_ref().map_or("", |(model, _)| model.id);
        let _ = std::fs::create_dir_all(self.folder.root());
        // Remembering is a convenience: without it, the next run starts with no model.
        let _ = std::fs::write(self.folder.root().join(IN_USE_FILE), saved);
        *self.in_use.borrow_mut() = next;
    }

    fn test(&self, index: usize, waker: UiWaker) {
        let mut entries = self.entries.borrow_mut();
        let Some(entry) = entries.get_mut(index) else { return };
        if entry.test.is_some() {
            return;
        }
        let run: TestRun = Arc::new(Mutex::new(None));
        entry.test = Some(run.clone());
        entry.tested = None;
        entry.did_not_run = false;
        let (model, folder) = (entry.model, self.folder.clone());
        std::thread::spawn(move || {
            let started = Instant::now();
            if model.family == ModelFamily::Whisper {
                let outcome = hearing_test(&folder, &model, started);
                if let Ok(mut done) = run.lock() {
                    *done = Some(outcome);
                }
                waker.wake();
                return;
            }
            let outcome = LocalEngine::load(&folder, &model).ok().and_then(|mut engine| {
                let generated = engine.generate(TEST_SYSTEM, TEST_USER, TEST_TOKENS, &mut |_| true).ok()?;
                (generated.tokens > 0).then(|| Tested {
                    seconds: started.elapsed().as_secs_f64(),
                    tokens_per_second: generated.tokens_per_second(),
                })
            });
            if let Ok(mut done) = run.lock() {
                *done = Some(outcome);
            }
            waker.wake();
        });
    }

    fn changed(&self) {
        self.changes.update(|n| *n += 1);
    }

    fn start(&self, index: usize, waker: UiWaker) {
        let mut entries = self.entries.borrow_mut();
        let Some(entry) = entries.get_mut(index) else { return };
        if entry.download.is_some() {
            return;
        }
        let download = Arc::new(Download::default());
        entry.download = Some(download.clone());
        entry.error = None;
        let (model, folder) = (entry.model, self.folder.clone());
        std::thread::spawn(move || {
            let step = (model.bytes() / STEPS).max(1);
            let told = AtomicU64::new(0);
            let progress = |received: u64, _total: u64| {
                download.received.store(received, Ordering::Relaxed);
                if received / step != told.swap(received / step, Ordering::Relaxed) {
                    waker.wake();
                }
            };
            let result = folder.download(&model, &progress, &download.cancelled);
            if let Ok(mut finished) = download.finished.lock() {
                *finished = Some(result);
            }
            waker.wake();
        });
    }

    fn cancel(&self, index: usize) {
        if let Some(download) = self.entries.borrow().get(index).and_then(|entry| entry.download.as_ref()) {
            download.cancelled.store(true, Ordering::Relaxed);
        }
    }

    fn remove(&self, index: usize) {
        if let Some(entry) = self.entries.borrow_mut().get_mut(index) {
            if self.is_in_use(&entry.model) {
                self.set_in_use(None);
            }
            match self.folder.remove(&entry.model) {
                Ok(()) => {
                    entry.installed = false;
                    entry.tested = None;
                }
                Err(error) => entry.error = Some(error),
            }
        }
    }

    /// Takes in the downloads that ended, and says how each model stands.
    fn choices(&self) -> Vec<ModelChoice> {
        let mut entries = self.entries.borrow_mut();
        entries
            .iter_mut()
            .map(|entry| {
                let ended = entry
                    .download
                    .as_ref()
                    .and_then(|download| download.finished.lock().ok().and_then(|mut done| done.take()));
                if let Some(result) = ended {
                    entry.download = None;
                    match result {
                        Ok(()) => entry.installed = true,
                        Err(ModelError::Cancelled) => {}
                        Err(error) => entry.error = Some(error),
                    }
                }
                let test_ended =
                    entry.test.as_ref().and_then(|run| run.lock().ok().and_then(|mut done| done.take()));
                if let Some(outcome) = test_ended {
                    entry.test = None;
                    entry.tested = outcome;
                    entry.did_not_run = outcome.is_none();
                }
                let status = if entry.test.is_some() {
                    ModelStatus::Testing
                } else if let Some(download) = &entry.download {
                    let received = download.received.load(Ordering::Relaxed);
                    ModelStatus::Downloading(received as f32 / entry.model.bytes().max(1) as f32)
                } else if entry.installed {
                    ModelStatus::Installed
                } else {
                    ModelStatus::Available
                };
                ModelChoice {
                    name: entry.model.name.to_owned(),
                    details: self.labels.details(&entry.model),
                    status,
                    in_use: self.is_in_use(&entry.model),
                    result: self.labels.result(entry.tested.as_ref()),
                    error: if entry.did_not_run {
                        Some(self.labels.did_not_run.clone())
                    } else {
                        entry.error.as_ref().and_then(|error| self.labels.error(error))
                    },
                }
            })
            .collect()
    }
}

impl View for ModelManager {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        // Read, so that a click rebuilds this view. A thread wakes it the other way.
        let _ = self.changes.get();
        let waker = cx.ui_waker();
        let (starting, cancelling, removing) = (self.clone(), self.clone(), self.clone());
        let (testing, using, test_waker) = (self.clone(), self.clone(), waker.clone());
        let picker = ModelPicker::new(self.choices())
            .labels(self.labels.picker.clone())
            .on_download(move |index| {
                starting.start(index, waker.clone());
                starting.changed();
            })
            .on_cancel(move |index| {
                cancelling.cancel(index);
                cancelling.changed();
            })
            .on_test(move |index| {
                testing.test(index, test_waker.clone());
                testing.changed();
            })
            .on_use(move |index| {
                using.toggle_use(index);
                using.changed();
            })
            .on_remove(move |index| {
                removing.remove(index);
                removing.changed();
            });
        cx.child(&picker)
    }
}
