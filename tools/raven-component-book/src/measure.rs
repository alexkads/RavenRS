//! What a story looks like, measured from what RavenRS lays out and paints rather than from a
//! picture of it: the parts of the accessibility tree (role, name, state and frame) and the
//! display list (each rectangle with its corners, fill, border and shadow, each run of text with
//! its font, size, weight, color and the place of every letter, each icon and each path). Colors
//! are named by the theme's tokens, so a design made from them follows the theme.

use std::cell::RefCell;
use std::collections::HashMap;

use ravenrs::core::{AccessNode, Brush, Color, PathElement, PathPaint, Primitive, Rect};
use ravenrs::layout::{Axis, LayoutStyle};
use ravenrs::prelude::*;
use ravenrs::widgets::book::{Args, Extent, Meta, Sizing, Story};
use serde_json::{Value, json};

/// The width a story is laid out in, in points: about a form column or a card. It is the
/// reference: the anatomy, the preview and the size of a story are measured at it.
pub const WIDTH: f32 = 760.0;
/// The widths of the parents a story is also laid out in, to tell what fills from what hugs
/// its content: a phone, the reference and a wide window.
pub const WIDTHS: [f32; 3] = [320.0, WIDTH, 1200.0];
/// The tallest a story may be, in points.
pub const HEIGHT: f32 = 1100.0;
/// The margin around a story, in points; what is measured starts after it.
pub const MARGIN: f32 = 16.0;

/// One story drawn with its settings, on the canvas of the theme.
struct Stage {
    content: AnyView,
    width: f32,
}

impl View for Stage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let inner = cx.child(&self.content);
        let style = LayoutStyle {
            width: Some(self.width),
            height: Some(HEIGHT),
            align: Align::Stretch,
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let fill = cx.theme().color(Semantic::Canvas);
        cx.container(style, &[inner], Paint::Quad(QuadPaint::new(fill)))
    }
}

/// The themes the book measures, by the id the design system gives them: Raven Dark, the
/// reference, Raven Light and Raven Glass.
pub fn themes() -> [(&'static str, Theme); 3] {
    [("dark", Theme::raven_dark()), ("light", Theme::raven_light()), ("glass", Theme::raven_glass())]
}

fn round(v: f32) -> f64 {
    (f64::from(v) * 100.0).round() / 100.0
}

/// A color as `#rrggbb`, or `#rrggbbaa` when it is not opaque.
pub fn hex(c: Color) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b, a) = (byte(c.r), byte(c.g), byte(c.b), byte(c.a));
    if a == 255 { format!("#{r:02x}{g:02x}{b:02x}") } else { format!("#{r:02x}{g:02x}{b:02x}{a:02x}") }
}

/// The token a color is, in `theme`: `accent`, or `accent@13` for the accent at 13%, or the
/// color itself when no token has it.
pub fn token_of(c: Color, theme: &Theme) -> String {
    if c.a <= 0.004 {
        return "transparent".to_owned();
    }
    // A translucent color is stored with fewer steps per channel once blended, so it gets a
    // wider margin: the accent at 18% comes back a few levels off.
    let margin = if c.a < 0.6 { 0.016 } else { 0.006 };
    let same = |a: Color| [a.r - c.r, a.g - c.g, a.b - c.b].iter().all(|d| d.abs() < margin);
    for role in published_roles() {
        let color = theme.color(role);
        if same(color) {
            let alpha = c.a / color.a.max(0.001);
            return if alpha > 0.995 {
                role.token().to_owned()
            } else {
                format!("{}@{:.0}", role.token(), alpha * 100.0)
            };
        }
    }
    mix_of(c, theme).unwrap_or_else(|| hex(c))
}

/// Every color the design system publishes in `tokens.css`, in the order a match is tried: the
/// ones a theme chooses first, so the accent is named `accent` and not `chart-1`.
pub fn published_roles() -> Vec<Semantic> {
    let derived = [Semantic::Scrim, Semantic::Chart1, Semantic::ChartGrid, Semantic::Guide];
    Semantic::TOKENS.iter().chain(Semantic::CHART_TOKENS.iter()).chain(derived.iter()).copied().collect()
}

