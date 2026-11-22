//! Reading the colors a program writes into its output (ANSI SGR), for [`LogView`](super::LogView).
//!
//! Only what build logs use is read: the 8 colors and their bright forms (30-37, 90-97), bold
//! (1), and the reset (0, 22, 39). Every other escape sequence is taken out, so a log never shows
//! raw `ESC[` codes. Colors map to the theme's semantic colors, so they stay readable in every
//! theme.

use ravenrs_style::Semantic;

/// A piece of a line, with how it looks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) text: String,
    pub(crate) color: Option<Semantic>,
    pub(crate) bold: bool,
}

/// The theme's color for an SGR color code.
fn color_of(code: u32) -> Option<Semantic> {
    match code {
        30 | 90 => Some(Semantic::InkMuted),
        31 | 91 => Some(Semantic::Danger),
        32 | 92 => Some(Semantic::Success),
        33 | 93 => Some(Semantic::Warning),
        34 | 94 => Some(Semantic::Accent),
        35 | 95 => Some(Semantic::Chart3),
        36 | 96 => Some(Semantic::Chart2),
        37 | 97 => Some(Semantic::Ink),
        _ => None,
    }
}

/// `line` as pieces of text with their look, and the text alone, with every escape taken out.
pub(crate) fn parse(line: &str) -> (Vec<Span>, String) {
    let mut spans: Vec<Span> = Vec::new();
    let mut plain = String::new();
    let (mut color, mut bold) = (None, false);
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let flush = |current: &mut String, spans: &mut Vec<Span>, color, bold| {
        if !current.is_empty() {
            spans.push(Span { text: std::mem::take(current), color, bold });
        }
    };
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            // Other control characters would show as boxes.
            if !c.is_control() || c == '\t' {
                current.push(c);
                plain.push(c);
            }
            continue;
        }
        if chars.peek() != Some(&'[') {
            // A lone ESC, or another kind of sequence: drop the next character with it.
            chars.next();
            continue;
        }
        chars.next();
        let mut params = String::new();
        let mut last = None;
        for next in chars.by_ref() {
            if next.is_ascii_digit() || next == ';' {
                params.push(next);
            } else {
                last = Some(next);
                break;
            }
        }
        if last != Some('m') {
            continue;
        }
        flush(&mut current, &mut spans, color, bold);
        let codes: Vec<u32> = if params.is_empty() {
            vec![0]
        } else {
            params.split(';').map(|code| code.parse().unwrap_or(0)).collect()
        };
        for code in codes {
            match code {
                0 => (color, bold) = (None, false),
                1 => bold = true,
                22 => bold = false,
                39 => color = None,
                other => {
                    if let Some(found) = color_of(other) {
                        color = Some(found);
                    }
                }
            }
        }
    }
    flush(&mut current, &mut spans, color, bold);
    (spans, plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(text: &str, color: Option<Semantic>, bold: bool) -> Span {
        Span { text: text.to_owned(), color, bold }
    }

    #[test]
    fn colors_bold_and_reset() {
        let (spans, plain) = parse("\u{1b}[32mPASS\u{1b}[0m teste \u{1b}[1;31mFAIL\u{1b}[22m fim");
        assert_eq!(plain, "PASS teste FAIL fim");
        assert_eq!(
            spans,
            vec![
                span("PASS", Some(Semantic::Success), false),
                span(" teste ", None, false),
                span("FAIL", Some(Semantic::Danger), true),
                span(" fim", Some(Semantic::Danger), false),
            ]
        );
    }

    #[test]
    fn bright_colors_and_the_default_color() {
        let (spans, _) = parse("\u{1b}[93mlento\u{1b}[39m normal");
        assert_eq!(spans, vec![span("lento", Some(Semantic::Warning), false), span(" normal", None, false)]);
    }

    #[test]
    fn other_sequences_and_controls_are_taken_out() {
        let (spans, plain) = parse("\u{1b}[2K\u{1b}[1Gbaixando\u{7} 50%\u{1b}]");
        assert_eq!(plain, "baixando 50%");
        assert_eq!(spans, vec![span("baixando 50%", None, false)]);
    }

    #[test]
    fn a_plain_line_is_one_span() {
        let (spans, plain) = parse("cargo nextest run");
        assert_eq!(plain, "cargo nextest run");
        assert_eq!(spans.len(), 1);
    }
}
