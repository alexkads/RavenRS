//! Spinner, carousel, splitter, menubar, one-time-code input and form fields.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Key, Quad, Rect, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, DragPhase, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::overlay::{Entry, MenuList};
use crate::{AnyView, ButtonSize, ButtonStyle, IconButton, IconName, MenuItem, View, util};

/// An indeterminate activity indicator. With reduced motion it is static.
#[derive(Clone, Copy, Debug)]
pub struct Spinner {
    size: f32,
}

impl Default for Spinner {
    fn default() -> Self {
        Self { size: 20.0 }
    }
}

impl Spinner {
    /// A 20 px spinner.
    pub fn new() -> Self {
        Self::default()
    }

    /// Diameter in logical pixels (default 20).
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

impl View for Spinner {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let now = cx.now();
        let still = theme.interaction.reduced_motion;
        if !still {
            cx.wake_at(now + 1.0 / 30.0);
        }
        let (size, color) = (self.size, theme.color(Semantic::Accent).lerp(theme.color(Semantic::Ink), 0.2));
        let step = if still { 0 } else { ((now * 10.0) as i64).rem_euclid(8) as usize };
        let node = cx.container(
            LayoutStyle { width: Some(size), height: Some(size), ..LayoutStyle::leaf() },
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                let c = f.center();
                let dot = (size * 0.18).max(2.0);
                let r = size / 2.0 - dot / 2.0;
                for i in 0..8 {
                    let a = i as f32 / 8.0 * std::f32::consts::TAU;
                    let age = (i + 8 - step) % 8;
                    let alpha = 1.0 - age as f32 / 9.0;
                    let rect =
                        Rect::new(c.x + r * a.cos() - dot / 2.0, c.y + r * a.sin() - dot / 2.0, dot, dot);
                    scene.push_quad(Quad::new(rect, color.with_alpha(alpha)).radius(dot / 2.0));
                }
            })),
        );
        cx.semantics(node, Semantics::new(Role::ProgressIndicator).name("Loading"));
        node
    }
}

/// Slides with previous/next buttons, position dots and optional autoplay.
pub struct Carousel {
    slides: Vec<AnyView>,
    height: f32,
    autoplay: Option<f64>,
}

impl fmt::Debug for Carousel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Carousel").field("slides", &self.slides.len()).finish_non_exhaustive()
    }
}

impl Carousel {
    /// A carousel over `slides`, starting at the first.
    pub fn new(slides: Vec<AnyView>) -> Self {
        Self { slides, height: 240.0, autoplay: None }
    }

    /// Slide area height in logical pixels (default 240).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Advances every `seconds`.
    pub fn autoplay(mut self, seconds: f64) -> Self {
        self.autoplay = Some(seconds.max(0.5));
        self
    }
}