/// A color between two tokens, as a hover that moves a border toward the ink: `mix(ink, border,
/// 25)` is 25% of `ink` over `border`, mixed in sRGB as CSS `color-mix` does. `None` when no pair
/// of tokens makes it.
fn mix_of(c: Color, theme: &Theme) -> Option<String> {
    if c.a < 0.995 {
        return None;
    }
    let roles = published_roles();
    let channels = |x: Color| [x.r, x.g, x.b];
    let mut best: Option<(f32, String)> = None;
    for a in &roles {
        for b in &roles {
            let (ca, cb) = (theme.color(*a), theme.color(*b));
            if a == b || ca.a < 0.995 || cb.a < 0.995 {
                continue;
            }
            // The share of `a` each channel asks for; they must agree.
            let shares: Vec<f32> = channels(ca)
                .iter()
                .zip(channels(cb))
                .zip(channels(c))
                .filter(|((x, y), _)| (*x - y).abs() > 0.02)
                .map(|((x, y), z)| (z - y) / (x - y))
                .collect();
            let Some(&first) = shares.first() else { continue };
            if !(0.02..=0.98).contains(&first) {
                continue;
            }
            let t = shares.iter().sum::<f32>() / shares.len() as f32;
            let mixed =
                channels(ca).iter().zip(channels(cb)).map(|(x, y)| y + (x - y) * t).collect::<Vec<_>>();
            let error = mixed.iter().zip(channels(c)).map(|(m, z)| (m - z).abs()).fold(0.0_f32, f32::max);
            if error < 0.006 && best.as_ref().is_none_or(|(e, _)| error < *e) {
                best = Some((error, format!("mix({}, {}, {:.0})", a.token(), b.token(), t * 100.0)));
            }
        }
    }
    best.map(|(_, name)| name)
}

fn rect(r: Rect) -> Value {
    json!([round(r.origin.x - MARGIN), round(r.origin.y - MARGIN), round(r.size.width), round(r.size.height)])
}

/// What a font is called and how heavy it is, and which letter each glyph draws.
struct FontFacts {
    family: String,
    weight: u16,
    letters: HashMap<u32, char>,
}

fn font_facts(bytes: &[u8], index: u32) -> FontFacts {
    let Some(font) = swash::FontRef::from_index(bytes, index as usize) else {
        return FontFacts { family: String::new(), weight: 400, letters: HashMap::new() };
    };
    let family = font
        .localized_strings()
        .find_by_id(swash::StringId::Family, None)
        .map(|s| s.to_string())
        .unwrap_or_default();
    // "Inter SemiBold" is Inter at 600, as a design tool names it.
    let family = family.split(' ').next().unwrap_or_default().to_owned();
    let charmap = font.charmap();
    let mut letters = HashMap::new();
    for code in (0x20..0xD800).chain(0xE000..0x10000) {
        let Some(letter) = char::from_u32(code) else { continue };
        let glyph = charmap.map(letter);
        if glyph != 0 {
            letters.entry(u32::from(glyph)).or_insert(letter);
        }
    }
    FontFacts { family, weight: font.attributes().weight().0, letters }
}

thread_local! {
    static FONTS: RefCell<HashMap<u64, FontFacts>> = RefCell::new(HashMap::new());
}

/// The name of the icon a letter of `RavenIcons` draws.
fn icon_named(letter: char) -> Option<&'static str> {
    IconName::ALL.iter().find(|icon| icon.glyph() == letter).map(|icon| icon.name())
}

fn node(n: &AccessNode) -> Value {
    let mut out = json!({
        "role": format!("{:?}", n.role),
        "bounds": rect(n.bounds),
    });
    let mut extra = serde_json::Map::new();
    let mut put = |k: &str, v: Value| {
        if !v.is_null() && v != json!(false) {
            extra.insert(k.to_owned(), v);
        }
    };
    put("name", json!(n.name));
    put("value", json!(n.value));
    put("selected", json!(n.selected));
    put("expanded", json!(n.expanded));
    put("toggled", json!(n.toggled.map(|t| format!("{t:?}"))));
    put("disabled", json!(n.disabled));
    put("focusable", json!(n.focusable));
    put("level", json!(n.level));
    put("test_id", json!(n.test_id));
    if let Some(map) = out.as_object_mut() {
        map.extend(extra);
    }
    out
}

