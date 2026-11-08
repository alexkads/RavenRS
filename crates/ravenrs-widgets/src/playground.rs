//! A place to write code and see what it makes: a [`CodeEditor`] beside the result,
//! examples to start from, and the problems the code has, marked where they are.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::Key;
use ravenrs_layout::Align;
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::code_editor::{CodeDiagnostic, CodeEditor, CodeLanguage};
use crate::context::{BuildCx, NodeId};
use crate::{
    Alert, AnyView, Badge, Button, ButtonStyle, HStack, IconName, Keyed, ScrollView, SegmentedControl,
    Select, Spacer, SplitPane, SplitPanes, Text, Tone, VStack, View, ViewExt,
};

/// Below this width the code and the result take turns, as two tabs.
const TABS_BREAKPOINT: f32 = 768.0;
const CODE_MIN_WIDTH: f32 = 320.0;
const RESULT_MIN_WIDTH: f32 = 320.0;

/// What turns the code of a [`Playground`] into something to show.
///
/// It runs when the person asks, never on every frame, so it may take its time and
/// build whatever it needs anew.
pub trait Runner {
    /// The view `source` makes, or where and why it cannot make one.
    fn run(&self, source: &str) -> Result<AnyView, Vec<CodeDiagnostic>>;
}

impl<F: Fn(&str) -> Result<AnyView, Vec<CodeDiagnostic>>> Runner for F {
    fn run(&self, source: &str) -> Result<AnyView, Vec<CodeDiagnostic>> {
        self(source)
    }
}

/// A view kept between builds, shown again until the next run.
#[derive(Clone)]
struct Shown(Rc<dyn View>);

impl View for Shown {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.0.build(cx)
    }
}

/// A button the app adds to the bar, which gets the code as it is.
type Extra = (String, IconName, Rc<dyn Fn(&str)>);

/// A place to write code and see what it makes, for documentation, tutorials and tools.
///
/// The code is on one side and the result on the other; below 768 pixels they become two
/// tabs. Run (or Ctrl/Cmd+Enter) hands the code to the [`Runner`]. When it has problems,
/// they are listed under the editor and marked on their lines, and the last result that
/// worked stays, labeled as such. Examples fill the editor, Restore puts the chosen example
/// back, and the app can add buttons that receive the code, such as one that copies a link.
///
/// ```
/// use ravenrs_widgets::{AnyView, CodeDiagnostic, Playground, Text, Ui};
///
/// let runner = |source: &str| -> Result<AnyView, Vec<CodeDiagnostic>> {
///     match source.trim().strip_prefix("say ") {
///         Some(words) => Ok(Box::new(Text::new(words.to_owned()))),
///         None => Err(vec![CodeDiagnostic::error(1, 1, 3, "Start with `say`")]),
///     }
/// };
/// let playground = Playground::new(runner).example("Hello", "say hello").height(400.0);
/// let mut ui = Ui::new(playground, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct Playground {
    runner: Rc<dyn Runner>,
    examples: Vec<(String, String)>,
    source: Option<Binding<String>>,
    example: Option<Binding<usize>>,
    language: CodeLanguage,
    title: Option<String>,
    note: Option<String>,
    extras: Vec<Extra>,
    height: Option<f32>,
}

impl fmt::Debug for Playground {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Playground").field("examples", &self.examples.len()).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Playground {
    /// A playground whose code `runner` runs.
    pub fn new(runner: impl Runner + 'static) -> Self {
        Self {
            runner: Rc::new(runner),
            examples: Vec::new(),
            source: None,
            example: None,
            language: CodeLanguage::Rust,
            title: None,
            note: None,
            extras: Vec::new(),
            height: None,
        }
    }

    /// An example to start from, listed in the bar; the first one fills the editor.
    pub fn example(mut self, name: impl Into<String>, source: impl Into<String>) -> Self {
        self.examples.push((name.into(), source.into()));
        self
    }

    /// Keeps the code in `source`, so the app can save it, share it or start from it.
    /// While it is empty, or without it, the playground starts from the first example.
    pub fn source(mut self, source: Binding<String>) -> Self {
        self.source = Some(source);
        self
    }

    /// Keeps which example is chosen in `example`.
    pub fn chosen_example(mut self, example: Binding<usize>) -> Self {
        self.example = Some(example);
        self
    }

    /// The language the editor colors; Rust by default.
    pub fn language(mut self, language: CodeLanguage) -> Self {
        self.language = language;
        self
    }

    /// A title at the start of the bar.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// A line under the bar, such as what this playground can and cannot run yet.
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// A button in the bar that gets the code as it is.
    pub fn action(mut self, label: impl Into<String>, icon: IconName, then: impl Fn(&str) + 'static) -> Self {
        self.extras.push((label.into(), icon, Rc::new(then)));
        self
    }

    /// A fixed height; by default it takes the room its parent gives.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
}

impl View for Playground {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let first = self.examples.first().map(|(_, code)| code.clone()).unwrap_or_default();
        let own_source = cx.use_state(|| first);
        let source = self.source.clone().unwrap_or_else(|| own_source.binding());
        let own_example = cx.use_state(|| 0usize);
        let example = self.example.clone().unwrap_or_else(|| own_example.binding());
        let result: State<Option<Shown>> = cx.use_state(|| None);
        let version: State<u64> = cx.use_state(|| 0);
        let problems: State<Vec<CodeDiagnostic>> = cx.use_state(Vec::new);
        let tab = cx.use_state(|| 0usize);
        let started = cx.use_state(|| false);

