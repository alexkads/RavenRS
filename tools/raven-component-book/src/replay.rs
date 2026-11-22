//! The test of reproduction: a screen comes back from the design tool and the book checks that
//! RavenRS builds each piece of it as it was drawn.
//!
//! A design keeps, on the element that draws each component, the mark of the card it comes from,
//! `data-rcb="Button/Secondary"`, and the settings it changed, `data-rcb-args='{"label":"Cancel"}'`.
//! A script in the artboard (the guide gives it) writes every marked element with the box the
//! browser laid it out in:
//!
//! ```json
//! {"screen": "New transaction",
//!  "elements": [{"rcb": "Button/Secondary", "args": {"label": "Cancel"}, "box": [690, 626, 101, 40]}]}
//! ```
//!
//! For each element the book draws that story with those settings in a parent as wide as the box,
//! which stretches its children as a column does, and compares the size it takes with the box.
//! The places of the pieces are the design's: what is compared is what each piece is.

use ravenrs::widgets::book::{ArgValue, Args, Control, Meta};
use serde_json::Value;

use crate::index::{self, Entry};
use crate::measure::{self, MARGIN};

/// How far a size may be from the drawing, in points.
pub const TOLERANCE: f64 = 1.0;
/// How far the width of a piece that hugs its content may be, as a share of it: the browser lays
/// text out a little wider than RavenRS (gap 16 of #768), about 2% of a short label.
pub const TEXT_SHARE: f64 = 0.03;

/// A piece of the screen that RavenRS does not build as it was drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Divergence {
    /// Which piece: its mark and the settings it changed.
    pub element: String,
    /// What differs, in a sentence.
    pub what: String,
}

impl std::fmt::Display for Divergence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.element, self.what)
    }
}

/// What came back from the design, checked: the pieces compared and what differs.
#[derive(Clone, Debug, Default)]
pub struct Replay {
    /// How many marked pieces were compared.
    pub compared: usize,
    /// What RavenRS does not build as it was drawn.
    pub divergences: Vec<Divergence>,
}

/// A setting as the design wrote it, in the kind its component declares.
fn arg_value(meta: &Meta, name: &str, v: &Value) -> Result<ArgValue, String> {
    let Some(arg) = meta.arg_types.iter().find(|a| a.name == name) else {
        let known: Vec<&str> = meta.arg_types.iter().map(|a| a.name).collect();
        return Err(format!("it has no setting \"{name}\" (it has: {})", known.join(", ")));
    };
    let wrong = || format!("the setting \"{name}\" does not take {v}");
    Ok(match &arg.control {
        Control::Text => ArgValue::Text(v.as_str().ok_or_else(wrong)?.to_owned()),
        Control::Bool => ArgValue::Bool(v.as_bool().ok_or_else(wrong)?),
        Control::Number { .. } => ArgValue::Number(v.as_f64().ok_or_else(wrong)?),
        Control::Choice(options) => {
            let chosen = v.as_str().ok_or_else(wrong)?;
            if !options.contains(&chosen) {
                return Err(format!(
                    "\"{name}\" has no option \"{chosen}\" (it has: {})",
                    options.join(", ")
                ));
            }
            ArgValue::choice(chosen)
        }
    })
}

/// Checks the screen that came back from the design, `json` as the artboard's script wrote it.
pub fn replay(json: &str) -> Result<Replay, String> {
    replay_with(json, &index::entries())
}

/// [`replay`] against these entries of the book.
pub fn replay_with(json: &str, entries: &[Entry]) -> Result<Replay, String> {
    let screen: Value = serde_json::from_str(json).map_err(|e| format!("not the JSON of a screen: {e}"))?;
    let elements = screen["elements"].as_array().ok_or("the JSON has no \"elements\" list")?;
    let theme = measure::themes()[0].1;
    let mut out = Replay::default();
    for element in elements {
        let mark = element["rcb"].as_str().unwrap_or_default();
        let changed = element.get("args").cloned().unwrap_or(Value::Object(Default::default()));
        let label = if changed.as_object().is_some_and(|o| !o.is_empty()) {
            format!("{mark} {changed}")
        } else {
            mark.to_owned()
        };
        let mut diverge = |what: String| out.divergences.push(Divergence { element: label.clone(), what });
        let Some((component, story_name)) = mark.split_once('/') else {
            diverge("the mark is not \"Component/Story\"".to_owned());
            continue;
        };
        let Some(entry) = entries.iter().find(|e| e.name() == component) else {
            diverge(format!("no component \"{component}\" in the library"));
            continue;
        };
        let Some(story) = entry.meta.stories.iter().find(|s| s.name == story_name) else {
            let known: Vec<&str> = entry.meta.stories.iter().map(|s| s.name).collect();
            diverge(format!("{component} has no story \"{story_name}\" (it has: {})", known.join(", ")));
            continue;
        };
        let Some([_, _, w, h]) = element["box"]
            .as_array()
            .and_then(|b| b.iter().map(Value::as_f64).collect::<Option<Vec<_>>>())
            .and_then(|b| <[f64; 4]>::try_from(b).ok())
        else {
            diverge("the box is not [x, y, width, height]".to_owned());
            continue;
        };
        let mut args: Args = entry.meta.args_of(story);
        let mut refused = false;
        for (name, v) in changed.as_object().into_iter().flatten() {
            match arg_value(&entry.meta, name, v) {
                Ok(value) => args.set(name, value),
                Err(why) => {
                    diverge(why);
                    refused = true;
                }
            }
        }
        if refused {
            continue;
        }
        out.compared += 1;
        // A parent as wide as the box, with the stage's margin on each side.
        let parent = (w as f32 + 2.0 * MARGIN).max(2.0 * MARGIN + 1.0);
        let built = measure::measure_stretched(&entry.meta, story, &args, theme, parent).main_box();
        let fills = (built.0 - f64::from(parent - 2.0 * MARGIN)).abs() <= TOLERANCE;
        let width_room = if fills { TOLERANCE } else { TOLERANCE.max(built.0 * TEXT_SHARE) };
        let mut said = Vec::new();
        if (built.0 - w).abs() > width_room {
            said.push(format!("width {w} in the design, {} in RavenRS ({:+})", built.0, round(w - built.0)));
        }
        if (built.1 - h).abs() > TOLERANCE {
            said.push(format!("height {h} in the design, {} in RavenRS ({:+})", built.1, round(h - built.1)));
        }
        if !said.is_empty() {
            diverge(said.join("; "));
        }
    }
    Ok(out)
}

fn round(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// The script an artboard runs to write what [`replay`] reads: every element marked with
/// `data-rcb`, its settings and its box, saved as `<screen>.rcb.json`.
pub const SCRIPT: &str = r#"const root = document.querySelector('[data-rcb-screen]') || document.body;
const origin = root.getBoundingClientRect();
const elements = [...root.querySelectorAll('[data-rcb]')].map((e) => {
  const r = e.getBoundingClientRect();
  return { rcb: e.dataset.rcb, args: JSON.parse(e.dataset.rcbArgs || '{}'),
           box: [r.x - origin.x, r.y - origin.y, r.width, r.height] };
});
const screen = root.dataset.rcbScreen || document.title;
const file = new Blob([JSON.stringify({ screen, elements }, null, 2)], { type: 'application/json' });
const link = Object.assign(document.createElement('a'), { href: URL.createObjectURL(file), download: screen + '.rcb.json' });
link.click();"#;