fn shape(p: &Primitive, theme: &Theme) -> Value {
    let color = |c: Color| json!(token_of(c, theme));
    match p {
        Primitive::Quad(q) => {
            let fill = match &q.fill {
                Brush::Solid(c) => color(*c),
                Brush::LinearGradient(g) => json!({
                    "from": token_of(g.from, theme), "to": token_of(g.to, theme),
                    "start": [round(g.start.x - MARGIN), round(g.start.y - MARGIN)],
                    "end": [round(g.end.x - MARGIN), round(g.end.y - MARGIN)],
                }),
                other => json!(format!("{other:?}")),
            };
            let mut out = json!({"kind": "rect", "rect": rect(q.rect), "radius": q.corner_radii.map(round), "fill": fill});
            let mut map = serde_json::Map::new();
            if let Some(b) = q.border {
                map.insert(
                    "border".into(),
                    json!({"width": round(b.width), "color": token_of(b.color, theme)}),
                );
            }
            if let Some(s) = q.shadow {
                map.insert(
                    "shadow".into(),
                    json!({"x": round(s.offset.x), "y": round(s.offset.y), "blur": round(s.blur), "color": token_of(s.color, theme)}),
                );
            }
            if q.rotation != 0.0 {
                map.insert("rotation".into(), json!(round(q.rotation)));
            }
            if let Some(o) = out.as_object_mut() {
                o.extend(map);
            }
            out
        }
        Primitive::Glyphs(run) => FONTS.with(|fonts| {
            let mut fonts = fonts.borrow_mut();
            let facts =
                fonts.entry(run.font.id).or_insert_with(|| font_facts(run.font.bytes(), run.font.index));
            let letters: String =
                run.glyphs.iter().map(|g| facts.letters.get(&g.id).copied().unwrap_or('\u{fffd}')).collect();
            let x: Vec<f64> = run.glyphs.iter().map(|g| round(g.x - MARGIN)).collect();
            let baseline = run.glyphs.first().map_or(0.0, |g| round(g.y - MARGIN));
            if facts.family == "RavenIcons" {
                let name = letters.chars().next().and_then(icon_named);
                return json!({
                    "kind": "icon", "icon": name, "glyph": letters, "size": round(run.size),
                    "x": x.first(), "baseline": baseline, "color": color(run.color),
                });
            }
            json!({
                "kind": "text", "text": letters, "font": facts.family, "weight": facts.weight,
                "size": round(run.size), "color": color(run.color), "x": x, "baseline": baseline,
            })
        }),
        Primitive::Image(i) => {
            json!({"kind": "image", "rect": rect(i.rect), "radius": i.corner_radii.map(round)})
        }
        Primitive::Path(path) => {
            let mut d = String::new();
            let pt = |p: Point| format!("{} {}", round(p.x), round(p.y));
            for element in path.elements.iter() {
                match element {
                    PathElement::MoveTo(p) => d.push_str(&format!("M{}", pt(*p))),
                    PathElement::LineTo(p) => d.push_str(&format!("L{}", pt(*p))),
                    PathElement::CubicTo(a, b, c) => {
                        d.push_str(&format!("C{} {} {}", pt(*a), pt(*b), pt(*c)))
                    }
                    PathElement::Close => d.push('Z'),
                }
            }
            let fill = match &path.paint {
                PathPaint::Solid(c) => color(*c),
                other => json!(format!("{other:?}")),
            };
            let [a, b, c, dd, e, f] = path.transform.map(round);
            json!({
                "kind": "path", "d": d, "rule": format!("{:?}", path.rule).to_lowercase(), "fill": fill,
                // The matrix places the path on screen; the story starts after the margin.
                "transform": [a, b, c, dd, e - f64::from(MARGIN), f - f64::from(MARGIN)],
            })
        }
        Primitive::PushClip(r) => json!({"kind": "clip", "rect": rect(*r)}),
        Primitive::PopClip => json!({"kind": "unclip"}),
        other => json!({"kind": format!("{other:?}").split('(').next().unwrap_or_default()}),
    }
}

