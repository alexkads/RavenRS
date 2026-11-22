//! What the book writes: the JSON of every component (its settings, its stories and how each
//! measures in each theme), an index of them as Storybook's `index.json`, and the files of the
//! design system the design tool reads (`project/` of the Design System type), with a guide, a
//! vector preview and the measurements of every component.

use std::fmt::Write as _;
use std::path::Path;

use ravenrs::widgets::book::{ArgValue, Control};
use serde_json::{Value, json};

use crate::index::{self, Entry};
use crate::measure::{self, Fit, Interaction, Measured};

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

/// A story measured in the three themes, with its settings.
#[derive(Clone, Debug)]
pub struct MeasuredStory {
    /// The story's name.
    pub name: &'static str,
    /// The settings it was drawn with.
    pub args: Value,
    /// Raven Dark, then Raven Light and Raven Glass.
    pub themes: Vec<(&'static str, Measured)>,
    /// In Raven Dark, in the narrowest parent of [`measure::WIDTHS`].
    pub narrow: Measured,
    /// How it sizes, from the parents of [`measure::WIDTHS`].
    pub fit: Fit,
    /// For the first story, in Raven Dark: the pointer over its first control, the focus on it
    /// and the button held down on it. Empty for the other stories and when it has no control.
    pub interactions: Vec<Reaction>,
    /// For the first story: the keys of [`measure::KEYS`] its first control handles once focused.
    pub keys: Vec<&'static str>,
}

/// A state of [`Interaction`] measured on a story's first control.
#[derive(Clone, Debug)]
pub struct Reaction {
    /// Which state.
    pub state: Interaction,
    /// The control it was brought to, by its accessible name.
    pub control: String,
    /// What the stage shows in it.
    pub measured: Measured,
    /// What changed against the story at rest; empty when the control does not react.
    pub changes: Vec<Value>,
}

/// Measures every story of `entry` in the three themes, and in Raven Dark in the parents of the
/// other [`measure::WIDTHS`] to tell how it sizes.
pub fn measure_entry(entry: &Entry) -> Vec<MeasuredStory> {
    entry
        .meta
        .stories
        .iter()
        .enumerate()
        .map(|(i, story)| {
            let args = entry.meta.args_of(story);
            let themes: Vec<(&'static str, Measured)> = measure::themes()
                .into_iter()
                .map(|(id, theme)| (id, measure::measure(&entry.meta, story, &args, theme)))
                .collect();
            let dark = measure::themes()[0].1;
            let [narrowest, reference, widest] = measure::WIDTHS;
            let narrow = measure::measure_at(&entry.meta, story, &args, dark, narrowest);
            let wide = measure::measure_at(&entry.meta, story, &args, dark, widest).main_box();
            let at_reference = themes.first().map_or((0.0, 0.0), |(_, m)| m.main_box());
            let stretched =
                measure::measure_stretched(&entry.meta, story, &args, dark, reference).main_box().0;
            let fit = measure::fit_of(
                &entry.meta.sizing,
                &[(narrowest, narrow.main_box()), (reference, at_reference), (widest, wide)],
                Some(stretched),
            );
            // A pattern's controls are measured on their own cards; its card shows the screen.
            let screen = entry.meta.family() == "Patterns";
            let interactions = match (i, themes.first()) {
                (0, Some((_, rest))) if !screen => Interaction::ALL
                    .into_iter()
                    .filter_map(|state| {
                        let (control, measured) =
                            measure::measure_interaction(&entry.meta, story, &args, dark, state)?;
                        let changes = measure::changes(rest, &measured);
                        Some(Reaction { state, control, measured, changes })
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let keys = if i == 0 && !screen {
                measure::keys_handled(&entry.meta, story, &args, dark)
            } else {
                Vec::new()
            };
            let args = json!(
                args.iter().map(|(k, v)| (k.to_owned(), arg_value(v))).collect::<serde_json::Map<_, _>>()
            );
            MeasuredStory { name: story.name, args, themes, narrow, fit, interactions, keys }
        })
        .collect()
}

fn id_of(title: &str) -> String {
    title.to_lowercase().replace(' ', "-").replace('/', "--")
}

/// The JSON of one component: where it sits, its settings, its API, the code of its example and
/// every story measured in the three themes, with the anatomy of the first.
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
        "sizing": stories.first().map(|s| s.fit.json()),
        "interaction": interaction_json(stories),
        "behavior": behavior_json(entry, stories),
        "stories": stories.iter().map(|s| json!({
            "id": format!("{}--{}", id_of(entry.meta.title), id_of(s.name)),
            "name": s.name,
            "args": s.args,
            "sizing": s.fit.json(),
            "themes": s.themes.iter().map(|(id, m)| (id.to_string(), m.json())).collect::<serde_json::Map<_, _>>(),
        })).collect::<Vec<_>>(),
    })
}

/// How the first story's first control reacts: for each state, the control, what changed and,
/// when something did, what the stage shows; `reacts_to` and `ignores` name the states.
fn interaction_json(stories: &[MeasuredStory]) -> Value {
    let Some(first) = stories.first() else { return Value::Null };
    if first.interactions.is_empty() {
        return Value::Null;
    }
    let mut out = serde_json::Map::new();
    let (mut reacts, mut ignores) = (Vec::new(), Vec::new());
    for r in &first.interactions {
        if r.changes.is_empty() {
            ignores.push(r.state.name());
        } else {
            reacts.push(r.state.name());
            out.insert(
                r.state.name().into(),
                json!({"control": r.control, "story": first.name, "changes": r.changes, "dark": r.measured.json()}),
            );
        }
    }
    out.insert("reacts_to".into(), json!(reacts));
    out.insert("ignores".into(), json!(ignores));
    Value::Object(out)
}

/// What a person does with the component, from its first story at rest: the events its builder
/// offers (`on_*`), the keys its first control handles once focused, the role and name the
/// accessibility tree gives its first part, and the controls in the order Tab reaches them.
fn behavior_json(entry: &Entry, stories: &[MeasuredStory]) -> Value {
    let events: Vec<&str> = entry.methods().into_iter().filter(|m| m.starts_with("on_")).collect();
    let Some(first) = stories.first() else { return json!({"events": events}) };
    let parts = first.themes.first().map(|(_, m)| m.parts.as_slice()).unwrap_or_default();
    let root = parts.first();
    let focus: Vec<Value> = parts
        .iter()
        .filter(|p| p["focusable"] == true)
        .map(|p| json!({"role": p["role"], "name": p.get("name")}))
        .collect();
    json!({
        "events": events,
        "keys": first.keys,
        "role": root.map(|p| p["role"].clone()),
        "name": root.and_then(|p| p.get("name").cloned()),
        "focus_order": focus,
    })
}

/// One line per state for a person: on which control, and the tokens that change.
fn interaction_lines(first: &MeasuredStory) -> Vec<String> {
    first
        .interactions
        .iter()
        .map(|r| {
            if r.changes.is_empty() {
                return format!("- **{}**: \"{}\" does not change.", r.state.name(), r.control);
            }
            let said: Vec<String> = r
                .changes
                .iter()
                .take(4)
                .map(|c| {
                    if let Some(added) = c.get("added") {
                        let look = added.get("border").map_or_else(
                            || added.get("fill").cloned().unwrap_or(Value::Null).to_string(),
                            |b| format!("border {} {}", b["width"], b["color"]),
                        );
                        format!(
                            "adds a {} ({})",
                            added["kind"].as_str().unwrap_or("shape"),
                            look.replace('"', "")
                        )
                    } else if c.get("removed").is_some() {
                        "takes a shape away".to_owned()
                    } else {
                        let to = c["to"].as_object().cloned().unwrap_or_default();
                        to.iter()
                            .map(|(k, v)| {
                                format!(
                                    "{k} {} → {}",
                                    c["from"][k].to_string().replace('"', ""),
                                    v.to_string().replace('"', "")
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                })
                .collect();
            format!("- **{}** on \"{}\": {}.", r.state.name(), r.control, said.join("; "))
        })
        .collect()
}

/// Every icon by the name the cards use, with its character in the `RavenIcons` font, so a
/// design draws it as RavenRS does: `<span style="font-family: RavenIcons">&#xe204;</span>`.
fn icon_table() -> String {
    use ravenrs::prelude::IconName;
    let mut t = String::from(
        "\n## Every icon\n\nEach icon is one character of the `RavenIcons` font (in `fonts/`): write it as `&#x…;` in a span whose `font-family` is `RavenIcons`, sized like text.\n\n| Name | `IconName` | Character |\n|---|---|---|\n",
    );
    for icon in IconName::ALL {
        let _ = writeln!(t, "| `{}` | `{icon:?}` | `&#x{:x};` |", icon.name(), u32::from(icon.glyph()));
    }
    t
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
    // The pieces a design draws: controls, and the surfaces and texts around them, so the panel
    // of a dialog, its title and the label over a field have their corner, fill and font too.
    const PIECES: [&str; 26] = [
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
        "Dialog",
        "Heading",
        "Label",
        "Navigation",
        "Toolbar",
        "Menu",
        "ListBox",
        "Tooltip",
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
        // The inner space is to whatever starts first: an icon before a label, or the title of
        // a dialog before its close button.
        let starts = [icon.and_then(|i| i["x"].as_f64()), text.and_then(|t| t["x"][0].as_f64())];
        let start = starts.into_iter().flatten().reduce(f64::min);
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
    if let Some(inner) = name.strip_prefix("mix(").and_then(|m| m.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(", ").collect();
        if let [a, b, share] = parts.as_slice() {
            return format!("color-mix(in srgb, var(--{a}) {share}%, var(--{b}))");
        }
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
    // The ids of the clips are unique in a page with several pictures, and the same on every run:
    // they start with a hash of what the picture draws.
    let tag = m.paint.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, p| {
        p.to_string().bytes().fold(h, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
    }) % 0xff_ffff;
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
                    "<clipPath id=\"c{tag:x}-{clip}\"><rect x=\"{x}\" y=\"{y}\" width=\"{rw}\" height=\"{rh}\"/></clipPath>"
                );
                let _ = write!(out, "<g clip-path=\"url(#c{tag:x}-{clip})\">");
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
         - **Keep the marks.** Each picture in a card carries `data-rcb=\"Component/Story\"`. Put the same attribute on the element that draws that component in a screen, with the story it starts from, the settings you changed in `data-rcb-args` as JSON (`data-rcb-args='{\"label\":\"Cancel\"}'`), and `data-rcb-screen=\"<name>\"` on the screen's root. RavenRS then checks the screen piece by piece (see *Checking a screen*).\n\
         - **Size them as the card says.** Its *Sizing* tells whether a component fills the width of its parent or hugs its content, with the size it takes in parents of 320, 760 and 1200 pt. A field that fills its column stretches with it; a button that hugs its label keeps its width.\n\
         - **Change a component only through its settings** (the `argTypes` of its card). A variant, size or state that is not there does not exist.\n\
         - **Compose, do not invent.** A piece the screen needs that no component gives goes on a note called *Missing from the library* on the artboard, with what it should do. It joins the library before the screen is built.\n\
         - Colors are tokens (`var(--accent)`), never new colors; spacing follows the 4px grid (`space-*`); corners follow `radius-*`.\n\
         - Icons are RavenRS icons, by the names in the cards (`trash-2`, `search`); the `RavenIcons` font draws them.\n\n\
         ## Checking a screen\n\n\
         An artboard that keeps the marks can be checked against RavenRS. Run this script in it: it saves `<screen>.rcb.json` with every marked element, its settings and its box, and `raven-component-book --replay <file>` says which pieces RavenRS would not build at that size (1 pt of tolerance; a little more for the width of a label, which a browser lays out wider).\n\n\
         ```js\n{script}\n```\n\n\
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
        "\nCharts take their series from `chart-1` to `chart-5`; alignment guides take `guide`.\n\n\
         ## Patterns\n\n\
         Small screens made only of the components above, measured like them, under `patterns/`: start a screen from the one closest to it, and keep its spaces and what stretches.\n\n\
         | Pattern | What it is |\n|---|---|\n",
    );
    for p in crate::patterns::entries() {
        let _ = writeln!(text, "| `{}` | {} |", p.name(), p.meta.summary.trim_end_matches('.'));
    }
    text.push_str(
        "\n## Themes of the applications\n\n\
         Applications made with RavenRS choose their own colors. A screen designed for one of them takes its theme: put `data-theme=\"<id>\"` on the screen's root, and every `var(--token)` follows it. Only the colors and shadows change; sizes, corners and type are the system's.\n\n\
         | Theme | `data-theme` | Accent |\n|---|---|---|\n",
    );
    for (id, name, theme) in crate::tokens::themes().into_iter().skip(3) {
        let accent = crate::tokens::hex(theme.color(ravenrs::prelude::Semantic::Accent));
        let _ = writeln!(text, "| {name} | `{id}` | `{accent}` |");
    }
    text.push_str("\n## ");
    text.push_str(rest);
    text.replace("`raven-style`", "`ravenrs-style`").replace("{script}", crate::replay::SCRIPT)
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
    if let Some(first) = stories.first() {
        let _ = writeln!(t, "\n## Sizing\n\n{}", first.fit.sentence());
        // A story that sizes another way is named with its own sentence.
        let differs = |s: &&MeasuredStory| {
            (s.fit.width, s.fit.reflows, s.fit.max_width)
                != (first.fit.width, first.fit.reflows, first.fit.max_width)
        };
        let others: Vec<&MeasuredStory> = stories.iter().skip(1).filter(differs).collect();
        if !others.is_empty() {
            t.push('\n');
            for s in others {
                let _ = writeln!(t, "- **{}**: {}", s.name, s.fit.sentence());
            }
        }
    }
    if let Some(first) = stories.first().filter(|s| !s.interactions.is_empty()) {
        let _ = writeln!(
            t,
            "\n## Interaction\n\nThe pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story {}:\n",
            first.name
        );
        for line in interaction_lines(first) {
            let _ = writeln!(t, "{line}");
        }
    }
    let behavior = behavior_json(entry, stories);
    let list = |v: &Value| {
        v.as_array().map(|a| {
            a.iter().filter_map(Value::as_str).map(|x| format!("`{x}`")).collect::<Vec<_>>().join(", ")
        })
    };
    let mut lines = Vec::new();
    if let Some(events) = list(&behavior["events"]).filter(|e| !e.is_empty()) {
        lines.push(format!("- **Events**: {events}."));
    }
    if let Some(keys) = list(&behavior["keys"]).filter(|k| !k.is_empty()) {
        lines.push(format!("- **Keys** its first control handles once focused: {keys}."));
    }
    if let Some(role) = behavior["role"].as_str() {
        let name = behavior["name"].as_str().map(|n| format!(" \"{n}\"")).unwrap_or_default();
        lines.push(format!("- **Accessibility**: role `{role}`{name}."));
    }
    if let Some(order) = behavior["focus_order"].as_array().filter(|o| !o.is_empty()) {
        let said: Vec<String> = order
            .iter()
            .map(|p| match p["name"].as_str() {
                Some(n) => format!("{} \"{n}\"", p["role"].as_str().unwrap_or_default()),
                None => p["role"].as_str().unwrap_or_default().to_owned(),
            })
            .collect();
        lines.push(format!("- **Focus order** (Tab): {}.", said.join(" → ")));
    }
    if !lines.is_empty() {
        let _ = writeln!(t, "\n## Behavior\n\n{}", lines.join("\n"));
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
    if entry.name() == "Icon" {
        t.push_str(&icon_table());
    }
    let _ = write!(t, "\nEvery number above is in `{}.json` beside this card.\n", entry.name());
    t
}

/// `svg` with every `color-mix` of two tokens written as the color it makes in Raven Dark: shorter,
/// for a story too large for the design tool with the mixes named.
fn flatten_mixes(svg: &str) -> String {
    const START: &str = "color-mix(in srgb, var(--";
    let theme = measure::themes()[0].1;
    let color_of = |token: &str| {
        measure::published_roles().into_iter().find(|r| r.token() == token).map(|r| theme.color(r))
    };
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(at) = rest.find(START) {
        out.push_str(&rest[..at]);
        let tail = &rest[at + START.len()..];
        // `accent) 18%, transparent)`: a token at a share of its opacity.
        let faded = tail.find(")").and_then(|close| {
            let (token, more) = (&tail[..close], &tail[close + 2..]);
            let (share, after) = more.split_once("%, transparent)")?;
            let c = color_of(token)?;
            let alpha = c.a * share.parse::<f32>().ok()? / 100.0;
            Some((measure::hex(ravenrs::prelude::Color::rgba(c.r, c.g, c.b, alpha)), after))
        });
        if let Some((color, after)) = faded {
            out.push_str(&color);
            rest = after;
            continue;
        }
        let Some(end) = tail.find("))") else { break };
        // `a) 25%, var(--b`
        let inner = &tail[..end];
        let parsed = inner.split_once(") ").and_then(|(a, more)| {
            let (share, b) = more.split_once("%, var(--")?;
            Some((a, share.parse::<f32>().ok()? / 100.0, b))
        });
        match parsed.and_then(|(a, t, b)| Some((color_of(a)?, t, color_of(b)?))) {
            Some((ca, t, cb)) if !inner.contains("transparent") => {
                let mix = |x: f32, y: f32| y + (x - y) * t;
                out.push_str(&measure::hex(ravenrs::prelude::Color::rgba(
                    mix(ca.r, cb.r),
                    mix(ca.g, cb.g),
                    mix(ca.b, cb.b),
                    1.0,
                )));
                rest = &tail[end + 2..];
            }
            _ => {
                out.push_str(START);
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `svg` marked with the component and story it draws, `data-rcb="Button/Primary"`: a design
/// keeps the mark on what it draws from it, so the screen can be built back from the design.
fn marked(svg: &str, component: &str, story: &str) -> String {
    svg.replacen("<svg ", &format!("<svg data-rcb=\"{}/{}\" ", escape(component), escape(story)), 1)
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
         .widths { display: flex; flex-wrap: wrap; gap: 24px; align-items: flex-start; }\n\
         .more { margin: 0; font: 400 13px/18px var(--font-sans); color: var(--ink-muted); }\n\
         .story h3 { margin: 0 0 8px; font: 600 12px/16px var(--font-sans); letter-spacing: .4px; text-transform: uppercase; color: var(--ink-muted); }\n\
         </style>\n<div class=\"book\">\n",
    );
    // The design tool reads a preview of at most 256 KB: stories go in while they fit, the first
    // always, and the rest are named, since the JSON beside the card has every one of them.
    let mut left_out = Vec::new();
    for (i, s) in stories.iter().enumerate() {
        if let Some((_, m)) = s.themes.first() {
            let mut section = format!(
                "<section class=\"story\"><h3>{}</h3>{}</section>\n",
                escape(s.name),
                marked(&svg(m), entry.name(), s.name)
            );
            if i == 0 {
                // The reference beside the narrowest parent, so the design sees how it sizes,
                // when both fit in what the design tool reads.
                let beside = format!(
                    "<div class=\"widths\">{}<section class=\"story\"><h3>{} · {} pt parent</h3>{}</section></div>\n",
                    section.trim_end(),
                    escape(s.name),
                    measure::WIDTHS[0],
                    marked(&svg(&s.narrow), entry.name(), s.name)
                );
                if t.len() + beside.len() <= PREVIEW_BUDGET {
                    section = beside;
                }
            }
            if i == 0 && t.len() + section.len() > PREVIEW_BUDGET {
                section = flatten_mixes(&section);
            }
            if i > 0 && t.len() + section.len() > PREVIEW_BUDGET {
                left_out.push(escape(s.name));
                continue;
            }
            t.push_str(&section);
        }
    }
    // The states the first control reacts to, while they fit.
    if let Some(first) = stories.first() {
        for r in first.interactions.iter().filter(|r| !r.changes.is_empty()) {
            let section = format!(
                "<section class=\"story\"><h3>{} · {}</h3>{}</section>\n",
                escape(first.name),
                r.state.name(),
                marked(&svg(&r.measured), entry.name(), &format!("{} · {}", first.name, r.state.name()))
            );
            if t.len() + section.len() > PREVIEW_BUDGET {
                left_out.push(format!("{} · {}", escape(first.name), r.state.name()));
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
    // The patterns: small screens made of components, measured and exported the same way.
    for pattern in crate::patterns::entries() {
        let stories = measure_entry(&pattern);
        let folder = format!("project/patterns/{}", pattern.name());
        std::fs::create_dir_all(dir.join(&folder))?;
        put(&format!("{folder}/README.md"), card_readme(&pattern, &stories).as_bytes())?;
        put(&format!("{folder}/preview.html"), preview(&pattern, &stories).as_bytes())?;
        put(
            &format!("{folder}/{}.json", pattern.name()),
            serde_json::to_string(&component_json(&pattern, &stories))?.as_bytes(),
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
