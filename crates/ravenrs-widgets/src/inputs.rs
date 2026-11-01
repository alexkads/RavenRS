//! Form controls bound to state through [`Binding`].

use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Key, Point, Quad, Rect, Role, Shadow};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use crate::context::{BuildCx, DragPhase, NodeId, Paint, QuadPaint, Semantics};
use crate::{IconName, View, util};

fn row(cx: &mut BuildCx<'_>, children: &[NodeId]) -> NodeId {
    let style = LayoutStyle {
        spacing: 10.0,
        align: Align::Center,
        align_self: Some(Align::Start),
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
}

impl Toggle {
    /// A switch bound to `value`.
    pub fn new(value: Binding<bool>) -> Self {
        Self { value, label: None }
    }

    /// Text after the switch; clicking it toggles too.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl View for Toggle {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let on = self.value.get();
        let hovered = cx.is_hovered();
        let track = if on {
            theme.color(if hovered { Semantic::AccentHover } else { Semantic::Accent })
        } else {
            theme.color(Semantic::Control).lerp(theme.color(Semantic::Ink), if hovered { 0.08 } else { 0.0 })
        };
        let border = theme.color(Semantic::Border);
        let switch = cx.container(
            LayoutStyle { width: Some(40.0), height: Some(24.0), ..LayoutStyle::leaf() },
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                let mut q = Quad::new(f, track).radius(12.0);
                if !on {
                    q = q.border(1.0, border);
                }
                scene.push_quad(q);
                let x = if on { f.max_x() - 21.0 } else { f.x() + 3.0 };
                let thumb = Rect::new(x, f.y() + 3.0, 18.0, 18.0);
                scene.push_quad(Quad::new(thumb, Color::WHITE).radius(9.0).shadow(Shadow {
                    offset: Point::new(0.0, 1.0),
                    blur: 3.0,
                    color: Color::rgba(0.0, 0.0, 0.0, 0.3),
                }));
            })),
        );
        let mut children = vec![switch];
        if let Some(label) = &self.label {
            children.push(util::label(cx, label, TextStyle::Body, None, theme.color(Semantic::Ink)));
        }
        let node = row(cx, &children);
        cx.semantics(node, Semantics::new(Role::Switch).toggled(on));
        let value = self.value.clone();
        make_interactive(cx, node, 8.0, Rc::new(move || value.update(|v| *v = !*v)));
        node
    }
}

/// A check box with a label.
#[derive(Debug)]
pub struct Checkbox {
    value: Binding<bool>,
    label: String,
    leading: Option<f32>,
}

impl Checkbox {
    /// A check box bound to `value`, with this label.
    pub fn new(label: impl Into<String>, value: Binding<bool>) -> Self {
        Self { value, label: label.into(), leading: None }
    }

    /// Reserves this width for the box, so labels line up in a column. A list
    /// that mixes check boxes with [`Toggle`]s passes the toggle's width, 40.
    pub fn leading(mut self, width: f32) -> Self {
        self.leading = Some(width);
        self
    }
}