/// A story measured in one theme.
#[derive(Clone, Debug)]
pub struct Measured {
    /// The size of what the story draws, in points, from its top left.
    pub size: (f64, f64),
    /// The parts of the accessibility tree, the window left out.
    pub parts: Vec<Value>,
    /// The display list, the canvas left out, in drawing order.
    pub paint: Vec<Value>,
}

impl Measured {
    /// The box a story sizes by: the panel of an open dialog, menu or list, or the column of a
    /// navigation, which float or sit beside the button or content that comes with them; else
    /// everything it draws.
    pub fn main_box(&self) -> (f64, f64) {
        const SURFACES: [&str; 5] = ["Dialog", "Navigation", "Menu", "ListBox", "Tooltip"];
        let surface = self.parts.iter().filter(|p| SURFACES.iter().any(|r| p["role"] == *r)).find_map(|p| {
            let r: Vec<f64> = p["bounds"].as_array()?.iter().filter_map(Value::as_f64).collect();
            (r.len() == 4 && r[2] > 0.0 && r[3] > 0.0).then_some((r[2], r[3]))
        });
        surface.unwrap_or(self.size)
    }

    /// As JSON.
    pub fn json(&self) -> Value {
        json!({"size": [self.size.0, self.size.1], "parts": self.parts, "paint": self.paint})
    }
}

/// Draws `story` of `meta` with `args` in `theme`, lets its motion settle, and measures it, in
/// a parent of the reference [`WIDTH`].
pub fn measure(meta: &Meta, story: &Story, args: &Args, theme: Theme) -> Measured {
    measure_at(meta, story, args, theme, WIDTH)
}

/// [`measure`] in a parent `width` points wide, that leaves its children at their own width.
pub fn measure_at(meta: &Meta, story: &Story, args: &Args, theme: Theme, width: f32) -> Measured {
    measure_in(meta, story, args, theme, width, Align::Start)
}

/// [`measure`] in a parent `width` points wide that stretches its children to its width, as a
/// form column does.
pub fn measure_stretched(meta: &Meta, story: &Story, args: &Args, theme: Theme, width: f32) -> Measured {
    measure_in(meta, story, args, theme, width, Align::Stretch)
}

fn measure_in(meta: &Meta, story: &Story, args: &Args, theme: Theme, width: f32, align: Align) -> Measured {
    // What declares that it fills its parent is given a parent that stretches it.
    let align = if meta.sizing.width == Some(Extent::Fill) { Align::Stretch } else { align };
    let mut ui = staged(meta, story, args, theme, width, align);
    read(&mut ui, &theme, width)
}