impl View for Carousel {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let n = self.slides.len().max(1);
        let index: State<usize> = cx.use_state(|| 0);
        let since: State<f64> = cx.use_state(|| f64::NAN);
        let now = cx.now();
        if let Some(every) = self.autoplay {
            let start = *since.peek();
            if start.is_nan() {
                since.set_if_changed(now);
            } else if now - start >= every {
                index.update(|i| *i = (*i + 1) % n);
                since.set(now);
            }
            cx.wake_at(if start.is_nan() { now + every } else { start + every });
        }
        let current = index.get() % n;
        let slide = match self.slides.get(current) {
            Some(s) => cx.keyed_child(current as u64, s.as_ref()),
            None => cx.empty(),
        };
        let stage = cx.container(
            LayoutStyle {
                height: Some(self.height),
                align: Align::Stretch,
                justify: Justify::Center,
                overflow_hidden: true,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[slide],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                ..QuadPaint::new(theme.color(Semantic::Control))
            }),
        );
        cx.clip(stage);
        let (prev, next, s1, s2) = (index.clone(), index.clone(), since.clone(), since.clone());
        let back = cx.child(
            &IconButton::new(IconName::ChevronLeft)
                .label("Previous slide")
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .on_click(move || {
                    prev.update(|i| *i = (*i + n - 1) % n);
                    s1.set(f64::NAN);
                }),
        );
        let forward = cx.child(
            &IconButton::new(IconName::ChevronRight)
                .label("Next slide")
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .on_click(move || {
                    next.update(|i| *i = (*i + 1) % n);
                    s2.set(f64::NAN);
                }),
        );
        let mut dots = Vec::new();
        for i in 0..n {
            let on = i == current;
            let color = if on {
                theme.color(Semantic::Accent)
            } else {
                theme.color(Semantic::InkMuted).with_alpha(0.4)
            };
            let dot = cx.container(
                LayoutStyle {
                    width: Some(if on { 20.0 } else { 8.0 }),
                    height: Some(8.0),
                    ..LayoutStyle::leaf()
                },
                &[],
                Paint::Quad(QuadPaint { corner_radius: 4.0, ..QuadPaint::new(color) }),
            );
            cx.semantics(
                dot,
                Semantics::new(Role::Button).name(format!("Slide {} of {n}", i + 1)).selected(on),
            );
            let idx = index.clone();
            cx.on_click(dot, Rc::new(move || idx.set(i)));
            cx.cursor(dot, CursorIcon::Pointer);
            dots.push(dot);
        }
        let dots = cx.container(
            LayoutStyle {
                spacing: 6.0,
                align: Align::Center,
                grow: 1.0,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &dots,
            Paint::None,
        );
        let controls = cx.container(
            LayoutStyle {
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[back, dots, forward],
            Paint::None,
        );
        cx.container(
            LayoutStyle {
                spacing: 12.0,
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[stage, controls],
            Paint::None,
        )
    }
}

/// Two panes with a draggable divider between them.
///
/// ```
/// use ravenrs_widgets::{Splitter, Text, Ui};
///
/// let split = Splitter::horizontal(Text::new("Files"), Text::new("Editor"))
///     .ratio(0.3)
///     .height(400.0);
/// let mut ui = Ui::new(split, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct Splitter<A, B> {
    first: A,
    second: B,
    vertical: bool,
    initial: f32,
    height: f32,
}

impl<A, B> fmt::Debug for Splitter<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Splitter").field("vertical", &self.vertical).finish_non_exhaustive()
    }
}

impl<A: View, B: View> Splitter<A, B> {
    /// Side-by-side panes.
    pub fn horizontal(first: A, second: B) -> Self {
        Self { first, second, vertical: false, initial: 0.5, height: 260.0 }
    }

    /// Stacked panes.
    pub fn vertical(first: A, second: B) -> Self {
        Self { first, second, vertical: true, initial: 0.5, height: 260.0 }
    }

    /// The first pane's initial share, 0.1 to 0.9.
    pub fn ratio(mut self, ratio: f32) -> Self {
        self.initial = ratio.clamp(0.1, 0.9);
        self
    }

    /// Total height in logical pixels (default 260).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
}

impl<A: View, B: View> View for Splitter<A, B> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let initial = self.initial;
        let ratio: State<f32> = cx.use_state(move || initial);
        let r = ratio.get();
        let pane = |grow: f32| LayoutStyle {
            grow,
            width: Some(0.0),
            height: Some(0.0),
            min_width: Some(0.0),
            min_height: Some(0.0),
            align: Align::Stretch,
            overflow_hidden: true,
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let a = cx.child(&self.first);
        let a = cx.container(pane(r), &[a], Paint::None);
        cx.clip(a);
        let b = cx.child(&self.second);
        let b = cx.container(pane(1.0 - r), &[b], Paint::None);
        cx.clip(b);
        let vertical = self.vertical;
        let (line, grip) = (theme.color(Semantic::Border), theme.color(Semantic::InkMuted));
        let hovered = cx.is_hover_within();
        let handle_style = if vertical {
            LayoutStyle { height: Some(10.0), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() }
        } else {
            LayoutStyle { width: Some(10.0), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() }
        };
        let handle = cx.container(
            handle_style,
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                let c = f.center();
                if vertical {
                    scene.push_quad(Quad::new(Rect::new(f.x(), c.y - 0.5, f.width(), 1.0), line));
                    scene.push_quad(
                        Quad::new(
                            Rect::new(c.x - 12.0, c.y - 2.0, 24.0, 4.0),
                            if hovered { grip } else { line },
                        )
                        .radius(2.0),
                    );
                } else {
                    scene.push_quad(Quad::new(Rect::new(c.x - 0.5, f.y(), 1.0, f.height()), line));
                    scene.push_quad(
                        Quad::new(
                            Rect::new(c.x - 2.0, c.y - 12.0, 4.0, 24.0),
                            if hovered { grip } else { line },
                        )
                        .radius(2.0),
                    );
                }
            })),
        );
        let drag = ratio.clone();
        cx.on_drag(
            handle,
            Rc::new(move |d| {
                if d.phase == DragPhase::End {
                    return;
                }
                let (pos, start, len) = if vertical {
                    (d.position.y, d.container.y(), d.container.height())
                } else {
                    (d.position.x, d.container.x(), d.container.width())
                };
                if len > 10.0 {
                    drag.set(((pos - start - 5.0) / (len - 10.0)).clamp(0.1, 0.9));
                }
            }),
        );
        cx.cursor(handle, if vertical { CursorIcon::RowResize } else { CursorIcon::ColResize });
        let node = cx.container(
            LayoutStyle {
                height: Some(self.height),
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(if vertical { Axis::Vertical } else { Axis::Horizontal })
            },
            &[a, handle, b],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        );
        cx.clip(node);
        node
    }
}

