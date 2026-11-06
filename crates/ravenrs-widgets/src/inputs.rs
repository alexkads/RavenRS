//! Form controls bound to state through [`Binding`].

use std::rc::Rc;

use ravenrs_core::{
    Border, Color, CursorIcon, Effect, Insets, Key, LinearGradient, Point, Quad, Rect, Role, Scene, Shadow,
};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use crate::context::{BuildCx, DragPhase, NodeId, Paint, QuadPaint, Semantics, WidgetId};
use crate::{IconName, Motion, View, motion, util};

fn row(cx: &mut BuildCx<'_>, children: &[NodeId]) -> NodeId {
    let style = LayoutStyle {
        spacing: 10.0,
        align: Align::Center,
        align_self: Some(Align::Fit),
        ..LayoutStyle::stack(Axis::Horizontal)
    };
    cx.container(style, children, Paint::None)
}

fn make_interactive(cx: &mut BuildCx<'_>, node: NodeId, radius: f32, on_click: Rc<dyn Fn()>) {
    cx.on_click(node, on_click);
    cx.focusable(node, radius);
    cx.cursor(node, CursorIcon::Pointer);
}

/// Option of a single-choice group (radio buttons, segments): arrow keys select
/// and focus the neighbor, following the WAI-ARIA radio group pattern.
struct ChoiceOption {
    roving: util::Roving,
    selected: Binding<usize>,
}

impl ChoiceOption {
    fn index(&self) -> usize {
        self.roving.index
    }

    fn wire(&self, cx: &mut BuildCx<'_>, node: NodeId, radius: f32) {
        let (selected, index) = (self.selected.clone(), self.index());
        make_interactive(cx, node, radius, Rc::new(move || selected.set(index)));
        let selected = self.selected.clone();
        self.roving.wire(cx, node, Some(Rc::new(move |i| selected.set(i))));
    }
}

/// An on/off switch.
#[derive(Debug)]
pub struct Toggle {
    value: Binding<bool>,
    label: Option<String>,
    small: bool,
    state: util::EditorState,
}

impl Toggle {
    /// A switch bound to `value`.
    pub fn new(value: Binding<bool>) -> Self {
        Self { value, label: None, small: false, state: util::EditorState::default() }
    }

    /// A smaller switch, 32 by 18 logical pixels instead of 40 by 24, for dense lists.
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    /// Text after the switch; clicking it toggles too.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Turns the control off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the value without letting it change; it can still be focused and read.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

impl View for Toggle {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let on = self.value.get();
        let state = self.state;
        let hovered = cx.is_hovered() && state.accepts_input();
        let track = if on {
            theme.color(if hovered { Semantic::AccentHover } else { Semantic::Accent })
        } else {
            theme.color(Semantic::Control).lerp(theme.color(Semantic::Ink), if hovered { 0.08 } else { 0.0 })
        };
        let track = state.fill(track);
        let border = (!on).then(|| Border { width: 1.0, color: state.ink(theme.color(Semantic::Border)) });
        let (track_width, track_height, knob, inset) =
            if self.small { (32.0, 18.0, 14.0, 2.0) } else { (40.0, 24.0, 18.0, 3.0) };
        // The thumb stretches while held and is kept apart from the track so it
        // can glide from one end to the other.
        let thumb = cx.container(
            LayoutStyle {
                width: Some(if cx.is_pressed() && state.accepts_input() { knob + 4.0 } else { knob }),
                height: Some(knob),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Quad(QuadPaint {
                corner_radius: knob / 2.0,
                shadow: Some(Shadow {
                    offset: Point::new(0.0, 1.0),
                    blur: 3.0,
                    color: Color::rgba(0.0, 0.0, 0.0, 0.3),
                }),
                ..QuadPaint::new(state.ink(Color::WHITE))
            }),
        );
        let switch = cx.container(
            LayoutStyle {
                width: Some(track_width),
                height: Some(track_height),
                padding: Insets::uniform(inset),
                justify: if on { Justify::End } else { Justify::Start },
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[thumb],
            Paint::Quad(QuadPaint { corner_radius: track_height / 2.0, border, ..QuadPaint::new(track) }),
        );
        cx.motion(thumb, Motion::new().layout(theme.motion.snappy));
        motion::control(cx, switch);
        let mut children = vec![switch];
        if let Some(label) = &self.label {
            children.push(util::label(
                cx,
                label,
                TextStyle::Body,
                None,
                state.ink(theme.color(Semantic::Ink)),
            ));
        }
        let node = row(cx, &children);
        cx.semantics(node, state.semantics(Semantics::new(Role::Switch).toggled(on)));
        if state.accepts_input() {
            let value = self.value.clone();
            make_interactive(cx, node, 8.0, Rc::new(move || value.update(|v| *v = !*v)));
        } else if !state.disabled {
            cx.focusable(node, 8.0);
        }
        node
    }
}

/// A check box with a label.
#[derive(Debug)]
pub struct Checkbox {
    value: Binding<bool>,
    label: String,
    leading: Option<f32>,
    state: util::EditorState,
}

impl Checkbox {
    /// A check box bound to `value`, with this label.
    pub fn new(label: impl Into<String>, value: Binding<bool>) -> Self {
        Self { value, label: label.into(), leading: None, state: util::EditorState::default() }
    }

