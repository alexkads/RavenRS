//! Controls that stay small until they are used: a value that opens a slider, and a name
//! that turns into a field.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{CursorIcon, Insets, Key, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::editors::parse_number;
use crate::{Button, ButtonSize, ButtonStyle, IconButton, IconName, Popover, Slider, TextField, View, util};

/// The width of the slider a [`ValueSlider`] opens, in logical pixels.
const SLIDER_WIDTH: f32 = 200.0;
/// The width of the number field of [`ValueSlider::field`], in logical pixels.
const FIELD_WIDTH: f32 = 84.0;

/// A value shown small, as "100% ▾", that opens a [`Slider`] in a popover, for bars with no
/// room for a slider of their own.
///
/// Enter or Space on the button opens it with the keyboard in the slider: the arrows, Home
/// and End change the value, and Escape closes it and goes back to the button. With
/// [`ValueSlider::field`] the number can also be typed.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Ui, ValueSlider};
///
/// let opacity = State::new(100.0_f32);
/// let control = ValueSlider::new(opacity.binding()).range(0.0, 100.0).step(1.0).unit("%").label("Opacity");
/// let mut ui = Ui::new(control, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ValueSlider {
    value: Binding<f32>,
    min: f32,
    max: f32,
    step: f32,
    unit: String,
    label: Option<String>,
    show_label: bool,
    format: Option<Rc<dyn Fn(f32) -> String>>,
    neutral: Option<f32>,
    slider_width: f32,
    field: bool,
    disabled: bool,
}

impl fmt::Debug for ValueSlider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValueSlider").field("min", &self.min).field("max", &self.max).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ValueSlider {
    /// A control over 0.0–1.0 bound to `value`, stepping by 0.01.
    pub fn new(value: Binding<f32>) -> Self {
        Self {
            value,
            min: 0.0,
            max: 1.0,
            step: 0.01,
            unit: String::new(),
            label: None,
            show_label: false,
            format: None,
            neutral: None,
            slider_width: SLIDER_WIDTH,
            field: false,
            disabled: false,
        }
    }

    /// The value range; also resets the step to 1/100 of the range.
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max.max(min + f32::EPSILON);
        self.step = (self.max - self.min) / 100.0;
        self
    }

    /// The keyboard increment (call after [`ValueSlider::range`]).
    pub fn step(mut self, step: f32) -> Self {
        self.step = step.max(f32::EPSILON);
        self
    }

    /// Text right after the number, such as `%` or ` px`.
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }

    /// What the value is, such as "Opacity", for screen readers. See [`Self::show_label`].
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Also writes the [`label`](Self::label) before the value.
    pub fn show_label(mut self) -> Self {
        self.show_label = true;
        self
    }

    /// Writes the value as this function does, instead of the number and the unit.
    /// [`Self::field`] goes on showing the number, since that is what can be typed.
    pub fn value_format(mut self, format: impl Fn(f32) -> String + 'static) -> Self {
        self.format = Some(Rc::new(format));
        self
    }

    /// The value at rest: see [`Slider::neutral`].
    pub fn neutral(mut self, neutral: f32) -> Self {
        self.neutral = Some(neutral);
        self
    }

    /// The width of the slider in the popover, in logical pixels (200 by default).
    pub fn slider_width(mut self, width: f32) -> Self {
        self.slider_width = width;
        self
    }

    /// A number field with the unit and a small arrow that opens the slider, instead of a
    /// button. Enter or leaving the field takes what was typed, up to the limits; text that
    /// is not a number, or Escape, brings the value back. Up and Down take a step.
    pub fn field(mut self) -> Self {
        self.field = true;
        self
    }

    /// Turns it off: it fades, leaves the tab order and does not open.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn number(&self, value: f32, decimal: char) -> String {
        ravenrs_criteria::NumberStyle {
            decimals: crate::inputs::step_decimals(self.step),
            group: None,
            decimal,
            prefix: "",
            suffix: "",
        }
        .format(f64::from(value))
    }

    fn number_field(&self, cx: &mut BuildCx<'_>, open: &State<bool>, name: &str, value: f32) -> TextField {
        let labels = cx.labels();
        let (decimal, group) = (labels.decimal_separator, labels.group_separator);
        // What was typed; nothing while the field shows the value itself.
        let draft: State<Option<String>> = cx.use_state(|| None);
        let shown = self.number(value, decimal);
        let (read, write) = (draft.clone(), draft.clone());
        let text = Binding::new(
            move || read.get().unwrap_or_else(|| shown.clone()),
            move |typed: String| write.set(Some(typed)),
        );
        let (min, max, step) = (self.min, self.max, self.step);
        let snap = move |v: f32| (((v - min) / step).round() * step + min).clamp(min, max);
        let commit: Rc<dyn Fn()> = {
            let (draft, target) = (draft.clone(), self.value.clone());
            Rc::new(move || {
                let typed = draft.get();
                draft.set_if_changed(None);
                let number = typed.and_then(|text| parse_number(&text, decimal, group));
                if let Some(number) = number {
                    let next = snap(number as f32);
                    if next != target.get() {
                        target.set(next);
                    }
                }
            })
        };
        let (on_enter, on_leave) = (commit.clone(), commit);
        let (cancelled, stepped, cleared) = (draft.clone(), self.value.clone(), draft);
        let suffix = FieldSuffix { unit: self.unit.clone(), open: open.clone(), name: name.to_owned() };
        TextField::new(text)
            .size(ButtonSize::Small)
            .width(FIELD_WIDTH)
            .end_aligned()
            .placeholder(name)
            .suffix(suffix)
            .on_submit(move |_| on_enter())
            .on_leave(move || on_leave())
            .on_cancel(move || cancelled.set_if_changed(None))
            .intercept_keys(Rc::new(move |key, _| {
                let delta = match key {
                    Key::ArrowUp => step,
                    Key::ArrowDown => -step,
                    _ => return false,
                };
                cleared.set_if_changed(None);
                stepped.update(|v| *v = snap(*v + delta));
                true
            }))
    }
}

