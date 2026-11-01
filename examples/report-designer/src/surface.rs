//! The design surface: bands stacked as on paper, with their controls placed
//! absolutely. Controls are selected with a click, moved by dragging (also
//! into another band) and resized from the corner handle; bands are resized
//! from their bottom edge.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use ravenrs::core::{Border, ImageQuad};
use ravenrs::layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs::prelude::*;
use ravenrs::style::Palette;
use ravenrs::widgets::{DragPhase, Paint, QuadPaint};
use ravenrs_report::{
    Control, ControlKind, DocumentImage, LineDirection, Report, ShapeKind, Style, TextAlign, decode_picture,
    picture_rect,
};

use crate::bands::BandRef;
use crate::editor::{Editor, Selection, snap};

/// The paper is content, not chrome: it stays light in every theme, so all that
/// is drawn on it comes from Raven Light.
const PAPER: Palette = Theme::raven_light().palette;
const ACCENT: Color = PAPER.accent;
/// What the report engine prints when a control sets no color.
const DOCUMENT_INK: Color = Color::rgba(0.0, 0.0, 0.0, 1.0);

/// Parses `#rrggbb` for the design view.
pub fn color(s: Option<&str>) -> Option<Color> {
    let hex = s?.trim().strip_prefix('#')?;
    let v = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok().map(|b| f32::from(b) / 255.0);
    Some(Color::rgba(v(0)?, v(2)?, v(4)?, if hex.len() >= 8 { v(6)? } else { 1.0 }))
}

/// The style a control shows with: its named style, then its own settings.
pub fn resolved(report: &Report, name: Option<&str>, own: &Style) -> Style {
    let base = name.and_then(|n| report.styles.get(n)).cloned().unwrap_or_default();
    base.merged(own)
}

/// The whole design surface.
pub struct Surface(pub Editor);

impl View for Surface {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.0;
        let theme = *cx.theme();
        let report = e.report.get();
        let zoom = e.zoom.get();
        let width = report.page.content_width() * zoom;
        e.band_frames.borrow_mut().clear();
        let bands: Vec<NodeId> = BandRef::all(&report)
            .into_iter()
            .map(|b| cx.keyed_child(b.key(), &BandView { editor: e.clone(), band: b, width, zoom }))
            .collect();
        let page = cx.container(
            LayoutStyle {
                width: Some(width + 2.0),
                spacing: 0.0,
                align: Align::Start,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &bands,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.sm,
                shadow: Some(theme.elevation.soft),
                ..QuadPaint::new(PAPER.bg_100)
            }),
        );
        cx.container(
            LayoutStyle {
                padding: Insets::uniform(theme.spacing.s6),
                // At least as wide as the pane between the panels, so the page sits in its center.
                min_width: Some(cx.viewport().width - crate::LEFT_WIDTH - crate::PROPERTIES_WIDTH - 2.0),
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[page],
            Paint::None,
        )
    }
}

struct BandView {
    editor: Editor,
    band: BandRef,
    width: f32,
    zoom: f32,
}