    /// Reserves this width for the box, so labels line up in a column. A list
    /// that mixes check boxes with [`Toggle`]s passes the toggle's width, 40.
    pub fn leading(mut self, width: f32) -> Self {
        self.leading = Some(width);
        self
    }

    /// Turns the control off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the value without letting it change; it can still be focused and read.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

pub(crate) fn check_box(cx: &mut BuildCx<'_>, theme: &Theme, checked: bool, hovered: bool) -> NodeId {
    let mark = if checked {
        vec![util::icon(cx, IconName::Check, 14.0, theme.color(Semantic::OnAccent))]
    } else {
        vec![]
    };
    check_box_around(cx, theme, checked, hovered, util::EditorState::default(), &mark)
}

pub(crate) fn check_box_around(
    cx: &mut BuildCx<'_>,
    theme: &Theme,
    checked: bool,
    hovered: bool,
    state: util::EditorState,
    mark: &[NodeId],
) -> NodeId {
    let paint = if checked {
        QuadPaint { corner_radius: 5.0, ..QuadPaint::new(state.fill(theme.color(Semantic::Accent))) }
    } else {
        let edge = if hovered { theme.color(Semantic::Ink) } else { theme.color(Semantic::InkMuted) };
        QuadPaint {
            corner_radius: 5.0,
            border: Some(Border { width: 1.5, color: state.ink(edge.with_alpha(0.7)) }),
            ..QuadPaint::new(Color::TRANSPARENT)
        }
    };
    let style = LayoutStyle {
        width: Some(18.0),
        height: Some(18.0),
        align: Align::Center,
        justify: Justify::Center,
        ..LayoutStyle::stack(Axis::Horizontal)
    };
    cx.container(style, mark, Paint::Quad(paint))
}

/// The check or dot inside a selected control pops in and fades away.
fn pop_mark(cx: &mut BuildCx<'_>, mark: NodeId) {
    let m = cx.theme().motion;
    cx.motion(
        mark,
        Motion::new()
            .enter(Effect::hidden().with_scale(0.5), m.snappy)
            .exit(Effect::hidden().with_scale(0.5), m.fast),
    );
}

impl View for Checkbox {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let checked = self.value.get();
        let state = self.state;
        let mark = checked.then(|| util::icon(cx, IconName::Check, 14.0, theme.color(Semantic::OnAccent)));
        let hovered = cx.is_hovered() && state.accepts_input();
        let mut b = check_box_around(cx, &theme, checked, hovered, state, mark.as_slice());
        motion::control(cx, b);
        if let Some(mark) = mark {
            pop_mark(cx, mark);
        }
        if let Some(width) = self.leading {
            b = cx.container(
                LayoutStyle { width: Some(width), ..LayoutStyle::stack(Axis::Horizontal) },
                &[b],
                Paint::None,
            );
        }
        let label =
            util::label(cx, &self.label, TextStyle::Body, None, state.ink(theme.color(Semantic::Ink)));
        let node = row(cx, &[b, label]);
        cx.semantics(node, state.semantics(Semantics::new(Role::CheckBox).toggled(checked)));
        if state.accepts_input() {
            let value = self.value.clone();
            make_interactive(cx, node, 6.0, Rc::new(move || value.update(|v| *v = !*v)));
        } else if !state.disabled {
            cx.focusable(node, 6.0);
        }
        node
    }
}

/// One choice among several, shown as radio buttons. Arrow keys move the
/// selection and the focus together (wrapping); Home and End jump to the ends.
#[derive(Debug)]
pub struct RadioGroup {
    selected: Binding<usize>,
    options: Vec<String>,
    label: Option<String>,
}

impl RadioGroup {
    /// Radio buttons for `options`; `selected` holds the zero-based index.
    pub fn new(selected: Binding<usize>, options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { selected, options: options.into_iter().map(Into::into).collect(), label: None }
    }