/// The story on the stage, its motion settled.
fn staged(meta: &Meta, story: &Story, args: &Args, theme: Theme, width: f32, align: Align) -> Ui {
    let view = meta.render(story, args);
    let content: AnyView = Box::new(VStack::new((view,)).align(align).padding(MARGIN));
    let mut ui = Ui::new(Stage { content, width }, theme);
    ui.set_viewport(Size::new(width, HEIGHT), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui.set_time(5.0);
    ui.update();
    ui.update();
    ui
}

/// What the stage shows now, measured.
fn read(ui: &mut Ui, theme: &Theme, width: f32) -> Measured {
    let tree = ui.access_tree();
    let parts: Vec<Value> = tree.nodes.iter().skip(1).map(node).collect();
    // The first rectangle is the canvas behind the story.
    let paint: Vec<Value> = ui.scene().primitives().iter().skip(1).map(|p| shape(p, theme)).collect();
    let (mut right, mut bottom) = (0.0_f64, 0.0_f64);
    let mut reach = |r: &Value| {
        if let Some([x, y, w, h]) =
            r.as_array().and_then(|a| a.iter().map(Value::as_f64).collect::<Option<Vec<_>>>()).as_deref()
        {
            if *w < f64::from(width) {
                right = right.max(x + w);
            }
            if *h < f64::from(HEIGHT) {
                bottom = bottom.max(y + h);
            }
        }
    };
    for p in &parts {
        reach(&p["bounds"]);
    }
    for p in &paint {
        reach(&p["rect"]);
    }
    // Text and icons are runs of letters, and drawings are paths: they reach as far as they
    // draw, so an icon alone measures its size and not a point.
    for primitive in ui.scene().primitives().iter().skip(1) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let mut take = |x: f32, y: f32| {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        };
        match primitive {
            Primitive::Glyphs(run) => {
                // A lone glyph is an icon, as wide as it is tall; a letter, about six tenths.
                let advance = if run.glyphs.len() == 1 { run.size } else { run.size * 0.6 };
                for g in run.glyphs.iter() {
                    take(g.x, g.y - run.size * 0.8);
                    take(g.x + advance, g.y + run.size * 0.2);
                }
            }
            Primitive::Path(path) => {
                let [a, b, c, d, e, f] = path.transform;
                let mut point = |q: Point| take(a * q.x + c * q.y + e, b * q.x + d * q.y + f);
                for element in path.elements.iter() {
                    match element {
                        PathElement::MoveTo(q) | PathElement::LineTo(q) => point(*q),
                        PathElement::CubicTo(q1, q2, q3) => {
                            point(*q1);
                            point(*q2);
                            point(*q3);
                        }
                        PathElement::Close => {}
                    }
                }
            }
            _ => continue,
        }
        if x1 >= x0 {
            let m = f64::from(MARGIN);
            reach(&json!([f64::from(x0) - m, f64::from(y0) - m, f64::from(x1 - x0), f64::from(y1 - y0)]));
        }
    }
    Measured { size: (right.ceil().max(1.0), bottom.ceil().max(1.0)), parts, paint }
}

/// How a story sizes: what its component declares, and for the rest what its layout does in
/// parents of the [`WIDTHS`], with the size it takes in each.
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
    /// How it takes the width.
    pub width: Extent,
    /// How it takes the height.
    pub height: Extent,
    /// The narrowest it gets, as the component declares it.
    pub min_width: Option<f64>,
    /// How narrow the narrowest parent squeezed it, below what its content asks for.
    pub narrows_to: Option<f64>,
    /// The widest it gets, declared or seen when it stopped growing in the widest parent.
    pub max_width: Option<f64>,
    /// Its height changes with the width, as text or a row that wraps.
    pub reflows: bool,
    /// Hugging its content, it takes the whole width of a parent that stretches its children,
    /// as a form column does.
    pub stretches: bool,
    /// The parent's width and the size taken in it, narrowest first.
    pub sizes: Vec<(f32, (f64, f64))>,
}

impl Fit {
    /// As JSON: `{"width": "fill", "height": "hug", "min_width": …, "max_width": …,
    /// "narrows_to": …, "reflows": false, "sizes": {"320": [w, h], …}}`.
    pub fn json(&self) -> Value {
        let extent = |e: Extent| match e {
            Extent::Fixed(points) => json!({"fixed": round(points)}),
            other => json!(other.word()),
        };
        let sizes: serde_json::Map<String, Value> =
            self.sizes.iter().map(|(w, (sw, sh))| (format!("{w}"), json!([sw, sh]))).collect();
        json!({
            "width": extent(self.width), "height": extent(self.height),
            "min_width": self.min_width, "max_width": self.max_width, "narrows_to": self.narrows_to,
            "reflows": self.reflows, "stretches": self.stretches, "sizes": sizes,
        })
    }