pub(crate) fn check_box(cx: &mut BuildCx<'_>, theme: &Theme, checked: bool, hovered: bool) -> NodeId {
    let mark = if checked {
        vec![util::icon(cx, IconName::Check, 14.0, theme.color(Semantic::OnAccent))]
    } else {
        vec![]
    };
    let paint = if checked {
        QuadPaint { corner_radius: 5.0, ..QuadPaint::new(theme.color(Semantic::Accent)) }
    } else {
        let edge = if hovered { theme.color(Semantic::Ink) } else { theme.color(Semantic::InkMuted) };
        QuadPaint {
            corner_radius: 5.0,
            border: Some(Border { width: 1.5, color: edge.with_alpha(0.7) }),
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
    cx.container(style, &mark, Paint::Quad(paint))
}

impl View for Checkbox {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let checked = self.value.get();
        let b = check_box(cx, &theme, checked, cx.is_hovered());
        let label = util::label(cx, &self.label, TextStyle::Body, None, theme.color(Semantic::Ink));
        let node = row(cx, &[b, label]);
        cx.semantics(node, Semantics::new(Role::CheckBox).toggled(checked));
        let value = self.value.clone();
        make_interactive(cx, node, 6.0, Rc::new(move || value.update(|v| *v = !*v)));
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
/// let slider = Slider::new(volume.binding()).range(0.0, 100.0).step(5.0).width(240.0);
/// let mut ui = Ui::new(slider, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[derive(Debug)]
pub struct Slider {
    value: Binding<f32>,
    min: f32,
    max: f32,
    step: f32,
    width: Option<f32>,
    label: Option<String>,
}

impl Slider {
    /// A slider over 0.0–1.0 bound to `value`, stepping by 0.01.
    pub fn new(value: Binding<f32>) -> Self {
        Self { value, min: 0.0, max: 1.0, step: 0.01, width: None, label: None }
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
}

impl View for Slider {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (min, max, step) = (self.min, self.max, self.step);
        let value = self.value.get().clamp(min, max);
        let t = (value - min) / (max - min);
        let (track, fill) = (theme.color(Semantic::Control), theme.color(Semantic::Accent));
        let ring = theme.color(Semantic::Accent).with_alpha(if cx.is_pressed() || cx.is_hovered() {
            0.25
        } else {
            0.0
        });
        let style = LayoutStyle {
            width: self.width,
            height: Some(24.0),
            min_width: Some(120.0),
            align_self: Some(if self.width.is_some() { Align::Start } else { Align::Stretch }),
            ..LayoutStyle::leaf()
        };
        let node = cx.container(
            style,
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                let usable = f.width() - 18.0;
                let cy = f.center().y;
                scene.push_quad(Quad::new(Rect::new(f.x() + 9.0, cy - 2.0, usable, 4.0), track).radius(2.0));
                scene.push_quad(
                    Quad::new(Rect::new(f.x() + 9.0, cy - 2.0, usable * t, 4.0), fill).radius(2.0),
                );
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
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, semantics);
        let snap = move |v: f32| ((v - min) / step).round() * step + min;
        let drag_value = self.value.clone();
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
        let key_value = self.value.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
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

/// Decrement / value / increment, for small integer quantities. When focused,
/// arrow keys step by one and Home / End jump to the limits.
#[derive(Debug)]
pub struct Stepper {
    value: Binding<i32>,
    min: i32,
    max: i32,
    label: Option<String>,
}

impl Stepper {
    /// A stepper bound to `value`, unbounded.
    pub fn new(value: Binding<i32>) -> Self {
        Self { value, min: i32::MIN, max: i32::MAX, label: None }
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
}

impl View for Stepper {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let v = self.value.get();
        let (min, max) = (self.min, self.max);
        let dec = self.value.clone();
        let inc = self.value.clone();
        let minus = cx.child(
            &crate::IconButton::new(IconName::Minus)
                .label("Decrease")
                .size(crate::ButtonSize::Small)
                .on_click(move || dec.update(|v| *v = (*v - 1).max(min))),
        );
        let mut attrs = util::attrs(&theme, TextStyle::Body, Some(Weight::SEMIBOLD));
        attrs.size = 15.0;
        let text = cx.text(&v.to_string(), &attrs, theme.color(Semantic::Ink), LayoutStyle::leaf());
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
                .label("Increase")
                .size(crate::ButtonSize::Small)
                .on_click(move || inc.update(|v| *v = (*v + 1).min(max))),
        );
        let style = LayoutStyle {
            padding: Insets::uniform(3.0),
            spacing: 2.0,
            align: Align::Center,
            align_self: Some(Align::Start),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = QuadPaint {
            corner_radius: theme.radii.md,
            border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
            ..QuadPaint::new(theme.color(Semantic::Control))
        };
        let node = cx.container(style, &[minus, value, plus], Paint::Quad(paint));
        let mut semantics =
            Semantics::new(Role::SpinButton).numeric(v as f64, min as f64, max as f64, Some(1.0));
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, semantics);
        let value = self.value.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
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
        options: impl IntoIterator<Item = (IconName, &'static str)>,
    ) -> Self {
        Self {
            selected,
            options: options.into_iter().map(|(i, l)| (Some(i), l.to_owned())).collect(),
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
        children.push(util::label(cx, &self.label, TextStyle::Callout, Some(Weight::SEMIBOLD), color));
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
        let segments: Vec<NodeId> = self
            .options
            .iter()
            .enumerate()
            .map(|(index, (icon, label))| {
                let roving = util::Roving { index, count, focus_to: focus_to.clone() };
                let choice = ChoiceOption { roving, selected: self.selected.clone() };
                cx.child(&Segment { choice, icon: *icon, label: label.clone() })
            })
            .collect();
        let style = LayoutStyle {
            padding: Insets::uniform(3.0),
            spacing: 2.0,
            align_self: Some(Align::Start),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint =
            QuadPaint { corner_radius: theme.radii.md, ..QuadPaint::new(theme.color(Semantic::Control)) };
        let node = cx.container(style, &segments, Paint::Quad(paint));
        cx.semantics(node, group_semantics(self.label.as_deref()));
        node
    }
}

/// Switches the app between Raven Dark, Raven Light and Raven Glass.
#[derive(Clone, Copy, Debug, Default)]
pub struct ThemeSwitcher;

impl View for ThemeSwitcher {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let themes = [Theme::raven_dark(), Theme::raven_light(), Theme::raven_glass()];
        let state = cx.theme_state();
        let read = state.clone();
        let binding = Binding::new(
            move || read.with(|t| themes.iter().position(|x| x.name == t.name).unwrap_or(0)),
            move |i: usize| state.set(themes[i.min(themes.len() - 1)]),
        );
        cx.child(
            &SegmentedControl::with_icons(
                binding,
                [(IconName::Moon, "Dark"), (IconName::Sun, "Light"), (IconName::Layers, "Glass")],
            )
            .label("Theme"),
        )
    }
}
