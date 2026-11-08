//! Going through a task one step at a time.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{Alert, AnyView, Button, ButtonStyle, IconName, Tone, View, util};

type StepCheck = Rc<dyn Fn() -> Result<(), String>>;

/// One step of a [`Wizard`].
pub struct WizardStep {
    title: String,
    content: AnyView,
    check: Option<StepCheck>,
    failed: bool,
}

impl fmt::Debug for WizardStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WizardStep").field("title", &self.title).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl WizardStep {
    /// A step with this title and content.
    pub fn new(title: impl Into<String>, content: impl View + 'static) -> Self {
        Self { title: title.into(), content: Box::new(content), check: None, failed: false }
    }

    /// What has to hold to leave this step forward. The error is shown on the step,
    /// and going back never asks.
    pub fn check(mut self, check: impl Fn() -> Result<(), String> + 'static) -> Self {
        self.check = Some(Rc::new(check));
        self
    }

    /// Marks the step as having a problem, in the row of steps at the top, whichever
    /// step is showing. It does not stop anyone from leaving it: [`Self::check`] does.
    pub fn failed(mut self, failed: bool) -> Self {
        self.failed = failed;
        self
    }
}

/// A task in steps: where it stands at the top, one step on the screen, and buttons
/// to go on. A step can refuse to be left, and steps already done can be reopened.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Text, Ui, Wizard, WizardStep};
///
/// let step = State::new(0);
/// let wizard = Wizard::new(
///     step.binding(),
///     vec![
///         WizardStep::new("Data", Text::new("Where it comes from")),
///         WizardStep::new("Layout", Text::new("How it looks")).check(|| Err("Pick one".into())),
///     ],
/// )
/// .on_finish(|| println!("done"));
/// let mut ui = Ui::new(wizard, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct Wizard {
    step: Binding<usize>,
    steps: Vec<WizardStep>,
    on_finish: Option<Rc<dyn Fn()>>,
    on_cancel: Option<Rc<dyn Fn()>>,
    finish_label: Option<String>,
}

impl fmt::Debug for Wizard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Wizard").field("steps", &self.steps).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Wizard {
    /// A wizard over these steps, bound to the index of the one showing.
    pub fn new(step: Binding<usize>, steps: Vec<WizardStep>) -> Self {
        Self { step, steps, on_finish: None, on_cancel: None, finish_label: None }
    }

    /// Runs after the last step's check passes.
    pub fn on_finish(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_finish = Some(Rc::new(handler));
        self
    }

    /// Shows a Cancel button that runs this.
    pub fn on_cancel(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }

    /// The text of the last button.
    pub fn finish_label(mut self, label: impl Into<String>) -> Self {
        self.finish_label = Some(label.into());
        self
    }
}

/// Where a step of a task stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum StepState {
    /// Not reached yet.
    #[default]
    Upcoming,
    /// The one being done now.
    Current,
    /// Finished.
    Done,
    /// Has a problem to look at.
    Error,
}

/// The round mark of a step. Besides its color it has a shape of its own in each state:
/// a number, a check mark, or an alert over a ring.
fn step_dot(cx: &mut BuildCx<'_>, number: usize, state: StepState, size: f32) -> NodeId {
    let theme = *cx.theme();
    let (muted, on_accent, danger) =
        (theme.color(Semantic::InkMuted), theme.color(Semantic::OnAccent), theme.color(Semantic::Danger));
    let icon_size = size * 7.0 / 12.0;
    let mark = match state {
        StepState::Done => util::icon(cx, IconName::Check, icon_size, on_accent),
        StepState::Error => util::icon(cx, IconName::CircleAlert, icon_size, danger),
        StepState::Current | StepState::Upcoming => {
            let color = if state == StepState::Current { on_accent } else { muted };
            util::label(cx, &number.to_string(), TextStyle::Callout, Some(Weight::SEMIBOLD), color)
        }
    };
    let paint = match state {
        StepState::Done | StepState::Current => QuadPaint::new(theme.color(Semantic::Accent)),
        StepState::Upcoming => QuadPaint::new(theme.color(Semantic::Ink).with_alpha(0.08)),
        StepState::Error => QuadPaint {
            border: Some(Border { width: 1.0, color: danger }),
            ..QuadPaint::new(Color::TRANSPARENT)
        },
    };
    cx.container(
        LayoutStyle {
            width: Some(size),
            height: Some(size),
            align: Align::Center,
            justify: Justify::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        },
        &[mark],
        Paint::Quad(QuadPaint { corner_radius: theme.radii.full, ..paint }),
    )
}

/// The word for a state, for screen readers; the current step says so by being selected.
fn step_state_text(cx: &BuildCx<'_>, state: StepState) -> Option<String> {
    let labels = cx.labels();
    match state {
        StepState::Done => Some(labels.wizard_done.clone()),
        StepState::Error => Some(labels.wizard_error.clone()),
        StepState::Current | StepState::Upcoming => None,
    }
}