    /// The name screen readers announce for the group, such as "Plan".
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

struct RadioOption {
    choice: ChoiceOption,
    label: String,
}

impl View for RadioOption {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let on = self.choice.selected.get() == self.choice.index();
        let hovered = cx.is_hovered();
        let dot = if on {
            vec![cx.container(
                LayoutStyle { width: Some(8.0), height: Some(8.0), ..LayoutStyle::leaf() },
                &[],
                Paint::Quad(QuadPaint {
                    corner_radius: 4.0,
                    ..QuadPaint::new(theme.color(Semantic::OnAccent))
                }),
            )]
        } else {
            vec![]
        };
        let edge = if hovered { theme.color(Semantic::Ink) } else { theme.color(Semantic::InkMuted) };
        let paint = if on {
            QuadPaint { corner_radius: 9.0, ..QuadPaint::new(theme.color(Semantic::Accent)) }
        } else {
            QuadPaint {
                corner_radius: 9.0,
                border: Some(Border { width: 1.5, color: edge.with_alpha(0.7) }),
                ..QuadPaint::new(Color::TRANSPARENT)
            }
        };
        let circle = cx.container(
            LayoutStyle {
                width: Some(18.0),
                height: Some(18.0),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &dot,
            Paint::Quad(paint),
        );
        motion::control(cx, circle);
        if let Some(&dot) = dot.first() {
            pop_mark(cx, dot);
        }
        let label = util::label(cx, &self.label, TextStyle::Body, None, theme.color(Semantic::Ink));
        let node = row(cx, &[circle, label]);
        cx.semantics(node, Semantics::new(Role::RadioButton).selected(on).toggled(on));
        self.choice.wire(cx, node, 6.0);
        node
    }
}

impl View for RadioGroup {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let focus_to = util::roving_cell(cx, None);
        let count = self.options.len();
        let options: Vec<NodeId> = self
            .options
            .iter()
            .enumerate()
            .map(|(index, label)| {
                let roving = util::Roving { index, count, focus_to: focus_to.clone() };
                let choice = ChoiceOption { roving, selected: self.selected.clone() };
                cx.child(&RadioOption { choice, label: label.clone() })
            })
            .collect();
        let node = cx.container(
            LayoutStyle { spacing: 10.0, ..LayoutStyle::stack(Axis::Vertical) },
            &options,
            Paint::None,
        );
        cx.semantics(node, group_semantics(self.label.as_deref()));
        node
    }
}

fn group_semantics(label: Option<&str>) -> Semantics {
    let semantics = Semantics::new(Role::RadioGroup);
    match label {
        Some(l) => semantics.name(l),
        None => semantics,
    }
}

/// A draggable value between `min` and `max`. Arrow keys adjust it by `step`.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Slider, Ui};
///
/// let volume = State::new(40.0_f32);
/// let slider = Slider::new(volume.binding()).range(0.0, 100.0).step(5.0).width(240.0).show_value();
/// let mut ui = Ui::new(slider, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct Slider {
    value: Binding<f32>,
    min: f32,
    max: f32,
    step: f32,
    width: Option<f32>,
    label: Option<String>,
    marks: Vec<f32>,
    track_colors: Vec<Color>,
    neutral: Option<f32>,
    show_value: bool,
    format: Option<Rc<dyn Fn(f32) -> String>>,
}

impl std::fmt::Debug for Slider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slider").field("min", &self.min).field("max", &self.max).finish_non_exhaustive()
    }
}

