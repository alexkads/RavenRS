//! Tables and tokens from the embedded Markdown and JSON documents.

use serde_json::{Map, Value, json};

use crate::api::{GUIDES, Index, has_word};

pub const README: &str = include_str!("../../../README.md");
pub const GETTING_STARTED: &str = include_str!("../../../docs/GETTING_STARTED.md");
const QUALITY: &str = include_str!("../../../docs/QUALITY.md");
const PARITY: &str = include_str!("../../../docs/PARITY.md");
const REPORTING_PARITY: &str = include_str!("../../../docs/REPORTING-PARITY.md");
const APP_PARITY: &str = include_str!("../../../docs/APP-PARITY.md");
const APP_CONTRACTS: &str = include_str!("../../../docs/APP-CONTRACTS.md");
const TOKENS: &str = include_str!("../../../docs/design-system/tokens.json");

/// The longest piece of a guide shown in a search, in characters.
const MAX_EXCERPT: usize = 400;
/// The longest part of a guide shown at once, in characters.
const MAX_SECTION: usize = 12_000;

/// Categories of `tokens.json`, under the names the tool accepts.
pub const TOKEN_CATEGORIES: [&str; 7] =
    ["color", "typography", "spacing", "radius", "shadow", "duration", "spring"];

/// One row of a Markdown table.
#[derive(Debug)]
pub struct Row {
    pub section: String,
    pub headers: Vec<String>,
    pub cells: Vec<String>,
}

/// Every table row of a Markdown document, skipping code blocks.
pub fn table_rows(markdown: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut section = String::new();
    let mut headers: Option<Vec<String>> = None;
    let mut in_code = false;
    for line in markdown.lines() {
        let line = line.trim();
        if line.starts_with("```") {
            in_code = !in_code;
        }
        if in_code {
            continue;
        }
        if line.starts_with('#') {
            section = line.trim_start_matches('#').trim().to_owned();
        }
        if !line.starts_with('|') {
            headers = None;
            continue;
        }
        let cells = split_row(line);
        if cells.iter().all(|cell| !cell.is_empty() && cell.chars().all(|c| matches!(c, '-' | ':'))) {
            continue;
        }
        match &headers {
            None => headers = Some(cells),
            Some(headers) => {
                rows.push(Row { section: section.clone(), headers: headers.clone(), cells });
            }
        }
    }
    rows
}

/// Splits a table line on `|`, except inside code spans or after a backslash.
fn split_row(line: &str) -> Vec<String> {
    let inner = line.trim_matches('|');
    let mut cells = vec![String::new()];
    let mut in_code = false;
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                chars.next();
                cells.last_mut().into_iter().for_each(|cell| cell.push('|'));
            }
            '`' => {
                in_code = !in_code;
                cells.last_mut().into_iter().for_each(|cell| cell.push('`'));
            }
            '|' if !in_code => cells.push(String::new()),
            c => cells.last_mut().into_iter().for_each(|cell| cell.push(c)),
        }
    }
    cells.iter().map(|cell| cell.trim().to_owned()).collect()
}

/// The group of a component in the README's components table.
pub fn component_group(name: &str) -> String {
    let rows = table_rows(README);
    let row = rows.iter().find(|row| {
        row.section == "Components"
            && row.cells.get(1).is_some_and(|cell| backticked_names(cell).contains(&name))
    });
    row.and_then(|row| row.cells.first()).map_or_else(|| "Other".to_owned(), Clone::clone)
}

/// The identifiers a table cell lists in backticks: `Button` from `` `Button` (4 styles) ``.
pub fn backticked_names(cell: &str) -> Vec<&str> {
    cell.split('`')
        .skip(1)
        .step_by(2)
        .filter_map(|code| code.split(|c: char| !c.is_alphanumeric() && c != '_').next())
        .filter(|name| !name.is_empty())
        .collect()
}

/// The component's row in the audit table of `docs/QUALITY.md`, if it has one.
pub fn quality_row(name: &str) -> Option<Value> {
    table_rows(QUALITY).into_iter().find_map(|row| {
        let listed =
            row.section == "Component audit" && row.cells.first().is_some_and(|cell| has_word(cell, name));
        listed.then(|| {
            let columns = row.headers.iter().zip(&row.cells).skip(1).filter(|(_, cell)| !cell.is_empty());
            Value::Object(columns.map(|(header, cell)| (header.clone(), json!(cell))).collect())
        })
    })
}