    /// One sentence for a person: how it takes the width, its limits and whether it reflows.
    pub fn sentence(&self) -> String {
        let at = |w: f32| self.sizes.iter().find(|(p, _)| *p == w).map(|(_, s)| *s);
        let parents = WIDTHS.iter().map(|w| format!("{w}")).collect::<Vec<_>>().join(", ");
        let mut t = match self.width {
            Extent::Fill => {
                let widths = self.sizes.iter().map(|(_, (w, _))| format!("{w}")).collect::<Vec<_>>();
                format!(
                    "Fills the width of its parent: {} pt wide in parents of {parents} pt",
                    widths.join(", ")
                )
            }
            Extent::Hug => {
                let (w, h) = at(WIDTH).unwrap_or_default();
                format!("Hugs its content: {w} × {h} pt in a parent of {WIDTH} pt")
            }
            Extent::Fixed(points) => format!("Always {} pt wide", round(points)),
        };
        if let Some(min) = self.min_width {
            t.push_str(&format!("; never narrower than {min} pt"));
        }
        if let Some(narrow) = self.narrows_to {
            t.push_str(&format!("; in a parent of {} pt it narrows to {narrow} pt", WIDTHS[0]));
        }
        if let Some(max) = self.max_width {
            t.push_str(&format!("; never wider than {max} pt"));
        }
        if self.stretches {
            t.push_str(
                "; in a parent that stretches its children, as a form column, it takes the whole width",
            );
        }
        if self.reflows {
            t.push_str("; its height follows the width, as its content wraps");
        }
        t.push('.');
        t
    }
}

/// How `story` of `meta` sizes, laid out with `args` in parents of the [`WIDTHS`] in `theme`.
/// What the component declares in its [`Sizing`] wins over what the layout shows.
pub fn fit(meta: &Meta, story: &Story, args: &Args, theme: Theme) -> Fit {
    let sizes: Vec<(f32, (f64, f64))> =
        WIDTHS.iter().map(|&w| (w, measure_at(meta, story, args, theme, w).main_box())).collect();
    let stretched = measure_stretched(meta, story, args, theme, WIDTH).main_box().0;
    fit_of(&meta.sizing, &sizes, Some(stretched))
}

/// [`fit`] from the sizes already measured, one per parent width, narrowest first, and the
/// width taken in a parent of [`WIDTH`] that stretches its children, when it was measured.
pub fn fit_of(declared: &Sizing, sizes: &[(f32, (f64, f64))], stretched: Option<f64>) -> Fit {
    // A part as wide as the room the stage leaves fills it; a point of slack for rounding.
    let room = |parent: f32| f64::from(parent - 2.0 * MARGIN);
    let fills: Vec<bool> = sizes.iter().map(|(p, (w, _))| (w - room(*p)).abs() <= 1.0).collect();
    let widths: Vec<f64> = sizes.iter().map(|(_, (w, _))| *w).collect();
    let (first, last) = (widths.first().copied().unwrap_or(0.0), widths.last().copied().unwrap_or(0.0));
    let reference = sizes.iter().find(|(p, _)| *p == WIDTH).map_or(first, |(_, (w, _))| *w);
    let (mut width, mut narrows_to, mut max_width) = (Extent::Hug, None, None);
    if fills.iter().all(|f| *f) {
        width = Extent::Fill;
    } else if fills.first() == Some(&true) && last > reference + 1.0 {
        // It grew with the parent and stopped short of the widest one: it fills up to a limit.
        width = Extent::Fill;
        max_width = Some(last);
    }
    if width == Extent::Hug && first + 1.0 < reference {
        // The narrowest parent squeezed it below the size its content asks for.
        narrows_to = Some(first);
    }
    let heights: Vec<f64> = sizes.iter().map(|(_, (_, h))| *h).collect();
    let reflows = heights.iter().any(|h| (h - heights[0]).abs() > 1.0);
    let stretches = width == Extent::Hug
        && stretched.is_some_and(|w| (w - room(WIDTH)).abs() <= 1.0 && w > reference + 1.0);
    Fit {
        width: declared.width.unwrap_or(width),
        height: declared.height.unwrap_or(Extent::Hug),
        min_width: declared.min_width.map(f64::from),
        narrows_to,
        max_width: declared.max_width.map(f64::from).or(max_width),
        reflows,
        stretches: declared.width.is_none() && stretches,
        sizes: sizes.to_vec(),
    }
}

/// A state a person brings a control to, which the book measures without a line in its
/// `ComponentBook`: the pointer over it, the keyboard focus on it, the button held down on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    /// The pointer rests over it.
    Hover,
    /// The keyboard brought the focus to it, with Tab.
    Focus,
    /// The primary button is held down on it.
    Pressed,
}

impl Interaction {
    /// The three, in the order a person meets them.
    pub const ALL: [Self; 3] = [Self::Hover, Self::Focus, Self::Pressed];