/// A horizontal bar of menus (File, Edit, View…).
#[derive(Default)]
pub struct Menubar {
    menus: Vec<(String, Vec<Entry>)>,
}

impl fmt::Debug for Menubar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.menus.iter().map(|m| &m.0)).finish()
    }
}

impl Menubar {
    /// An empty menubar; add menus with [`Menubar::menu`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a new menu; following `item`/`separator` calls add to it.
    pub fn menu(mut self, title: impl Into<String>) -> Self {
        self.menus.push((title.into(), Vec::new()));
        self
    }

    /// Adds an item to the current menu (ignored before the first `menu`).
    pub fn item(mut self, item: MenuItem) -> Self {
        if let Some((_, e)) = self.menus.last_mut() {
            e.push(Entry::Item(item));
        }
        self
    }

    /// Adds a separator to the current menu (ignored before the first `menu`).
    pub fn separator(mut self) -> Self {
        if let Some((_, e)) = self.menus.last_mut() {
            e.push(Entry::Separator);
        }
        self
    }
}

struct MenuTitle<'a> {
    bar: &'a Menubar,
    open: State<Option<usize>>,
    roving: util::Roving,
}

impl View for MenuTitle<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (i, count) = (self.roving.index, self.roving.count);
        let is_open = self.open.get() == Some(i);
        let hovered = cx.is_hovered();
        let ink = theme.color(Semantic::Ink);
        let label = util::label(cx, &self.bar.menus[i].0, TextStyle::Callout, Some(Weight::SEMIBOLD), ink);
        let fill = if is_open {
            theme.color(Semantic::Control)
        } else if hovered {
            ink.with_alpha(0.06)
        } else {
            Color::TRANSPARENT
        };
        let node = cx.container(
            LayoutStyle {
                height: Some(30.0),
                padding: Insets::symmetric(0.0, 10.0),
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[label],
            Paint::Quad(QuadPaint { corner_radius: 6.0, ..QuadPaint::new(fill) }),
        );
        cx.semantics(node, Semantics::new(Role::MenuItem).expanded(is_open));
        let open = self.open.clone();
        cx.on_click(node, Rc::new(move || open.update(|o| *o = if *o == Some(i) { None } else { Some(i) })));
        cx.cursor(node, CursorIcon::Pointer);
        cx.focusable(node, 6.0);
        // WAI-ARIA menubar: Left and Right move between titles (switching the
        // open menu if one is open); Down, Enter and Space open the menu.
        let opener = self.open.clone();
        let before: crate::KeyHandler = Rc::new(move |key, _| {
            let opens = matches!(key, Key::ArrowDown | Key::Enter | Key::Space);
            if opens {
                opener.set(Some(i));
            }
            opens || *key == Key::ArrowUp
        });
        let follow = self.open.clone();
        let on_move: Rc<dyn Fn(usize)> = Rc::new(move |to| {
            if follow.get().is_some() {
                follow.set(Some(to));
            }
        });
        self.roving.wire_with(cx, node, Some(on_move), Some(before));
        let was_open = cx.use_state(|| Rc::new(Cell::new(false))).get();
        if is_open {
            was_open.set(true);
            let close: Rc<dyn Fn()> = {
                let open = self.open.clone();
                Rc::new(move || open.set(None))
            };
            // The neighbor menu opens and focuses its first item.
            let side_open = self.open.clone();
            let on_side: Rc<dyn Fn(bool)> = Rc::new(move |forward| {
                side_open.set(Some(if forward { (i + 1) % count } else { (i + count - 1) % count }));
            });
            cx.overlay(
                node,
                Placement::Below,
                &MenuList { entries: &self.bar.menus[i].1, close: close.clone(), on_side: Some(on_side) },
                Some(close),
            );
        } else if was_open.replace(false) && self.open.get().is_none() {
            // Focus returns to the title when the menubar's menu closes.
            cx.request_focus_node(node);
        }
        node
    }
}