/// Rows of the parity documents that match every given filter, with counts.
pub fn parity(
    document: Option<&str>,
    query: Option<&str>,
    status: Option<&str>,
    limit: usize,
) -> Result<Value, String> {
    let documents: &[(&str, &str)] = match document {
        None => &[("components", PARITY), ("reporting", REPORTING_PARITY), ("application", APP_PARITY)],
        Some("components") => &[("components", PARITY)],
        Some("reporting") => &[("reporting", REPORTING_PARITY)],
        Some("application") => &[("application", APP_PARITY)],
        Some(other) => {
            return Err(format!(
                "unknown document '{other}': use 'components', 'reporting' or 'application'"
            ));
        }
    };
    if let Some(status) = status
        && !["done", "partial", "planned"].contains(&status)
    {
        return Err(format!("unknown status '{status}': use 'done', 'partial' or 'planned'"));
    }
    let query = query.map(str::to_lowercase);

    let mut matches = Vec::new();
    let (mut done, mut partial, mut planned) = (0, 0, 0);
    for (name, text) in documents {
        for row in table_rows(text) {
            // The status is in the last cell that has one: the last column of the component and
            // reporting trackers, the middle one of the application tracker.
            let Some((at, row_status)) = row
                .cells
                .iter()
                .enumerate()
                .skip(1)
                .rev()
                .find_map(|(at, cell)| row_status(cell).map(|s| (at, s)))
            else {
                continue;
            };
            let detail = &row.cells[at];
            if status.is_some_and(|status| status != row_status) {
                continue;
            }
            let searchable = format!("{} {}", row.section, row.cells.join(" ")).to_lowercase();
            if query.as_ref().is_some_and(|query| !searchable.contains(query)) {
                continue;
            }
            match row_status {
                "done" => done += 1,
                "partial" => partial += 1,
                _ => planned += 1,
            }
            let others = row.headers.iter().zip(&row.cells).enumerate().skip(1).filter(|(i, _)| *i != at);
            let reference: Map<String, Value> = others
                .map(|(_, pair)| pair)
                .filter(|(_, cell)| !cell.is_empty())
                .map(|(header, cell)| (header.clone(), json!(cell)))
                .collect();
            matches.push(json!({
                "document": name,
                "section": row.section,
                "feature": row.cells.first(),
                "status": row_status,
                "ravenrs": detail,
                "reference": reference,
            }));
        }
    }
    let total = matches.len();
    matches.truncate(limit);
    Ok(json!({
        "total": total,
        "done": done,
        "partial": partial,
        "planned": planned,
        "shown": matches.len(),
        "rows": matches,
    }))
}

/// One part of a guide: what is under a heading, up to the next heading.
struct Section<'a> {
    heading: &'a str,
    text: &'a str,
}

/// The parts of a Markdown document, skipping code blocks when looking for headings.
fn sections(markdown: &str) -> Vec<Section<'_>> {
    let mut starts: Vec<(usize, &str)> = Vec::new();
    let mut at = 0;
    let mut in_code = false;
    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim_end();
        if trimmed.trim_start().starts_with("```") {
            in_code = !in_code;
        } else if !in_code && trimmed.starts_with('#') {
            starts.push((at, trimmed.trim_start_matches('#').trim()));
        }
        at += line.len();
    }
    let mut parts = Vec::with_capacity(starts.len() + 1);
    if starts.first().is_none_or(|(first, _)| *first > 0) {
        let end = starts.first().map_or(markdown.len(), |(first, _)| *first);
        parts.push(Section { heading: "", text: &markdown[..end] });
    }
    for (position, (start, heading)) in starts.iter().enumerate() {
        let end = starts.get(position + 1).map_or(markdown.len(), |(next, _)| *next);
        parts.push(Section { heading, text: &markdown[*start..end] });
    }
    parts
}

fn matching_guides(document: Option<&str>) -> Result<Vec<(&'static str, &'static str)>, String> {
    let Some(document) = document else { return Ok(GUIDES.to_vec()) };
    let wanted = document.to_lowercase();
    let found: Vec<_> =
        GUIDES.iter().copied().filter(|(path, _)| path.to_lowercase().contains(&wanted)).collect();
    if found.is_empty() {
        return Err(format!("no guide matches '{document}'. Call guides with no arguments to list them."));
    }
    Ok(found)
}

/// A piece of `text` around the first of `words`, on whole characters.
fn excerpt(text: &str, words: &[String]) -> String {
    let lower = text.to_lowercase();
    let first = words.iter().filter_map(|word| lower.find(word.as_str())).min().unwrap_or(0);
    // Lowercasing can change byte lengths, so the place is only a hint, measured in characters.
    let at = lower[..first].chars().count();
    let start = at.saturating_sub(MAX_EXCERPT / 3);
    let piece: String = text.chars().skip(start).take(MAX_EXCERPT).collect();
    let piece = piece.trim();
    if start > 0 { format!("...{piece}") } else { piece.to_owned() }
}