impl View for BandView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.editor;
        let report = e.report.get();
        let Some(band) = self.band.get(&report).cloned() else { return cx.empty() };
        let selection = e.selection.get();
        let band_selected = selection == Selection::Band(self.band);
        let zoom = self.zoom;
        let height = (band.height * zoom).max(8.0);

        // Caption strip.
        let caption_color = if band_selected { PAPER.on_accent } else { PAPER.ink_muted };
        let caption = cx.child(
            &HStack::new((
                Icon::new(if matches!(self.band, BandRef::Detail | BandRef::DetailReportDetail(_)) {
                    IconName::Rows3
                } else {
                    IconName::Columns3
                })
                .size(12.0)
                .color(caption_color),
                Text::new(self.band.title(&report)).style(TextStyle::Caption).foreground(caption_color),
                Spacer::new(),
                Text::new(format!("{:.0} pt", band.height))
                    .style(TextStyle::Caption)
                    .foreground(caption_color),
            ))
            .spacing(6.0)
            .align(Align::Center)
            .padding(Insets::symmetric(4.0, 8.0))
            .width(self.width + 2.0)
            .background(if band_selected { ACCENT.into() } else { Fill::from(PAPER.bg_000) }),
        );
        let select_band = {
            let (sel, b) = (e.selection.clone(), self.band);
            Rc::new(move || sel.set(Selection::Band(b)))
        };
        cx.on_click(caption, select_band.clone());
        cx.cursor(caption, CursorIcon::Pointer);

        // Body: a dotted grid that also records where the band is, the
        // controls, and the bottom resize grip.
        let frames = e.band_frames.clone();
        let b = self.band;
        let grid = cx.container(
            LayoutStyle {
                absolute: Some((0.0, 0.0)),
                width: Some(self.width),
                height: Some(height),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
                frames.borrow_mut().push((b, frame));
                let step = 8.0 * zoom;
                let dot = PAPER.border;
                let mut y = frame.y() + step;
                while y < frame.max_y() {
                    let mut x = frame.x() + step;
                    while x < frame.max_x() {
                        scene.push_quad(Quad::new(Rect::new(x, y, 1.0, 1.0), dot));
                        x += step;
                    }
                    y += step;
                }
            })),
        );
        let mut children = vec![grid];
        for (i, c) in band.controls.iter().enumerate() {
            let selected = selection == Selection::Control(self.band, i);
            children.push(cx.keyed_child(
                i as u64,
                &ControlView {
                    editor: e.clone(),
                    band: self.band,
                    index: i,
                    control: c.clone(),
                    zoom,
                    selected,
                    width: self.width,
                },
            ));
            if selected {
                children.push(cx.keyed_child(
                    10_000 + i as u64,
                    &ResizeHandle { editor: e.clone(), band: self.band, index: i, control: c.clone(), zoom },
                ));
            }
        }
        children.push(cx.child(&BandGrip {
            editor: e.clone(),
            band: self.band,
            zoom,
            width: self.width,
            height,
        }));
        let body = cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: Some(height),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::Quad(QuadPaint {
                border: Some(Border { width: 1.0, color: if band_selected { ACCENT } else { PAPER.border } }),
                ..QuadPaint::new(PAPER.bg_100)
            }),
        );
        cx.on_click(body, select_band);
        cx.container(LayoutStyle::stack(Axis::Vertical), &[caption, body], Paint::None)
    }
}

/// The bottom edge of a band: drag to change its height.
struct BandGrip {
    editor: Editor,
    band: BandRef,
    zoom: f32,
    width: f32,
    height: f32,
}

impl View for BandGrip {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let node = cx.container(
            LayoutStyle {
                absolute: Some((0.0, self.height - 5.0)),
                width: Some(self.width),
                height: Some(5.0),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::None,
        );
        let (e, b, zoom) = (self.editor.clone(), self.band, self.zoom);
        let start: Rc<Cell<(f32, f32)>> = Rc::new(Cell::new((0.0, 0.0)));
        cx.on_drag(
            node,
            Rc::new(move |d| match d.phase {
                DragPhase::Start => {
                    let h = e.report.with(|r| b.get(r).map_or(0.0, |band| band.height));
                    start.set((d.position.y, h));
                    e.checkpoint();
                    e.selection.set(Selection::Band(b));
                }
                DragPhase::Move => {
                    let (y0, h0) = start.get();
                    e.apply(|r| {
                        if let Some(band) = b.get_mut(r) {
                            // Never shorter than its controls.
                            let content = band.controls.iter().map(|c| c.y + c.height).fold(0.0, f32::max);
                            band.height = snap(h0 + (d.position.y - y0) / zoom).max(content).max(4.0);
                        }
                    });
                }
                _ => {}
            }),
        );
        cx.cursor(node, CursorIcon::RowResize);
        node
    }
}

struct ControlView {
    editor: Editor,
    band: BandRef,
    index: usize,
    control: Control,
    zoom: f32,
    selected: bool,
    width: f32,
}

