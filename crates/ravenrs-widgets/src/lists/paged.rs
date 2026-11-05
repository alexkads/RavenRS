//! The pages of a document in a column.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, ImageData, Insets, Key, Role, Size};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use super::virtual_list::{ListProbe, ScrollCommand, VirtualList};
use crate::ViewExt;
use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::imaging::{ZoomControl, ZoomView, zoom_stop};
use crate::view::View;
use crate::{ButtonSize, ContentMode, HStack, IconButton, IconName, Image, Skeleton, Spacer, Text};

/// Room around the pages.
const MARGIN: f32 = 16.0;
/// Gap between pages.
const PAGE_GAP: f32 = 12.0;
const LEAST_ZOOM: f32 = 0.1;
const MOST_ZOOM: f32 = 8.0;

/// A bar with the page, the total and the zoom, over a column of pages of which only those
/// that show are built, so the pictures of the others need not exist. Pictures come from whoever
/// uses it, page by page: until one is ready, the page shows a placeholder of the right size.
/// The fit follows the width of the view. Page Up, Page Down, Home and End move between pages,
/// and `+`, `-` and `0` zoom.
///
/// ```
/// use ravenrs_core::Size;
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{PagedDocumentView, Ui};
///
/// let page = State::new(0_usize);
/// let document = PagedDocumentView::new(120, |_| Size::new(595.0, 842.0), |_| None).page(page.binding());
/// let mut ui = Ui::new(document, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct PagedDocumentView {
    count: usize,
    page_size: Rc<dyn Fn(usize) -> Size>,
    picture: Rc<dyn Fn(usize) -> Option<ImageData>>,
    page: Option<Binding<usize>>,
    zoom: Option<Binding<ZoomView>>,
    height: Option<f32>,
    label: Option<String>,
}

impl fmt::Debug for PagedDocumentView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PagedDocumentView").field("count", &self.count).finish_non_exhaustive()
    }
}

impl PagedDocumentView {
    /// A document of `count` pages, each of the size given by `page_size` at 100%, with the
    /// picture given by `picture` when it shows. Asked only for what shows, on each build, so it
    /// should hand over what is ready and start on what is not.
    pub fn new(
        count: usize,
        page_size: impl Fn(usize) -> Size + 'static,
        picture: impl Fn(usize) -> Option<ImageData> + 'static,
    ) -> Self {
        Self {
            count,
            page_size: Rc::new(page_size),
            picture: Rc::new(picture),
            page: None,
            zoom: None,
            height: None,
            label: None,
        }
    }

    /// The page in view, from 0. It follows the scroll, and setting it goes there.
    pub fn page(mut self, page: Binding<usize>) -> Self {
        self.page = Some(page);
        self
    }

    /// The zoom, to share with other controls. Its `center` and `viewport` are not used.
    pub fn zoom(mut self, zoom: Binding<ZoomView>) -> Self {
        self.zoom = Some(zoom);
        self
    }

    /// A fixed height. It takes the height its parent gives it by default.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// What screen readers call it, instead of the name in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// A sheet with its picture, or the promise of one.
struct Sheet {
    width: f32,
    height: f32,
    picture: Option<ImageData>,
    name: String,
}

impl View for Sheet {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let child = match &self.picture {
            Some(data) => cx.child(
                &Image::new(data.clone())
                    .size(self.width, self.height)
                    .content_mode(ContentMode::Fill)
                    .alt(self.name.clone()),
            ),
            None => cx.child(&Skeleton::block(self.width, self.height)),
        };
        let node = cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: Some(self.height),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[child],
            Paint::Quad(QuadPaint {
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        );
        cx.semantics(node, Semantics::new(Role::Image).name(self.name.clone()));
        node
    }
}

impl View for PagedDocumentView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let local_page: State<usize> = cx.use_state(|| 0);
        let page = self.page.clone().unwrap_or_else(|| local_page.binding());
        let local_zoom: State<ZoomView> = cx.use_state(ZoomView::fit);
        let zoom = self.zoom.clone().unwrap_or_else(|| local_zoom.binding());
        let commands: State<Option<ScrollCommand>> = cx.use_state(|| None);
        let probe = cx.use_state(|| Rc::new(Cell::new(ListProbe::default()))).peek().clone();
        // The page this view last went to or reported, and the offset that went with it.
        let known = cx.use_state(|| Rc::new(Cell::new((0_usize, 0.0_f32)))).peek().clone();