impl Slider {
    /// A slider over 0.0–1.0 bound to `value`, stepping by 0.01.
    pub fn new(value: Binding<f32>) -> Self {
        Self {
            value,
            min: 0.0,
            max: 1.0,
            step: 0.01,
            width: None,
            label: None,
            marks: Vec::new(),
            track_colors: Vec::new(),
            neutral: None,
            show_value: false,
            format: None,
        }
    }

    /// The value range; also resets the step to 1/100 of the range.
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max.max(min + f32::EPSILON);
        self.step = (self.max - self.min) / 100.0;
        self
    }

    /// The keyboard increment (call after [`Slider::range`]).
    pub fn step(mut self, step: f32) -> Self {
        self.step = step.max(f32::EPSILON);
        self
    }

    /// Fixed width in logical pixels; by default the slider stretches.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// The name screen readers announce, such as "Volume".
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Notches on the track at these values, such as where chapters begin.
    pub fn marks(mut self, marks: impl IntoIterator<Item = f32>) -> Self {
        self.marks = marks.into_iter().collect();
        self
    }

    /// Paints the track with these colors, evenly spaced from one end to the other, when
    /// the colors are what the value means, as from cold to warm. There is then no fill.
    /// The colors are data, so they come from whoever knows what they mean.
    pub fn track_colors(mut self, colors: impl IntoIterator<Item = Color>) -> Self {
        self.track_colors = colors.into_iter().collect();
        self
    }

    /// The value at rest, such as zero in the middle of the range. The fill starts there
    /// instead of at the minimum, and a double click, Backspace or Delete return to it.
    pub fn neutral(mut self, neutral: f32) -> Self {
        self.neutral = Some(neutral);
        self
    }

    /// Shows the value at the right of the track, with as many decimals as the step has.
    pub fn show_value(mut self) -> Self {
        self.show_value = true;
        self
    }

    /// Shows the value at the right of the track as this function writes it, which screen
    /// readers read too.
    pub fn value_format(mut self, format: impl Fn(f32) -> String + 'static) -> Self {
        self.show_value = true;
        self.format = Some(Rc::new(format));
        self
    }

    fn value_text(&self, value: f32) -> String {
        match &self.format {
            Some(format) => format(value),
            None => {
                let decimals = step_decimals(self.step);
                format!("{value:.decimals$}")
            }
        }
    }
}

/// As many decimals as a value moving by `step` needs.
pub(crate) fn step_decimals(step: f32) -> usize {
    // Less a hair, since the logarithm of 0.1 can land just above 1.
    (-step.log10() - 1e-3).ceil().clamp(0.0, 4.0) as usize
}

