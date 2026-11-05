//! Looking at whole pictures: two side by side or across a divider, and one over everything.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{
    Color, CursorIcon, Effect, ImageData, Insets, Key, PathFill, Quad, Rect, Role, Scene, Size,
};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, Observer};
use ravenrs_style::{Semantic, TextStyle, Weight};

use super::overlays::{fitted, gesture_paint, picture_inks};
use super::{ZoomContent, ZoomControl, ZoomPanView, ZoomView, triangle};
use crate::context::{BuildCx, DragPhase, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::{ButtonSize, ButtonStyle, IconButton, IconName, Motion, View, util};

// ---------------------------------------------------------------------------------------------
// CompareView
// ---------------------------------------------------------------------------------------------

/// How a [`CompareView`] shows its two pictures.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompareMode {
    /// One beside the other.
    #[default]
    SideBySide,
    /// One over the other, each shown up to a divider that can be dragged.
    Divider,
}

/// One of the two pictures of a [`CompareView`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareSide {
    /// The first picture.
    First,
    /// The second picture.
    Second,
}

/// The space between the two pictures side by side.
const COMPARE_GAP: f32 = 2.0;
const DIVIDER_KNOB: f32 = 15.0;

/// Paints a whole picture fitted in `area`, shown only inside `window`.
fn paint_fitted(scene: &mut Scene, picture: &ZoomContent, content: Size, area: Rect, window: Rect) {
    let placed = fitted(area, content);
    let scale = placed.width() / content.width.max(f32::EPSILON);
    picture.paint(scene, window, placed, scale, 0.0);
}

/// Two pictures to tell apart: side by side, or one over the other with a divider between what
/// shows of each. The divider follows a drag from anywhere, the arrows (with Shift, further),
/// Home and End, and a double click puts it in the middle. A picture that does not have the
/// proportions of the box is fitted in it, centered.
///
/// With [`chosen`](Self::chosen), side by side, a click or the arrows pick one of the two, which
/// is marked with a check as well as a colored outline.
///
/// A drag of the divider paints without a build, so it costs no rebuilds if nothing else reads
/// its position.
///
/// ```
/// use ravenrs_core::{Color, ImageData};
/// use ravenrs_widgets::{CompareMode, CompareView};
///
/// let before = ImageData::from_fn(64, 48, |_, _| Color::rgb(0.3, 0.3, 0.3));
/// let after = ImageData::from_fn(64, 48, |_, _| Color::rgb(0.6, 0.5, 0.3));
/// if let (Some(before), Some(after)) = (before, after) {
///     let compare = CompareView::new(before, after).mode(CompareMode::Divider).size(480.0, 320.0);
/// }
/// ```
pub struct CompareView {
    first: ImageData,
    second: ImageData,
    mode: CompareMode,
    divider: Option<Binding<f32>>,
    chosen: Option<Binding<CompareSide>>,
    captions: Option<(Option<String>, Option<String>)>,
    width: Option<f32>,
    height: Option<f32>,
    grow: f32,
    label: Option<String>,
}

impl fmt::Debug for CompareView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompareView").field("mode", &self.mode).finish_non_exhaustive()
    }
}

impl CompareView {
    /// Compares two pictures, side by side.
    pub fn new(first: ImageData, second: ImageData) -> Self {
        Self {
            first,
            second,
            mode: CompareMode::SideBySide,
            divider: None,
            chosen: None,
            captions: Some((None, None)),
            width: None,
            height: None,
            grow: 1.0,
            label: None,
        }
    }

    /// How it shows them.
    pub fn mode(mut self, mode: CompareMode) -> Self {
        self.mode = mode;
        self
    }

    /// Where the divider is, from 0 (all of the second) to 1 (all of the first). Without this it
    /// keeps its own, starting in the middle.
    pub fn divider(mut self, divider: Binding<f32>) -> Self {
        self.divider = Some(divider);
        self
    }

    /// Which of the two is picked, side by side.
    pub fn chosen(mut self, chosen: Binding<CompareSide>) -> Self {
        self.chosen = Some(chosen);
        self
    }

    /// What the small captions over the pictures say, instead of those in
    /// [`Labels`](crate::Labels).
    pub fn captions(mut self, first: impl Into<String>, second: impl Into<String>) -> Self {
        self.captions = Some((Some(first.into()), Some(second.into())));
        self
    }

