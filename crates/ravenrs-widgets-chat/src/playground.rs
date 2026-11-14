//! A place to write code and see what it makes: a [`CodeEditor`] beside the result,
//! examples to start from, and the problems the code has, marked where they are.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::Key;
use ravenrs_layout::Align;
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::code_editor::{CodeDiagnostic, CodeEditor, CodeLanguage};
use crate::context::{BuildCx, NodeId};
use crate::{
    Alert, AnyView, Badge, Button, ButtonStyle, Divider, Dot, DropdownMenu, HStack, IconButton, IconName,
    Keyed, MenuItem, ScrollView, SegmentedControl, Select, Spacer, SplitPane, SplitPanes, Text, Tone, VStack,
    View, ViewExt,
};

/// Below this width the code and the result take turns, as two tabs.
const TABS_BREAKPOINT: f32 = 768.0;
const CODE_MIN_WIDTH: f32 = 320.0;
const RESULT_MIN_WIDTH: f32 = 320.0;
/// The share of the width the result starts with; the code, with its longer lines, gets the rest.
const RESULT_SHARE: f32 = 0.4;

/// What turns the code of a [`Playground`] into something to show.
///
/// It runs when the person asks, never on every frame, so it may take its time and
/// build whatever it needs anew.
pub trait Runner {
    /// The view `source` makes, or where and why it cannot make one.
    fn run(&self, source: &str) -> Result<AnyView, Vec<CodeDiagnostic>>;

    /// What the code printed, and the problems of what it runs later, such as a closure on a
    /// click, since the last time this was asked. Nothing, unless the runner keeps them.
    fn take_output(&self) -> RunOutput {
        RunOutput::default()
    }

    /// Calls `wake` whenever there is something new for [`Self::take_output`], so the
    /// playground shows it as it comes. Nothing, unless the runner can tell.
    fn watch(&self, wake: Rc<dyn Fn()>) {
        let _ = wake;
    }
}

/// What a [`Runner`] heard from the code since it was last asked.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct RunOutput {
    /// Lines it printed, in order.
    pub lines: Vec<String>,
    /// Problems of what it ran after the run itself, with their places.
    pub problems: Vec<CodeDiagnostic>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl RunOutput {
    /// Output with these lines and problems.
    pub fn new(lines: Vec<String>, problems: Vec<CodeDiagnostic>) -> Self {
        Self { lines, problems }
    }

    /// Whether there is nothing in it.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.problems.is_empty()
    }
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
type Download = Rc<dyn Fn(&str) -> Result<(String, Vec<u8>), Vec<CodeDiagnostic>>>;
/// An extra button with the code already bound to it.
type Bound = (String, IconName, Rc<dyn Fn()>);

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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Playground {
    runner: Rc<dyn Runner>,
    examples: Vec<(String, String)>,
    source: Option<Binding<String>>,
    example: Option<Binding<usize>>,
    language: CodeLanguage,
    title: Option<String>,
    note: Option<String>,
    extras: Vec<Extra>,
    download: Option<Download>,
    height: Option<f32>,
    output_open: bool,
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
            download: None,
            height: None,
            output_open: true,
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

    /// A button that saves what `make` turns the code into, such as a project to build, with
    /// the system's dialog on a desktop or as a download in a browser. `make` gives the file's
    /// name and bytes, or the problems to mark in the code.
    pub fn download(
        mut self,
        make: impl Fn(&str) -> Result<(String, Vec<u8>), Vec<CodeDiagnostic>> + 'static,
    ) -> Self {
        self.download = Some(Rc::new(make));
        self
    }

    /// Whether the output starts open (it does by default). Folded, the result keeps the
    /// room, as on a small page about one example; a problem opens it all the same.
    pub fn output_open(mut self, open: bool) -> Self {
        self.output_open = open;
        self
    }

    /// A fixed height; by default it takes the room its parent gives.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
}

/// What the code said since the last run: the lines it printed and the problems of what ran
/// later. Kept apart from the reactive state, so reading it while building changes nothing.
#[derive(Default)]
struct Log {
    lines: Vec<String>,
    problems: Vec<CodeDiagnostic>,
}

/// Whether the playground is building, and what the code printed while it was.
#[derive(Default)]
struct Building {
    now: std::cell::Cell<bool>,
    printed: std::cell::Cell<bool>,
    /// The lines the last build printed, to tell a build that prints the same from one that
    /// has something new.
    last: RefCell<Vec<String>>,
    /// Whether the console already said that the same lines come on every build.
    warned: std::cell::Cell<bool>,
}