/// The round mark of a step by itself: its number, a check mark when done, or an alert
/// when it has a problem. For a list of steps, see [`StepIndicator`].
///
/// ```
/// use ravenrs_widgets::{StepBadge, StepState, Ui};
///
/// let mut ui = Ui::new(StepBadge::new(2).state(StepState::Done), ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[derive(Clone, Copy, Debug)]
pub struct StepBadge {
    number: usize,
    state: StepState,
    size: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl StepBadge {
    /// The mark of the step with this number, not reached yet.
    pub fn new(number: usize) -> Self {
        Self { number, state: StepState::Upcoming, size: 24.0 }
    }

    /// Where the step stands.
    pub fn state(mut self, state: StepState) -> Self {
        self.state = state;
        self
    }

    /// Its diameter in logical pixels (24 by default).
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

impl View for StepBadge {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let node = step_dot(cx, self.number, self.state, self.size);
        let mut semantics = Semantics::new(Role::Image)
            .name(self.number.to_string())
            .selected(self.state == StepState::Current);
        if let Some(text) = step_state_text(cx, self.state) {
            semantics = semantics.description(text);
        }
        cx.semantics(node, semantics);
        node
    }
}

/// One line of the row of steps: its mark, its title, and what it says under the title.
struct StepRow<'a> {
    number: usize,
    title: &'a str,
    note: Option<&'a str>,
    state: StepState,
    on_click: Option<Rc<dyn Fn()>>,
    stretch: bool,
}

impl View for StepRow<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let here = self.state == StepState::Current;
        let dot = step_dot(cx, self.number, self.state, 24.0);
        cx.semantics(dot, Semantics::hidden());
        let weight = here.then_some(Weight::SEMIBOLD);
        let color = if self.state == StepState::Upcoming { muted } else { ink };
        let title = util::label(cx, self.title, TextStyle::Callout, weight, color);
        let text = match self.note {
            Some(note) => {
                let note = util::label(cx, note, TextStyle::Caption, None, muted);
                cx.container(
                    LayoutStyle { grow: 1.0, spacing: 2.0, ..LayoutStyle::stack(Axis::Vertical) },
                    &[title, note],
                    Paint::None,
                )
            }
            None => title,
        };
        let clickable = self.on_click.is_some();
        let node = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(4.0, 6.0),
                spacing: 8.0,
                align: Align::Center,
                align_self: self.stretch.then_some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[dot, text],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.sm,
                ..QuadPaint::new(if clickable && cx.is_hovered() {
                    ink.with_alpha(0.05)
                } else {
                    Color::TRANSPARENT
                })
            }),
        );
        let role = if clickable { Role::Button } else { Role::ListItem };
        let mut semantics = Semantics::new(role).name(self.title.to_owned());
        // The note goes before the state, the way it reads on the screen.
        let description: Vec<String> =
            self.note.map(str::to_owned).into_iter().chain(step_state_text(cx, self.state)).collect();
        if !description.is_empty() {
            semantics = semantics.description(description.join(", "));
        }
        if here || !clickable {
            semantics = semantics.selected(here);
        }
        cx.semantics(node, semantics);
        if let Some(on_click) = &self.on_click {
            cx.on_click(node, on_click.clone());
            cx.focusable(node, theme.radii.sm);
            cx.cursor(node, CursorIcon::Pointer);
        }
        node
    }
}

/// One step shown by a [`StepIndicator`].
#[derive(Clone, Debug)]
pub struct IndicatorStep {
    title: String,
    state: StepState,
    note: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl IndicatorStep {
    /// A step with this title, in this state.
    pub fn new(title: impl Into<String>, state: StepState) -> Self {
        Self { title: title.into(), state, note: None }
    }

    /// A short line under the title, such as what is missing.
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// Where a task in steps stands, for an app that lays out the steps itself. A
/// [`Wizard`] has its own row of these.
///
/// ```
/// use ravenrs_widgets::{IndicatorStep, StepIndicator, StepState, Ui};
///
/// let steps = StepIndicator::new(vec![
///     IndicatorStep::new("Client", StepState::Done),
///     IndicatorStep::new("Photos", StepState::Error).note("Two are missing"),
///     IndicatorStep::new("Review", StepState::Current),
/// ])
/// .vertical()
/// .on_select(|step| println!("go to {step}"));
/// let mut ui = Ui::new(steps, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct StepIndicator {
    steps: Vec<IndicatorStep>,
    vertical: bool,
    on_select: Option<Rc<dyn Fn(usize)>>,
}

impl fmt::Debug for StepIndicator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StepIndicator").field("steps", &self.steps).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl StepIndicator {
    /// A row of these steps.
    pub fn new(steps: Vec<IndicatorStep>) -> Self {
        Self { steps, vertical: false, on_select: None }
    }

