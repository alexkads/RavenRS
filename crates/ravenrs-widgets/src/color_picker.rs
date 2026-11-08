//! Choosing a color: a field that opens a palette, a shade square and a hue bar.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Key, LinearGradient, Point, Quad, Rect, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, DragPhase, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::overlay::panel;
use crate::{Button, ButtonSize, ButtonStyle, ColorSwatch, IconName, TextField, View, util};

const PANEL_WIDTH: f32 = 248.0;
/// The least it can show, since it fills what it is in unless given a width.
const FIELD_MIN_WIDTH: f32 = 140.0;
const SHADE_HEIGHT: f32 = 148.0;
const BAR_HEIGHT: f32 = 14.0;
const THUMB: f32 = 12.0;
/// The colors of the hue bar, in order, a sixth of the way each.
const HUES: [Color; 7] = [
    Color::rgb(1.0, 0.0, 0.0),
    Color::rgb(1.0, 1.0, 0.0),
    Color::rgb(0.0, 1.0, 0.0),
    Color::rgb(0.0, 1.0, 1.0),
    Color::rgb(0.0, 0.0, 1.0),
    Color::rgb(1.0, 0.0, 1.0),
    Color::rgb(1.0, 0.0, 0.0),
];

/// A color as people think of it. A gray keeps its hue, so moving through one
/// does not throw the choice back to red.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hsv {
    /// 0 to 360.
    pub(crate) hue: f32,
    /// 0 to 1.
    pub(crate) saturation: f32,
    /// 0 to 1.
    pub(crate) value: f32,
    /// 0 to 1.
    pub(crate) alpha: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Hsv {
    fn of(color: Color) -> Self {
        let (r, g, b) = (color.r, color.g, color.b);
        let top = r.max(g).max(b);
        let span = top - r.min(g).min(b);
        let hue = if span <= f32::EPSILON {
            0.0
        } else if top == r {
            60.0 * ((g - b) / span).rem_euclid(6.0)
        } else if top == g {
            60.0 * ((b - r) / span + 2.0)
        } else {
            60.0 * ((r - g) / span + 4.0)
        };
        let saturation = if top <= f32::EPSILON { 0.0 } else { span / top };
        Self { hue, saturation, value: top, alpha: color.a }
    }

    pub(crate) fn color(self) -> Color {
        let chroma = self.value * self.saturation;
        let part = self.hue.rem_euclid(360.0) / 60.0;
        let second = chroma * (1.0 - (part.rem_euclid(2.0) - 1.0).abs());
        let (r, g, b) = match part as u32 {
            0 => (chroma, second, 0.0),
            1 => (second, chroma, 0.0),
            2 => (0.0, chroma, second),
            3 => (0.0, second, chroma),
            4 => (second, 0.0, chroma),
            _ => (chroma, 0.0, second),
        };
        let lift = self.value - chroma;
        Color::rgba(r + lift, g + lift, b + lift, self.alpha)
    }
}

/// `#RRGGBB`, or `#RRGGBBAA` when it is not opaque.
pub(crate) fn to_hex(color: Color) -> String {
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    let rgb = format!("#{:02X}{:02X}{:02X}", byte(color.r), byte(color.g), byte(color.b));
    if byte(color.a) == 255 { rgb } else { format!("{rgb}{:02X}", byte(color.a)) }
}

/// Reads `RGB`, `RRGGBB` or `RRGGBBAA`, with or without the `#`.
fn parse_hex(text: &str) -> Option<Color> {
    let digits = text.trim().trim_start_matches('#');
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let number = u32::from_str_radix(digits, 16).ok();
    match digits.len() {
        3 => number.map(|n| {
            let spread = |d: u32| d * 0x11;
            Color::hex(spread(n >> 8) << 16 | spread((n >> 4) & 0xF) << 8 | spread(n & 0xF))
        }),
        6 => number.map(Color::hex),
        8 => number.map(Color::hex_alpha),
        _ => None,
    }
}