/// The unit and the arrow of a [`ValueSlider::field`].
struct FieldSuffix {
    unit: String,
    open: State<bool>,
    name: String,
}

impl View for FieldSuffix {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let mut children = Vec::with_capacity(2);
        let unit = self.unit.trim();
        if !unit.is_empty() {
            // As wide as it is: in a tight field, a text that gives way breaks into lines.
            let attrs = util::attrs(&theme, TextStyle::Callout, None);
            let width = cx.measure_text(unit, &attrs).width.ceil();
            let style = LayoutStyle { width: Some(width), ..LayoutStyle::leaf() };
            children.push(cx.text(unit, &attrs, theme.color(Semantic::InkMuted), style));
        }
        let toggle = self.open.clone();
        let arrow = cx.child(
            &IconButton::new(IconName::ChevronDown)
                .label(self.name.clone())
                .size(ButtonSize::Mini)
                .on_click(move || toggle.update(|open| *open = !*open)),
        );
        let is_open = self.open.get();
        cx.update_semantics(arrow, |s| s.expanded = Some(is_open));
        children.push(arrow);
        cx.container(
            LayoutStyle {
                spacing: theme.spacing.s1,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::None,
        )
    }
}

/// The content of the popover: the slider, which gets the keyboard when it opens.
struct SliderPanel(Slider);

impl View for SliderPanel {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let slider = cx.child(&self.0);
        let focused = cx.use_state(|| Rc::new(Cell::new(false))).get();
        if !focused.replace(true) {
            cx.request_focus_node(slider);
        }
        slider
    }
}

impl View for ValueSlider {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let open: State<bool> = cx.use_state(|| false);
        let value = self.value.get().clamp(self.min, self.max);
        let name = self.label.clone().unwrap_or_else(|| labels.value_adjust.clone());