    /// Lists the steps one under the other.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    /// Makes the steps buttons: a click, Enter or Space runs `handler` with the step's
    /// position, 0 being the first.
    pub fn on_select(mut self, handler: impl Fn(usize) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl View for StepIndicator {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let mut rows = Vec::with_capacity(self.steps.len());
        for (i, step) in self.steps.iter().enumerate() {
            let on_click = self.on_select.clone().map(|select| Rc::new(move || select(i)) as Rc<dyn Fn()>);
            rows.push(cx.keyed_child(
                i as u64,
                &StepRow {
                    number: i + 1,
                    title: &step.title,
                    note: step.note.as_deref(),
                    state: step.state,
                    on_click,
                    stretch: self.vertical,
                },
            ));
        }
        let style = if self.vertical {
            LayoutStyle { spacing: 4.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) }
        } else {
            LayoutStyle {
                spacing: 4.0,
                wrap: true,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            }
        };
        let node = cx.container(style, &rows, Paint::None);
        let current = self.steps.iter().position(|s| s.state == StepState::Current).map_or(0, |i| i + 1);
        cx.semantics(
            node,
            Semantics::new(Role::List).name(labels.wizard_progress(current, self.steps.len())),
        );
        node
    }
}

/// What the forward button does: checks the step, then moves on or finishes.
fn advance(
    check: Option<StepCheck>,
    error: &State<Option<String>>,
    step: &Binding<usize>,
    next: Option<usize>,
    finish: Option<&Rc<dyn Fn()>>,
) {
    if let Some(check) = check
        && let Err(message) = check()
    {
        error.set(Some(message));
        return;
    }
    error.set(None);
    match next {
        Some(next) => step.set(next),
        None => {
            if let Some(finish) = finish {
                finish();
            }
        }
    }
}

impl View for Wizard {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let error: State<Option<String>> = cx.use_state(|| None);
        let count = self.steps.len();
        if count == 0 {
            return cx.empty();
        }
        let current = self.step.get().min(count - 1);
        let mut marks = Vec::with_capacity(count);
        for (index, step) in self.steps.iter().enumerate() {
            let state = match index.cmp(&current) {
                _ if step.failed => StepState::Error,
                std::cmp::Ordering::Less => StepState::Done,
                std::cmp::Ordering::Equal => StepState::Current,
                std::cmp::Ordering::Greater => StepState::Upcoming,
            };
            // Steps already gone through can be reopened.
            let on_click = (index < current).then(|| {
                let (moved, error) = (self.step.clone(), error.clone());
                Rc::new(move || {
                    error.set(None);
                    moved.set(index);
                }) as Rc<dyn Fn()>
            });
            let mark = StepRow {
                number: index + 1,
                title: &step.title,
                note: None,
                state,
                on_click,
                stretch: false,
            };
            marks.push(cx.keyed_child(index as u64, &mark));
        }
        let header = cx.container(
            LayoutStyle {
                spacing: 4.0,
                wrap: true,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &marks,
            Paint::None,
        );
        cx.semantics(header, Semantics::new(Role::List).name(labels.wizard_progress(current + 1, count)));
        let step = &self.steps[current];
        let mut body = Vec::new();
        if let Some(message) = error.get() {
            body.push(cx.child(&Alert::new(step.title.clone(), message).tone(Tone::Danger)));
        }
        body.push(cx.keyed_child(current as u64 + 1, step.content.as_ref()));
        let content = cx.container(
            LayoutStyle {
                grow: 1.0,
                spacing: 12.0,
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &body,
            Paint::None,
        );
        let mut buttons = Vec::new();
        if let Some(cancel) = &self.on_cancel {
            let cancel = cancel.clone();
            buttons.push(
                cx.child(
                    &Button::new(labels.wizard_cancel.clone())
                        .style(ButtonStyle::Ghost)
                        .on_click(move || cancel()),
                ),
            );
        }
        buttons.push(cx.container(LayoutStyle { grow: 1.0, ..LayoutStyle::leaf() }, &[], Paint::None));
        if current > 0 {
            let (back, failed) = (self.step.clone(), error.clone());
            buttons.push(cx.child(
                &Button::new(labels.wizard_back.clone()).style(ButtonStyle::Secondary).on_click(move || {
                    failed.set(None);
                    back.set(current - 1);
                }),
            ));
        }
        let last = current + 1 == count;
        let text = if last {
            self.finish_label.clone().unwrap_or_else(|| labels.wizard_finish.clone())
        } else {
            labels.wizard_next.clone()
        };
        let (check, failed, moved, finish) =
            (step.check.clone(), error.clone(), self.step.clone(), self.on_finish.clone());
        buttons.push(cx.child(&Button::new(text).on_click(move || {
            advance(check.clone(), &failed, &moved, (!last).then_some(current + 1), finish.as_ref());
        })));
        let footer = cx.container(
            LayoutStyle {
                spacing: 8.0,
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &buttons,
            Paint::None,
        );
        cx.container(
            LayoutStyle { spacing: 16.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[header, content, footer],
            Paint::None,
        )
    }
}