/// The most lines the console keeps, the oldest going first.
const MOST_LINES: usize = 500;

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Log {
    fn add(&mut self, output: RunOutput) {
        self.lines.extend(output.lines);
        let over = self.lines.len().saturating_sub(MOST_LINES);
        self.lines.drain(..over);
        self.problems.extend(output.problems);
    }
}

impl View for Playground {
    #[allow(clippy::too_many_lines, reason = "one screen, built top to bottom")]
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
        // 0 code, 1 result, 2 output: the tabs of a narrow playground.
        let tab = cx.use_state(|| 0usize);
        // 0 console, 1 problems.
        let output_tab = cx.use_state(|| 0usize);
        let open_at_first = self.output_open;
        let output_open = cx.use_state(move || open_at_first);
        let log: State<Rc<RefCell<Log>>> = cx.use_state(Rc::default);
        let heard: State<u64> = cx.use_state(|| 0);
        // While the playground builds, what the code prints is only kept: a `println!` in a
        // closure that runs on every build must not ask for one more build each time (#167).
        let building: State<Rc<Building>> = cx.use_state(Rc::default);
        let building = building.get();
        building.now.set(true);
        let started = cx.use_state(|| false);
        // Bumped on each request, so the bar redraws even where the screen does not resize.
        let asked_screen = cx.use_state(|| 0u32);
        asked_screen.get();
        let log = log.get();

        let run: Rc<dyn Fn()> = {
            let (runner, source, result, version, problems) =
                (self.runner.clone(), source.clone(), result.clone(), version.clone(), problems.clone());
            let (tab, output_tab, output_open, log, heard) =
                (tab.clone(), output_tab.clone(), output_open.clone(), log.clone(), heard.clone());
            Rc::new(move || {
                // What earlier runs left is not this run's.
                let _ = runner.take_output();
                *log.borrow_mut() = Log::default();
                match runner.run(&source.get()) {
                    Ok(view) => {
                        result.set(Some(Shown(Rc::from(view))));
                        version.update(|v| *v += 1);
                        problems.set(Vec::new());
                        if tab.get() == 0 {
                            tab.set(1);
                        }
                        if output_tab.get() == 1 {
                            output_tab.set(0);
                        }
                    }
                    Err(found) => {
                        problems.set(found);
                        output_tab.set(1);
                        output_open.set(true);
                    }
                }
                log.borrow_mut().add(runner.take_output());
                heard.update(|n| *n += 1);
            })
        };
        if !started.get() {
            started.set(true);
            // What the code prints, or gets wrong later, shows as it comes.
            let (wake, during) = (heard.clone(), building.clone());
            self.runner.watch(Rc::new(move || {
                if during.now.get() {
                    during.printed.set(true);
                } else {
                    wake.update(|n| *n += 1);
                }
            }));
            // An empty kept code starts from the first example too.
            if source.get().is_empty()
                && let Some((_, code)) = self.examples.first()
            {
                source.set(code.clone());
            }
            run();
        }
        cx.shortcut(Key::Enter, true, run.clone());
        // Read so that what is heard builds the playground again; then taken in.
        let _ = heard.get();
        let fresh = self.runner.take_output();
        if !fresh.is_empty() {
            log.borrow_mut().add(fresh);
        }

        let narrow = cx.viewport().width < TABS_BREAKPOINT;
        let found = problems.get();
        let late = log.borrow().problems.clone();
        let printed = log.borrow().lines.clone();
        let all_problems: Vec<CodeDiagnostic> = found.iter().chain(late.iter()).cloned().collect();

