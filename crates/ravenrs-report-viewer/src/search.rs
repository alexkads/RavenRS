//! Finds text in a document, ignoring case and accents: "regiao" finds "Região".

use ravenrs_report::{Document, Item};

/// Where a search result is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    /// The page.
    pub page: usize,
    /// The text block's position in the page's items.
    pub item: usize,
    /// The line within the block.
    pub line: usize,
    /// Byte range of the match in the line's text.
    pub range: std::ops::Range<usize>,
}

/// Folds a character to what searches compare: lowercase, without accents.
fn fold(c: char) -> impl Iterator<Item = char> {
    c.to_lowercase().map(|c| match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        // A number formatted with a non-breaking space is found with a plain one.
        '\u{a0}' => ' ',
        other => other,
    })
}

/// Every match of `query`, in reading order of the document's blocks.
pub fn find_matches(document: &Document, query: &str) -> Vec<Match> {
    let needle: Vec<char> = query.trim().chars().flat_map(fold).collect();
    let mut matches = Vec::new();
    if needle.is_empty() {
        return matches;
    }
    for (page_index, page) in document.pages.iter().enumerate() {
        for (item_index, item) in page.items.iter().enumerate() {
            let Item::Text(block) = item else { continue };
            for (line_index, line) in block.lines.iter().enumerate() {
                find_in_line(&line.text, &needle, |range| {
                    matches.push(Match { page: page_index, item: item_index, line: line_index, range });
                });
            }
        }
    }
    matches
}

fn find_in_line(text: &str, needle: &[char], mut found: impl FnMut(std::ops::Range<usize>)) {
    // Folded characters with the byte each came from, since folding can change lengths.
    let folded: Vec<(char, usize)> =
        text.char_indices().flat_map(|(at, c)| fold(c).map(move |f| (f, at))).collect();
    let mut start = 0;
    while start + needle.len() <= folded.len() {
        if folded[start..start + needle.len()].iter().map(|(c, _)| c).eq(needle.iter()) {
            let from = folded[start].1;
            let last = folded[start + needle.len() - 1].1;
            let end = last + text[last..].chars().next().map_or(0, char::len_utf8);
            found(from..end);
            start += needle.len();
        } else {
            start += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use ravenrs_report::{Band, Control, Report, ReportEngine};
    use serde_json::json;

    use super::*;

    fn document() -> Document {
        let report =
            Report::new("T").detail(Band::new(20.0).control(Control::field("[A]").at(0.0, 0.0, 400.0, 20.0)));
        ReportEngine::new().render(
            &report,
            &json!([{ "A": "Região Sul, região norte" }, { "A": "Ação" }, { "A": "R$\u{a0}10,00" }, { "A": "İstanbul" }]),
        )
    }

    fn found(query: &str) -> Vec<String> {
        let doc = document();
        find_matches(&doc, query)
            .into_iter()
            .map(|m| {
                let Item::Text(block) = &doc.pages[m.page].items[m.item] else { panic!("not text") };
                block.lines[m.line].text[m.range].to_owned()
            })
            .collect()
    }

    #[test]
    fn case_and_accents_do_not_matter() {
        assert_eq!(found("regiao"), ["Região", "região"]);
        assert_eq!(found("REGIÃO"), ["Região", "região"]);
        assert_eq!(found("acao"), ["Ação"]);
    }

    #[test]
    fn a_plain_space_finds_a_non_breaking_one() {
        assert_eq!(found("r$ 10"), ["R$\u{a0}10"]);
    }

    #[test]
    fn nothing_to_find_finds_nothing() {
        assert!(found("").is_empty());
        assert!(found("   ").is_empty());
        assert!(found("xyz").is_empty());
    }

    #[test]
    fn characters_that_change_length_when_folded_keep_valid_ranges() {
        assert_eq!(found("stanbul"), ["stanbul"]);
        assert_eq!(found("i"), ["i", "i", "İ"]);
    }

    #[test]
    fn matches_do_not_overlap() {
        let report =
            Report::new("T").detail(Band::new(20.0).control(Control::field("[A]").at(0.0, 0.0, 400.0, 20.0)));
        let doc = ReportEngine::new().render(&report, &json!([{ "A": "aaaa" }]));
        assert_eq!(find_matches(&doc, "aa").len(), 2);
    }
}