        let run: Rc<dyn Fn()> = {
            let (runner, source, result, version, problems) =
                (self.runner.clone(), source.clone(), result.clone(), version.clone(), problems.clone());
            let tab = tab.clone();
            Rc::new(move || match runner.run(&source.get()) {
                Ok(view) => {
                    result.set(Some(Shown(Rc::from(view))));
                    version.update(|v| *v += 1);
                    problems.set(Vec::new());
                    tab.set(1);
                }
                Err(found) => problems.set(found),
            })
        };
        if !started.get() {
            started.set(true);
            // An empty kept code starts from the first example too.
            if source.get().is_empty()
                && let Some((_, code)) = self.examples.first()
            {
                source.set(code.clone());
            }
            run();
        }
        cx.shortcut(Key::Enter, true, run.clone());

        // The bar: what it is, the examples, and what to do with the code.
        let mut bar: Vec<AnyView> = Vec::new();
        if let Some(title) = &self.title {
            bar.push(Box::new(Text::new(title.clone()).style(TextStyle::Headline)));
        }
        bar.push(Box::new(Spacer::new()));
        if self.examples.len() > 1 {
            let (examples, chosen, fill) = (self.examples.clone(), example.clone(), source.clone());
            let read = example.clone();
            let pick = Binding::new(
                move || read.get(),
                move |i: usize| {
                    chosen.set(i);
                    if let Some((_, code)) = examples.get(i) {
                        fill.set(code.clone());
                    }
                },
            );
            bar.push(Box::new(
                Select::new(pick, self.examples.iter().map(|(name, _)| name.clone()))
                    .label(labels.playground_example.clone()),
            ));
        }
        if !self.examples.is_empty() {
            let (examples, chosen, fill) = (self.examples.clone(), example.clone(), source.clone());
            bar.push(Box::new(
                Button::new(labels.playground_restore.clone())
                    .icon(IconName::RotateCcw)
                    .style(ButtonStyle::Secondary)
                    .on_click(move || {
                        if let Some((_, code)) = examples.get(chosen.get()) {
                            fill.set(code.clone());
                        }
                    }),
            ));
        }
        for (label, icon, then) in &self.extras {
            let (then, code) = (then.clone(), source.clone());
            bar.push(Box::new(
                Button::new(label.clone())
                    .icon(*icon)
                    .style(ButtonStyle::Secondary)
                    .on_click(move || then(&code.get())),
            ));
        }
        let go = run.clone();
        bar.push(Box::new(
            Button::new(labels.playground_run.clone())
                .icon(IconName::Play)
                .shortcut(if cfg!(target_os = "macos") { "⌘↵" } else { "Ctrl+↵" })
                .on_click(move || go()),
        ));
        let narrow = cx.viewport().width < TABS_BREAKPOINT;
        let bar = HStack::new(bar).spacing(8.0).align(Align::Center);
        let bar = if narrow { bar.wrap() } else { bar };

        // The code, with its problems under it.
        let found = problems.get();
        let mut code: Vec<AnyView> = vec![Box::new(
            CodeEditor::new(source.clone()).language(self.language).diagnostics(found.clone()).fill(),
        )];
        for problem in found.iter().take(3) {
            let place = format!(
                "{} {}, {} {}",
                labels.playground_line, problem.line, labels.playground_column, problem.column
            );
            let text = match &problem.hint {
                Some(hint) => format!("{}. {hint}", problem.message),
                None => problem.message.clone(),
            };
            let tone = if problem.tone == Tone::Warning { Tone::Warning } else { Tone::Danger };
            code.push(Box::new(Alert::new(place, text).tone(tone)));
        }
        let code = VStack::new(code).spacing(8.0).align(Align::Stretch).grow(1.0);

        // The result: the last one that worked, labeled when the code has changed since.
        let shown = result.get();
        let mut head: Vec<AnyView> = vec![Box::new(
            Text::new(labels.playground_result.to_uppercase())
                .style(TextStyle::Caption)
                .foreground(Semantic::InkMuted),
        )];
        if shown.is_some() && !found.is_empty() {
            head.push(Box::new(Badge::new(labels.playground_last_good.clone()).tone(Tone::Warning)));
        }
        let outcome: AnyView = match shown {
            Some(view) => Box::new(ScrollView::new((Keyed::new(version.get(), view),)).grow(1.0)),
            None => Box::new(Spacer::new()),
        };
        let outcome = VStack::new((HStack::new(head).spacing(8.0).align(Align::Center), outcome))
            .spacing(8.0)
            .align(Align::Stretch)
            .grow(1.0)
            .padding(12.0)
            .background(Semantic::Canvas)
            .corner_radius(cx.theme().radii.lg);

        let body: AnyView = if narrow {
            let tabs = SegmentedControl::new(
                tab.binding(),
                [labels.playground_code.clone(), labels.playground_result.clone()],
            );
            let page: AnyView = if tab.get() == 0 { Box::new(code) } else { Box::new(outcome) };
            Box::new(VStack::new((tabs, page)).spacing(8.0).align(Align::Stretch).grow(1.0))
        } else {
            Box::new(
                SplitPanes::horizontal()
                    .pane(SplitPane::new(code).range(CODE_MIN_WIDTH, f32::MAX))
                    .pane(SplitPane::new(outcome).range(RESULT_MIN_WIDTH, f32::MAX))
                    .grow(1.0),
            )
        };

        let mut parts: Vec<AnyView> = vec![Box::new(bar)];
        if let Some(note) = &self.note {
            parts.push(Box::new(
                Text::new(note.clone()).style(TextStyle::Callout).foreground(Semantic::InkMuted),
            ));
        }
        parts.push(body);
        let whole = VStack::new(parts).spacing(12.0).align(Align::Stretch);
        match self.height {
            Some(height) => cx.child(&whole.height(height)),
            None => cx.child(&whole.grow(1.0)),
        }
    }
}
