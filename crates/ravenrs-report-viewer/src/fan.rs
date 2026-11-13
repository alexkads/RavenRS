//! [`PageFan`]: real pages of rendered reports, spread like a hand of cards.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{Color, CursorIcon, Effect, Point, Quad, Rect, Role, Scene, Shadow};
use ravenrs_layout::{Axis, LayoutStyle};
use ravenrs_report::{Document, Page, paint_page};
use ravenrs_style::Semantic;
use ravenrs_text::TextSystem;
use ravenrs_widgets::{
    Badge, BuildCx, Motion, NodeId, Paint, ScrollLink, ScrollLinked, ScrollRange, Semantics, View,
};

/// One page of the fan: what it shows, what it is called and where a click goes.
#[derive(Clone, Debug)]
struct FanPage {
    page: Rc<Page>,
    label: String,
    url: String,
}

/// Pages of rendered reports spread like a hand of cards. They open as the fan
/// scrolls into view; the page under the pointer (or keyboard focus) comes to the front
/// and lifts with an accent ring while the others step back, and a click opens its
/// address through [`ravenrs_widgets::Links`]. Each page is painted once at its size and
/// reused, so scrolling and hovering repaint nothing but positions.
///
/// ```
/// use ravenrs_report::{ReportEngine, samples};
/// use ravenrs_report_viewer::PageFan;
///
/// let invoice = ReportEngine::new().render(&samples::invoice(), &samples::invoice_data());
/// let fan = PageFan::new().page(&invoice, 0, "Invoice", "designer/");
/// ```
#[derive(Clone, Debug)]
pub struct PageFan {
    pages: Vec<FanPage>,
    width: f32,
    spread: f32,
    max_width: Option<f32>,
    hint: String,
}

impl Default for PageFan {
    fn default() -> Self {
        Self::new()
    }
}

impl PageFan {
    /// An empty fan of pages 260 pixels wide.
    pub fn new() -> Self {
        Self {
            pages: Vec::new(),
            width: 260.0,
            spread: 62.0,
            max_width: None,
            hint: "Open in the designer".to_owned(),
        }
    }

    /// Adds page `index` of `document`, called `label`, opening `url`. A page that does
    /// not exist is skipped. Pages are stacked back to front in the order added.
    pub fn page(
        mut self,
        document: &Document,
        index: usize,
        label: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        if let Some(page) = document.pages.get(index) {
            self.pages.push(FanPage { page: Rc::new(page.clone()), label: label.into(), url: url.into() });
        }
        self
    }

    /// The width of each page (260 by default); the height follows its paper.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(40.0);
        self
    }

    /// How far apart neighboring pages open (62 by default).
    pub fn spread(mut self, spread: f32) -> Self {
        self.spread = spread.max(0.0);
        self
    }

    /// The most room the open fan may take across. When pages and spread need more, both
    /// shrink in the same proportion, so the fan keeps its shape and never runs past the
    /// edge of a narrow column.
    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = Some(max_width.max(40.0));
        self
    }

    /// The page width and spread the fan opens with, after [`Self::max_width`].
    fn fitted(&self) -> (f32, f32) {
        let span = self.spread * self.pages.len().saturating_sub(1) as f32;
        let scale = self.max_width.map_or(1.0, |max| (max / (self.width + span)).min(1.0));
        (self.width * scale, self.spread * scale)
    }

    /// The words shown under a page in front, after its name.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    fn height(&self, width: f32) -> f32 {
        self.pages.iter().map(|p| p.page.height / p.page.width.max(1.0) * width).fold(0.0, f32::max)
    }
}

impl View for PageFan {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let count = self.pages.len();
        // The page in front, and whether keyboard focus (not the pointer) put it there.
        let front = cx.use_state(|| None::<(usize, bool)>);
        let lifted = match front.get() {
            Some((i, true)) => Some(i),
            // Leaving the fan with the pointer lets every page go back.
            Some((i, false)) if cx.is_hover_within() => Some(i),
            _ => None,
        };
        let cache = cx.use_state(|| Rc::new(RefCell::new(Pictures::default()))).get();
        let (width, spread) = self.fitted();
        let height = self.height(width);
        let span = spread * count.saturating_sub(1) as f32;
        let total = Rect::new(0.0, 0.0, width + span, height + 64.0);
        let mut order: Vec<usize> = (0..count).filter(|i| Some(*i) != lifted).collect();
        order.extend(lifted);
        let mut children = Vec::with_capacity(count);
        for i in order {
            let page = &self.pages[i];
            let off = i as f32 - (count as f32 - 1.0) / 2.0;
            let card = Card {
                page: page.clone(),
                index: i,
                width,
                height: page.page.height / page.page.width.max(1.0) * width,
                lifted: lifted == Some(i),
                dimmed: lifted.is_some_and(|l| l != i),
                front: front.clone(),
                hint: self.hint.clone(),
                cache: cache.clone(),
            };
            // Closed, the pages sit in a pile in the middle; they open as the fan comes in.
            let closed = Effect::moved(-off * spread, -3.0 * i as f32);
            let link =
                ScrollLink::keyframes([(0.0, closed), (1.0, Effect::IDENTITY)]).range(ScrollRange::Enter);
            let placed = Placed {
                x: span / 2.0 + off * spread,
                y: 24.0 + (off.abs() * 6.0),
                content: ScrollLinked::new(card, link),
            };
            children.push(cx.keyed_child(i as u64, &placed));
        }
        cx.container(
            LayoutStyle {
                width: Some(total.width()),
                height: Some(total.height()),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        )
    }
}

