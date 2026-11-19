//! The tokens of the design system, as RavenRS draws with them: every value read from
//! `ravenrs-style` (the three themes, the type scale, the spacing, the corners, the shadows and
//! the motion), never typed. `docs/design-system/tokens.json` gives only what people write about
//! them: what each token is for, the samples of the type scale, the fonts and the notes.

use ravenrs::animation::Transition;
use ravenrs::prelude::{Color, Semantic, TextStyle, Theme};
use ravenrs::style::{RADII, SPACING};
use serde_json::{Map, Value, json};

/// What people write about the tokens: usage, samples, fonts, notes.
const NOTES: &str = include_str!("../../../docs/design-system/tokens.json");

/// The themes, the first one first, as the design tool reads them.
pub fn themes() -> [(&'static str, &'static str, Theme); 3] {
    [
        ("dark", "Raven Dark", Theme::raven_dark()),
        ("light", "Raven Light", Theme::raven_light()),
        ("glass", "Raven Glass", Theme::raven_glass()),
    ]
}

/// Every color role with a token name, in the order the guide speaks of them.
pub fn color_roles() -> Vec<Semantic> {
    let mut roles = Semantic::TOKENS.to_vec();
    roles.extend([Semantic::Scrim, Semantic::Chart1]);
    roles.extend(Semantic::CHART_TOKENS);
    roles.extend([Semantic::ChartGrid, Semantic::Guide]);
    roles
}

/// A color as the design tool writes it: `#rrggbb`, or `#rrggbbaa` when it is see-through.
pub fn hex(c: Color) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b, a) = (byte(c.r), byte(c.g), byte(c.b), byte(c.a));
    if a == 255 { format!("#{r:02x}{g:02x}{b:02x}") } else { format!("#{r:02x}{g:02x}{b:02x}{a:02x}") }
}

fn px(v: f32) -> String {
    let v = (f64::from(v) * 100.0).round() / 100.0;
    if v.fract() == 0.0 { format!("{}px", v as i64) } else { format!("{v}px") }
}

fn rgba(c: Color) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("rgba({}, {}, {}, {})", byte(c.r), byte(c.g), byte(c.b), (f64::from(c.a) * 100.0).round() / 100.0)
}

/// The words people wrote for each token, by name, from `docs/design-system/tokens.json`.
struct Notes {
    doc: Value,
}

impl Notes {
    fn read() -> Self {
        Self { doc: serde_json::from_str(NOTES).unwrap_or_else(|_| json!({})) }
    }

    /// The `usage` of the token `name` in the family `family`.
    fn usage(&self, family: &str, name: &str) -> String {
        self.doc[family]["tokens"]
            .as_array()
            .and_then(|tokens| tokens.iter().find(|t| t["name"] == name))
            .and_then(|t| t["usage"].as_str())
            .unwrap_or_default()
            .to_owned()
    }

    /// The type style `name` as people wrote it: its sample and usage.
    fn style(&self, name: &str) -> (String, String) {
        let found = self.doc["type"]["groups"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|g| g["styles"].as_array().cloned().unwrap_or_default())
            .find(|s| s["name"] == name)
            .unwrap_or_default();
        (
            found["sample"].as_str().unwrap_or_default().to_owned(),
            found["usage"].as_str().unwrap_or_default().to_owned(),
        )
    }

    fn note(&self, family: &str) -> Option<String> {
        self.doc[family]["note"].as_str().map(str::to_owned)
    }
}

/// The token families' entries that have no words in the notes: each needs a sentence there.
pub fn without_usage() -> Vec<String> {
    let tokens = tokens("check");
    let mut missing = Vec::new();
    for family in ["color", "spacing", "radius", "shadow", "duration", "spring"] {
        for t in tokens[family]["tokens"].as_array().into_iter().flatten() {
            if t["usage"].as_str().is_none_or(str::is_empty) {
                missing.push(format!("{family}/{}", t["name"].as_str().unwrap_or_default()));
            }
        }
    }
    missing
}

fn family(tokens: Vec<Value>, note: Option<String>) -> Value {
    let mut out = Map::new();
    if let Some(note) = note {
        out.insert("note".into(), json!(note));
    }
    out.insert("tokens".into(), json!(tokens));
    Value::Object(out)
}