/// The guides: all of them, the parts of some, the text of a part, or the parts that have some words.
pub fn guides(
    document: Option<&str>,
    section: Option<&str>,
    query: Option<&str>,
    limit: usize,
) -> Result<Value, String> {
    let chosen = matching_guides(document)?;
    if let Some(query) = query {
        let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_owned).collect();
        if words.is_empty() {
            return Err("the query has no words".to_owned());
        }
        let mut found = Vec::new();
        for (path, text) in &chosen {
            for part in sections(text) {
                let lower = part.text.to_lowercase();
                if !words.iter().all(|word| lower.contains(word.as_str())) {
                    continue;
                }
                let heading = part.heading.to_lowercase();
                let in_heading = words.iter().filter(|word| heading.contains(word.as_str())).count();
                let times: usize = words.iter().map(|word| lower.matches(word.as_str()).count()).sum();
                found.push((in_heading, times, *path, part.heading, excerpt(part.text, &words)));
            }
        }
        found.sort_by_key(|part| std::cmp::Reverse((part.0, part.1)));
        let total = found.len();
        found.truncate(limit);
        let matches: Vec<Value> = found
            .into_iter()
            .map(|(_, _, path, heading, text)| json!({ "document": path, "section": heading, "excerpt": text }))
            .collect();
        return Ok(json!({ "query": query, "total": total, "shown": matches.len(), "matches": matches }));
    }
    if let Some(section) = section {
        let wanted = section.to_lowercase();
        let mut found = Vec::new();
        for (path, text) in &chosen {
            for part in sections(text) {
                if part.heading.to_lowercase().contains(&wanted) {
                    let whole = part.text.len() <= MAX_SECTION;
                    let shown: String = part.text.chars().take(MAX_SECTION).collect();
                    found.push(json!({
                        "document": path,
                        "section": part.heading,
                        "text": shown,
                        "whole": whole,
                    }));
                }
            }
        }
        if found.is_empty() {
            return Err(format!("no section matches '{section}'. Pass only `document` to list them."));
        }
        found.truncate(limit);
        return Ok(json!({ "sections": found }));
    }
    if document.is_some() {
        let listed: Vec<Value> = chosen
            .iter()
            .map(|(path, text)| {
                let headings: Vec<&str> =
                    sections(text).into_iter().map(|part| part.heading).filter(|h| !h.is_empty()).collect();
                json!({ "document": path, "sections": headings })
            })
            .collect();
        return Ok(json!({ "documents": listed }));
    }
    let listed: Vec<Value> = chosen
        .iter()
        .map(|(path, text)| {
            let title = sections(text).into_iter().map(|part| part.heading).find(|h| !h.is_empty());
            json!({ "document": path, "title": title.unwrap_or(path) })
        })
        .collect();
    Ok(json!({ "total": listed.len(), "documents": listed }))
}

/// The scenarios C01–C19 and the groups M01–M19 of docs/APP-CONTRACTS.md, with the issues that own
/// them, only those whose text has the query when there is one.
pub fn app_contracts(query: Option<&str>) -> Value {
    let query = query.map(str::to_lowercase);
    let wanted = |cells: &[String]| {
        query.as_ref().is_none_or(|query| cells.join(" ").to_lowercase().contains(query.as_str()))
    };
    let (mut scenarios, mut groups) = (Vec::new(), Vec::new());
    for row in table_rows(APP_CONTRACTS) {
        if !wanted(&row.cells) {
            continue;
        }
        let entry: Map<String, Value> = row
            .headers
            .iter()
            .zip(&row.cells)
            .map(|(header, cell)| (header.to_lowercase(), json!(cell)))
            .collect();
        match row.headers.first().map(String::as_str) {
            Some("Scenario") => scenarios.push(Value::Object(entry)),
            Some("Group") => groups.push(Value::Object(entry)),
            _ => {}
        }
    }
    json!({
        "document": "docs/APP-CONTRACTS.md",
        "status": "docs/APP-PARITY.md, through parity_status with document 'application'",
        "scenarios": scenarios,
        "groups": groups,
    })
}

/// The status a parity cell opens with. A row like "✅ basics. Export ⬜" counts as done.
fn row_status(cell: &str) -> Option<&'static str> {
    cell.chars().find_map(|c| match c {
        '✅' => Some("done"),
        '🟡' => Some("partial"),
        '⬜' => Some("planned"),
        _ => None,
    })
}