    /// Shows no captions.
    pub fn no_captions(mut self) -> Self {
        self.captions = None;
        self
    }

    /// Fixed size in logical pixels, instead of taking the free space.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self.grow = 0.0;
        self
    }

    /// How much of the free space along the parent stack it takes (1 by default).
    pub fn grow(mut self, grow: f32) -> Self {
        self.grow = grow;
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// A small caption over a picture.
fn compare_caption(cx: &mut BuildCx<'_>, text: &str, x: f32, chosen: bool) -> NodeId {
    let theme = *cx.theme();
    let (fill, ink) = if chosen {
        (theme.color(Semantic::Accent), theme.color(Semantic::OnAccent))
    } else {
        (theme.color(Semantic::SurfaceElevated), theme.color(Semantic::Ink))
    };
    let mut parts = Vec::with_capacity(2);
    if chosen {
        parts.push(util::icon(cx, IconName::Check, 12.0, ink));
    }
    parts.push(util::label(cx, text, TextStyle::Caption, Some(Weight::MEDIUM), ink));
    cx.container(
        LayoutStyle {
            absolute: Some((x, theme.spacing.s2)),
            padding: Insets::symmetric(2.0, 8.0),
            spacing: theme.spacing.s1,
            align: Align::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        },
        &parts,
        Paint::Quad(QuadPaint { corner_radius: theme.radii.sm, ..QuadPaint::new(fill) }),
    )
}

/// The width of a caption with its padding, to place one against the right side of a box.
fn caption_width(cx: &mut BuildCx<'_>, text: &str, chosen: bool) -> f32 {
    let theme = *cx.theme();
    let attrs = util::attrs(&theme, TextStyle::Caption, Some(Weight::MEDIUM));
    let extra = if chosen { 12.0 + theme.spacing.s1 } else { 0.0 };
    cx.measure_text(text, &attrs).width.ceil() + 16.0 + extra
}

impl View for CompareView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let live = cx.use_state(|| false);
        let is_live = live.get();
        let own = cx.use_state(|| 0.5_f32);
        let divider = self.divider.clone().unwrap_or_else(|| own.binding());
        let at = if is_live { Observer::untracked(|| divider.get()) } else { divider.get() };
        let chosen = self.chosen.as_ref().map(Binding::get);
        let side_by_side = self.mode == CompareMode::SideBySide;
        let (first, second) =
            (ZoomContent::Image(self.first.clone()), ZoomContent::Image(self.second.clone()));
        let size_of = |image: &ImageData| Size::new(image.width() as f32, image.height() as f32);
        let (first_size, second_size) = (size_of(&self.first), size_of(&self.second));
        let (background, accent) = (theme.color(Semantic::Canvas), theme.color(Semantic::Accent));
        let (light, dark) = picture_inks(&theme);
        let shown_divider = divider.clone();
        let paint = gesture_paint(is_live, move |frame: Rect, scene: &mut Scene| {
            scene.push_quad(Quad::new(frame, background));
            if side_by_side {
                let half = (frame.width() - COMPARE_GAP) / 2.0;
                let left = Rect::new(frame.x(), frame.y(), half, frame.height());
                let right = Rect::new(left.max_x() + COMPARE_GAP, frame.y(), half, frame.height());
                paint_fitted(scene, &first, first_size, left, left);
                paint_fitted(scene, &second, second_size, right, right);
                if let Some(side) = chosen {
                    let pane = if side == CompareSide::First { left } else { right };
                    scene.push_quad(Quad::new(pane, Color::TRANSPARENT).border(2.0, accent));
                }
                return;
            }
            let x = frame.x() + frame.width() * shown_divider.get().clamp(0.0, 1.0);
            let left = Rect::new(frame.x(), frame.y(), x - frame.x(), frame.height());
            let right = Rect::new(x, frame.y(), frame.max_x() - x, frame.height());
            paint_fitted(scene, &first, first_size, frame, left);
            paint_fitted(scene, &second, second_size, frame, right);
            // A dark line each side, so that it shows over a bright picture as well.
            scene.push_quad(Quad::new(
                Rect::new(x - 1.5, frame.y(), 3.0, frame.height()),
                dark.with_alpha(0.5),
            ));
            scene.push_quad(Quad::new(Rect::new(x - 0.5, frame.y(), 1.0, frame.height()), light));
            let middle = frame.center().y;
            let knob =
                Rect::new(x - DIVIDER_KNOB, middle - DIVIDER_KNOB, DIVIDER_KNOB * 2.0, DIVIDER_KNOB * 2.0);
            scene.push_quad(Quad::new(knob, light).radius(DIVIDER_KNOB).border(1.0, dark.with_alpha(0.5)));
            for (tip, base) in [(-9.0, -3.5), (9.0, 3.5)] {
                let arrow = triangle([(x + tip, middle), (x + base, middle - 4.5), (x + base, middle + 4.5)]);
                scene.push_path(PathFill::solid(arrow, dark));
            }
        });
        let width = cx.scroll_viewport().map_or(self.width.unwrap_or(0.0), |s| s.width);
        let mut captions = Vec::with_capacity(2);
        if let Some((a, b)) = &self.captions {
            let a = a.clone().unwrap_or_else(|| labels.compare_first.clone());
            let b = b.clone().unwrap_or_else(|| labels.compare_second.clone());
            let inset = theme.spacing.s2;
            let (a_chosen, b_chosen) =
                (chosen == Some(CompareSide::First), chosen == Some(CompareSide::Second));
            captions.push(compare_caption(cx, &a, inset, side_by_side && a_chosen));
            let x = if side_by_side {
                (width + COMPARE_GAP) / 2.0 + inset
            } else {
                width - inset - caption_width(cx, &b, false)
            };
            captions.push(compare_caption(cx, &b, x, side_by_side && b_chosen));
        }
        let node = cx.container(
            LayoutStyle {
                width: self.width,
                height: self.height,
                grow: self.grow,
                align_self: Some(if self.width.is_some() { Align::Start } else { Align::Stretch }),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &captions,
            paint,
        );
        cx.report_frame(node);
        let name = self.label.clone().unwrap_or_else(|| labels.compare.clone());
        cx.focusable(node, theme.radii.sm);
        if side_by_side {
            let mut semantics = Semantics::new(Role::Group).name(name);
            if let (Some(side), Some((a, b))) = (chosen, &self.captions) {
                let (a, b) = (
                    a.clone().unwrap_or_else(|| labels.compare_first.clone()),
                    b.clone().unwrap_or_else(|| labels.compare_second.clone()),
                );
                semantics = semantics.value(if side == CompareSide::First { a } else { b });
            }
            cx.semantics(node, semantics);
            let Some(chosen) = self.chosen.clone() else { return node };
            cx.cursor(node, CursorIcon::Pointer);
            let clicked = chosen.clone();
            cx.on_drag(
                node,
                Rc::new(move |d| {
                    if d.phase == DragPhase::Start {
                        let side = if d.position.x < d.frame.center().x {
                            CompareSide::First
                        } else {
                            CompareSide::Second
                        };
                        if clicked.get() != side {
                            clicked.set(side);
                        }
                    }
                }),
            );
            cx.on_key(
                node,
                Rc::new(move |key, _| {
                    let side = match key {
                        Key::ArrowLeft => CompareSide::First,
                        Key::ArrowRight => CompareSide::Second,
                        _ => return false,
                    };
                    chosen.set(side);
                    true
                }),
            );
            return node;
        }
        let percent = (at.clamp(0.0, 1.0) * 100.0).round();
        cx.semantics(
            node,
            Semantics::new(Role::Slider).name(name).value(format!("{percent}%")).numeric(
                f64::from(percent),
                0.0,
                100.0,
                Some(1.0),
            ),
        );
        cx.cursor(node, CursorIcon::ColResize);
        let dragged = divider.clone();
        cx.on_drag(
            node,
            Rc::new(move |d| {
                match d.phase {
                    DragPhase::Start => live.set_if_changed(true),
                    DragPhase::End => live.set_if_changed(false),
                    DragPhase::Move => {}
                }
                let share = d.fraction_x();
                if share != dragged.get() {
                    dragged.set(share);
                }
            }),
        );
        let middle = divider.clone();
        cx.on_double_click(node, Rc::new(move || middle.set(0.5)));
        cx.on_key(
            node,
            Rc::new(move |key, modifiers| {
                let step = if modifiers.shift { 0.1 } else { 0.01 };
                let at = divider.get();
                let next = match key {
                    Key::ArrowLeft => at - step,
                    Key::ArrowRight => at + step,
                    Key::Home => 0.0,
                    Key::End => 1.0,
                    _ => return false,
                };
                divider.set(next.clamp(0.0, 1.0));
                true
            }),
        );
        node
    }
}