        // The examples: picking one puts its code in and runs it.
        let picker: Option<AnyView> = (self.examples.len() > 1).then(|| {
            let (examples, chosen, fill, go) =
                (self.examples.clone(), example.clone(), source.clone(), run.clone());
            let read = example.clone();
            let pick = Binding::new(
                move || read.get(),
                move |i: usize| {
                    chosen.set(i);
                    if let Some((_, code)) = examples.get(i) {
                        fill.set(code.clone());
                        go();
                    }
                },
            );
            Box::new(
                Select::new(pick, self.examples.iter().map(|(name, _)| name.clone()))
                    .label(labels.playground_example.clone()),
            ) as AnyView
        });
        let restore: Option<Rc<dyn Fn()>> = (!self.examples.is_empty()).then(|| {
            let (examples, chosen, fill) = (self.examples.clone(), example.clone(), source.clone());
            Rc::new(move || {
                if let Some((_, code)) = examples.get(chosen.get()) {
                    fill.set(code.clone());
                }
            }) as Rc<dyn Fn()>
        });
        let download: Option<Rc<dyn Fn()>> = self.download.as_ref().map(|make| {
            let (make, code, problems, picker) =
                (make.clone(), source.clone(), problems.clone(), cx.file_picker());
            let (output_tab, output_open) = (output_tab.clone(), output_open.clone());
            Rc::new(move || match make(&code.get()) {
                Ok((name, bytes)) => picker.save(&name, &bytes),
                Err(found) => {
                    problems.set(found);
                    output_tab.set(1);
                    output_open.set(true);
                }
            }) as Rc<dyn Fn()>
        });
        let extras: Vec<Bound> = self
            .extras
            .iter()
            .map(|(label, icon, then)| {
                let (then, code) = (then.clone(), source.clone());
                (label.clone(), *icon, Rc::new(move || then(&code.get())) as Rc<dyn Fn()>)
            })
            .collect();
        // Full screen, where the platform can give it.
        let screen = cx.full_screen();
        let toggle_screen: Option<Bound> = screen.available().then(|| {
            let (icon, label) = if screen.is_on() {
                (IconName::Minimize2, labels.playground_exit_full_screen.clone())
            } else {
                (IconName::Maximize2, labels.playground_full_screen.clone())
            };
            let asked = asked_screen.clone();
            (
                label,
                icon,
                Rc::new(move || {
                    screen.toggle();
                    asked.update(|n| *n = n.wrapping_add(1));
                }) as Rc<dyn Fn()>,
            )
        });
        let run_button = || {
            let go = run.clone();
            Button::new(labels.playground_run.clone())
                .icon(IconName::Play)
                .shortcut(if ravenrs_widgets_base::apple_keys() { "⌘↵" } else { "Ctrl+↵" })
                .on_click(move || go())
        };

        // The bar. Wide, every action is a button; narrow, the ones besides Run go in a menu.
        let mut bar: Vec<AnyView> = Vec::new();
        // Narrow, the examples take the title's place.
        if let Some(title) = self.title.as_ref().filter(|_| !narrow || self.examples.len() < 2) {
            bar.push(Box::new(Text::new(title.clone()).style(TextStyle::Headline)));
        }
        if narrow {
            if let Some(picker) = picker {
                bar.push(Box::new(VStack::new((picker,)).align(Align::Stretch).grow(1.0)));
            } else {
                bar.push(Box::new(Spacer::new()));
            }
            let mut menu = DropdownMenu::new(labels.playground_more.clone())
                .icon(IconName::Ellipsis)
                .icon_only()
                .style(ButtonStyle::Ghost);
            for (label, icon, then) in &extras {
                let then = then.clone();
                menu = menu.item(MenuItem::new(label.clone()).icon(*icon).on_select(move || then()));
            }
            if let Some(save) = &download {
                let save = save.clone();
                menu = menu.item(
                    MenuItem::new(labels.playground_download.clone())
                        .icon(IconName::Download)
                        .on_select(move || save()),
                );
            }
            if let Some(restore) = &restore {
                let restore = restore.clone();
                menu = menu.item(
                    MenuItem::new(labels.playground_restore.clone())
                        .icon(IconName::RotateCcw)
                        .on_select(move || restore()),
                );
            }
            if let Some((label, icon, toggle)) = &toggle_screen {
                let toggle = toggle.clone();
                menu = menu.item(MenuItem::new(label.clone()).icon(*icon).on_select(move || toggle()));
            }
            bar.push(Box::new(menu));
        } else {
            if let Some(picker) = picker {
                bar.push(picker);
            }
            bar.push(Box::new(Spacer::new()));
            for (label, icon, then) in &extras {
                let then = then.clone();
                bar.push(Box::new(
                    Button::new(label.clone()).icon(*icon).style(ButtonStyle::Ghost).on_click(move || then()),
                ));
            }
            if let Some(restore) = &restore {
                let restore = restore.clone();
                bar.push(Box::new(
                    Button::new(labels.playground_restore.clone())
                        .icon(IconName::RotateCcw)
                        .style(ButtonStyle::Ghost)
                        .on_click(move || restore()),
                ));
            }
            if let Some(save) = &download {
                let save = save.clone();
                bar.push(Box::new(
                    Button::new(labels.playground_download.clone())
                        .icon(IconName::Download)
                        .style(ButtonStyle::Ghost)
                        .on_click(move || save()),
                ));
            }
            if let Some((label, icon, toggle)) = &toggle_screen {
                let toggle = toggle.clone();
                bar.push(Box::new(
                    IconButton::new(*icon)
                        .label(label.clone())
                        .style(ButtonStyle::Ghost)
                        .on_click(move || toggle()),
                ));
            }
            bar.push(Box::new(run_button()));
        }
        let bar = HStack::new(bar)
            .spacing(8.0)
            .align(Align::Center)
            .padding((8.0, 12.0))
            .background(Semantic::Surface);