impl View for Menubar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let open: State<Option<usize>> = cx.use_state(|| None);
        let (count, focus_to) = (self.menus.len(), util::roving_cell(cx, None));
        let titles: Vec<NodeId> = (0..count)
            .map(|index| {
                let roving = util::Roving { index, count, focus_to: focus_to.clone() };
                cx.keyed_child(index as u64, &MenuTitle { bar: self, open: open.clone(), roving })
            })
            .collect();
        let node = cx.container(
            LayoutStyle {
                padding: Insets::uniform(4.0),
                spacing: 2.0,
                align_self: Some(Align::Start),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &titles,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        );
        cx.semantics(node, Semantics::new(Role::MenuBar));
        node
    }
}

/// Separate boxes for a one-time code; accepts digits by default.
pub struct InputOtp {
    value: Binding<String>,
    length: usize,
    on_complete: Option<Rc<dyn Fn(String)>>,
}

impl fmt::Debug for InputOtp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InputOtp").field("length", &self.length).finish_non_exhaustive()
    }
}

impl InputOtp {
    /// `length` boxes (clamped to 1–12) editing `value`.
    pub fn new(value: Binding<String>, length: usize) -> Self {
        Self { value, length: length.clamp(1, 12), on_complete: None }
    }

    /// Called once all boxes are filled.
    pub fn on_complete(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_complete = Some(Rc::new(handler));
        self
    }
}

impl View for InputOtp {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let focused = cx.is_focused();
        let text: Vec<char> = self.value.get().chars().take(self.length).collect();
        let mut boxes = Vec::new();
        let mut attrs = util::attrs(&theme, TextStyle::Title, Some(Weight::SEMIBOLD));
        attrs.size = 20.0;
        for i in 0..self.length {
            let ch = text.get(i).map(|c| c.to_string()).unwrap_or_default();
            let label = cx.text(&ch, &attrs, theme.color(Semantic::Ink), LayoutStyle::leaf());
            let active = focused && i == text.len().min(self.length - 1);
            let border = if active {
                Border { width: 1.5, color: theme.color(Semantic::Accent) }
            } else {
                Border { width: 1.0, color: theme.color(Semantic::Border) }
            };
            boxes.push(cx.container(
                LayoutStyle {
                    width: Some(44.0),
                    height: Some(52.0),
                    align: Align::Center,
                    justify: Justify::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[label],
                Paint::Quad(QuadPaint {
                    corner_radius: theme.radii.md,
                    border: Some(border),
                    ..QuadPaint::new(theme.color(Semantic::Surface))
                }),
            ));
        }
        let node = cx.container(
            LayoutStyle {
                spacing: 8.0,
                align_self: Some(Align::Start),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &boxes,
            Paint::None,
        );
        for b in &boxes {
            cx.semantics(*b, Semantics::hidden());
        }
        let typed: String = self.value.get().chars().take(self.length).collect();
        let mut semantics = Semantics::new(Role::TextInput)
            .name("Verification code")
            .value(typed)
            .description(format!("{} digits", self.length));
        semantics.fallback_name = true;
        cx.semantics(node, semantics);
        let (value, len, done) = (self.value.clone(), self.length, self.on_complete.clone());
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                let mut v: String = value.get().chars().take(len).collect();
                match key {
                    Key::Character(s) => {
                        for c in s.chars().filter(char::is_ascii_digit) {
                            if v.chars().count() < len {
                                v.push(c);
                            }
                        }
                    }
                    Key::Backspace => {
                        v.pop();
                    }
                    _ => return false,
                }
                let complete = v.chars().count() == len;
                value.set(v.clone());
                if complete && let Some(d) = &done {
                    d(v);
                }
                true
            }),
        );
        cx.focusable(node, theme.radii.md);
        cx.cursor(node, CursorIcon::Text);
        node
    }
}

/// Common validation rules for [`FormField::validate`].
pub mod validate {
    /// The field must not be empty.
    pub fn required(value: &str) -> Result<(), String> {
        if value.trim().is_empty() { Err("This field is required.".into()) } else { Ok(()) }
    }

