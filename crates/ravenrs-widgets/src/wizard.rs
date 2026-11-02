//! Going through a task one step at a time.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{CursorIcon, Insets, Role};
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
}

impl fmt::Debug for WizardStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WizardStep").field("title", &self.title).finish_non_exhaustive()
    }
}

impl WizardStep {
    /// A step with this title and content.
    pub fn new(title: impl Into<String>, content: impl View + 'static) -> Self {
        Self { title: title.into(), content: Box::new(content), check: None }
    }

    /// What has to hold to leave this step forward. The error is shown on the step,
    /// and going back never asks.
    pub fn check(mut self, check: impl Fn() -> Result<(), String> + 'static) -> Self {
        self.check = Some(Rc::new(check));
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

struct StepMark<'a> {
    index: usize,
    title: &'a str,
    current: usize,
    step: Binding<usize>,
    error: State<Option<String>>,
}

impl View for StepMark<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (done, here) = (self.index < self.current, self.index == self.current);
        let ink = theme.color(Semantic::Ink);
        let muted = theme.color(Semantic::InkMuted);
        let on_accent = theme.color(Semantic::OnAccent);
        let mark = if done {
            util::icon(cx, IconName::Check, 14.0, on_accent)
        } else {
            let number = (self.index + 1).to_string();
            let color = if here { on_accent } else { muted };
            util::label(cx, &number, TextStyle::Callout, Some(Weight::SEMIBOLD), color)
        };
        let fill = if done || here { theme.color(Semantic::Accent) } else { ink.with_alpha(0.08) };
        let dot = cx.container(
            LayoutStyle {
                width: Some(24.0),
                height: Some(24.0),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[mark],
            Paint::Quad(QuadPaint { corner_radius: theme.radii.full, ..QuadPaint::new(fill) }),
        );
        let weight = here.then_some(Weight::SEMIBOLD);
        let title =
            util::label(cx, self.title, TextStyle::Callout, weight, if here || done { ink } else { muted });
        let node = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(4.0, 6.0),
                spacing: 8.0,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[dot, title],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.sm,
                ..QuadPaint::new(if done && cx.is_hovered() {
                    ink.with_alpha(0.05)
                } else {
                    ravenrs_core::Color::TRANSPARENT
                })
            }),
        );
        cx.semantics(dot, Semantics::hidden());
        let labels = cx.labels();
        let description = if done { labels.wizard_done.clone() } else { String::new() };
        if done {
            let (step, error, index) = (self.step.clone(), self.error.clone(), self.index);
            cx.semantics(
                node,
                Semantics::new(Role::Button).name(self.title.to_owned()).description(description),
            );
            cx.on_click(
                node,
                Rc::new(move || {
                    error.set(None);
                    step.set(index);
                }),
            );
            cx.focusable(node, theme.radii.sm);
            cx.cursor(node, CursorIcon::Pointer);
        } else {
            cx.semantics(node, Semantics::new(Role::ListItem).name(self.title.to_owned()).selected(here));
        }
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
            let mark = StepMark {
                index,
                title: &step.title,
                current,
                step: self.step.clone(),
                error: error.clone(),
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
        cx.semantics(header, Semantics::new(Role::List).name((labels.wizard_progress)(current + 1, count)));
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