        // How the code stands, under the editor and in the narrow footer.
        let status = status_line(&labels, all_problems.len());
        // Where the caret is, as the status line of an editor says it (#163, #166).
        let caret: State<crate::text_field::Cursor> = cx.use_state(Default::default);
        let (line, column) = crate::line_and_column(&source.get(), caret.get().caret);
        let position = Text::new(
            labels
                .playground_position
                .replace("{line}", &line.to_string())
                .replace("{column}", &column.to_string()),
        )
        .style(TextStyle::Caption)
        .foreground(Semantic::InkMuted);

        let code = VStack::new((
            CodeEditor::new(source.clone())
                .language(self.language)
                .diagnostics(found.clone())
                .caret(caret)
                .fill(),
            HStack::new((status, Spacer::new(), position)).align(Align::Center).padding((6.0, 12.0)),
        ))
        .align(Align::Stretch)
        .grow(1.0)
        .background(Semantic::Surface)
        .grow(1.0);

        // The result: the last one that worked, faded and labeled when the code has changed since.
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
            Some(view) => {
                let view = ScrollView::new((Keyed::new(version.get(), view),)).grow(1.0);
                if found.is_empty() { Box::new(view) } else { Box::new(view.opacity(0.45)) }
            }
            None => Box::new(Spacer::new()),
        };
        let radius = cx.theme().radii.lg;
        let outcome = VStack::new((
            HStack::new(head).spacing(8.0).align(Align::Center),
            VStack::new((outcome,))
                .align(Align::Stretch)
                .grow(1.0)
                .padding(16.0)
                .background(Semantic::SurfaceElevated)
                .corner_radius(radius)
                .grow(1.0),
        ))
        .spacing(8.0)
        .align(Align::Stretch)
        .grow(1.0)
        .padding(12.0)
        .grow(1.0);

        // The output: what the code printed, and its problems with their places.
        let console: AnyView = if printed.is_empty() {
            Box::new(
                Text::new(labels.playground_nothing_printed.clone())
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
            )
        } else {
            let text = printed.join("\n");
            let shown = Binding::new(move || text.clone(), |_: String| {});
            Box::new(
                CodeEditor::new(shown)
                    .language(CodeLanguage::Plain)
                    .line_numbers(false)
                    .read_only(true)
                    .fill(),
            )
        };
        let problem_list: AnyView = if all_problems.is_empty() {
            Box::new(
                Text::new(labels.playground_no_problems.clone())
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
            )
        } else {
            let rows: Vec<AnyView> = all_problems
                .iter()
                .map(|problem| {
                    let place = format!(
                        "{} {}, {} {}",
                        labels.playground_line, problem.line, labels.playground_column, problem.column
                    );
                    let text = match &problem.hint {
                        Some(hint) => format!("{}. {hint}", problem.message),
                        None => problem.message.clone(),
                    };
                    let tone = if problem.tone == Tone::Warning { Tone::Warning } else { Tone::Danger };
                    Box::new(Alert::new(place, text).tone(tone)) as AnyView
                })
                .collect();
            Box::new(ScrollView::new((VStack::new(rows).spacing(8.0).align(Align::Stretch),)).grow(1.0))
        };
        let output_tabs = SegmentedControl::new(
            output_tab.binding(),
            [
                format!("{} {}", labels.playground_console, printed.len()),
                format!("{} {}", labels.playground_problems, all_problems.len()),
            ],
        )
        .label(labels.playground_output.clone());
        let output_body: AnyView = if output_tab.get() == 0 { console } else { problem_list };

        let clear = {
            let (log, problems, heard) = (log.clone(), problems.clone(), heard.clone());
            IconButton::new(IconName::Trash2)
                .label(labels.playground_clear_output.clone())
                .style(ButtonStyle::Ghost)
                .on_click(move || {
                    *log.borrow_mut() = Log::default();
                    problems.set(Vec::new());
                    heard.update(|n| *n += 1);
                })
        };

        let body: AnyView = if narrow {
            let tabs = SegmentedControl::new(
                tab.binding(),
                [
                    labels.playground_code.clone(),
                    labels.playground_result.clone(),
                    format!("{} {}", labels.playground_output, printed.len() + all_problems.len()),
                ],
            );
            let page: AnyView = match tab.get() {
                0 => Box::new(code),
                1 => Box::new(outcome),
                _ => Box::new(
                    VStack::new((output_tabs, output_body))
                        .spacing(8.0)
                        .align(Align::Stretch)
                        .grow(1.0)
                        .padding(12.0)
                        .grow(1.0),
                ),
            };
            let footer = HStack::new((
                VStack::new((status_line(&labels, all_problems.len()),)).grow(1.0),
                run_button(),
            ))
            .spacing(8.0)
            .align(Align::Center)
            .padding((8.0, 12.0))
            .background(Semantic::Surface);
            Box::new(
                VStack::new((VStack::new((tabs,)).align(Align::Stretch).padding((8.0, 12.0)), page, footer))
                    .spacing(8.0)
                    .align(Align::Stretch)
                    .grow(1.0),
            )
        } else {
            let open = output_open.get();
            let toggle = {
                let open_now = output_open.clone();
                IconButton::new(if open { IconName::ChevronDown } else { IconName::ChevronUp })
                    .label(if open {
                        labels.playground_hide_output.clone()
                    } else {
                        labels.playground_show_output.clone()
                    })
                    .style(ButtonStyle::Ghost)
                    .on_click(move || open_now.update(|o| *o = !*o))
            };
            let output_head =
                HStack::new((output_tabs, Spacer::new(), clear, toggle)).spacing(4.0).align(Align::Center);
            let output: AnyView = if open {
                Box::new(
                    VStack::new((output_head, output_body))
                        .spacing(8.0)
                        .align(Align::Stretch)
                        .height(OUTPUT_HEIGHT)
                        .padding(12.0)
                        .background(Semantic::Surface),
                )
            } else {
                Box::new(
                    VStack::new((output_head,))
                        .align(Align::Stretch)
                        .padding((6.0, 12.0))
                        .background(Semantic::Surface),
                )
            };
            let right = VStack::new((outcome, Divider::horizontal(), output)).align(Align::Stretch).grow(1.0);
            Box::new(
                SplitPanes::horizontal()
                    .pane(SplitPane::new(code).range(CODE_MIN_WIDTH, f32::MAX))
                    .pane(
                        SplitPane::new(right)
                            .size((cx.viewport().width * RESULT_SHARE).max(RESULT_MIN_WIDTH))
                            .range(RESULT_MIN_WIDTH, f32::MAX),
                    )
                    .grow(1.0),
            )
        };

        let mut parts: Vec<AnyView> = vec![Box::new(bar), Box::new(Divider::horizontal())];
        if let Some(note) = &self.note {
            parts.push(Box::new(
                Text::new(note.clone())
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted)
                    .padding((8.0, 12.0)),
            ));
        }
        parts.push(body);
        // The stack fills its box, so the body and the narrow footer reach its bottom.
        let whole = VStack::new(parts).align(Align::Stretch).grow(1.0);
        let node = match self.height {
            Some(height) => cx.child(&whole.background(Semantic::Canvas).height(height)),
            None => cx.child(&whole.background(Semantic::Canvas).grow(1.0)),
        };
        building.now.set(false);
        // What the build printed shows in the next one. Asked for only when it differs from
        // what the last build printed: the same lines on every build are printed each frame,
        // and asking again for them would never stop.
        if building.printed.replace(false) {
            let fresh = self.runner.take_output();
            let lines = fresh.lines.clone();
            let repeated = *building.last.borrow() == lines;
            if !fresh.is_empty() {
                log.borrow_mut().add(fresh);
            }
            // Said once, so whoever wrote it knows what keeps coming; one more build shows it.
            let warn = repeated && !lines.is_empty() && !building.warned.replace(true);
            if warn {
                log.borrow_mut().add(RunOutput::new(vec![labels.printed_every_build.clone()], Vec::new()));
            }
            if !repeated || warn {
                *building.last.borrow_mut() = lines;
                heard.update(|n| *n += 1);
            }
        }
        node
    }
}

/// The height of the open output panel.
const OUTPUT_HEIGHT: f32 = 200.0;

fn status_line(labels: &crate::Labels, count: usize) -> AnyView {
    if count == 0 {
        Box::new(
            HStack::new((
                Dot::new().tone(Tone::Success),
                Text::new(labels.playground_no_problems.clone()).style(TextStyle::Callout),
            ))
            .spacing(6.0)
            .align(Align::Center),
        )
    } else {
        Box::new(
            HStack::new((
                Dot::new().tone(Tone::Danger),
                Text::new(format!("{}: {count}", labels.playground_problems)).style(TextStyle::Callout),
            ))
            .spacing(6.0)
            .align(Align::Center),
        )
    }
}