/// The text a control shows at design time.
pub fn design_text(c: &Control) -> String {
    match &c.kind {
        ControlKind::Label(l) => match l.expression.as_deref().filter(|s| !s.trim().is_empty()) {
            Some(expr) => {
                let t = expr.trim();
                // A plain field reads as itself; anything else as an expression.
                if t.starts_with('[') && t.ends_with(']') && t.matches('[').count() == 1 {
                    t.to_owned()
                } else {
                    format!("ƒ {t}")
                }
            }
            None => l.text.clone(),
        },
        ControlKind::CheckBox(cb) => format!("☐ {}", cb.text),
        ControlKind::Link(l) => l.label.expression.clone().unwrap_or_else(|| l.label.text.clone()),
        ControlKind::Bookmark(b) => {
            format!("Marcador: {}", b.expression.clone().unwrap_or_else(|| b.text.clone()))
        }
        ControlKind::Chart(c) => {
            format!("Gráfico: {}", if c.title.is_empty() { "sem título" } else { &c.title })
        }
        ControlKind::Barcode(b) => {
            format!("Código: {}", b.expression.clone().unwrap_or_else(|| b.text.clone()))
        }
        ControlKind::Picture(p) => match p.expression.as_deref().filter(|s| !s.trim().is_empty()) {
            Some(field) => format!("Imagem: {field}"),
            None if p.source.trim().is_empty() => "Imagem".to_owned(),
            None => "Imagem inválida".to_owned(),
        },
        _ => String::new(),
    }
}

fn text_view(text: &str, style: &Style, zoom: f32, one_line: bool) -> HStack<(Text,)> {
    let size = style.font_size.unwrap_or(10.0) * zoom;
    let color = color(style.color.as_deref()).unwrap_or(DOCUMENT_INK);
    let mut t = Text::new(text).font_size(size).foreground(color);
    if style.bold == Some(true) {
        t = t.font_weight(Weight::SEMIBOLD);
    }
    if one_line {
        t = t.truncate();
    }
    let justify = match style.text_align {
        Some(TextAlign::Center) => Justify::Center,
        Some(TextAlign::Right) => Justify::End,
        _ => Justify::Start,
    };
    HStack::new((t,))
        .justify(justify)
        .align(Align::Center)
        .padding(Insets::symmetric(0.0, style.padding.unwrap_or(2.0) * zoom))
}

thread_local! {
    /// The image last decoded for each source: the surface rebuilds often.
    static DESIGN_IMAGES: RefCell<HashMap<String, Option<DocumentImage>>> = RefCell::new(HashMap::new());
}

/// A picture's image, decoded once.
fn design_image(source: &str) -> Option<DocumentImage> {
    if source.trim().is_empty() {
        return None;
    }
    DESIGN_IMAGES.with_borrow_mut(|images| {
        if images.len() > 64 {
            images.clear();
        }
        images.entry(source.to_owned()).or_insert_with(|| decode_picture(source)).clone()
    })
}