    /// Its name, as the book lists it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Hover => "Hover",
            Self::Focus => "Focus",
            Self::Pressed => "Pressed",
        }
    }
}

/// `story` of `meta` brought to `state` in `theme`, in a parent of the reference [`WIDTH`]: the
/// pointer on its first control that is neither selected nor checked (else its first), the focus
/// where Tab takes it. Returns the control's name and what the stage shows then; `None` when the
/// story has no control.
pub fn measure_interaction(
    meta: &Meta,
    story: &Story,
    args: &Args,
    theme: Theme,
    state: Interaction,
) -> Option<(String, Measured)> {
    use ravenrs::core::{Event, Key, Modifiers, PointerButton};
    let mut ui = staged(meta, story, args, theme, WIDTH, Align::Start);
    let tree = ui.access_tree();
    // The pointer goes to a control at rest: one already selected or checked often looks the
    // same under it, and would hide how the others react. The focus goes where Tab takes it.
    let controls: Vec<&AccessNode> =
        tree.nodes.iter().skip(1).filter(|n| n.focusable && !n.disabled).collect();
    let at_rest = controls
        .iter()
        .find(|n| n.selected != Some(true) && !matches!(n.toggled, Some(ravenrs::core::Toggled::True)));
    let target = match state {
        Interaction::Focus => controls.first(),
        _ => at_rest.or(controls.first()),
    }?;
    let name = target.name.clone().unwrap_or_default();
    let b = target.bounds;
    let center = Point::new(b.origin.x + b.size.width / 2.0, b.origin.y + b.size.height / 2.0);
    match state {
        Interaction::Hover => {
            ui.handle_event(&Event::PointerMove { position: center });
        }
        Interaction::Focus => {
            ui.handle_event(&Event::KeyDown { key: Key::Tab, modifiers: Modifiers::default() });
        }
        Interaction::Pressed => {
            ui.handle_event(&Event::PointerMove { position: center });
            ui.handle_event(&Event::PointerDown { position: center, button: PointerButton::Primary });
        }
    }
    // Frame by frame, as a window would, for two seconds: a tween advances by the frames it is
    // given, and a jump of the clock would leave it halfway.
    for frame in 1..=120 {
        ui.set_time(5.0 + f64::from(frame) / 60.0);
        ui.update();
    }
    Some((name, read(&mut ui, &theme, WIDTH)))
}

/// What a state changed in what the stage paints: each shape that took another fill, border,
/// shadow, color or frame, from what to what, and each shape that came or went, as a focus ring.
/// Shapes pair in drawing order when both lists are as long, else by kind and place.
pub fn changes(before: &Measured, after: &Measured) -> Vec<Value> {
    const LOOKS: [&str; 5] = ["fill", "border", "shadow", "color", "rect"];
    let visible = |p: &&Value| p["kind"] != "clip" && p["kind"] != "unclip";
    let (was, now): (Vec<&Value>, Vec<&Value>) =
        (before.paint.iter().filter(visible).collect(), after.paint.iter().filter(visible).collect());
    let place = |p: &Value| json!([p["kind"], p.get("rect").or_else(|| p.get("x")), p.get("text")]);
    let pairs: Vec<(Option<&Value>, Option<&Value>)> = if was.len() == now.len() {
        was.iter().zip(&now).map(|(a, b)| (Some(*a), Some(*b))).collect()
    } else {
        let mut pairs: Vec<(Option<&Value>, Option<&Value>)> =
            now.iter().map(|b| (was.iter().find(|a| place(a) == place(b)).copied(), Some(*b))).collect();
        pairs.extend(
            was.iter().filter(|a| !now.iter().any(|b| place(b) == place(a))).map(|a| (Some(*a), None)),
        );
        pairs
    };
    let mut out = Vec::new();
    for (a, b) in pairs {
        match (a, b) {
            (Some(a), Some(b)) => {
                let (mut from, mut to) = (serde_json::Map::new(), serde_json::Map::new());
                for look in LOOKS {
                    if a.get(look) != b.get(look) {
                        from.insert(look.into(), a.get(look).cloned().unwrap_or(Value::Null));
                        to.insert(look.into(), b.get(look).cloned().unwrap_or(Value::Null));
                    }
                }
                if !to.is_empty() {
                    out.push(json!({"shape": b["kind"], "text": b.get("text"), "from": from, "to": to}));
                }
            }
            (None, Some(b)) => out.push(json!({"added": b})),
            (Some(a), None) => out.push(json!({"removed": a})),
            (None, None) => {}
        }
    }
    out
}

