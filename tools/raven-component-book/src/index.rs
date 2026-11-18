//! Every component in the book, as Storybook's index lists them: the ones that give their own
//! stories through `ComponentBook`, and the ones that do not yet, shown by their example in the
//! component gallery and marked as pending.

use ravenrs::widgets::book::{Args, Meta};

/// The list of components the site keeps, with each one's family and sentence.
const CATALOG: &str = include_str!("../../../site/components/index.html");
/// The examples of the gallery, whose code is shown for the components still pending.
const SAMPLES: [&str; 11] = [
    include_str!("../../../examples/component-gallery/src/samples/buttons.rs"),
    include_str!("../../../examples/component-gallery/src/samples/color.rs"),
    include_str!("../../../examples/component-gallery/src/samples/data.rs"),
    include_str!("../../../examples/component-gallery/src/samples/display.rs"),
    include_str!("../../../examples/component-gallery/src/samples/fields.rs"),
    include_str!("../../../examples/component-gallery/src/samples/imaging.rs"),
    include_str!("../../../examples/component-gallery/src/samples/layout.rs"),
    include_str!("../../../examples/component-gallery/src/samples/media.rs"),
    include_str!("../../../examples/component-gallery/src/samples/navigation.rs"),
    include_str!("../../../examples/component-gallery/src/samples/overlays.rs"),
    include_str!("../../../examples/component-gallery/src/samples/pickers.rs"),
];
/// The public API of the components, one line per thing a user can name.
const PUBLIC_API: &str = include_str!("../../../crates/ravenrs-widgets/public-api.txt");

/// A component of the book.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Where it sits, its settings and its stories.
    pub meta: Meta,
    /// It gives no `ComponentBook` yet: its only story is its example in the gallery.
    pub pending: bool,
    /// The code of its example in the gallery, when there is one.
    pub code: Option<String>,
}

impl Entry {
    /// The component's name.
    pub fn name(&self) -> &'static str {
        self.meta.component()
    }

    /// The builder methods of the component, from the public API.
    pub fn methods(&self) -> Vec<&'static str> {
        let prefix = format!("method {}::", self.name());
        PUBLIC_API.lines().filter_map(|line| line.strip_prefix(prefix.as_str())).collect()
    }

    /// The enums named after the component, as `ButtonStyle` and `ButtonSize`, with their
    /// variants.
    pub fn variants(&self) -> Vec<(&'static str, Vec<&'static str>)> {
        let name = self.name();
        PUBLIC_API
            .lines()
            .filter_map(|line| line.strip_prefix("enum "))
            .filter(|e| e.starts_with(name) && *e != name)
            .map(|e| {
                let prefix = format!("variant {e}::");
                let variants =
                    PUBLIC_API.lines().filter_map(|line| line.strip_prefix(prefix.as_str())).collect();
                (e, variants)
            })
            .collect()
    }
}

/// One row of the site's list: family, name and sentence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogRow {
    /// The section it is in, as "Actions and inputs".
    pub family: String,
    /// The type's name.
    pub name: String,
    /// What it is.
    pub description: String,
}

/// The rows of the site's list, in its order, each name once.
pub fn catalog() -> Vec<CatalogRow> {
    let mut rows: Vec<CatalogRow> = Vec::new();
    let mut family = String::new();
    for line in CATALOG.lines() {
        let line = line.trim();
        if line.starts_with("<h2 id=") {
            family = between(line, "#</a>", "</h2>").unwrap_or_default().to_owned();
        } else if line.starts_with("<li><a href=") && line.contains("data-api=") && !family.is_empty() {
            let Some(name) = between(line, "\">", "</a>") else { continue };
            let description = between(line, "<span class=\"desc\">", "</span>").unwrap_or_default();
            if !rows.iter().any(|r| r.name == name) {
                rows.push(CatalogRow {
                    family: family.clone(),
                    name: name.to_owned(),
                    description: unescape(description),
                });
            }
        }
    }
    rows
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = text.find(start)? + start.len();
    let to = text[from..].find(end)? + from;
    Some(&text[from..to])
}

fn unescape(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// The code of the gallery's example of `name`, between its `// sample:` and `// end` markers.
pub fn sample_code(name: &str) -> Option<String> {
    let marker = format!("// sample: {name}, ");
    for file in SAMPLES {
        if let Some(at) = file.find(&marker) {
            let after = &file[at..];
            let start = after.find('\n')? + 1;
            let end = after.find("\n// end")?;
            return Some(after[start..end].to_owned());
        }
    }
    None
}

fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// Every component of the book: first those that give their stories, then, for every example
/// of the gallery with no `ComponentBook` yet, its example as a pending story. Families follow
/// the site's list.
pub fn entries() -> Vec<Entry> {
    let mut out: Vec<Entry> = ravenrs::widgets::book::all()
        .into_iter()
        .map(|meta| Entry { code: sample_code(meta.component()), meta, pending: false })
        .collect();
    let rows = catalog();
    for name in component_gallery::samples::NAMES {
        if out.iter().any(|e| e.name() == *name) {
            continue;
        }
        let row = rows.iter().find(|r| r.name == *name);
        let family = row.map_or("Application", |r| r.family.as_str());
        let summary = row.map_or_else(
            || "The window of an application made with ravenrs-app.".to_owned(),
            |r| r.description.clone(),
        );
        let title = leak(format!("{family}/{name}"));
        let name: &'static str = name;
        let meta = Meta::new(title, move |_: &Args| {
            component_gallery::samples::sample(name)
                .unwrap_or_else(|| Box::new(ravenrs::prelude::Text::new(name)))
        })
        .summary(leak(format!("{summary}.")))
        .story("Example", Args::new());
        out.push(Entry { meta, pending: true, code: sample_code(name) });
    }
    let order: Vec<String> = rows.iter().map(|r| r.family.clone()).collect();
    out.sort_by_key(|e| order.iter().position(|f| f == e.meta.family()).unwrap_or(usize::MAX));
    out
}