    /// A plausible email address (something@something.tld).
    pub fn email(value: &str) -> Result<(), String> {
        let ok = value.split_once('@').is_some_and(|(user, domain)| {
            !user.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
        });
        if ok || value.is_empty() { Ok(()) } else { Err("Enter an email like name@example.com.".into()) }
    }

    /// At least `n` characters.
    pub fn min_length(n: usize) -> impl Fn(&str) -> Result<(), String> {
        move |value| {
            if value.chars().count() >= n || value.is_empty() {
                Ok(())
            } else {
                Err(format!("Use at least {n} characters."))
            }
        }
    }
}

/// A validation rule: `Ok(())` or a message for the user.
pub type Rule = Rc<dyn Fn(&str) -> Result<(), String>>;

/// A labeled form field with a hint and validation messages.
///
/// Errors appear once the value is non-empty or the form was submitted
/// (see [`FormField::show_errors`]), so empty forms do not start out red.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{FormField, TextField, Ui, validate};
///
/// let email = State::new("raven@".to_string());
/// let field = FormField::new("Email", TextField::new(email.binding()))
///     .hint("We never share it.")
///     .validate(email.binding(), validate::email);
/// assert!(field.error().is_some());
///
/// email.set("raven@example.com".into());
/// assert_eq!(field.error(), None);
///
/// let mut ui = Ui::new(field, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct FormField<V> {
    label: crate::Label,
    content: V,
    hint: Option<String>,
    value: Option<Binding<String>>,
    rules: Vec<Rule>,
    show_errors: bool,
}

impl<V> fmt::Debug for FormField<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FormField").field("rules", &self.rules.len()).finish_non_exhaustive()
    }
}

impl<V: View> FormField<V> {
    /// A field with this label above `content`.
    pub fn new(label: impl Into<String>, content: V) -> Self {
        Self {
            label: crate::Label::new(label),
            content,
            hint: None,
            value: None,
            rules: Vec::new(),
            show_errors: false,
        }
    }

    /// Marks the label with `*` and adds the [`validate::required`] rule (for the value set by [`FormField::validate`]).
    pub fn required(mut self) -> Self {
        self.label = self.label.required();
        self.rules.push(Rc::new(validate::required));
        self
    }

    /// A muted caption under the content.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Validates `value` with `rule` (rules run in order; the first error shows).
    pub fn validate(
        mut self,
        value: Binding<String>,
        rule: impl Fn(&str) -> Result<(), String> + 'static,
    ) -> Self {
        self.value = Some(value);
        self.rules.push(Rc::new(rule));
        self
    }

    /// Shows errors even for empty values (e.g. after a submit attempt).
    pub fn show_errors(mut self, show: bool) -> Self {
        self.show_errors = show;
        self
    }

    /// The first error for the current value, if any.
    pub fn error(&self) -> Option<String> {
        let v = self.value.as_ref()?.get();
        self.rules.iter().find_map(|r| r(&v).err())
    }
}

impl<V: View> View for FormField<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let label = cx.child(&self.label);
        let content = cx.child(&self.content);
        let mut children = vec![label, content];
        let value = self.value.as_ref().map(Binding::get).unwrap_or_default();
        let error = (self.show_errors || !value.is_empty()).then(|| self.error()).flatten();
        cx.label_control(content, self.label.text(), error.as_deref().or(self.hint.as_deref()));
        if let Some(e) = error {
            let icon = util::icon(cx, IconName::CircleAlert, 14.0, theme.color(Semantic::Danger));
            let text = util::label(cx, &e, TextStyle::Callout, None, theme.color(Semantic::Danger));
            children.push(cx.container(
                LayoutStyle { spacing: 6.0, align: Align::Center, ..LayoutStyle::stack(Axis::Horizontal) },
                &[icon, text],
                Paint::None,
            ));
        } else if let Some(h) = &self.hint {
            children.push(util::label(cx, h, TextStyle::Callout, None, theme.color(Semantic::InkMuted)));
        }
        cx.container(
            LayoutStyle { spacing: 6.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &children,
            Paint::None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::validate;

    #[test]
    fn validators() {
        assert!(validate::required("  ").is_err());
        assert!(validate::email("ana@raven.rs").is_ok());
        assert!(validate::email("ana@raven").is_err());
        assert!(validate::email("").is_ok(), "emptiness is required()'s job");
        assert!(validate::min_length(8)("short").is_err());
        assert!(validate::min_length(8)("long enough").is_ok());
    }
}