/// The keys the book presses on a focused control to learn which it handles.
pub const KEYS: [&str; 9] =
    ["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight", "Enter", "Space", "Escape", "Home", "End"];

fn key_named(name: &str) -> Option<ravenrs::core::Key> {
    use ravenrs::core::Key;
    Some(match name {
        "ArrowDown" => Key::ArrowDown,
        "ArrowUp" => Key::ArrowUp,
        "ArrowLeft" => Key::ArrowLeft,
        "ArrowRight" => Key::ArrowRight,
        "Enter" => Key::Enter,
        "Space" => Key::Space,
        "Escape" => Key::Escape,
        "Home" => Key::Home,
        "End" => Key::End,
        _ => return None,
    })
}

/// Lets the stage run a second, frame by frame, as a window would.
fn settle(ui: &mut Ui, from: f64) -> f64 {
    for frame in 1..=60 {
        ui.set_time(from + f64::from(frame) / 60.0);
        ui.update();
    }
    from + 1.0
}

/// The keys of [`KEYS`] that change what `story` of `meta` shows once Tab brought the focus to
/// its first control, in `theme`: what the control does with the keyboard, measured rather than
/// declared. A key whose effect the story does not show, as Enter on a button that only calls
/// its `on_click`, is not among them. Empty when the story has no control.
pub fn keys_handled(meta: &Meta, story: &Story, args: &Args, theme: Theme) -> Vec<&'static str> {
    use ravenrs::core::{Event, Key, Modifiers};
    let focused = || {
        let mut ui = staged(meta, story, args, theme, WIDTH, Align::Start);
        let any = ui.access_tree().nodes.iter().skip(1).any(|n| n.focusable && !n.disabled);
        any.then(|| {
            ui.handle_event(&Event::KeyDown { key: Key::Tab, modifiers: Modifiers::default() });
            let clock = settle(&mut ui, 5.0);
            (ui, clock)
        })
    };
    let Some((mut ui, _)) = focused() else { return Vec::new() };
    let rest = read(&mut ui, &theme, WIDTH);
    let press = |name: &str| {
        let (Some((mut ui, clock)), Some(key)) = (focused(), key_named(name)) else { return None };
        ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
        let clock = settle(&mut ui, clock);
        Some((ui, clock))
    };
    // Losing the focus ring is not handling the key: the focus only went away.
    let ring = |c: &Value| c.get("removed").is_some_and(|r| r["border"]["color"] == "focus-ring");
    let mut handled: Vec<&'static str> = KEYS
        .into_iter()
        .filter(|name| {
            press(name).is_some_and(|(mut ui, _)| {
                changes(&rest, &read(&mut ui, &theme, WIDTH)).iter().any(|c| !ring(c))
            })
        })
        .collect();
    // Escape closes what a key opened: a list, a menu or a dialog.
    const OPENED: [&str; 3] = ["ListBox", "Menu", "Dialog"];
    let opens = |ui: &mut Ui| {
        ui.access_tree().nodes.iter().any(|n| OPENED.iter().any(|r| format!("{:?}", n.role) == *r))
    };
    if !handled.contains(&"Escape") {
        let opener =
            handled.iter().copied().find(|name| press(name).is_some_and(|(mut ui, _)| opens(&mut ui)));
        if let Some((mut ui, clock)) = opener.and_then(press) {
            let open = read(&mut ui, &theme, WIDTH);
            ui.handle_event(&Event::KeyDown { key: Key::Escape, modifiers: Modifiers::default() });
            settle(&mut ui, clock);
            if !changes(&open, &read(&mut ui, &theme, WIDTH)).is_empty() {
                handled.push("Escape");
            }
        }
    }
    handled
}