/// Picks a color: from a palette, on a shade square and a hue bar, or typed in
/// hexadecimal. The square moves with the arrows, the bars with Left and Right.
///
/// ```
/// use ravenrs_core::Color;
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{ColorPicker, Ui};
///
/// let color = State::new(Some(Color::hex(0x2563EB)));
/// let picker = ColorPicker::new(color.binding()).alpha(true);
/// let mut ui = Ui::new(picker, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct ColorPicker {
    value: Binding<Option<Color>>,
    palette: Vec<(Color, String)>,
    alpha: bool,
    width: Option<f32>,
    label: Option<String>,
    state: util::EditorState,
}

impl fmt::Debug for ColorPicker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ColorPicker").field("alpha", &self.alpha).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ColorPicker {
    /// A field bound to a color, or to none yet.
    pub fn new(value: Binding<Option<Color>>) -> Self {
        Self {
            value,
            palette: Vec::new(),
            alpha: false,
            width: None,
            label: None,
            state: util::EditorState::default(),
        }
    }

    /// Named colors to pick with one click.
    pub fn palette(mut self, colors: impl IntoIterator<Item = (Color, impl Into<String>)>) -> Self {
        self.palette = colors.into_iter().map(|(color, name)| (color, name.into())).collect();
        self
    }

    /// Lets the color be see-through, with one more bar.
    pub fn alpha(mut self, alpha: bool) -> Self {
        self.alpha = alpha;
        self
    }

    /// Width in logical pixels. 180 by default.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// The name screen readers announce. Inside a [`crate::FormField`], its label is used instead.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Turns the field off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the color without letting it change; it can still be focused and read.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

/// The choice being made: what the panel moves, and the field it writes to.
#[derive(Clone)]
struct Draft {
    hsv: State<Hsv>,
    value: Binding<Option<Color>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Draft {
    fn change(&self, f: impl FnOnce(&mut Hsv)) {
        let mut hsv = *self.hsv.peek();
        f(&mut hsv);
        self.hsv.set(hsv);
        self.value.set(Some(hsv.color()));
    }
}

fn thumb(scene: &mut ravenrs_core::Scene, center: Point, fill: Color, ring: Color) {
    let half = THUMB / 2.0;
    let frame = Rect::new(center.x - half, center.y - half, THUMB, THUMB);
    scene.push_quad(Quad::new(frame, fill).radius(half).border(2.0, ring));
}

/// Focus and drags go by view, so each control of the panel is one.
struct ShadeSquare(Draft);

impl View for ShadeSquare {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        shade_square(cx, &self.0)
    }
}

fn shade_square(cx: &mut BuildCx<'_>, draft: &Draft) -> NodeId {
    let theme = *cx.theme();
    let hsv = draft.hsv.get();
    let ring = Color::WHITE;
    let node = cx.container(
        LayoutStyle { height: Some(SHADE_HEIGHT), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() },
        &[],
        Paint::Custom(Rc::new(move |f: Rect, scene| {
            let radius = theme.radii.sm;
            let pure = Hsv { saturation: 1.0, value: 1.0, alpha: 1.0, ..hsv }.color();
            let across = LinearGradient {
                start: Point::new(0.0, 0.0),
                end: Point::new(1.0, 0.0),
                from: Color::WHITE,
                to: pure,
            };
            scene.push_quad(Quad::new(f, across).radius(radius));
            let down = LinearGradient::vertical(Color::BLACK.with_alpha(0.0), Color::BLACK);
            scene.push_quad(Quad::new(f, down).radius(radius));
            let at = Point::new(f.x() + hsv.saturation * f.width(), f.y() + (1.0 - hsv.value) * f.height());
            thumb(scene, at, Hsv { alpha: 1.0, ..hsv }.color(), ring);
        })),
    );
    let name = cx.labels().color_shade.clone();
    cx.semantics(
        node,
        Semantics::new(Role::Slider).name(name).value(format!(
            "{}%, {}%",
            (hsv.saturation * 100.0).round(),
            (hsv.value * 100.0).round()
        )),
    );
    cx.focusable(node, theme.radii.sm);
    cx.cursor(node, CursorIcon::Pointer);
    let dragged = draft.clone();
    cx.on_drag(
        node,
        Rc::new(move |d| {
            if d.phase == DragPhase::End {
                return;
            }
            let x = ((d.position.x - d.frame.x()) / d.frame.width().max(1.0)).clamp(0.0, 1.0);
            let y = ((d.position.y - d.frame.y()) / d.frame.height().max(1.0)).clamp(0.0, 1.0);
            dragged.change(|hsv| {
                hsv.saturation = x;
                hsv.value = 1.0 - y;
            });
        }),
    );
    let keyed = draft.clone();
    cx.on_key(
        node,
        Rc::new(move |key, _| {
            let (x, y) = match key {
                Key::ArrowLeft => (-0.02, 0.0),
                Key::ArrowRight => (0.02, 0.0),
                Key::ArrowUp => (0.0, 0.02),
                Key::ArrowDown => (0.0, -0.02),
                _ => return false,
            };
            keyed.change(|hsv| {
                hsv.saturation = (hsv.saturation + x).clamp(0.0, 1.0);
                hsv.value = (hsv.value + y).clamp(0.0, 1.0);
            });
            true
        }),
    );
    node
}