/// Draws a track through `colors`. Gradients have two stops and square ends: one for each
/// pair of colors, between round caps in the end colors.
fn paint_color_track(scene: &mut Scene, track: Rect, colors: &[Color]) {
    let (Some(first), Some(last)) = (colors.first(), colors.last()) else { return };
    let radius = track.height() / 2.0;
    let cap = track.height();
    scene.push_quad(Quad::new(Rect::new(track.x(), track.y(), cap, cap), *first).radius(radius));
    scene.push_quad(Quad::new(Rect::new(track.max_x() - cap, track.y(), cap, cap), *last).radius(radius));
    let inner = (track.width() - cap).max(0.0);
    let bands = colors.len().saturating_sub(1);
    if bands == 0 {
        scene.push_quad(Quad::new(Rect::new(track.x() + radius, track.y(), inner, cap), *first));
        return;
    }
    let band = inner / bands as f32;
    for (i, pair) in colors.windows(2).enumerate() {
        // A little overlap, so that no seam shows between bands on fractional pixels.
        let overlap = if i + 1 < bands { 0.5 } else { 0.0 };
        let rect = Rect::new(track.x() + radius + band * i as f32, track.y(), band + overlap, track.height());
        let gradient = LinearGradient {
            start: Point::new(0.0, 0.5),
            end: Point::new(1.0, 0.5),
            from: pair[0],
            to: pair[1],
        };
        scene.push_quad(Quad::new(rect, gradient));
    }
}

/// The part of a [`Slider`] that is dragged. Its own view when the value sits beside it, so
/// that hover and focus are about the track.
struct SliderTrack<'a> {
    slider: &'a Slider,
    /// Takes what the row leaves it, instead of being the whole slider.
    in_row: bool,
}

impl View for SliderTrack<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let slider = self.slider;
        let theme = *cx.theme();
        let (min, max, step) = (slider.min, slider.max, slider.step);
        let value = slider.value.get().clamp(min, max);
        let t = (value - min) / (max - min);
        let rest = slider.neutral.map_or(0.0, |n| ((n - min) / (max - min)).clamp(0.0, 1.0));
        let (track, fill) = (theme.color(Semantic::Control), theme.color(Semantic::Accent));
        let ring = theme.color(Semantic::Accent).with_alpha(if cx.is_pressed() || cx.is_hovered() {
            0.25
        } else {
            0.0
        });
        let style = LayoutStyle {
            width: if self.in_row { None } else { slider.width },
            height: Some(24.0),
            min_width: Some(120.0),
            grow: if self.in_row { 1.0 } else { 0.0 },
            align_self: Some(if slider.width.is_some() && !self.in_row {
                Align::Start
            } else {
                Align::Stretch
            }),
            ..LayoutStyle::leaf()
        };
        let notch = theme.color(Semantic::InkMuted);
        let marks: Vec<f32> =
            slider.marks.iter().map(|m| ((m - min) / (max - min)).clamp(0.0, 1.0)).collect();
        let colors = slider.track_colors.clone();
        let node = cx.container(
            style,
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                let usable = f.width() - 18.0;
                let cy = f.center().y;
                let bar = Rect::new(f.x() + 9.0, cy - 2.0, usable, 4.0);
                if colors.is_empty() {
                    scene.push_quad(Quad::new(bar, track).radius(2.0));
                    let (from, to) = (rest.min(t), rest.max(t));
                    scene.push_quad(
                        Quad::new(
                            Rect::new(bar.x() + usable * from, bar.y(), usable * (to - from), 4.0),
                            fill,
                        )
                        .radius(2.0),
                    );
                } else {
                    paint_color_track(scene, bar, &colors);
                }
                for mark in &marks {
                    let x = f.x() + 9.0 + usable * mark;
                    scene.push_quad(Quad::new(Rect::new(x - 1.0, cy - 5.0, 2.0, 10.0), notch).radius(1.0));
                }
                let cx_ = f.x() + 9.0 + usable * t;
                if ring.a > 0.0 {
                    scene.push_quad(
                        Quad::new(Rect::new(cx_ - 14.0, cy - 14.0, 28.0, 28.0), ring).radius(14.0),
                    );
                }
                scene.push_quad(
                    Quad::new(Rect::new(cx_ - 9.0, cy - 9.0, 18.0, 18.0), Color::WHITE).radius(9.0).shadow(
                        Shadow {
                            offset: Point::new(0.0, 1.0),
                            blur: 4.0,
                            color: Color::rgba(0.0, 0.0, 0.0, 0.35),
                        },
                    ),
                );
            })),
        );
        let mut semantics =
            Semantics::new(Role::Slider).numeric(value as f64, min as f64, max as f64, Some(step as f64));
        if let Some(label) = &slider.label {
            semantics = semantics.name(label.clone());
        }
        if slider.show_value {
            semantics = semantics.value(slider.value_text(value));
        }
        cx.semantics(node, semantics);
        let snap = move |v: f32| ((v - min) / step).round() * step + min;
        let drag_value = slider.value.clone();
        cx.on_drag(
            node,
            Rc::new(move |d| {
                if d.phase == DragPhase::End {
                    return;
                }
                let usable = (d.frame.width() - 18.0).max(1.0);
                let frac = ((d.position.x - d.frame.x() - 9.0) / usable).clamp(0.0, 1.0);
                drag_value.set(snap(min + frac * (max - min)).clamp(min, max));
            }),
        );
        let neutral = slider.neutral.map(|n| n.clamp(min, max));
        if let Some(neutral) = neutral {
            let reset = slider.value.clone();
            cx.on_double_click(node, Rc::new(move || reset.set(neutral)));
        }
        let key_value = slider.value.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                if let (Key::Backspace | Key::Delete, Some(neutral)) = (key, neutral) {
                    key_value.set(neutral);
                    return true;
                }
                let delta = match key {
                    Key::ArrowLeft | Key::ArrowDown => -step,
                    Key::ArrowRight | Key::ArrowUp => step,
                    Key::Home => -(max - min),
                    Key::End => max - min,
                    _ => return false,
                };
                key_value.update(|v| *v = snap(*v + delta).clamp(min, max));
                true
            }),
        );
        cx.focusable(node, 12.0);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

