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
use ravenrs::widgets::book::{Args, Meta, Story};
use serde_json::{Value, json};

/// The width a story is laid out in, in points: about a form column or a card.
pub const WIDTH: f32 = 760.0;
/// The tallest a story may be, in points.
pub const HEIGHT: f32 = 1100.0;
/// The margin around a story, in points; what is measured starts after it.
pub const MARGIN: f32 = 16.0;

/// One story drawn with its settings, on the canvas of the theme.
struct Stage {
    content: AnyView,
}

impl View for Stage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let inner = cx.child(&self.content);
        let style = LayoutStyle {
            width: Some(WIDTH),
            height: Some(HEIGHT),
            align: Align::Stretch,
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let fill = cx.theme().color(Semantic::Canvas);
        cx.container(style, &[inner], Paint::Quad(QuadPaint::new(fill)))
    }
}

/// The two themes the book measures, by the id the design system gives them.
pub fn themes() -> [(&'static str, Theme); 2] {
    [("dark", Theme::raven_dark()), ("light", Theme::raven_light())]
}

fn round(v: f32) -> f64 {
    (f64::from(v) * 100.0).round() / 100.0
}

fn hex(c: Color) -> String {
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
    let same = |a: Color| [a.r - c.r, a.g - c.g, a.b - c.b].iter().all(|d| d.abs() < 0.006);
    let roles = Semantic::TOKENS.iter().chain(Semantic::CHART_TOKENS.iter());
    for role in roles {
        let color = theme.color(*role);
        if same(color) {
            let alpha = c.a / color.a.max(0.001);
            return if alpha > 0.995 {
                role.token().to_owned()
            } else {
                format!("{}@{:.0}", role.token(), alpha * 100.0)
            };
        }
    }
    hex(c)
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
    /// As JSON.
    pub fn json(&self) -> Value {
        json!({"size": [self.size.0, self.size.1], "parts": self.parts, "paint": self.paint})
    }
}

/// Draws `story` of `meta` with `args` in `theme`, lets its motion settle, and measures it.
pub fn measure(meta: &Meta, story: &Story, args: &Args, theme: Theme) -> Measured {
    let view = meta.render(story, args);
    let content: AnyView = Box::new(VStack::new((view,)).align(Align::Start).padding(MARGIN));
    let mut ui = Ui::new(Stage { content }, theme);
    ui.set_viewport(Size::new(WIDTH, HEIGHT), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui.set_time(5.0);
    ui.update();
    ui.update();
    let tree = ui.access_tree();
    let parts: Vec<Value> = tree.nodes.iter().skip(1).map(node).collect();
    // The first rectangle is the canvas behind the story.
    let paint: Vec<Value> = ui.scene().primitives().iter().skip(1).map(|p| shape(p, &theme)).collect();
    let (mut right, mut bottom) = (0.0_f64, 0.0_f64);
    let mut reach = |r: &Value| {
        if let Some([x, y, w, h]) =
            r.as_array().and_then(|a| a.iter().map(Value::as_f64).collect::<Option<Vec<_>>>()).as_deref()
        {
            if *w < f64::from(WIDTH) {
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
    Measured { size: (right.ceil().max(1.0), bottom.ceil().max(1.0)), parts, paint }
}