/// Design tokens, flattened, with the Rust name where one exists.
pub fn tokens(category: Option<&str>, query: Option<&str>) -> Result<Value, String> {
    let category = match category {
        Some("type") => Some("typography"),
        other => other,
    };
    if let Some(category) = category
        && !TOKEN_CATEGORIES.contains(&category)
    {
        return Err(format!("unknown category '{category}': use one of {}", TOKEN_CATEGORIES.join(", ")));
    }
    let document: Value = serde_json::from_str(TOKENS).map_err(|error| error.to_string())?;
    let query = query.map(str::to_lowercase);

    let mut tokens = Vec::new();
    for name in TOKEN_CATEGORIES {
        if category.is_some_and(|category| category != name) {
            continue;
        }
        for token in category_tokens(&document, name) {
            let mut token = token.clone();
            let token_name = token["name"].as_str().unwrap_or_default().to_owned();
            if let Some(rust) = rust_name(name, &token_name) {
                token["rust"] = json!(rust);
            }
            token["category"] = json!(name);
            let matches = query.as_ref().is_none_or(|query| token.to_string().to_lowercase().contains(query));
            if matches {
                tokens.push(token);
            }
        }
    }
    Ok(json!({
        "themes": document["color"]["themes"],
        "total": tokens.len(),
        "tokens": tokens,
        "note": "In views, use the names in `rust` instead of literal values. Tokens without a `rust` name are read from the Theme, whose API is still evolving; check it with component_api (Theme).",
    }))
}

fn category_tokens<'a>(document: &'a Value, category: &str) -> Vec<&'a Value> {
    if category == "typography" {
        let groups = document["type"]["groups"].as_array().map_or(&[][..], Vec::as_slice);
        groups.iter().flat_map(|group| group["styles"].as_array().map_or(&[][..], Vec::as_slice)).collect()
    } else {
        document[category]["tokens"].as_array().map_or(Vec::new(), |tokens| tokens.iter().collect())
    }
}

/// The Rust name of a token, from the docs of `Semantic` and the variants of `TextStyle`.
fn rust_name(category: &str, token: &str) -> Option<String> {
    let (enum_name, matches): (&str, &dyn Fn(&crate::api::Member) -> bool) = match category {
        "color" => ("Semantic", &|variant| variant.docs.starts_with(&format!("`{token}`"))),
        "typography" => ("TextStyle", &|variant| variant.name.eq_ignore_ascii_case(token)),
        _ => return None,
    };
    let items = Index::get().find(enum_name);
    let variant = items.first()?.variants.iter().find(|variant| matches(variant))?;
    Some(format!("{enum_name}::{}", variant.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipes_in_code_spans_stay_in_their_cell() {
        assert_eq!(split_row("| a | `x || y` | b\\|c |"), ["a", "`x || y`", "b|c"]);
    }

    #[test]
    fn tables_need_a_header_and_skip_code() {
        let rows = table_rows("# T\n| A | B |\n|---|---|\n| 1 | 2 |\n```\n| 3 | 4 |\n```\n");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].section, "T");
        assert_eq!(rows[0].headers, ["A", "B"]);
        assert_eq!(rows[0].cells, ["1", "2"]);
    }

    #[test]
    fn every_color_token_and_type_style_has_a_rust_name() {
        let all = tokens(None, None).expect("tokens");
        for token in all["tokens"].as_array().expect("array") {
            if matches!(token["category"].as_str(), Some("color" | "typography")) {
                assert!(token["rust"].is_string(), "no Rust name for {token}");
            }
        }
    }

    #[test]
    fn every_category_in_the_token_file_is_served() {
        let document: Value = serde_json::from_str(TOKENS).expect("valid JSON");
        for (key, value) in document.as_object().expect("an object") {
            if value.get("tokens").is_some() || key == "type" {
                let name = if key == "type" { "typography" } else { key.as_str() };
                assert!(TOKEN_CATEGORIES.contains(&name), "design_tokens does not serve '{key}'");
                assert!(!category_tokens(&document, name).is_empty(), "no tokens in '{key}'");
            }
        }
    }

    #[test]
    fn a_guide_is_cut_at_its_headings_and_not_at_comments_in_its_code() {
        let parts = sections("intro\n# One\ntext\n```sh\n# not a heading\n```\n## Two\nmore\n");
        let headings: Vec<&str> = parts.iter().map(|part| part.heading).collect();
        assert_eq!(headings, ["", "One", "Two"]);
        assert!(parts[1].text.contains("not a heading"));
    }

    #[test]
    fn an_excerpt_does_not_break_a_character() {
        let text = "ação ".repeat(400);
        let piece = excerpt(&text, &["ação".to_owned()]);
        assert!(piece.chars().count() <= MAX_EXCERPT);
        let later = format!("{text}alvo{text}");
        assert!(excerpt(&later, &["alvo".to_owned()]).contains("alvo"));
    }

    #[test]
    fn every_component_in_the_readme_exists() {
        let index = Index::get();
        for row in table_rows(README).iter().filter(|row| row.section == "Components") {
            for name in row.cells.get(1).map(|cell| backticked_names(cell)).unwrap_or_default() {
                if name.starts_with(char::is_uppercase) {
                    assert!(!index.find(name).is_empty(), "README lists {name}, which the API does not have");
                }
            }
        }
    }
}