impl View for Slider {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        if !self.show_value {
            return SliderTrack { slider: self, in_row: false }.build(cx);
        }
        let theme = *cx.theme();
        let track = cx.child(&SliderTrack { slider: self, in_row: true });
        let value = self.value.get().clamp(self.min, self.max);
        let text = util::label(
            cx,
            &self.value_text(value),
            TextStyle::Callout,
            None,
            theme.color(Semantic::InkMuted),
        );
        // Part of the track's semantics, as its value.
        cx.semantics(text, Semantics::hidden());
        // As wide as the widest value, so that the track does not move while it is dragged.
        let attrs = util::attrs(&theme, TextStyle::Callout, None);
        let widest = cx
            .measure_text(&self.value_text(self.min), &attrs)
            .width
            .max(cx.measure_text(&self.value_text(self.max), &attrs).width);
        let number = cx.container(
            LayoutStyle {
                min_width: Some(widest.ceil()),
                justify: Justify::End,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[text],
            Paint::None,
        );
        cx.container(
            LayoutStyle {
                width: self.width,
                spacing: theme.spacing.s2,
                align: Align::Center,
                align_self: Some(if self.width.is_some() { Align::Start } else { Align::Stretch }),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[track, number],
            Paint::None,
        )
    }
}

/// Decrement / value / increment, for small integer quantities. When focused,
/// arrow keys step by one and Home / End jump to the limits.
#[derive(Debug)]
pub struct Stepper {
    value: Binding<i32>,
    min: i32,
    max: i32,
    label: Option<String>,
    state: util::EditorState,
}

impl Stepper {
    /// A stepper bound to `value`, unbounded.
    pub fn new(value: Binding<i32>) -> Self {
        Self { value, min: i32::MIN, max: i32::MAX, label: None, state: util::EditorState::default() }
    }

    /// Clamps the value to `min..=max`.
    pub fn range(mut self, min: i32, max: i32) -> Self {
        self.min = min;
        self.max = max.max(min);
        self
    }

