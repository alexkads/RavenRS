//! Choosing a local AI model: download it, try it, use it or remove it.

use std::fmt;
use std::rc::Rc;

use ravenrs_layout::Align;
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId};
use crate::view::{AnyView, View};
use crate::{
    Alert, Button, ButtonSize, ButtonStyle, Card, HStack, IconName, ProgressBar, Select, Text, Tone, VStack,
};

/// Where a [`ModelChoice`] stands.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum ModelStatus {
    /// It can be downloaded.
    Available,
    /// It is arriving, from 0 to 1.
    Downloading(f32),
    /// It is on this machine.
    Installed,
    /// It is being tried.
    Testing,
}

/// A model offered by a [`ModelPicker`].
#[derive(Clone, Debug, PartialEq)]
pub struct ModelChoice {
    /// Its name.
    pub name: String,
    /// What to know before downloading it, such as its size and license.
    pub details: String,
    /// Where it stands.
    pub status: ModelStatus,
    /// Whether the app is using it.
    pub in_use: bool,
    /// How it did when tried, in words.
    pub result: Option<String>,
    /// What went wrong the last time, in words.
    pub error: Option<String>,
}

/// The words a [`ModelPicker`] shows. English by default.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ModelPickerLabels {
    /// Downloads a model.
    pub download: String,
    /// Stops a download.
    pub cancel: String,
    /// Tries a model.
    pub test: String,
    /// While a model is being tried.
    pub testing: String,
    /// Starts using a model.
    pub use_it: String,
    /// Stops using a model.
    pub stop_using: String,
    /// Marks the model in use.
    pub in_use: String,
    /// Removes a model from the machine.
    pub remove: String,
    /// When no model fits this machine.
    pub nothing_fits: String,
    /// The list of models.
    pub model: String,
    /// Marks a model that is on this machine.
    pub downloaded: String,
}

impl Default for ModelPickerLabels {
    fn default() -> Self {
        Self {
            download: "Download".into(),
            cancel: "Cancel".into(),
            test: "Test speed".into(),
            testing: "Testing…".into(),
            use_it: "Use".into(),
            stop_using: "Stop using".into(),
            in_use: "In use".into(),
            remove: "Remove".into(),
            nothing_fits: "No model fits this machine.".into(),
            model: "Model".into(),
            downloaded: "Downloaded".into(),
        }
    }
}

impl ModelPickerLabels {
    /// The words in Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self {
            download: "Baixar".into(),
            cancel: "Cancelar".into(),
            test: "Testar velocidade".into(),
            testing: "Testando…".into(),
            use_it: "Usar".into(),
            stop_using: "Deixar de usar".into(),
            in_use: "Em uso".into(),
            remove: "Remover".into(),
            nothing_fits: "Nenhum modelo cabe nesta máquina.".into(),
            model: "Modelo".into(),
            downloaded: "Baixado".into(),
        }
    }
}

type OnChoice = Rc<dyn Fn(usize)>;

/// A choice of local AI models, however many there are, to download, try, use and remove. It shows what it is
/// given: deciding which models fit and doing the work is up to the app, or to
/// `ravenrs-ai`.
///
/// ```
/// use ravenrs_widgets::{ModelChoice, ModelPicker, ModelStatus};
///
/// let picker = ModelPicker::new(vec![ModelChoice {
///     name: "Small model".into(),
///     details: "1.1 GB · Apache 2.0".into(),
///     status: ModelStatus::Available,
///     in_use: false,
///     result: None,
///     error: None,
/// }])
/// .on_download(|index| println!("download {index}"));
/// ```
pub struct ModelPicker {
    choices: Vec<ModelChoice>,
    labels: ModelPickerLabels,
    on_download: Option<OnChoice>,
    on_cancel: Option<OnChoice>,
    on_test: Option<OnChoice>,
    on_use: Option<OnChoice>,
    on_remove: Option<OnChoice>,
}

impl fmt::Debug for ModelPicker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelPicker").field("choices", &self.choices).finish_non_exhaustive()
    }
}

impl ModelPicker {
    /// A list of these models.
    pub fn new(choices: Vec<ModelChoice>) -> Self {
        Self {
            choices,
            labels: ModelPickerLabels::default(),
            on_download: None,
            on_cancel: None,
            on_test: None,
            on_use: None,
            on_remove: None,
        }
    }

    /// The words it shows.
    #[must_use]
    pub fn labels(mut self, labels: ModelPickerLabels) -> Self {
        self.labels = labels;
        self
    }

    /// Called with a model's position when someone asks to download it.
    #[must_use]
    pub fn on_download(mut self, handler: impl Fn(usize) + 'static) -> Self {
        self.on_download = Some(Rc::new(handler));
        self
    }