// ---------------------------------------------------------------------------------------------
// ImageLightbox
// ---------------------------------------------------------------------------------------------

/// One picture of an [`ImageLightbox`].
#[derive(Clone, Debug)]
pub struct LightboxItem {
    image: ImageData,
    caption: Option<String>,
}

impl LightboxItem {
    /// A picture with no caption.
    pub fn new(image: ImageData) -> Self {
        Self { image, caption: None }
    }

    /// What it says under the picture.
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }
}

/// Pictures over the whole app, one at a time, with zoom by [`ZoomPanView`], a caption and a
/// counter. It shows while `open` holds the index of a picture, and Escape or its X set it to
/// `None`.
///
/// The arrows go to the previous and the next, and pan instead once zoomed in, Home and End go
/// to the ends, and the keys of [`ZoomPanView`] zoom. Each picture starts fitted.
///
/// ```
/// use ravenrs_core::{Color, ImageData};
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{ImageLightbox, LightboxItem};
///
/// let open = State::new(None::<usize>);
/// let pictures: Vec<LightboxItem> = ImageData::from_fn(64, 48, |_, _| Color::rgb(0.3, 0.3, 0.3))
///     .map(|image| LightboxItem::new(image).caption("IMG_0001.JPG"))
///     .into_iter()
///     .collect();
/// let lightbox = ImageLightbox::new(open.binding(), pictures);
/// ```
pub struct ImageLightbox {
    open: Binding<Option<usize>>,
    items: Rc<[LightboxItem]>,
    wrap: bool,
    label: Option<String>,
}