type BarPaint = Rc<dyn Fn(Rect, &mut ravenrs_core::Scene)>;

/// A bar from 0 to `max`.
struct ColorBar {
    name: String,
    value: f32,
    max: f32,
    step: f32,
    paint: BarPaint,
    set: Rc<dyn Fn(f32)>,
}

impl View for ColorBar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (value, max, step) = (self.value, self.max, self.step);
        let paint = self.paint.clone();
        let node = cx.container(
            LayoutStyle { height: Some(BAR_HEIGHT), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() },
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                paint(f, scene);
                let at = Point::new(f.x() + value / max * f.width(), f.center().y);
                thumb(scene, at, Color::TRANSPARENT, Color::WHITE);
            })),
        );
        cx.semantics(
            node,
            Semantics::new(Role::Slider).name(self.name.clone()).numeric(
                f64::from(value),
                0.0,
                f64::from(max),
                Some(f64::from(step)),
            ),
        );
        cx.focusable(node, theme.radii.full);
        cx.cursor(node, CursorIcon::Pointer);
        let dragged = self.set.clone();
        cx.on_drag(
            node,
            Rc::new(move |d| {
                if d.phase != DragPhase::End {
                    dragged(d.fraction_x() * max);
                }
            }),
        );
        let set = self.set.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                match key {
                    Key::ArrowLeft | Key::ArrowDown => set((value - step).max(0.0)),
                    Key::ArrowRight | Key::ArrowUp => set((value + step).min(max)),
                    _ => return false,
                }
                true
            }),
        );
        node
    }
}

struct ColorPanel<'a> {
    picker: &'a ColorPicker,
    draft: Draft,
    open: State<bool>,
}