    /// The name screen readers announce, such as "Seats".
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Turns the control off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the value without letting it change; it can still be focused and read.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

impl View for Stepper {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let v = self.value.get();
        let (min, max, state) = (self.min, self.max, self.state);
        let dec = self.value.clone();
        let inc = self.value.clone();
        let minus = cx.child(
            &crate::IconButton::new(IconName::Minus)
                .label(cx.labels().decrease.clone())
                .size(crate::ButtonSize::Small)
                .disabled(!state.accepts_input())
                .on_click(move || dec.update(|v| *v = (*v - 1).max(min))),
        );
        let mut attrs = util::attrs(&theme, TextStyle::Body, Some(Weight::SEMIBOLD));
        attrs.size = 15.0;
        let text =
            cx.text(&v.to_string(), &attrs, state.ink(theme.color(Semantic::Ink)), LayoutStyle::leaf());
        let value = cx.container(
            LayoutStyle {
                min_width: Some(36.0),
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[text],
            Paint::None,
        );
        // Read through the spin button's value instead.
        cx.semantics(value, Semantics::hidden());
        let plus = cx.child(
            &crate::IconButton::new(IconName::Plus)
                .label(cx.labels().increase.clone())
                .size(crate::ButtonSize::Small)
                .disabled(!state.accepts_input())
                .on_click(move || inc.update(|v| *v = (*v + 1).min(max))),
        );
        let style = LayoutStyle {
            padding: Insets::uniform(3.0),
            spacing: 2.0,
            align: Align::Center,
            align_self: Some(Align::Fit),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = QuadPaint {
            corner_radius: theme.radii.md,
            border: Some(Border { width: 1.0, color: state.ink(theme.color(Semantic::Border)) }),
            ..QuadPaint::new(state.fill(theme.color(Semantic::Control)))
        };
        let node = cx.container(style, &[minus, value, plus], Paint::Quad(paint));
        let mut semantics =
            Semantics::new(Role::SpinButton).numeric(v as f64, min as f64, max as f64, Some(1.0));
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, state.semantics(semantics));
        if state.disabled {
            return node;
        }
        let value = self.value.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                if !state.accepts_input() {
                    return false;
                }
                let next = |v: i32| match key {
                    Key::ArrowUp | Key::ArrowRight => Some(v.saturating_add(1)),
                    Key::ArrowDown | Key::ArrowLeft => Some(v.saturating_sub(1)),
                    Key::Home => Some(min),
                    Key::End => Some(max),
                    _ => None,
                };
                let Some(v) = next(value.get()) else { return false };
                value.set(v.clamp(min, max));
                true
            }),
        );
        cx.focusable(node, theme.radii.md);
        node
    }
}

/// A row of mutually exclusive segments, like a compact tab bar. Arrow keys
/// move the selection and the focus together (wrapping); Home and End jump to the ends.
#[derive(Debug)]
pub struct SegmentedControl {
    selected: Binding<usize>,
    options: Vec<(Option<IconName>, String)>,
    label: Option<String>,
}

impl SegmentedControl {
    /// Segments for `options`; `selected` holds the zero-based index.
    pub fn new(selected: Binding<usize>, options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { selected, options: options.into_iter().map(|o| (None, o.into())).collect(), label: None }
    }

    /// Segments with an icon before each label.
    pub fn with_icons(
        selected: Binding<usize>,
        options: impl IntoIterator<Item = (IconName, impl Into<String>)>,
    ) -> Self {
        Self {
            selected,
            options: options.into_iter().map(|(i, l)| (Some(i), l.into())).collect(),
            label: None,
        }
    }

