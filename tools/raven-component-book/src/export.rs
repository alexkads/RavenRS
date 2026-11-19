//! What the book writes: the JSON of every component (its settings, its stories and how each
//! measures in each theme), an index of them as Storybook's `index.json`, and the files of the
//! design system the design tool reads (`project/` of the Design System type), with a guide, a
//! vector preview and the measurements of every component.

use std::fmt::Write as _;
use std::path::Path;

use ravenrs::widgets::book::{ArgValue, Control};
use serde_json::{Value, json};

use crate::index::{self, Entry};
use crate::measure::{self, Measured};

/// The design system's guide, which the book puts in front of the index of components.
const GUIDE: &str = include_str!("../../../docs/design-system/README.md");
/// The font of RavenRS's icons, so the previews draw them as RavenRS does.
const ICON_FONT: &[u8] = include_bytes!("../../../crates/ravenrs-text/fonts/RavenIcons.ttf");
/// The text faces RavenRS embeds in every build, the same files the previews draw with.
const INTER_REGULAR: &[u8] = include_bytes!("../../../crates/ravenrs-text/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../crates/ravenrs-text/fonts/Inter-SemiBold.ttf");
/// When the design system was made in the design tool: the index keeps it as its marker.
const CREATED_ON: &str = "2026-10-31T22:28:36Z";

/// How big a preview may grow, in bytes, under the 256 KB the design tool reads.
const PREVIEW_BUDGET: usize = 240 * 1024;

/// The radius tokens, by value, as `docs/design-system/tokens.json` names them.
const RADII: [(f64, &str); 4] =
    [(6.0, "radius-sm"), (10.0, "radius-md"), (16.0, "radius-lg"), (24.0, "radius-xl")];
/// The spacing tokens, by value.
const SPACES: [(f64, &str); 8] = [
    (4.0, "space-1"),
    (8.0, "space-2"),
    (12.0, "space-3"),
    (16.0, "space-4"),
    (20.0, "space-5"),
    (24.0, "space-6"),
    (32.0, "space-8"),
    (48.0, "space-12"),
];

fn arg_value(v: &ArgValue) -> Value {
    match v {
        ArgValue::Text(s) | ArgValue::Choice(s) => json!(s),
        ArgValue::Bool(b) => json!(b),
        ArgValue::Number(n) => json!(n),
    }
}

/// The settings of a component, as Storybook's `argTypes`.
fn arg_types(entry: &Entry) -> Value {
    let rows: Vec<Value> = entry
        .meta
        .arg_types
        .iter()
        .map(|a| {
            let control = match &a.control {
                Control::Text => json!({"type": "text"}),
                Control::Bool => json!({"type": "boolean"}),
                Control::Number { min, max, step } => json!({"type": "number", "min": min, "max": max, "step": step}),
                Control::Choice(options) => json!({"type": "select", "options": options}),
            };
            json!({"name": a.name, "description": a.description, "control": control, "default": arg_value(&a.default)})
        })
        .collect();
    json!(rows)
}

/// A story measured in both themes, with its settings.
#[derive(Clone, Debug)]
pub struct MeasuredStory {
    /// The story's name.
    pub name: &'static str,
    /// The settings it was drawn with.
    pub args: Value,
    /// Raven Dark, then Raven Light.
    pub themes: Vec<(&'static str, Measured)>,
}

/// Measures every story of `entry` in both themes.
pub fn measure_entry(entry: &Entry) -> Vec<MeasuredStory> {
    entry
        .meta
        .stories
        .iter()
        .map(|story| {
            let args = entry.meta.args_of(story);
            let themes = measure::themes()
                .into_iter()
                .map(|(id, theme)| (id, measure::measure(&entry.meta, story, &args, theme)))
                .collect();
            let args = json!(
                args.iter().map(|(k, v)| (k.to_owned(), arg_value(v))).collect::<serde_json::Map<_, _>>()
            );
            MeasuredStory { name: story.name, args, themes }
        })
        .collect()
}

fn id_of(title: &str) -> String {
    title.to_lowercase().replace(' ', "-").replace('/', "--")
}

/// The JSON of one component: where it sits, its settings, its API, the code of its example and
/// every story measured in both themes, with the anatomy of the first.
pub fn component_json(entry: &Entry, stories: &[MeasuredStory]) -> Value {
    let variants: serde_json::Map<String, Value> =
        entry.variants().into_iter().map(|(e, v)| (e.to_owned(), json!(v))).collect();
    let anatomy = anatomy_of(stories);
    json!({
        "id": id_of(entry.meta.title),
        "title": entry.meta.title,
        "component": entry.name(),
        "family": entry.meta.family(),
        "summary": entry.meta.summary,
        "pending": entry.pending,
        "argTypes": arg_types(entry),
        "api": {"methods": entry.methods(), "variants": variants},
        "code": entry.code,
        "anatomy": anatomy,
        "stories": stories.iter().map(|s| json!({
            "id": format!("{}--{}", id_of(entry.meta.title), id_of(s.name)),
            "name": s.name,
            "args": s.args,
            "themes": s.themes.iter().map(|(id, m)| (id.to_string(), m.json())).collect::<serde_json::Map<_, _>>(),
        })).collect::<Vec<_>>(),
    })
}

fn rect_of(v: &Value) -> Option<[f64; 4]> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?, a.get(3)?.as_f64()?])
}