impl View for ColorPanel<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let draft = &self.draft;
        let hsv = draft.hsv.get();
        let mut rows = Vec::new();
        if !self.picker.palette.is_empty() {
            let current = self.picker.value.get();
            let mut swatches = Vec::with_capacity(self.picker.palette.len());
            for (i, (color, name)) in self.picker.palette.iter().enumerate() {
                let (d, color) = (draft.clone(), *color);
                let on = current.is_some_and(|c| to_hex(c) == to_hex(color));
                swatches.push(cx.keyed_child(
                    i as u64,
                    &ColorSwatch::new(color, name.clone()).selected(on).on_click(move || {
                        d.hsv.set(Hsv::of(color));
                        d.value.set(Some(color));
                    }),
                ));
            }
            rows.push(cx.container(
                LayoutStyle { wrap: true, ..LayoutStyle::stack(Axis::Horizontal) },
                &swatches,
                Paint::None,
            ));
        }
        rows.push(cx.child(&ShadeSquare(draft.clone())));
        let radius = theme.radii.full;
        let d = draft.clone();
        rows.push(cx.child(&ColorBar {
            name: labels.color_hue.clone(),
            value: hsv.hue,
            max: 360.0,
            step: 3.0,
            paint: Rc::new(move |f, scene| {
                let part = f.width() / (HUES.len() - 1) as f32;
                for (i, pair) in HUES.windows(2).enumerate() {
                    let piece = Rect::new(f.x() + i as f32 * part, f.y(), part + 0.5, f.height());
                    let gradient = LinearGradient {
                        start: Point::new(0.0, 0.0),
                        end: Point::new(1.0, 0.0),
                        from: pair[0],
                        to: pair[1],
                    };
                    scene.push_quad(Quad::new(piece, gradient));
                }
            }),
            set: Rc::new(move |value| d.change(|hsv| hsv.hue = value.min(359.9))),
        }));
        if self.picker.alpha {
            let d = draft.clone();
            let solid = Hsv { alpha: 1.0, ..hsv }.color();
            rows.push(cx.child(&ColorBar {
                name: labels.color_alpha.clone(),
                value: hsv.alpha,
                max: 1.0,
                step: 0.02,
                paint: Rc::new(move |f, scene| {
                    let gradient = LinearGradient {
                        start: Point::new(0.0, 0.0),
                        end: Point::new(1.0, 0.0),
                        from: solid.with_alpha(0.0),
                        to: solid,
                    };
                    scene.push_quad(Quad::new(f, gradient).radius(radius));
                }),
                set: Rc::new(move |value| d.change(|hsv| hsv.alpha = value)),
            }));
        }
        // Holds what is being typed, which is only a color once it is complete.
        let typing: State<Option<String>> = cx.use_state(|| None);
        let (shown, kept, d, opaque) = (typing.clone(), typing.clone(), draft.clone(), !self.picker.alpha);
        let current = to_hex(hsv.color());
        let hex = Binding::new(
            move || shown.get().unwrap_or_else(|| current.clone()),
            move |text: String| {
                if let Some(color) = parse_hex(&text) {
                    let color = if opaque { color.with_alpha(1.0) } else { color };
                    d.hsv.set(Hsv::of(color));
                    d.value.set(Some(color));
                }
                kept.set(Some(text));
            },
        );
        let field =
            cx.child(&TextField::new(hex).placeholder(labels.color_hex.clone()).width(116.0).compact());
        let value = self.picker.value.clone();
        let close = self.open.clone();
        let none = cx.child(
            &Button::new(labels.no_color.clone()).style(ButtonStyle::Ghost).size(ButtonSize::Small).on_click(
                move || {
                    value.set(None);
                    close.set(false);
                },
            ),
        );
        rows.push(cx.container(
            LayoutStyle { spacing: 8.0, align: Align::Center, ..LayoutStyle::stack(Axis::Horizontal) },
            &[field, none],
            Paint::None,
        ));
        let inner = cx.container(
            LayoutStyle { spacing: 10.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &rows,
            Paint::None,
        );
        let node = panel(cx, &[inner], 12.0, PANEL_WIDTH);
        cx.semantics(node, Semantics::new(Role::Dialog).name(labels.pick_color.clone()));
        let closing = self.open.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                let done = *key == Key::Escape;
                if done {
                    closing.set(false);
                }
                done
            }),
        );
        node
    }
}