    /// The name screen readers announce for the group, such as "Density".
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

struct Segment {
    scope: WidgetId,
    choice: ChoiceOption,
    icon: Option<IconName>,
    label: String,
}

impl View for Segment {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let on = self.choice.selected.get() == self.choice.index();
        let hovered = cx.is_hovered();
        let color = if on || hovered { theme.color(Semantic::Ink) } else { theme.color(Semantic::InkMuted) };
        let mut children = Vec::new();
        if let Some(icon) = self.icon {
            children.push(util::icon(cx, icon, 14.0, color));
        }
        let label = util::label(cx, &self.label, TextStyle::Callout, Some(Weight::SEMIBOLD), color);
        motion::control(cx, label);
        children.push(label);
        let style = LayoutStyle {
            padding: Insets::symmetric(0.0, 14.0),
            height: Some(30.0),
            spacing: 6.0,
            align: Align::Center,
            justify: Justify::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = if on {
            Paint::Quad(QuadPaint {
                corner_radius: 7.0,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                shadow: Some(Shadow {
                    offset: Point::new(0.0, 1.0),
                    blur: 4.0,
                    color: Color::rgba(0.0, 0.0, 0.0, 0.25),
                }),
                ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
            })
        } else {
            Paint::None
        };
        let node = cx.container(style, &children, paint);
        if on {
            // The selected pill is one object that slides under the labels.
            cx.motion(
                node,
                Motion::new().shared_layout(self.scope, 0, theme.motion.snappy).shape_only().underlay(),
            );
        }
        cx.semantics(node, Semantics::new(Role::RadioButton).selected(on).toggled(on));
        self.choice.wire(cx, node, 7.0);
        node
    }
}

impl View for SegmentedControl {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let focus_to = util::roving_cell(cx, None);
        let count = self.options.len();
        let scope = cx.widget_id();
        let segments: Vec<NodeId> = self
            .options
            .iter()
            .enumerate()
            .map(|(index, (icon, label))| {
                let roving = util::Roving { index, count, focus_to: focus_to.clone() };
                let choice = ChoiceOption { roving, selected: self.selected.clone() };
                cx.child(&Segment { scope, choice, icon: *icon, label: label.clone() })
            })
            .collect();
        let style = LayoutStyle {
            padding: Insets::uniform(3.0),
            spacing: 2.0,
            align_self: Some(Align::Fit),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint =
            QuadPaint { corner_radius: theme.radii.md, ..QuadPaint::new(theme.color(Semantic::Control)) };
        let node = cx.container(style, &segments, Paint::Quad(paint));
        cx.semantics(node, group_semantics(self.label.as_deref()));
        node
    }
}

/// Switches the app between its dark, light and glass themes: Raven Dark, Raven Light and
/// Raven Glass, unless the app has its own.
#[derive(Clone)]
pub struct ThemeSwitcher {
    themes: [Theme; 3],
    on_change: Option<Rc<dyn Fn(Theme)>>,
}

impl std::fmt::Debug for ThemeSwitcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThemeSwitcher")
            .field("themes", &self.themes.map(|theme| theme.name))
            .field("on_change", &self.on_change.is_some())
            .finish()
    }
}

impl Default for ThemeSwitcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeSwitcher {
    /// A switcher over the themes that come with RavenRS.
    pub fn new() -> Self {
        Self { themes: [Theme::raven_dark(), Theme::raven_light(), Theme::raven_glass()], on_change: None }
    }

    /// Offers the application's own themes in the same three places. Each is told apart by
    /// its name, so it needs one of its own.
    pub fn themes(mut self, dark: Theme, light: Theme, glass: Theme) -> Self {
        self.themes = [dark, light, glass];
        self
    }

    /// Runs `action` with the theme the user picks, after the app switches to it, so the
    /// app can remember the choice for the next launch.
    pub fn on_change(mut self, action: impl Fn(Theme) + 'static) -> Self {
        self.on_change = Some(Rc::new(action));
        self
    }
}

impl View for ThemeSwitcher {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let themes = self.themes;
        let on_change = self.on_change.clone();
        let labels = cx.labels();
        let state = cx.theme_state();
        let read = state.clone();
        let binding = Binding::new(
            move || read.with(|t| themes.iter().position(|x| x.name == t.name).unwrap_or(0)),
            move |i: usize| {
                let theme = themes[i.min(themes.len() - 1)];
                state.set(theme);
                if let Some(action) = &on_change {
                    action(theme);
                }
            },
        );
        cx.child(
            &SegmentedControl::with_icons(
                binding,
                [
                    (IconName::Moon, labels.theme_dark.as_str()),
                    (IconName::Sun, labels.theme_light.as_str()),
                    (IconName::Layers, labels.theme_glass.as_str()),
                ],
            )
            .label(labels.theme.clone()),
        )
    }
}