impl View for ControlView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.editor;
        let c = &self.control;
        let z = self.zoom;
        let report = e.report.get();
        let style = resolved(&report, c.style_name.as_deref(), &c.style);
        let (w, h) = (c.width * z, c.height * z);
        let hovered = cx.is_hover_within();
        let content: Vec<NodeId> = match &c.kind {
            ControlKind::Table(table) => {
                let rows: Vec<NodeId> = table
                    .rows
                    .iter()
                    .enumerate()
                    .map(|(ri, row)| {
                        let cells: Vec<NodeId> = row
                            .cells
                            .iter()
                            .enumerate()
                            .map(|(ci, cell)| {
                                let s = resolved(&report, c.style_name.as_deref(), &c.style)
                                    .merged(&resolved(&report, cell.style_name.as_deref(), &cell.style));
                                let text = design_text(&Control {
                                    kind: ControlKind::Label(cell.label.clone()),
                                    ..Control::default()
                                });
                                let bg = color(s.background.as_deref());
                                let inner =
                                    cx.keyed_child((ri * 100 + ci) as u64, &text_view(&text, &s, z, true));
                                cx.container(
                                    LayoutStyle {
                                        width: Some(cell.width * z),
                                        height: Some(row.height * z),
                                        overflow_hidden: true,
                                        align: Align::Stretch,
                                        justify: Justify::Center,
                                        ..LayoutStyle::stack(Axis::Vertical)
                                    },
                                    &[inner],
                                    Paint::Quad(QuadPaint {
                                        border: Some(Border { width: 0.5, color: PAPER.border }),
                                        ..QuadPaint::new(bg.unwrap_or(Color::TRANSPARENT))
                                    }),
                                )
                            })
                            .collect();
                        cx.container(LayoutStyle::stack(Axis::Horizontal), &cells, Paint::None)
                    })
                    .collect();
                rows
            }
            ControlKind::Line(line) => {
                let ink = color(style.color.as_deref()).unwrap_or(DOCUMENT_INK);
                let lw = line.thickness.unwrap_or(1.0) * z;
                let dir = line.direction;
                vec![cx.container(
                    LayoutStyle { width: Some(w), height: Some(h), ..LayoutStyle::leaf() },
                    &[],
                    Paint::Custom(Rc::new(move |f: Rect, scene: &mut Scene| {
                        let r = match dir {
                            LineDirection::Vertical => {
                                Rect::new(f.center().x - lw / 2.0, f.y(), lw, f.height())
                            }
                            _ => Rect::new(f.x(), f.center().y - lw / 2.0, f.width(), lw),
                        };
                        scene.push_quad(Quad::new(r, ink));
                    })),
                )]
            }
            ControlKind::Picture(picture)
                if picture.expression.as_deref().is_none_or(|s| s.trim().is_empty())
                    && design_image(&picture.source).is_some() =>
            {
                let Some(image) = design_image(&picture.source) else { return cx.child(&Text::new("")) };
                let mode = picture.mode;
                let (cw, ch) = (c.width, c.height);
                vec![cx.container(
                    LayoutStyle { width: Some(w), height: Some(h), ..LayoutStyle::leaf() },
                    &[],
                    Paint::Custom(Rc::new(move |f: Rect, scene: &mut Scene| {
                        // Placed in points, as the engine does, then scaled to the zoom.
                        let placed = picture_rect(mode, Rect::new(0.0, 0.0, cw, ch), &image);
                        let scale = f.width() / cw.max(1.0);
                        let drawn = Rect::new(
                            f.x() + placed.x() * scale,
                            f.y() + placed.y() * scale,
                            placed.width() * scale,
                            placed.height() * scale,
                        );
                        scene.push_clip(f);
                        scene.push_image(ImageQuad::new(drawn, image.pixels().clone()));
                        scene.pop_clip();
                    })),
                )]
            }
            ControlKind::Shape(shape) => {
                let fill = color(style.background.as_deref()).unwrap_or(Color::TRANSPARENT);
                let radius =
                    if shape.shape == ShapeKind::Ellipse { w.min(h) / 2.0 } else { shape.radius * z };
                let border = style.border_width.filter(|w| *w > 0.0).map(|bw| Border {
                    width: bw * z,
                    color: color(style.border_color.as_deref()).unwrap_or(DOCUMENT_INK),
                });
                vec![cx.container(
                    LayoutStyle { width: Some(w), height: Some(h), ..LayoutStyle::leaf() },
                    &[],
                    Paint::Quad(QuadPaint { corner_radius: radius, border, ..QuadPaint::new(fill) }),
                )]
            }
            _ => {
                // Room for less than two lines: one line that ends with an ellipsis.
                let one_line = c.height < style.font_size.unwrap_or(10.0) * 2.4;
                vec![cx.child(&text_view(&design_text(c), &style, z, one_line))]
            }
        };
        let fill = match &c.kind {
            ControlKind::Shape(_) | ControlKind::Line(_) => Color::TRANSPARENT,
            _ => color(style.background.as_deref()).unwrap_or(Color::TRANSPARENT),
        };
        let outline = if self.selected {
            Border { width: 1.5, color: ACCENT }
        } else if hovered {
            Border { width: 1.0, color: ACCENT.with_alpha(0.6) }
        } else {
            Border { width: 0.5, color: PAPER.border }
        };
        let node = cx.container(
            LayoutStyle {
                absolute: Some((c.x * z, c.y * z)),
                width: Some(w),
                height: Some(h.max(2.0)),
                overflow_hidden: true,
                justify: Justify::Center,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &content,
            Paint::Quad(QuadPaint { border: Some(outline), ..QuadPaint::new(fill) }),
        );
        cx.clip(node);
        // Click selects; dragging moves, also into another band.
        let (e2, band, index, zoom) = (e.clone(), self.band, self.index, z);
        let band_width = self.width / z;
        let start: Rc<Cell<Option<(Point, f32, f32)>>> = Rc::new(Cell::new(None));
        cx.on_drag(
            node,
            Rc::new(move |d| match d.phase {
                DragPhase::Start => {
                    let pos = e2
                        .report
                        .with(|r| band.get(r).and_then(|b| b.controls.get(index)).map(|c| (c.x, c.y)));
                    if let Some((x, y)) = pos {
                        start.set(Some((d.position, x, y)));
                        e2.checkpoint();
                        e2.selection.set(Selection::Control(band, index));
                    }
                }
                DragPhase::Move => {
                    if let Some((p0, x0, y0)) = start.get() {
                        let (dx, dy) = ((d.position.x - p0.x) / zoom, (d.position.y - p0.y) / zoom);
                        e2.apply(|r| {
                            if let Some(c) = band.get_mut(r).and_then(|b| b.controls.get_mut(index)) {
                                c.x = snap(x0 + dx).clamp(0.0, (band_width - c.width).max(0.0));
                                // Within the band while moving; a drop elsewhere moves it across.
                                c.y = snap(y0 + dy).clamp(-1000.0, 10_000.0);
                            }
                        });
                    }
                }
                DragPhase::End => {
                    start.set(None);
                    let target = e2.band_at(d.position).filter(|(to, _)| *to != band);
                    let mut moved_to = None;
                    e2.apply(|r| {
                        match target {
                            // Dropped on another band: move it there, under the pointer.
                            Some((to, at)) => {
                                let Some(mut c) = band
                                    .get_mut(r)
                                    .filter(|b| index < b.controls.len())
                                    .map(|b| b.controls.remove(index))
                                else {
                                    return;
                                };
                                c.y = snap((at.y - c.height / 2.0).max(0.0));
                                if let Some(b) = to.get_mut(r) {
                                    b.height = b.height.max(c.y + c.height);
                                    b.controls.push(c);
                                    moved_to = Some((to, b.controls.len() - 1));
                                }
                            }
                            // Dropped in its own band: keep it inside, growing the band if needed.
                            None => {
                                if let Some(b) = band.get_mut(r)
                                    && let Some(c) = b.controls.get_mut(index)
                                {
                                    c.y = c.y.max(0.0);
                                    let bottom = c.y + c.height;
                                    b.height = b.height.max(bottom);
                                }
                            }
                        }
                    });
                    if let Some((to, i)) = moved_to {
                        e2.selection.set(Selection::Control(to, i));
                    }
                }
                _ => {}
            }),
        );
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

/// The corner handle of the selected control: drag to resize.
struct ResizeHandle {
    editor: Editor,
    band: BandRef,
    index: usize,
    control: Control,
    zoom: f32,
}

impl View for ResizeHandle {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let c = &self.control;
        let z = self.zoom;
        let s = 9.0;
        let node = cx.container(
            LayoutStyle {
                absolute: Some(((c.x + c.width) * z - s / 2.0, (c.y + c.height) * z - s / 2.0)),
                width: Some(s),
                height: Some(s),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Quad(QuadPaint {
                corner_radius: 2.0,
                border: Some(Border { width: 1.5, color: PAPER.bg_100 }),
                ..QuadPaint::new(ACCENT)
            }),
        );
        let (e, band, index) = (self.editor.clone(), self.band, self.index);
        let start: Rc<Cell<Option<(Point, f32, f32)>>> = Rc::new(Cell::new(None));
        cx.on_drag(
            node,
            Rc::new(move |d| match d.phase {
                DragPhase::Start => {
                    let size = e.report.with(|r| {
                        band.get(r).and_then(|b| b.controls.get(index)).map(|c| (c.width, c.height))
                    });
                    if let Some((w, h)) = size {
                        start.set(Some((d.position, w, h)));
                        e.checkpoint();
                    }
                }
                DragPhase::Move => {
                    if let Some((p0, w0, h0)) = start.get() {
                        e.apply(|r| {
                            let width = r.page.content_width();
                            if let Some(b) = band.get_mut(r) {
                                if let Some(c) = b.controls.get_mut(index) {
                                    c.width = snap(w0 + (d.position.x - p0.x) / z)
                                        .clamp(4.0, (width - c.x).max(4.0));
                                    c.height = snap(h0 + (d.position.y - p0.y) / z).max(2.0);
                                }
                                let bottom = b.controls.get(index).map_or(0.0, |c| c.y + c.height);
                                b.height = b.height.max(bottom);
                            }
                        });
                    }
                }
                _ => start.set(None),
            }),
        );
        cx.cursor(node, CursorIcon::ColResize);
        node
    }
}