impl View for ColorPicker {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let open: State<bool> = cx.use_state(|| false);
        let state = self.state;
        let is_open = open.get() && state.accepts_input();
        let hovered = cx.is_hovered() && state.accepts_input();
        let current = self.value.get();
        let start = current.unwrap_or(Color::hex(0x3B82F6));
        let hsv: State<Hsv> = cx.use_state(move || Hsv::of(start));
        // Something outside changed the color.
        if let Some(color) = current
            && to_hex(hsv.peek().color()) != to_hex(color)
        {
            hsv.set_if_changed(Hsv::of(color));
        }
        let (text, color) = match current {
            Some(color) => (to_hex(color), theme.color(Semantic::Ink)),
            None => (cx.labels().no_color.clone(), theme.color(Semantic::InkMuted)),
        };
        let edge = theme.color(Semantic::Border);
        let sample = cx.container(
            LayoutStyle { width: Some(20.0), height: Some(20.0), ..LayoutStyle::leaf() },
            &[],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.sm,
                border: Some(Border { width: 1.0, color: edge }),
                ..QuadPaint::new(state.ink(current.unwrap_or(Color::TRANSPARENT)))
            }),
        );
        let label = util::label(cx, &text, TextStyle::Body, None, state.ink(color));
        cx.semantics(label, Semantics::hidden());
        let chevron = util::icon(cx, IconName::ChevronDown, 16.0, state.ink(theme.color(Semantic::InkMuted)));
        let filler = cx.container(LayoutStyle { grow: 1.0, ..LayoutStyle::leaf() }, &[], Paint::None);
        let border = if is_open {
            Border { width: 1.5, color: theme.color(Semantic::Accent) }
        } else {
            Border {
                width: 1.0,
                color: state.ink(if hovered { edge.lerp(theme.color(Semantic::Ink), 0.25) } else { edge }),
            }
        };
        let node = cx.container(
            LayoutStyle {
                width: self.width,
                min_width: Some(FIELD_MIN_WIDTH),
                height: Some(40.0),
                padding: Insets::symmetric(0.0, 12.0),
                spacing: 8.0,
                align: Align::Center,
                align_self: Some(if self.width.is_some() { Align::Fit } else { Align::Stretch }),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[sample, label, filler, chevron],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(border),
                ..QuadPaint::new(state.fill(theme.color(Semantic::Control)))
            }),
        );
        let mut semantics = Semantics::new(Role::ComboBox).value(text).expanded(is_open);
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, state.semantics(semantics));
        if state.disabled {
            return node;
        }
        cx.focusable(node, theme.radii.md);
        if !state.accepts_input() {
            return node;
        }
        let toggle = open.clone();
        cx.on_click(node, Rc::new(move || toggle.update(|o| *o = !*o)));
        let opener = open.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                let opens = matches!(key, Key::ArrowDown | Key::ArrowUp);
                if opens {
                    opener.set(true);
                }
                opens
            }),
        );
        cx.cursor(node, CursorIcon::Pointer);
        let was_open = cx.use_state(|| Rc::new(Cell::new(false))).get();
        if is_open {
            was_open.set(true);
            let close = open.clone();
            let body = ColorPanel {
                picker: self,
                draft: Draft { hsv, value: self.value.clone() },
                open: open.clone(),
            };
            cx.overlay(node, Placement::Below, &body, Some(Rc::new(move || close.set(false))));
        } else if was_open.replace(false) {
            cx.request_focus();
        }
        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_go_to_hsv_and_back() {
        for hex in [0xFF0000, 0x00FF00, 0x0000FF, 0x2563EB, 0xF59E0B, 0x808080, 0x000000, 0xFFFFFF] {
            let color = Color::hex(hex);
            assert_eq!(to_hex(Hsv::of(color).color()), to_hex(color));
        }
    }

    #[test]
    fn known_hues() {
        assert_eq!(Hsv::of(Color::hex(0xFF0000)).hue, 0.0);
        assert_eq!(Hsv::of(Color::hex(0x00FF00)).hue, 120.0);
        assert_eq!(Hsv::of(Color::hex(0x0000FF)).hue, 240.0);
        assert_eq!(Hsv::of(Color::hex(0x808080)).saturation, 0.0);
    }

    #[test]
    fn hexadecimal_is_read_in_every_length() {
        assert_eq!(parse_hex("#2563EB").map(to_hex).as_deref(), Some("#2563EB"));
        assert_eq!(parse_hex("2563eb").map(to_hex).as_deref(), Some("#2563EB"));
        assert_eq!(parse_hex("#F0A").map(to_hex).as_deref(), Some("#FF00AA"));
        assert_eq!(parse_hex("#2563EB80").map(to_hex).as_deref(), Some("#2563EB80"));
        for wrong in ["", "#12", "#12345", "#GGGGGG", "+12345", "#1234567"] {
            assert_eq!(parse_hex(wrong), None, "{wrong}");
        }
    }
}