    /// Called with a model's position when someone stops its download.
    #[must_use]
    pub fn on_cancel(mut self, handler: impl Fn(usize) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }

    /// Called with a model's position when someone asks to try it.
    #[must_use]
    pub fn on_test(mut self, handler: impl Fn(usize) + 'static) -> Self {
        self.on_test = Some(Rc::new(handler));
        self
    }

    /// Called with a model's position when someone starts or stops using it.
    #[must_use]
    pub fn on_use(mut self, handler: impl Fn(usize) + 'static) -> Self {
        self.on_use = Some(Rc::new(handler));
        self
    }

    /// Called with a model's position when someone removes it.
    #[must_use]
    pub fn on_remove(mut self, handler: impl Fn(usize) + 'static) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }

    fn button(
        label: &str,
        style: ButtonStyle,
        icon: IconName,
        index: usize,
        handler: Option<&OnChoice>,
    ) -> Button {
        let handler = handler.cloned();
        Button::new(label.to_owned()).style(style).size(ButtonSize::Small).icon(icon).on_click(move || {
            if let Some(handler) = &handler {
                handler(index);
            }
        })
    }

    fn actions(&self, index: usize, choice: &ModelChoice) -> AnyView {
        let labels = &self.labels;
        match choice.status {
            ModelStatus::Available => Box::new(Self::button(
                &labels.download,
                ButtonStyle::Primary,
                IconName::Download,
                index,
                self.on_download.as_ref(),
            )),
            ModelStatus::Downloading(_) => Box::new(Self::button(
                &labels.cancel,
                ButtonStyle::Secondary,
                IconName::X,
                index,
                self.on_cancel.as_ref(),
            )),
            ModelStatus::Testing => Box::new(
                Text::new(labels.testing.clone()).style(TextStyle::Callout).foreground(Semantic::InkMuted),
            ),
            _ => Box::new(
                HStack::new((
                    self.on_test.as_ref().map(|handler| {
                        Self::button(
                            &labels.test,
                            ButtonStyle::Secondary,
                            IconName::Zap,
                            index,
                            Some(handler),
                        )
                    }),
                    self.on_use.as_ref().map(|handler| {
                        Self::button(
                            if choice.in_use { &labels.stop_using } else { &labels.use_it },
                            if choice.in_use { ButtonStyle::Secondary } else { ButtonStyle::Primary },
                            IconName::Sparkles,
                            index,
                            Some(handler),
                        )
                    }),
                    Self::button(
                        &labels.remove,
                        ButtonStyle::Ghost,
                        IconName::Trash2,
                        index,
                        self.on_remove.as_ref(),
                    ),
                ))
                .spacing(8.0)
                .wrap(),
            ),
        }
    }
}

impl ModelPicker {
    /// How a model reads in the list: its name and where it stands.
    fn option(&self, choice: &ModelChoice) -> String {
        let labels = &self.labels;
        match choice.status {
            _ if choice.in_use => format!("{} · {}", choice.name, labels.in_use),
            ModelStatus::Downloading(fraction) => {
                format!("{} · {:.0}%", choice.name, fraction.clamp(0.0, 1.0) * 100.0)
            }
            ModelStatus::Installed | ModelStatus::Testing => {
                format!("{} · {}", choice.name, labels.downloaded)
            }
            _ => choice.name.clone(),
        }
    }
}

impl View for ModelPicker {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        if self.choices.is_empty() {
            let text = Text::new(self.labels.nothing_fits.clone())
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted);
            return cx.child(&text);
        }
        // It opens on the model that matters: the one in use, or else one already here.
        let selected = cx.use_state(|| {
            let installed = |choice: &ModelChoice| choice.status != ModelStatus::Available;
            self.choices
                .iter()
                .position(|choice| choice.in_use)
                .or_else(|| self.choices.iter().position(installed))
                .unwrap_or(0)
        });
        let index = selected.get().min(self.choices.len() - 1);
        let choice = &self.choices[index];
        let options: Vec<String> = self.choices.iter().map(|choice| self.option(choice)).collect();
        let progress = match choice.status {
            ModelStatus::Downloading(fraction) => Some(ProgressBar::new(fraction)),
            _ => None,
        };
        let card = Card::new(
            VStack::new((
                Select::new(selected.binding(), options).label(self.labels.model.clone()),
                Text::new(choice.details.clone()).style(TextStyle::Callout).foreground(Semantic::InkMuted),
                progress,
                choice.result.clone().map(|result| Text::new(result).style(TextStyle::Callout)),
                choice.error.clone().map(|error| Alert::new("", error).tone(Tone::Danger)),
                self.actions(index, choice),
            ))
            .spacing(10.0)
            .align(Align::Stretch),
        );
        cx.child(&card)
    }
}