        let mut slider = Slider::new(self.value.clone())
            .range(self.min, self.max)
            .step(self.step)
            .width(self.slider_width)
            .label(name.clone());
        if let Some(neutral) = self.neutral {
            slider = slider.neutral(neutral);
        }
        let panel = SliderPanel(slider);
        let shows = if self.disabled { Binding::constant(false) } else { open.binding() };
        let anchor = if self.field {
            let field = self.number_field(cx, &open, &name, value).disabled(self.disabled);
            cx.child(&Popover::new(shows, field, panel).padding(theme.spacing.s3).min_width(0.0))
        } else {
            let text = match &self.format {
                Some(format) => format(value),
                None => format!("{}{}", self.number(value, labels.decimal_separator), self.unit),
            };
            let toggle = open.clone();
            let button = Button::new(text.clone())
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .trailing_icon(IconName::ChevronDown)
                .accessible_name(format!("{name}: {text}"))
                .disabled(self.disabled)
                .on_click(move || toggle.update(|open| *open = !*open));
            let node = cx.child(&Popover::new(shows, button, panel).padding(theme.spacing.s3).min_width(0.0));
            let is_open = open.get();
            cx.update_semantics(node, |s| s.expanded = Some(is_open));
            node
        };
        let was_open = cx.use_state(|| Rc::new(Cell::new(false))).get();
        if open.get() {
            was_open.set(true);
        } else if was_open.replace(false) {
            // The keyboard was in the popover, which is gone.
            cx.request_focus_node(anchor);
        }
        let Some(label) = self.label.as_ref().filter(|_| self.show_label) else { return anchor };
        let text = util::label(cx, label, TextStyle::Callout, None, theme.color(Semantic::InkMuted));
        // The control says it already.
        cx.semantics(text, Semantics::hidden());
        cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[text, anchor],
            Paint::None,
        )
    }
}

/// Whether this is the F2 key, however the platform spells it.
#[doc(hidden)]
pub fn is_f2(key: &Key) -> bool {
    matches!(key, Key::Other(name) if name.trim_start_matches("Named(").trim_end_matches(')') == "F2")
}

/// A name that turns into a field: a double click, F2 or Enter starts, Enter confirms,
/// Escape cancels, and clicking elsewhere confirms. The keyboard goes back to the name after
/// Enter or Escape. An empty name, or one that did not change, changes nothing.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{InlineRename, Ui};
///
/// let name = State::new(String::from("Warm evening"));
/// let rename = InlineRename::new(name.binding()).label("Preset name").on_rename(|new| println!("{new}"));
/// let mut ui = Ui::new(rename, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct InlineRename {
    value: Binding<String>,
    editing: Option<Binding<bool>>,
    on_rename: Option<Rc<dyn Fn(String)>>,
    on_click: Option<Rc<dyn Fn()>>,
    size: ButtonSize,
    width: Option<f32>,
    label: Option<String>,
    placeholder: String,
    disabled: bool,
}

impl fmt::Debug for InlineRename {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InlineRename").field("label", &self.label).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl InlineRename {
    /// A name bound to `value`, which changes only when the new name is confirmed.
    pub fn new(value: Binding<String>) -> Self {
        Self {
            value,
            editing: None,
            on_rename: None,
            on_click: None,
            size: ButtonSize::Small,
            width: None,
            label: None,
            placeholder: String::new(),
            disabled: false,
        }
    }

    /// Whether it is being edited, for starting from elsewhere, such as a menu.
    pub fn editing(mut self, editing: Binding<bool>) -> Self {
        self.editing = Some(editing);
        self
    }

    /// Called with the new name when it is confirmed and differs from the old one.
    pub fn on_rename(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_rename = Some(Rc::new(handler));
        self
    }

    /// What a single click on the name does, such as selecting its row, since the name
    /// takes the clicks that land on it.
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// The height and text size, in both forms so that nothing moves (`Small` by default).
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Fixed width in logical pixels. It takes the width of its content by default.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// What is being named, such as "Preset name", for screen readers.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Muted text shown while the name is empty.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Shows the name and does not let it change.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// The field of an [`InlineRename`]. Its own view, so that each edit starts from the name
/// of now and not from what the last one left.
struct RenameField<'a> {
    rename: &'a InlineRename,
    editing: Binding<bool>,
    refocus: State<bool>,
}