/// A child placed at `(x, y)` inside its parent, out of the flow.
struct Placed<C> {
    x: f32,
    y: f32,
    content: C,
}

impl<C: View> View for Placed<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        cx.container(
            LayoutStyle { absolute: Some((self.x, self.y)), ..LayoutStyle::stack(Axis::Vertical) },
            &[child],
            Paint::None,
        )
    }
}

/// Page pictures painted once, by page and width.
#[derive(Default)]
struct Pictures {
    text: Option<TextSystem>,
    scenes: Vec<(usize, u32, Rc<Scene>)>,
}

impl Pictures {
    fn get(&mut self, index: usize, page: &Page, width: f32) -> Rc<Scene> {
        let key = width.to_bits();
        if let Some((_, _, scene)) = self.scenes.iter().find(|(i, w, _)| *i == index && *w == key) {
            return scene.clone();
        }
        let mut scene = Scene::new();
        paint_page(
            page,
            self.text.get_or_insert_with(TextSystem::new),
            &mut scene,
            Point::ZERO,
            width / page.width.max(1.0),
        );
        let scene = Rc::new(scene);
        self.scenes.push((index, key, scene.clone()));
        scene
    }
}

struct Card {
    page: FanPage,
    index: usize,
    width: f32,
    height: f32,
    lifted: bool,
    dimmed: bool,
    front: ravenrs_reactive::State<Option<(usize, bool)>>,
    hint: String,
    cache: Rc<RefCell<Pictures>>,
}

impl View for Card {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let picture = self.cache.borrow_mut().get(self.index, &self.page.page, self.width);
        let (lifted, dimmed) = (self.lifted, self.dimmed);
        let ring = theme.color(Semantic::Accent);
        let paint = Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
            let start = scene.len();
            scene.extend_from_slice(picture.primitives());
            scene.apply_effect(start..scene.len(), Effect::moved(frame.x(), frame.y()), Point::ZERO);
            if dimmed {
                scene.push_quad(Quad::new(frame, Color::rgba(0.04, 0.05, 0.07, 0.38)));
            }
            if lifted {
                let shadowed = Quad::new(frame, Color::TRANSPARENT).border(2.0, ring).shadow(Shadow {
                    offset: Point::new(0.0, 16.0),
                    blur: 36.0,
                    color: Color::rgba(0.0, 0.0, 0.0, 0.45),
                });
                scene.push_quad(shadowed);
            }
        }));
        let page = cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: Some(self.height),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[],
            paint,
        );
        let mut parts = vec![page];
        if lifted {
            let label = format!("{} · {}", self.page.label, self.hint);
            parts.push(cx.child(&Badge::new(label)));
        }
        let node = cx.container(
            LayoutStyle {
                spacing: 8.0,
                align: ravenrs_layout::Align::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &parts,
            Paint::None,
        );
        let lift = if lifted { Effect::moved(0.0, -22.0).with_scale(1.06) } else { Effect::IDENTITY };
        cx.motion(node, Motion::new().effect(lift, theme.motion.snappy));
        let (links, url) = (cx.links(), self.page.url.clone());
        cx.on_click(node, Rc::new(move || links.open(&url)));
        let (front, index) = (self.front.clone(), self.index);
        cx.on_pointer_move(
            node,
            Rc::new(move |_, _| {
                if front.get() != Some((index, false)) {
                    front.set(Some((index, false)));
                }
            }),
        );
        let by_focus = Some((self.index, true));
        if cx.is_focused() && self.front.get() != by_focus {
            self.front.set(by_focus);
        } else if !cx.is_focused() && self.front.get() == by_focus {
            self.front.set(None);
        }
        cx.semantics(node, Semantics::new(Role::Link).name(format!("{}. {}", self.page.label, self.hint)));
        cx.focusable(node, 10.0);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

#[cfg(test)]
mod tests {
    use ravenrs_report::{ReportEngine, samples};

    use super::*;

    fn fan(pages: usize) -> PageFan {
        let invoice = ReportEngine::new().render(&samples::invoice(), &samples::invoice_data());
        (0..pages).fold(PageFan::new().width(260.0).spread(56.0), |fan, _| {
            fan.page(&invoice, 0, "Invoice", "designer/")
        })
    }

    #[test]
    fn a_fan_that_fits_keeps_its_size() {
        assert_eq!(fan(6).max_width(1000.0).fitted(), (260.0, 56.0));
        assert_eq!(fan(6).fitted(), (260.0, 56.0));
    }

    #[test]
    fn a_fan_too_wide_shrinks_in_proportion() {
        // 260 + 5 × 56 = 540 across, into 270: half of each.
        let (width, spread) = fan(6).max_width(270.0).fitted();
        assert!((width - 130.0).abs() < 1e-4 && (spread - 28.0).abs() < 1e-4);
        assert!(width + 5.0 * spread <= 270.0 + 1e-3);
    }
}