impl fmt::Debug for ImageLightbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageLightbox").field("items", &self.items.len()).finish_non_exhaustive()
    }
}

impl ImageLightbox {
    /// A lightbox over these pictures, shown while `open` holds the index of one.
    pub fn new(open: Binding<Option<usize>>, items: impl IntoIterator<Item = LightboxItem>) -> Self {
        Self { open, items: items.into_iter().collect(), wrap: false, label: None }
    }

    /// Whether going past the last picture comes back to the first (it does not by default).
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

struct LightboxLayer<'a> {
    open: &'a Binding<Option<usize>>,
    items: &'a Rc<[LightboxItem]>,
    index: usize,
    wrap: bool,
    label: Option<&'a str>,
}

/// A row across the whole width, floating over the picture, that takes no input of its own.
fn lightbox_row(cx: &mut BuildCx<'_>, y: f32, justify: Justify, children: &[NodeId]) -> NodeId {
    let viewport = cx.viewport();
    let inset = cx.theme().spacing.s3;
    cx.container(
        LayoutStyle {
            absolute: Some((0.0, y)),
            width: Some(viewport.width),
            padding: Insets::symmetric(0.0, inset),
            justify,
            align: Align::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        },
        children,
        Paint::None,
    )
}

/// A chip over the picture: on the surface of the theme, which keeps its words readable.
fn lightbox_chip(cx: &mut BuildCx<'_>, children: &[NodeId]) -> NodeId {
    let theme = *cx.theme();
    let node = cx.container(
        LayoutStyle {
            padding: Insets::symmetric(theme.spacing.s1, theme.spacing.s2),
            spacing: theme.spacing.s2,
            align: Align::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        },
        children,
        Paint::Quad(QuadPaint {
            corner_radius: theme.radii.md,
            ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
        }),
    );
    // A click on it must not reach the picture.
    cx.cursor(node, CursorIcon::Default);
    node
}