impl View for RenameField<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let rename = self.rename;
        let draft: State<String> = cx.use_state(|| rename.value.get());
        let finish: Rc<dyn Fn(bool)> = {
            let (draft, value, handler) = (draft.clone(), rename.value.clone(), rename.on_rename.clone());
            let (editing, refocus) = (self.editing.clone(), self.refocus.clone());
            Rc::new(move |keyboard_stays| {
                let name = draft.get().trim().to_owned();
                if !name.is_empty() && name != value.get() {
                    value.set(name.clone());
                    if let Some(handler) = &handler {
                        handler(name);
                    }
                }
                editing.set(false);
                refocus.set(keyboard_stays);
            })
        };
        let (on_enter, on_leave) = (finish.clone(), finish);
        let (editing, refocus) = (self.editing.clone(), self.refocus.clone());
        let labels = cx.labels();
        let mut field = TextField::new(draft.binding())
            .size(rename.size)
            .placeholder(rename.label.clone().unwrap_or_else(|| labels.rename.clone()))
            .autofocus()
            .select_on_open()
            .on_submit(move |_| on_enter(true))
            .on_leave(move || on_leave(false))
            .on_cancel(move || {
                editing.set(false);
                refocus.set(true);
            });
        if let Some(width) = rename.width {
            field = field.width(width);
        }
        field.build(cx)
    }
}

impl View for InlineRename {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let own: State<bool> = cx.use_state(|| false);
        let refocus: State<bool> = cx.use_state(|| false);
        let editing = self.editing.clone().unwrap_or_else(|| own.binding());
        if editing.get() && !self.disabled {
            let field = RenameField { rename: self, editing, refocus };
            return cx.keyed_child(1, &field);
        }
        if *refocus.peek() {
            refocus.set(false);
            cx.request_focus();
        }

        let name = self.value.get();
        let empty = name.is_empty();
        let attrs = util::attrs(&theme, self.size.text_style(), None);
        let (shown, ink) = if empty {
            (self.placeholder.as_str(), Semantic::InkMuted)
        } else {
            (name.as_str(), Semantic::Ink)
        };
        let text = cx.text_truncated(
            shown,
            &attrs,
            theme.color(ink),
            LayoutStyle { min_width: Some(0.0), ..LayoutStyle::leaf() },
        );
        let hovered = cx.is_hovered() && !self.disabled;
        let fill = if hovered {
            theme.color(Semantic::Ink).with_alpha(0.05)
        } else {
            ravenrs_core::Color::TRANSPARENT
        };
        let node = cx.container(
            LayoutStyle {
                width: self.width,
                min_width: Some(0.0),
                // The same room as the field, so that the name does not move when it opens.
                min_height: Some(self.size.height()),
                padding: Insets::symmetric(0.0, theme.spacing.s2),
                align: Align::Center,
                justify: Justify::Start,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[text],
            Paint::Quad(QuadPaint { corner_radius: theme.radii.sm, ..QuadPaint::new(fill) }),
        );
        cx.semantics(text, Semantics::hidden());
        let announced = if empty { self.label.clone().unwrap_or_default() } else { name };
        if self.disabled {
            cx.semantics(node, Semantics::new(Role::Label).name(announced).disabled(true));
            return node;
        }
        cx.semantics(
            node,
            Semantics::new(Role::Button).name(announced).description(labels.rename_hint.clone()),
        );
        let start = editing.clone();
        cx.on_double_click(node, Rc::new(move || start.set(true)));
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                if matches!(key, Key::Enter | Key::Space) || is_f2(key) {
                    editing.set(true);
                    return true;
                }
                false
            }),
        );
        if let Some(click) = &self.on_click {
            cx.on_click(node, click.clone());
        }
        cx.focusable(node, theme.radii.sm);
        cx.cursor(node, CursorIcon::Default);
        node
    }
}
