//! The screens the plugins on put in the panel (P-00, #884): a page of the menu, or a tab on the
//! page of a build. Each piece shows the JSON of its route: a list of records in a grid, a record
//! as rows of name and value.

use std::rc::Rc;

use ravenrs::prelude::*;
use serde_json::Value as Json;

use super::{S, escaped, nada};

impl S {
    /// Reads the routes of the page `name` of a plugin (of the build `build`, for a tab).
    pub(super) fn load_plugin_page(self: &Rc<Self>, name: &str, build: Option<(&str, u64)>) {
        let screens = self.screens.get();
        let Some(screen) = screens.as_array().into_iter().flatten().find(|sc| sc["name"] == name) else {
            return;
        };
        for source in sources(screen, build) {
            self.load_source(&source);
        }
    }

    /// Reads `source` and keeps its JSON for the screens that show it.
    fn load_source(self: &Rc<Self>, source: &str) {
        let mut data = self.plugin_data.get();
        data.insert(source.to_owned(), Json::Null);
        self.plugin_data.set(data);
        let key = source.to_owned();
        self.read(source, move |s, body| {
            let mut data = s.plugin_data.get();
            data.insert(key, body);
            s.plugin_data.set(data);
        });
    }
}

/// The routes of the pieces of `screen`, with `{job}` and `{number}` of `build` put in.
fn sources(screen: &Json, build: Option<(&str, u64)>) -> Vec<String> {
    screen["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["source"].as_str())
        .map(|source| match build {
            Some((job, number)) => {
                source.replace("{job}", &escaped(job)).replace("{number}", &number.to_string())
            }
            None => source.to_owned(),
        })
        .collect()
}

/// The page `name` a plugin put in the menu.
pub(super) fn page(s: &Rc<S>, name: &str) -> AnyView {
    let screens = s.screens.get();
    let Some(screen) = screens.as_array().into_iter().flatten().find(|sc| sc["name"] == name).cloned() else {
        return Box::new(
            VStack::new((
                PageHeader::new("Página de plugin"),
                Alert::new("Esta página saiu", "O plugin que a punha foi desligado.").tone(Tone::Warning),
            ))
            .spacing(16.0)
            .padding(24.0)
            .align(Align::Stretch),
        );
    };
    let plugin = screen["plugin"].as_str().unwrap_or_default();
    Box::new(
        VStack::new((
            PageHeader::new(screen["caption"].as_str().unwrap_or(name))
                .description(format!("Página do plugin {plugin}. Desligado, ele sai do menu."))
                .badge(Badge::new(plugin.to_owned()).outline()),
            screen_view(s, &screen, None),
        ))
        .spacing(16.0)
        .padding(24.0)
        .align(Align::Stretch),
    )
}

/// The pieces of `screen`, each with its caption over what its route answered.
pub(super) fn screen_view(s: &Rc<S>, screen: &Json, build: Option<(&str, u64)>) -> AnyView {
    let data = s.plugin_data.get();
    let pieces: Vec<AnyView> = screen["items"]
        .as_array()
        .into_iter()
        .flatten()
        .zip(sources(screen, build).into_iter().map(Some).chain(std::iter::repeat(None)))
        .map(|(item, source)| -> AnyView {
            let caption = item["caption"].as_str().unwrap_or_default().to_owned();
            let shown: AnyView = match source.as_deref().map(|src| (src, data.get(src))) {
                None => {
                    Box::new(Text::new("Esta parte não tem rota de dados.").foreground(Semantic::InkMuted))
                }
                Some((src, None)) => {
                    s.load_source(src);
                    Box::new(Text::new("Lendo…").foreground(Semantic::InkMuted))
                }
                Some((_, Some(Json::Null))) => Box::new(Text::new("Lendo…").foreground(Semantic::InkMuted)),
                Some((_, Some(value))) => json_view(value),
            };
            Box::new(
                VStack::new((Text::new(caption).style(TextStyle::Headline), shown))
                    .spacing(8.0)
                    .align(Align::Stretch),
            )
        })
        .collect();
    if pieces.is_empty() {
        return nada();
    }
    Box::new(VStack::new(pieces).spacing(16.0).align(Align::Stretch))
}

/// JSON as the panel shows it: records in a grid, one record in rows, anything else as text.
pub(super) fn json_view(value: &Json) -> AnyView {
    match value {
        Json::Array(items) if items.iter().all(Json::is_object) && !items.is_empty() => {
            let mut columns: Vec<String> = Vec::new();
            for item in items {
                for key in item.as_object().into_iter().flatten().map(|(k, _)| k) {
                    if !columns.contains(key) {
                        columns.push(key.clone());
                    }
                }
            }
            let rows: Vec<Vec<ravenrs::prelude::Value>> =
                items.iter().map(|item| columns.iter().map(|c| text_of(&item[c]).into()).collect()).collect();
            let grid_columns: Vec<GridColumn> =
                columns.iter().map(|c| GridColumn::text(c.clone()).width(180.0)).collect();
            Box::new(DataGrid::new(GridData::new(rows), grid_columns).height(320.0))
        }
        Json::Object(fields) => Box::new(
            VStack::new(
                fields.iter().map(|(k, v)| PropertyRow::new(k.clone(), text_of(v))).collect::<Vec<_>>(),
            )
            .spacing(4.0)
            .align(Align::Stretch),
        ),
        Json::Array(items) if items.is_empty() => {
            Box::new(Text::new("Nada por aqui ainda.").foreground(Semantic::InkMuted))
        }
        other => Box::new(Text::new(text_of(other))),
    }
}

/// A value of JSON in one line.
fn text_of(value: &Json) -> String {
    match value {
        Json::String(s) => s.clone(),
        Json::Null => String::new(),
        Json::Bool(true) => "sim".to_owned(),
        Json::Bool(false) => "não".to_owned(),
        other => other.to_string(),
    }
}