        let count = self.count;
        let widest = (0..count).map(|i| (self.page_size)(i).width).fold(0.0_f32, f32::max).max(1.0);
        // Its own frame: the list inside is built after this, and would be one step behind.
        let room = cx.scroll_viewport().map_or(cx.viewport().width, |size| size.width);
        let current = zoom.get();
        let scale = if current.fit {
            let fit = ((room - 2.0 * MARGIN) / widest).clamp(LEAST_ZOOM, MOST_ZOOM);
            if (current.scale - fit).abs() > 0.0005 {
                zoom.set(ZoomView { scale: fit, ..current });
            }
            fit
        } else {
            current.scale.clamp(LEAST_ZOOM, MOST_ZOOM)
        };

        let wanted = page.get().min(count.saturating_sub(1));
        let (known_page, _) = known.get();
        let jumped = wanted != known_page;
        if jumped {
            commands.set(Some(ScrollCommand::ToIndex(wanted)));
        }

        let (back_to, on_to) = (page.clone(), page.clone());
        let last_page = count.saturating_sub(1);
        let bar = HStack::new((
            IconButton::new(IconName::ChevronUp)
                .label(labels.previous_page.clone())
                .size(ButtonSize::Small)
                .disabled(wanted == 0)
                .on_click(move || back_to.set(wanted.saturating_sub(1))),
            IconButton::new(IconName::ChevronDown)
                .label(labels.next_page.clone())
                .size(ButtonSize::Small)
                .disabled(wanted >= last_page)
                .on_click(move || on_to.set((wanted + 1).min(last_page))),
            Text::new(labels.page_name(wanted + 1, count)).style(TextStyle::Callout),
            Spacer::new(),
            ZoomControl::new(zoom.clone()).range(LEAST_ZOOM, MOST_ZOOM),
        ))
        .spacing(cx.theme().spacing.s3)
        .align(Align::Center)
        .padding(Insets::symmetric(cx.theme().spacing.s2, cx.theme().spacing.s4))
        .background(Semantic::Surface);
        let bar = cx.child(&bar);

        let (size_of, heights, picture, names) =
            (self.page_size.clone(), self.page_size.clone(), self.picture.clone(), labels.clone());
        let list = VirtualList::new(count, move |index| {
            let size = size_of(index);
            Sheet {
                width: size.width * scale,
                height: size.height * scale,
                picture: picture(index),
                name: names.page_name(index + 1, count),
            }
        })
        .row_heights(move |index| heights(index).height * scale)
        .spacing(PAGE_GAP)
        .padding(MARGIN)
        .bare()
        .commands(commands.clone())
        .probe(probe.clone())
        .scroll_across(widest * scale + 2.0 * MARGIN)
        .row_align(Align::Center);
        let column = cx.child(&list);
        let column = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                align: Align::Stretch,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[column],
            Paint::Quad(QuadPaint::new(cx.theme().color(Semantic::Canvas))),
        );

        // Only a scroll speaks for the page: right after a jump, the view is where it was sent.
        let seen = probe.get();
        if jumped {
            known.set((wanted, seen.offset));
        } else if (seen.offset - known.get().1).abs() > 0.5 {
            known.set((seen.current, seen.offset));
            if seen.current != wanted {
                page.set(seen.current);
            }
        }

        let root = cx.container(
            LayoutStyle {
                height: self.height,
                grow: if self.height.is_some() { 0.0 } else { 1.0 },
                min_height: Some(0.0),
                min_width: Some(0.0),
                // Otherwise the column would make it as tall as all its pages.
                overflow_hidden: true,
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[bar, column],
            Paint::None,
        );
        cx.report_frame(root);
        let name = self.label.clone().unwrap_or_else(|| labels.document_name.clone());
        cx.semantics(root, Semantics::new(Role::Group).name(name));
        cx.focusable(root, cx.theme().radii.sm);
        let pinched = zoom.clone();
        cx.on_pinch(
            column,
            Rc::new(move |delta| {
                pinched.update(|v| {
                    v.scale = (v.scale * (1.0 + delta)).clamp(LEAST_ZOOM, MOST_ZOOM);
                    v.fit = false;
                });
            }),
        );
        cx.on_key(
            root,
            Rc::new(move |key, _| {
                let last = count.saturating_sub(1);
                let change = |up: bool| {
                    zoom.update(|v| {
                        v.scale = zoom_stop(v.scale, up).clamp(LEAST_ZOOM, MOST_ZOOM);
                        v.fit = false;
                    });
                };
                match key {
                    Key::PageUp => page.set(wanted.saturating_sub(1)),
                    Key::PageDown => page.set((wanted + 1).min(last)),
                    Key::Home => page.set(0),
                    Key::End => page.set(last),
                    Key::Character(c) if c == "+" || c == "=" => change(true),
                    Key::Character(c) if c == "-" => change(false),
                    Key::Character(c) if c == "0" => zoom.update(|v| v.fit = true),
                    _ => return false,
                }
                true
            }),
        );
        root
    }
}