impl View for LightboxLayer<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let viewport = cx.viewport();
        let count = self.items.len();
        let index = self.index;
        let item = &self.items[index];
        let view = cx.use_state(ZoomView::fit);
        let seen = cx.use_state(|| Rc::new(Cell::new(index))).get();
        // Each picture starts fitted, however it was reached.
        if seen.replace(index) != index {
            view.set(ZoomView::fit());
        }
        let scrim = {
            let color = theme.color(Semantic::Scrim);
            let node = cx.container(
                LayoutStyle {
                    absolute: Some((0.0, 0.0)),
                    width: Some(viewport.width),
                    height: Some(viewport.height),
                    ..LayoutStyle::leaf()
                },
                &[],
                // The picture is what counts here: the backdrop goes much darker than a dialog's.
                Paint::Quad(QuadPaint::new(color.with_alpha(color.a.max(0.92)))),
            );
            let m = theme.motion;
            cx.motion(node, Motion::new().enter(Effect::hidden(), m.base).exit(Effect::hidden(), m.base));
            node
        };
        let mut stage =
            ZoomPanView::image(view.binding(), item.image.clone()).size(viewport.width, viewport.height);
        stage.defer_arrows = true;
        stage.autofocus = true;
        // Told apart by picture: its memory of a gesture must not carry over to the next one.
        let stage = cx.keyed_child(index as u64, &stage);
        let stage = cx.container(
            LayoutStyle {
                absolute: Some((0.0, 0.0)),
                width: Some(viewport.width),
                height: Some(viewport.height),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[stage],
            Paint::None,
        );
        let open = self.open.clone();
        let wrap = self.wrap;
        let target = move |by: isize| -> Option<usize> {
            let next = index as isize + by;
            if (0..count as isize).contains(&next) {
                Some(next as usize)
            } else if wrap && count > 0 {
                Some(next.rem_euclid(count as isize) as usize)
            } else {
                None
            }
        };
        let go = |by: isize| {
            let open = open.clone();
            Rc::new(move || {
                if let Some(next) = target(by) {
                    open.set(Some(next));
                }
            })
        };
        let (previous, next) = (go(-1), go(1));
        cx.shortcut(Key::ArrowLeft, false, previous.clone());
        cx.shortcut(Key::ArrowRight, false, next.clone());
        let to_first = open.clone();
        cx.shortcut(Key::Home, false, Rc::new(move || to_first.set(Some(0))));
        let to_last = open.clone();
        cx.shortcut(Key::End, false, Rc::new(move || to_last.set(Some(count.saturating_sub(1)))));

        let closing = open.clone();
        let zoom = cx.child(&ZoomControl::new(view.binding()));
        let close = cx.child(
            &IconButton::new(IconName::X)
                .label(labels.close.clone())
                .size(ButtonSize::Small)
                .on_click(move || closing.set(None)),
        );
        let top = lightbox_chip(cx, &[zoom, close]);
        let top = lightbox_row(cx, theme.spacing.s3, Justify::End, &[top]);

        let arrow = |cx: &mut BuildCx<'_>, icon: IconName, name: &str, enabled: bool, run: Rc<dyn Fn()>| {
            let button = IconButton::new(icon)
                .label(name.to_owned())
                .style(ButtonStyle::Secondary)
                .disabled(!enabled)
                .on_click(move || run());
            cx.child(&button)
        };
        let back =
            arrow(cx, IconName::ChevronLeft, &labels.lightbox_previous, target(-1).is_some(), previous);
        let onward = arrow(cx, IconName::ChevronRight, &labels.lightbox_next, target(1).is_some(), next);
        let arrows = lightbox_row(cx, viewport.height / 2.0 - 18.0, Justify::SpaceBetween, &[back, onward]);

        let ink = theme.color(Semantic::Ink);
        let mut words = Vec::with_capacity(2);
        if let Some(caption) = &item.caption {
            words.push(util::label(cx, caption, TextStyle::Callout, None, ink));
        }
        let counter = (labels.lightbox_counter)(index + 1, count);
        words.push(util::label(cx, &counter, TextStyle::Caption, None, theme.color(Semantic::InkMuted)));
        let words = lightbox_chip(cx, &words);
        let bottom = lightbox_row(cx, viewport.height - 44.0, Justify::Center, &[words]);

        let root = cx.container(
            LayoutStyle {
                width: Some(viewport.width),
                height: Some(viewport.height),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[scrim, stage, top, arrows, bottom],
            Paint::None,
        );
        let name = self
            .label
            .map(str::to_owned)
            .or_else(|| item.caption.clone())
            .unwrap_or_else(|| labels.lightbox.clone());
        cx.semantics(root, Semantics::new(Role::Dialog).name(name).value(counter).modal());
        root
    }
}

impl View for ImageLightbox {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let anchor = cx.empty();
        let Some(index) = self.open.get().filter(|i| *i < self.items.len()) else { return anchor };
        let open = self.open.clone();
        cx.overlay(
            anchor,
            Placement::Fill,
            &LightboxLayer {
                open: &self.open,
                items: &self.items,
                index,
                wrap: self.wrap,
                label: self.label.as_deref(),
            },
            Some(Rc::new(move || open.set(None))),
        );
        anchor
    }
}