fn named(value: f64, table: &[(f64, &'static str)]) -> Value {
    table.iter().find(|(v, _)| (v - value).abs() < 0.01).map_or_else(|| json!(value), |(_, n)| json!(n))
}

/// The measurements of each part a person sees as one piece (a button, a field, a tab): its
/// size, the rectangle drawn for it (corners, fill, border, shadow), its text and icon, and the
/// space from its edge to its text. Parts that measure the same are listed once.
pub fn anatomy(m: &Measured) -> Vec<Value> {
    const PIECES: [&str; 18] = [
        "Button",
        "Link",
        "CheckBox",
        "Switch",
        "RadioButton",
        "TextInput",
        "MultilineTextInput",
        "Slider",
        "SpinButton",
        "ComboBox",
        "ListBoxOption",
        "MenuItem",
        "Tab",
        "Alert",
        "Status",
        "ProgressIndicator",
        "Image",
        "Row",
    ];
    let mut rows: Vec<Value> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for part in &m.parts {
        let role = part["role"].as_str().unwrap_or_default();
        let Some(bounds) = rect_of(&part["bounds"]) else { continue };
        if !PIECES.contains(&role) && part.get("test_id").is_none() {
            continue;
        }
        let box_ =
            m.paint.iter().filter(|p| p["kind"] == "rect").find(|p| {
                rect_of(&p["rect"]).is_some_and(|r| (0..4).all(|i| (r[i] - bounds[i]).abs() < 1.0))
            });
        let text = m.paint.iter().find(|p| {
            p["kind"] == "text"
                && p["x"]
                    .get(0)
                    .and_then(Value::as_f64)
                    .is_some_and(|x| x >= bounds[0] - 0.5 && x <= bounds[0] + bounds[2])
                && p["baseline"].as_f64().is_some_and(|y| y >= bounds[1] && y <= bounds[1] + bounds[3])
        });
        let icon = m.paint.iter().find(|p| {
            p["kind"] == "icon"
                && p["x"].as_f64().is_some_and(|x| x >= bounds[0] - 0.5 && x <= bounds[0] + bounds[2])
                && p["baseline"].as_f64().is_some_and(|y| y >= bounds[1] && y <= bounds[1] + bounds[3] + 4.0)
        });
        let mut row = serde_json::Map::new();
        row.insert("part".into(), json!(role));
        if let Some(name) = part.get("name") {
            row.insert("name".into(), name.clone());
        }
        row.insert("width".into(), json!(bounds[2]));
        row.insert("height".into(), json!(bounds[3]));
        if let Some(b) = box_ {
            let radius = b["radius"].get(0).and_then(Value::as_f64).unwrap_or(0.0);
            let radius = if radius * 2.0 >= bounds[3] - 0.5 && radius > 0.0 {
                json!("radius-full")
            } else {
                named(radius, &RADII)
            };
            row.insert("radius".into(), radius);
            row.insert("fill".into(), b["fill"].clone());
            for key in ["border", "shadow"] {
                if let Some(v) = b.get(key) {
                    row.insert(key.into(), v.clone());
                }
            }
        }
        let start = icon.and_then(|i| i["x"].as_f64()).or_else(|| text.and_then(|t| t["x"][0].as_f64()));
        if let Some(x) = start {
            row.insert("padding_left".into(), named((x - bounds[0]).round(), &SPACES));
        }
        if let Some(t) = text {
            row.insert(
                "text".into(),
                json!({"font": t["font"], "weight": t["weight"], "size": t["size"], "color": t["color"], "baseline_from_top": ((t["baseline"].as_f64().unwrap_or(0.0) - bounds[1]) * 100.0).round() / 100.0}),
            );
        }
        if let Some(i) = icon {
            row.insert("icon".into(), json!({"name": i["icon"], "size": i["size"], "color": i["color"]}));
        }
        let mut key = Value::Object(row.clone());
        if let Some(o) = key.as_object_mut() {
            o.remove("name");
        }
        let key = key.to_string();
        if !seen.contains(&key) {
            seen.push(key);
            rows.push(Value::Object(row));
        }
    }
    rows
}

/// The anatomy of every story in Raven Dark, each part that measures differently once, with the
/// story it comes from.
pub fn anatomy_of(stories: &[MeasuredStory]) -> Vec<Value> {
    let mut rows: Vec<Value> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for story in stories {
        let Some((_, m)) = story.themes.first() else { continue };
        for mut row in anatomy(m) {
            let mut key = row.clone();
            if let Some(o) = key.as_object_mut() {
                o.remove("name");
                o.remove("width");
            }
            let key = key.to_string();
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            row["story"] = json!(story.name);
            rows.push(row);
        }
    }
    rows
}

/// A color of the measurements as CSS: a token as its variable, a token at a share of its
/// opacity mixed with transparent, any other color as it is.
fn css_color(v: &Value) -> String {
    let Some(name) = v.as_str() else { return "none".to_owned() };
    if name == "transparent" {
        return "none".to_owned();
    }
    if name.starts_with('#') {
        return name.to_owned();
    }
    match name.split_once('@') {
        Some((token, share)) => format!("color-mix(in srgb, var(--{token}) {share}%, transparent)"),
        None => format!("var(--{name})"),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The story drawn again as SVG from its measurements: every rectangle, letter, icon and path
/// where RavenRS put it, its colors by token, so the preview follows the theme of the page.
pub fn svg(m: &Measured) -> String {
    let (w, h) = m.size;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" style=\"overflow:visible;display:block\">"
    );
    let mut defs = String::new();
    let mut open = 0usize;
    let mut clip = 0usize;
    for p in &m.paint {
        match p["kind"].as_str().unwrap_or_default() {
            "rect" => {
                let Some([x, y, rw, rh]) = rect_of(&p["rect"]) else { continue };
                let r = p["radius"].get(0).and_then(Value::as_f64).unwrap_or(0.0).min(rw / 2.0).min(rh / 2.0);
                let mut style = format!("fill:{}", css_color(&p["fill"]));
                if p["fill"].is_object() {
                    style = format!("fill:{}", css_color(&p["fill"]["from"]));
                }
                if let Some(b) = p.get("border") {
                    let _ = write!(style, ";stroke:{};stroke-width:{}", css_color(&b["color"]), b["width"]);
                }
                if let Some(s) = p.get("shadow") {
                    let _ = write!(
                        style,
                        ";filter:drop-shadow({}px {}px {}px {})",
                        s["x"],
                        s["y"],
                        s["blur"].as_f64().unwrap_or(0.0) / 2.0,
                        css_color(&s["color"])
                    );
                }
                let inset = p.get("border").and_then(|b| b["width"].as_f64()).unwrap_or(0.0) / 2.0;
                let _ = write!(
                    out,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" style=\"{style}\"/>",
                    x + inset,
                    y + inset,
                    (rw - inset * 2.0).max(0.0),
                    (rh - inset * 2.0).max(0.0),
                    (r - inset).max(0.0)
                );
            }
            "text" => {
                let xs: Vec<String> =
                    p["x"].as_array().map_or_else(Vec::new, |a| a.iter().map(Value::to_string).collect());
                let _ = write!(
                    out,
                    "<text x=\"{}\" y=\"{}\" style=\"font-family:{},sans-serif;font-weight:{};font-size:{}px;fill:{};white-space:pre\">{}</text>",
                    xs.join(" "),
                    p["baseline"],
                    p["font"].as_str().unwrap_or("Inter"),
                    p["weight"],
                    p["size"],
                    css_color(&p["color"]),
                    escape(p["text"].as_str().unwrap_or_default())
                );
            }
            "icon" => {
                let _ = write!(
                    out,
                    "<text x=\"{}\" y=\"{}\" style=\"font-family:RavenIcons;font-size:{}px;fill:{}\">{}</text>",
                    p["x"],
                    p["baseline"],
                    p["size"],
                    css_color(&p["color"]),
                    escape(p["glyph"].as_str().unwrap_or_default())
                );
            }
            "path" => {
                let t: Vec<String> = p["transform"]
                    .as_array()
                    .map_or_else(Vec::new, |a| a.iter().map(Value::to_string).collect());
                let _ = write!(
                    out,
                    "<path d=\"{}\" transform=\"matrix({})\" style=\"fill:{};fill-rule:{}\"/>",
                    p["d"].as_str().unwrap_or_default(),
                    t.join(" "),
                    css_color(&p["fill"]),
                    if p["rule"] == "evenodd" { "evenodd" } else { "nonzero" }
                );
            }
            "image" => {
                let Some([x, y, rw, rh]) = rect_of(&p["rect"]) else { continue };
                let _ = write!(
                    out,
                    "<rect x=\"{x}\" y=\"{y}\" width=\"{rw}\" height=\"{rh}\" style=\"fill:var(--bg-300)\"/>"
                );
            }
            "clip" => {
                let Some([x, y, rw, rh]) = rect_of(&p["rect"]) else { continue };
                clip += 1;
                let _ = write!(
                    defs,
                    "<clipPath id=\"c{clip}\"><rect x=\"{x}\" y=\"{y}\" width=\"{rw}\" height=\"{rh}\"/></clipPath>"
                );
                let _ = write!(out, "<g clip-path=\"url(#c{clip})\">");
                open += 1;
            }
            "unclip" if open > 0 => {
                out.push_str("</g>");
                open -= 1;
            }
            _ => {}
        }
    }
    out.push_str(&"</g>".repeat(open));
    if !defs.is_empty() {
        out = out.replacen(
            "style=\"overflow:visible;display:block\">",
            &format!("style=\"overflow:visible;display:block\"><defs>{defs}</defs>"),
            1,
        );
    }
    out.push_str("</svg>");
    out
}

/// Writes `<dir>/<Component>.json` for every component and `<dir>/index.json`, as Storybook's.
pub fn write_book(dir: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(dir)?;
    let mut index = Vec::new();
    let entries = index::entries();
    for entry in &entries {
        let stories = measure_entry(entry);
        let value = component_json(entry, &stories);
        std::fs::write(dir.join(format!("{}.json", entry.name())), serde_json::to_string_pretty(&value)?)?;
        for s in &stories {
            index.push(json!({
                "id": format!("{}--{}", id_of(entry.meta.title), id_of(s.name)),
                "title": entry.meta.title, "name": s.name, "component": entry.name(), "pending": entry.pending,
            }));
        }
    }
    std::fs::write(
        dir.join("index.json"),
        serde_json::to_string_pretty(&json!({"v": 1, "entries": index}))?,
    )?;
    Ok(entries.len())
}

fn guide_with_index(entries: &[Entry]) -> String {
    let mut text = String::new();
    // The guide speaks of the components by families; the book lists every one below.
    let guide = GUIDE.split("\n## Components\n").collect::<Vec<_>>();
    let rest = guide.get(1).map_or("", |r| r.split_once("\n## ").map_or("", |(_, tail)| tail));
    text.push_str(guide.first().copied().unwrap_or_default().trim_end());
    text.push_str(
        "\n\n## Designing with this system\n\n\
         Every screen designed with this system must be buildable with RavenRS as it is. So:\n\n\
         - **Use only the components listed below**, by their names. Each one has a card with its settings, its stories and its anatomy: the sizes, corners, fills, borders, fonts and inner spaces RavenRS draws, measured from the library itself by RavenComponentBook. Draw them with those numbers.\n\
         - **Change a component only through its settings** (the `argTypes` of its card). A variant, size or state that is not there does not exist.\n\
         - **Compose, do not invent.** A piece the screen needs that no component gives goes on a note called *Missing from the library* on the artboard, with what it should do. It joins the library before the screen is built.\n\
         - Colors are tokens (`var(--accent)`), never new colors; spacing follows the 4px grid (`space-*`); corners follow `radius-*`.\n\
         - Icons are RavenRS icons, by the names in the cards (`trash-2`, `search`); the `RavenIcons` font draws them.\n\n\
         ## Changing this system\n\n\
         RavenComponentBook writes every file of this system from the RavenRS code, and the design tool takes them as they are: nothing here is edited by hand. To change a component, change its stories (`ComponentBook`) or the component itself; to change a color, a size, a corner or a font, change the tokens of `ravenrs-style`. Then RavenComponentBook writes the system again.\n\n\
         ## Components\n\n\
         | Family | Component | What it is | Stories |\n|---|---|---|---|\n",
    );
    for e in entries {
        let _ = writeln!(
            text,
            "| {} | `{}` | {} | {} |",
            e.meta.family(),
            e.name(),
            e.meta.summary.trim_end_matches('.'),
            if e.pending { "its example (pending)".to_owned() } else { e.meta.stories.len().to_string() }
        );
    }
    text.push_str(
        "\nCharts take their series from `chart-1` to `chart-5`; alignment guides take `guide`.\n\n## ",
    );
    text.push_str(rest);
    text.replace("`raven-style`", "`ravenrs-style`")
}

fn card_readme(entry: &Entry, stories: &[MeasuredStory]) -> String {
    let mut t = format!("# {}\n\n{}\n\n", entry.name(), entry.meta.summary);
    let _ =
        writeln!(t, "**Family:** {} · **Rust:** `ravenrs::prelude::{}`", entry.meta.family(), entry.name());
    if entry.pending {
        t.push_str("\nIts stories are pending: the card shows its example in the component gallery, measured the same way.\n");
    }
    if !entry.meta.arg_types.is_empty() {
        t.push_str("\n## Settings\n\n| Setting | Control | Default | What it does |\n|---|---|---|---|\n");
        for a in &entry.meta.arg_types {
            let control = match &a.control {
                Control::Text => "text".to_owned(),
                Control::Bool => "on/off".to_owned(),
                Control::Number { min, max, step } => format!("number {min}–{max} by {step}"),
                Control::Choice(o) => o.join(" · "),
            };
            let _ =
                writeln!(t, "| `{}` | {} | `{}` | {} |", a.name, control, a.default.to_text(), a.description);
        }
    }
    t.push_str("\n## Stories\n\n");
    for s in stories {
        let size = s.themes.first().map_or((0.0, 0.0), |(_, m)| m.size);
        let _ = writeln!(t, "- **{}**: {} × {} pt, settings `{}`", s.name, size.0, size.1, s.args);
    }
    let rows = anatomy_of(stories);
    if !rows.is_empty() {
        t.push_str("\n## Anatomy (Raven Dark, every story)\n\nMeasured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.\n\n| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |\n|---|---|---|---|---|---|---|---|---|\n");
        for r in rows.iter().take(60) {
            let s = |v: &Value| {
                v.as_str()
                    .map_or_else(|| if v.is_null() { "—".to_owned() } else { v.to_string() }, str::to_owned)
            };
            let part = match r.get("name").and_then(Value::as_str) {
                Some(name) => format!("{} \"{}\"", s(&r["part"]), name),
                None => s(&r["part"]),
            };
            let border = r
                .get("border")
                .map_or_else(|| "—".to_owned(), |b| format!("{} {}", b["width"], s(&b["color"])));
            let text = r.get("text").map_or_else(
                || "—".to_owned(),
                |x| format!("{} {} {}px {}", s(&x["font"]), x["weight"], x["size"], s(&x["color"])),
            );
            let icon = r.get("icon").map_or_else(
                || "—".to_owned(),
                |i| format!("{} {}px {}", s(&i["name"]), i["size"], s(&i["color"])),
            );
            let _ = writeln!(
                t,
                "| {} | {part} | {} × {} | {} | {} | {border} | {text} | {icon} | {} |",
                s(&r["story"]),
                r["width"],
                r["height"],
                r.get("radius").map_or_else(|| "—".to_owned(), s),
                r.get("fill").map_or_else(|| "—".to_owned(), s),
                r.get("padding_left").map_or_else(|| "—".to_owned(), s),
            );
        }
    }
    let methods = entry.methods();
    if !methods.is_empty() {
        let _ = write!(
            t,
            "\n## API\n\nBuilder methods: {}.\n",
            methods.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", ")
        );
        for (e, v) in entry.variants() {
            let _ = writeln!(
                t,
                "\n`{e}`: {}.",
                v.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", ")
            );
        }
    }
    if let Some(code) = &entry.code {
        let _ = write!(t, "\n## Example\n\n```rust\n{code}\n```\n");
    }
    let _ = write!(t, "\nEvery number above is in `{}.json` beside this card.\n", entry.name());
    t
}

fn preview(entry: &Entry, stories: &[MeasuredStory]) -> String {
    let height: f64 =
        stories.iter().map(|s| s.themes.first().map_or(0.0, |(_, m)| m.size.1) + 44.0).sum::<f64>() + 24.0;
    let mut t = format!(
        "<!-- @dsCard group=\"{}\" height={} subtitle=\"{}\" -->\n",
        entry.meta.family(),
        height.clamp(60.0, 4000.0).round(),
        escape(&stories.iter().map(|s| s.name).collect::<Vec<_>>().join(", "))
    );
    t.push_str(
        "<style>\n@font-face { font-family: RavenIcons; src: url(../../fonts/RavenIcons.ttf); }\n\
         body { margin: 0; background: var(--bg-000); }\n\
         .book { display: flex; flex-direction: column; gap: 20px; padding: 16px; font-family: var(--font-sans); }\n\
         .more { margin: 0; font: 400 13px/18px var(--font-sans); color: var(--ink-muted); }\n\
         .story h3 { margin: 0 0 8px; font: 600 12px/16px var(--font-sans); letter-spacing: .4px; text-transform: uppercase; color: var(--ink-muted); }\n\
         </style>\n<div class=\"book\">\n",
    );
    // The design tool reads a preview of at most 256 KB: stories go in while they fit, the first
    // always, and the rest are named, since the JSON beside the card has every one of them.
    let mut left_out = Vec::new();
    for (i, s) in stories.iter().enumerate() {
        if let Some((_, m)) = s.themes.first() {
            let section =
                format!("<section class=\"story\"><h3>{}</h3>{}</section>\n", escape(s.name), svg(m));
            if i > 0 && t.len() + section.len() > PREVIEW_BUDGET {
                left_out.push(escape(s.name));
                continue;
            }
            t.push_str(&section);
        }
    }
    if !left_out.is_empty() {
        let _ = writeln!(t, "<p class=\"more\">Also in {}.json: {}.</p>", entry.name(), left_out.join(", "));
    }
    t.push_str("</div>\n<script></script>\n");
    t
}

/// What differs between a copy of the published design system (its `project/` folder, as the
/// design tool serves it) and what RavenComponentBook writes now: files that changed, that are
/// missing and that should not be there. The design tool's own files (`api/`, `manifest.json`)
/// and the time and commit of the index's `lastChange` do not count.
#[derive(Debug, Default)]
pub struct Drift {
    /// Published, with other content.
    pub changed: Vec<String>,
    /// Written by RavenComponentBook, not published.
    pub missing: Vec<String>,
    /// Published, not written by RavenComponentBook.
    pub extra: Vec<String>,
}

impl Drift {
    /// Nothing differs.
    pub fn is_clean(&self) -> bool {
        self.changed.is_empty() && self.missing.is_empty() && self.extra.is_empty()
    }
}

/// A file as it compares: the index without the time and commit of its last change, the
/// tokens without the commit they came from.
fn comparable(path: &str, bytes: Vec<u8>) -> Vec<u8> {
    let mut json: Value = match path {
        "project/design-system.json" | "project/tokens.json" => {
            serde_json::from_slice(&bytes).unwrap_or_default()
        }
        _ => return bytes,
    };
    if let Some(change) = json["lastChange"].as_object_mut() {
        change.remove("at");
        change.remove("via");
    }
    if let Some(meta) = json["meta"].as_object_mut() {
        meta.remove("ref");
    }
    serde_json::to_vec(&json).unwrap_or_default()
}

/// Compares `published` (a folder holding `project/`) with what RavenComponentBook writes now.
pub fn verify(published: &Path) -> std::io::Result<Drift> {
    let fresh = std::env::temp_dir().join(format!("raven-component-book-verify-{}", std::process::id()));
    let written = write_claude_design(&fresh, "verify")?;
    let mut drift = Drift::default();
    for path in &written {
        match std::fs::read(published.join(path)) {
            Ok(bytes) => {
                let new = std::fs::read(fresh.join(path))?;
                if comparable(path, bytes) != comparable(path, new) {
                    drift.changed.push(path.clone());
                }
            }
            Err(_) => drift.missing.push(path.clone()),
        }
    }
    let mut stack = vec![published.join("project")];
    while let Some(dir) = stack.pop() {
        for item in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = item.path();
            let rel = path
                .strip_prefix(published)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if rel.starts_with("project/api/") || rel == "project/manifest.json" {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if !written.contains(&rel) {
                drift.extra.push(rel);
            }
        }
    }
    std::fs::remove_dir_all(&fresh).ok();
    drift.extra.sort();
    Ok(drift)
}

/// The time now, as ISO-8601 in UTC, for the index's `lastChange`.
fn now_utc() -> String {
    let secs =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

/// The index of the design system: its name, the marker of the day it was made, and
/// `"editing": "code"`, which keeps the design tool's page from changing what RavenComponentBook
/// writes. A change goes through a component's `ComponentBook` or the tokens of `ravenrs-style`.
fn design_system_index(sha: &str, components: usize) -> Value {
    json!({
        "v": 3,
        "layout": "files",
        "createdOnFiles": {"v": 1, "at": CREATED_ON},
        "editing": "code",
        "title": "RavenRS",
        "namespace": "RavenRS",
        "libraries": [],
        "sections": {},
        "groups": [],
        "assetGroups": {},
        "blobs": {},
        "docs": {"sections": []},
        "lastChange": {
            "by": "RavenComponentBook",
            "at": now_utc(),
            "via": format!("RavenComponentBook · alexkads/RavenRS@{sha}"),
            "note": format!("Written whole by raven-component-book --claude-design: {components} components and the tokens of ravenrs-style."),
        },
    })
}

/// The cover, drawn from the tokens: the palette as blocks and the tiles of a dense interface cut
/// by the radius tokens, right of the name, as the design tool's cover asks.
fn cover() -> String {
    // Blocks, tiles and gaps on the spacing scale; corners from the radius scale.
    let tiles = [
        ("b-panel r-md", 240, 24, 120, 48),
        ("b-panel r-md", 384, 24, 72, 48),
        ("b-panel r-md", 240, 96, 48, 48),
        ("b-panel r-md", 312, 96, 48, 48),
        ("b-panel r-md", 384, 96, 48, 48),
        ("b-panel r-md", 240, 168, 48, 48),
        ("b-accent r-md", 312, 168, 48, 48),
        ("b-panel r-md", 384, 168, 48, 48),
    ];
    let mut svg = String::from(
        "  <!--\n    blocks: bg-200 slab 288x276 (radius-xl), bg-300 168x120 (radius-xl), accent 168x108 (radius-xl), accent-hover pill 120x24, ink pill 72x24\n    arrangement: one tall slab bleeding off the top with satellites to its left, like a window with a sidebar and a primary button\n    pattern: geometric, modular, dense UI (a GPU-drawn component kit): tiles cut by radius-md on the slab, one merged wide like a toolbar\n    scales: pitch 48 (space-12) and gap 24 (space-6); radii radius-xl, radius-md, pills at half their height\n  -->\n  <svg width=\"480\" height=\"300\" viewBox=\"0 0 480 300\" aria-hidden=\"true\">\n    <rect class=\"b-canvas r-xl\" x=\"216\" y=\"-24\" width=\"288\" height=\"276\"/>\n    <rect class=\"b-panel r-xl\" x=\"24\" y=\"24\" width=\"168\" height=\"120\"/>\n    <rect class=\"b-accent r-xl\" x=\"24\" y=\"168\" width=\"168\" height=\"108\"/>\n",
    );
    for (class, x, y, w, h) in tiles {
        let _ =
            writeln!(svg, "    <rect class=\"{class}\" x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"/>");
    }
    svg.push_str("    <rect class=\"b-hover\" x=\"48\" y=\"204\" width=\"120\" height=\"24\" rx=\"12\"/>\n    <rect class=\"b-ink\" x=\"48\" y=\"56\" width=\"72\" height=\"24\" rx=\"12\"/>\n  </svg>\n");
    let tagline = GUIDE
        .lines()
        .find(|l| !l.starts_with('#') && !l.trim().is_empty())
        .and_then(|l| l.split(". ").next())
        .unwrap_or("Beautiful native interfaces, powered by Rust");
    format!(
        "<!-- @dsCard height=300 -->\n<style>\n  .cover {{ position: relative; width: 960px; height: 300px; background: var(--bg-000); overflow: hidden; font-family: var(--font-sans); }}\n  .cover svg {{ position: absolute; left: 480px; top: 0; }}\n  .b-canvas {{ fill: var(--bg-200); }}\n  .b-panel {{ fill: var(--bg-300); }}\n  .b-accent {{ fill: var(--accent); }}\n  .b-hover {{ fill: var(--accent-hover); }}\n  .b-ink {{ fill: var(--ink); }}\n  .r-xl {{ rx: var(--radius-xl); }}\n  .r-md {{ rx: var(--radius-md); }}\n  .name {{ position: absolute; left: 40px; bottom: 72px; margin: 0; max-width: 440px; font-size: 96px; line-height: .92; font-weight: 600; letter-spacing: -2px; color: var(--ink); }}\n  .tag {{ position: absolute; left: 40px; bottom: 40px; margin: 0; max-width: 440px; font-size: 14px; line-height: 20px; color: var(--ink-muted); }}\n</style>\n<div class=\"cover\">\n{svg}  <p class=\"name\">RavenRS</p>\n  <p class=\"tag\">{}.</p>\n</div>\n<script></script>\n",
        escape(tagline.trim_end_matches('.'))
    )
}

/// Writes the whole design system under `<dir>/project/`, everything the design tool shows: the
/// index, the guide with every component, the tokens read from `ravenrs-style` and their CSS, the
/// fonts, the cover, and for each component its card, its vector preview and its JSON. Nothing
/// else belongs to the design system: a published file not in the returned list is removed.
/// `sha` is the commit it comes from.
pub fn write_claude_design(dir: &Path, sha: &str) -> std::io::Result<Vec<String>> {
    let project = dir.join("project");
    std::fs::create_dir_all(project.join("fonts"))?;
    std::fs::create_dir_all(project.join("components/Cover"))?;
    let entries = index::entries();
    let tokens = crate::tokens::tokens(sha);
    let mut written = Vec::new();
    let mut put = |path: &str, bytes: &[u8]| -> std::io::Result<()> {
        std::fs::write(dir.join(path), bytes)?;
        written.push(path.to_owned());
        Ok(())
    };
    put("project/README.md", guide_with_index(&entries).as_bytes())?;
    put("project/tokens.json", serde_json::to_string_pretty(&tokens)?.as_bytes())?;
    put("project/tokens.css", crate::tokens::css(&tokens).as_bytes())?;
    put("project/fonts/Inter-Regular.ttf", INTER_REGULAR)?;
    put("project/fonts/Inter-SemiBold.ttf", INTER_SEMIBOLD)?;
    put("project/fonts/RavenIcons.ttf", ICON_FONT)?;
    put("project/components/Cover/preview.html", cover().as_bytes())?;
    for entry in &entries {
        let stories = measure_entry(entry);
        let folder = format!("project/components/{}", entry.name());
        std::fs::create_dir_all(dir.join(&folder))?;
        put(&format!("{folder}/README.md"), card_readme(entry, &stories).as_bytes())?;
        put(&format!("{folder}/preview.html"), preview(entry, &stories).as_bytes())?;
        put(
            &format!("{folder}/{}.json", entry.name()),
            serde_json::to_string(&component_json(entry, &stories))?.as_bytes(),
        )?;
    }
    // The index last, as the design tool asks; then the list of everything, for the publisher.
    put(
        "project/design-system.json",
        serde_json::to_string(&design_system_index(sha, entries.len()))?.as_bytes(),
    )?;
    let list = written.join("\n") + "\n";
    std::fs::write(dir.join("files.txt"), list)?;
    Ok(written)
}