/// The tokens object of the design system, every value from `ravenrs-style`. `sha` is the commit
/// it comes from.
pub fn tokens(sha: &str) -> Value {
    let notes = Notes::read();
    let themes = themes();
    let colors: Vec<Value> = color_roles()
        .into_iter()
        .map(|role| {
            let value: Map<String, Value> = themes
                .iter()
                .map(|(id, _, theme)| ((*id).to_owned(), json!(hex(theme.color(role)))))
                .collect();
            json!({"name": role.token(), "value": value, "usage": notes.usage("color", role.token())})
        })
        .collect();
    let dark = &themes[0].2;
    let styles: Vec<Value> = TextStyle::TOKENS
        .iter()
        .map(|style| {
            let t = dark.text(*style);
            let (sample, usage) = notes.style(style.token());
            let mut s = json!({
                "name": style.token(),
                "fontSize": px(t.size),
                "lineHeight": px(t.line_height),
                "fontWeight": t.weight.0.round() as i64,
            });
            if t.letter_spacing != 0.0 {
                s["letterSpacing"] = json!(px(t.letter_spacing));
            }
            if !sample.is_empty() {
                s["sample"] = json!(sample);
            }
            if !usage.is_empty() {
                s["usage"] = json!(usage);
            }
            s
        })
        .collect();
    let spaces = [
        ("space-1", SPACING.s1),
        ("space-2", SPACING.s2),
        ("space-3", SPACING.s3),
        ("space-4", SPACING.s4),
        ("space-5", SPACING.s5),
        ("space-6", SPACING.s6),
        ("space-8", SPACING.s8),
        ("space-12", SPACING.s12),
    ];
    let spacing: Vec<Value> = spaces
        .iter()
        .map(|(name, v)| json!({"name": name, "value": px(*v), "usage": notes.usage("spacing", name)}))
        .collect();
    let corners = [
        ("radius-sm", RADII.sm),
        ("radius-md", RADII.md),
        ("radius-lg", RADII.lg),
        ("radius-xl", RADII.xl),
        ("radius-full", RADII.full),
    ];
    let radius: Vec<Value> = corners
        .iter()
        .map(|(name, v)| json!({"name": name, "value": px(*v), "usage": notes.usage("radius", name)}))
        .collect();
    let shadow_of = |pick: fn(&Theme) -> ravenrs::prelude::Shadow| -> Map<String, Value> {
        themes
            .iter()
            .map(|(id, _, theme)| {
                let s = pick(theme);
                (
                    (*id).to_owned(),
                    json!(format!("{} {} {} {}", px(s.offset.x), px(s.offset.y), px(s.blur), rgba(s.color))),
                )
            })
            .collect()
    };
    let shadows = vec![
        json!({"name": "shadow-soft", "value": shadow_of(|t| t.elevation.soft), "usage": notes.usage("shadow", "shadow-soft")}),
        json!({"name": "shadow-raised", "value": shadow_of(|t| t.elevation.raised), "usage": notes.usage("shadow", "shadow-raised")}),
    ];
    let motion = &dark.motion;
    let ms = |t: Transition| match t {
        Transition::Tween { duration, .. } => format!("{}ms", (duration * 1000.0).round() as i64),
        _ => "0ms".to_owned(),
    };
    let durations: Vec<Value> = [
        ("duration-fast", motion.fast),
        ("duration-base", motion.base),
        ("duration-slow", motion.slow),
        ("duration-draw", motion.draw_in),
    ]
    .iter()
    .map(|(name, t)| json!({"name": name, "value": ms(*t), "usage": notes.usage("duration", name)}))
    .collect();
    let spring = |t: Transition| match t {
        Transition::Spring(s) => {
            json!({"stiffness": s.stiffness.round() as i64, "damping": s.damping.round() as i64})
        }
        _ => json!({}),
    };
    let springs = vec![
        json!({"name": "spring-snappy", "value": spring(motion.snappy), "usage": notes.usage("spring", "spring-snappy")}),
        json!({"name": "spring-gentle", "value": spring(motion.gentle), "usage": notes.usage("spring", "spring-gentle")}),
    ];
    let mut fonts = notes.doc["type"]["fonts"].as_array().cloned().unwrap_or_default();
    fonts.push(json!({"family": "RavenIcons", "file": "fonts/RavenIcons.ttf", "weight": "400"}));
    json!({
        "name": "RavenRS",
        "version": 1,
        "meta": {
            "source": "github",
            "repo": "alexkads/RavenRS",
            "ref": sha,
            "paths": {"tokens": ["crates/ravenrs-style"], "notes": ["docs/design-system/tokens.json"], "components": ["tools/raven-component-book"]},
            "note": "Written whole by RavenComponentBook (raven-component-book --claude-design): every value read from ravenrs-style.",
        },
        "color": {
            "themes": themes.iter().map(|(id, name, _)| json!({"id": id, "name": name})).collect::<Vec<_>>(),
            "tokens": colors,
        },
        "type": {
            "fonts": fonts,
            "families": notes.doc["type"]["families"].clone(),
            "groups": [{"name": "Text", "family": "sans", "styles": styles}],
        },
        "spacing": family(spacing, notes.note("spacing")),
        "radius": family(radius, notes.note("radius")),
        "shadow": family(shadows, notes.note("shadow")),
        "duration": family(durations, notes.note("duration")),
        "spring": family(springs, notes.note("spring")),
    })
}

