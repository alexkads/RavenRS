//! Form controls bound to state through [`Binding`].

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Key, Point, Quad, Rect, Shadow};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use crate::context::{BuildCx, DragPhase, NodeId, Paint, QuadPaint};
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

/// Option `index` of `count` in a single-choice group (radio buttons, segments).
/// Arrow keys select and focus the neighbor, wrapping around; Home and End
/// jump to the ends, following the WAI-ARIA radio group pattern.
struct ChoiceOption {
    index: usize,
    count: usize,
    selected: Binding<usize>,
    /// The option that takes focus on its next build, after an arrow key.
    focus_to: Rc<Cell<Option<usize>>>,
}

impl ChoiceOption {
    fn wire(&self, cx: &mut BuildCx<'_>, node: NodeId, radius: f32) {
        if self.focus_to.get() == Some(self.index) {
            self.focus_to.set(None);
            cx.request_focus();
        }
        let (selected, index, count) = (self.selected.clone(), self.index, self.count);
        make_interactive(cx, node, radius, Rc::new(move || selected.set(index)));
        let (selected, focus_to) = (self.selected.clone(), self.focus_to.clone());
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                let target = match key {
                    Key::ArrowRight | Key::ArrowDown => (index + 1) % count,
                    Key::ArrowLeft | Key::ArrowUp => (index + count - 1) % count,
                    Key::Home => 0,
                    Key::End => count - 1,
                    _ => return false,
                };
                focus_to.set(Some(target));
                selected.set(target);
                true
            }),
        );
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
}

impl Checkbox {
    /// A check box bound to `value`, with this label.
    pub fn new(label: impl Into<String>, value: Binding<bool>) -> Self {
        Self { value, label: label.into() }
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
        let b = check_box(cx, &theme, self.value.get(), cx.is_hovered());
        let label = util::label(cx, &self.label, TextStyle::Body, None, theme.color(Semantic::Ink));
        let node = row(cx, &[b, label]);
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
}

impl RadioGroup {
    /// Radio buttons for `options`; `selected` holds the zero-based index.
    pub fn new(selected: Binding<usize>, options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { selected, options: options.into_iter().map(Into::into).collect() }
    }
}

struct RadioOption {
    choice: ChoiceOption,
    label: String,
}

impl View for RadioOption {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let on = self.choice.selected.get() == self.choice.index;
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
        self.choice.wire(cx, node, 6.0);
        node
    }
}

impl View for RadioGroup {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let focus_to = cx.use_state(|| Rc::new(Cell::new(None))).get();
        let count = self.options.len();
        let options: Vec<NodeId> = self
            .options
            .iter()
            .enumerate()
            .map(|(index, label)| {
                let choice = ChoiceOption {
                    index,
                    count,
                    selected: self.selected.clone(),
                    focus_to: focus_to.clone(),
                };
                cx.child(&RadioOption { choice, label: label.clone() })
            })
            .collect();
        cx.container(
            LayoutStyle { spacing: 10.0, ..LayoutStyle::stack(Axis::Vertical) },
            &options,
            Paint::None,
        )
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
}

impl Slider {
    /// A slider over 0.0–1.0 bound to `value`, stepping by 0.01.
    pub fn new(value: Binding<f32>) -> Self {
        Self { value, min: 0.0, max: 1.0, step: 0.01, width: None }
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
}

impl Stepper {
    /// A stepper bound to `value`, unbounded.
    pub fn new(value: Binding<i32>) -> Self {
        Self { value, min: i32::MIN, max: i32::MAX }
    }

    /// Clamps the value to `min..=max`.
    pub fn range(mut self, min: i32, max: i32) -> Self {
        self.min = min;
        self.max = max.max(min);
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
        let plus = cx.child(
            &crate::IconButton::new(IconName::Plus)
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
            ..QuadPaint::new(theme.color(Semantic::Surface))
        };
        let node = cx.container(style, &[minus, value, plus], Paint::Quad(paint));
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
}

impl SegmentedControl {
    /// Segments for `options`; `selected` holds the zero-based index.
    pub fn new(selected: Binding<usize>, options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { selected, options: options.into_iter().map(|o| (None, o.into())).collect() }
    }

    /// Segments with an icon before each label.
    pub fn with_icons(
        selected: Binding<usize>,
        options: impl IntoIterator<Item = (IconName, &'static str)>,
    ) -> Self {
        Self { selected, options: options.into_iter().map(|(i, l)| (Some(i), l.to_owned())).collect() }
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
        let on = self.choice.selected.get() == self.choice.index;
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
        self.choice.wire(cx, node, 7.0);
        node
    }
}

impl View for SegmentedControl {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let focus_to = cx.use_state(|| Rc::new(Cell::new(None))).get();
        let count = self.options.len();
        let segments: Vec<NodeId> = self
            .options
            .iter()
            .enumerate()
            .map(|(index, (icon, label))| {
                let choice = ChoiceOption {
                    index,
                    count,
                    selected: self.selected.clone(),
                    focus_to: focus_to.clone(),
                };
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
        cx.container(style, &segments, Paint::Quad(paint))
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
        cx.child(&SegmentedControl::with_icons(
            binding,
            [(IconName::Moon, "Dark"), (IconName::Sun, "Light"), (IconName::Layers, "Glass")],
        ))
    }
}