/// The tokens as CSS, compiled as the design tool compiles them: the colors and shadows of the
/// first theme on `:root`, each other theme's in its own `[data-theme]` block, the lengths and
/// fonts on `:root`, a class per type style and an `@font-face` per font file.
pub fn css(tokens: &Value) -> String {
    let mut out = String::from("/* RavenRS — generated from tokens.json */\n");
    let ids: Vec<&str> =
        tokens["color"]["themes"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str()).collect();
    let themed = |theme: &str| -> String {
        let mut s = String::new();
        for family in ["color", "shadow"] {
            for t in tokens[family]["tokens"].as_array().into_iter().flatten() {
                if let Some(v) = t["value"][theme].as_str() {
                    s.push_str(&format!("  --{}: {v};\n", t["name"].as_str().unwrap_or_default()));
                }
            }
        }
        s
    };
    if let Some(first) = ids.first() {
        out.push_str(&format!(":root, [data-theme=\"{first}\"] {{\n{}}}\n", themed(first)));
    }
    for id in ids.iter().skip(1) {
        out.push_str(&format!("[data-theme=\"{id}\"] {{\n{}}}\n", themed(id)));
    }
    out.push_str(":root {\n");
    for family in ["spacing", "radius", "duration"] {
        for t in tokens[family]["tokens"].as_array().into_iter().flatten() {
            if let Some(v) = t["value"].as_str() {
                out.push_str(&format!("  --{}: {v};\n", t["name"].as_str().unwrap_or_default()));
            }
        }
    }
    if let Some(families) = tokens["type"]["families"].as_object() {
        for (key, stack) in families {
            out.push_str(&format!("  --font-{key}: {};\n", stack.as_str().unwrap_or_default()));
        }
    }
    out.push_str("}\n");
    for group in tokens["type"]["groups"].as_array().into_iter().flatten() {
        let family = group["family"].as_str().unwrap_or("sans");
        for s in group["styles"].as_array().into_iter().flatten() {
            out.push_str(&format!(
                ".{} {{ font-family: var(--font-{family}); font-size: {}; line-height: {}; font-weight: {};{} }}\n",
                s["name"].as_str().unwrap_or_default(),
                s["fontSize"].as_str().unwrap_or_default(),
                s["lineHeight"].as_str().unwrap_or_default(),
                s["fontWeight"],
                s["letterSpacing"].as_str().map(|l| format!(" letter-spacing: {l};")).unwrap_or_default()
            ));
        }
    }
    for font in tokens["type"]["fonts"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "@font-face {{ font-family: \"{}\"; src: url(\"{}\"); font-weight: {}; font-display: block; }}\n",
            font["family"].as_str().unwrap_or_default(),
            font["file"].as_str().unwrap_or_default(),
            font["weight"].as_str().unwrap_or("400")
        ));
    }
    out
}
