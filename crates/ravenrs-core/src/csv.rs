//! CSV (RFC 4180), written the same way by every engine that exports it and read the same way by
//! everything that takes it in: quoted cells, line breaks inside them, a byte order mark, and `;`
//! or `,` between cells, as spreadsheets in different languages save it.

/// Appends `text` as one field, quoted when it holds `separator`, a quote or a line break.
///
/// ```
/// let mut line = String::new();
/// ravenrs_core::csv::push_field(&mut line, "Smith, \"J\"", ',');
/// assert_eq!(line, "\"Smith, \"\"J\"\"\"");
/// ```
pub fn push_field(out: &mut String, text: &str, separator: char) {
    if text.contains([separator, '"', '\n', '\r']) {
        out.push('"');
        out.push_str(&text.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(text);
    }
}

/// The cells of a whole CSV text, line by line, split by `separator`. A quoted cell can hold the
/// separator, a doubled quote and line breaks; `\r\n` ends a line like `\n`; a byte order mark at
/// the start is left out, and so is a last empty line.
///
/// ```
/// let lines = ravenrs_core::csv::read("name;city\n\"Ana; Bia\";\"São\nPaulo\"\n", ';');
/// assert_eq!(lines, vec![vec!["name", "city"], vec!["Ana; Bia", "São\nPaulo"]]);
/// ```
pub fn read(text: &str, separator: char) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = Vec::new();
    let (mut line, mut current, mut quoted) = (Vec::new(), String::new(), false);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                chars.next();
                current.push('"');
            }
            ('"', _) => quoted = !quoted,
            (c, false) if c == separator => line.push(std::mem::take(&mut current)),
            ('\n', false) => {
                line.push(std::mem::take(&mut current));
                lines.push(std::mem::take(&mut line));
            }
            ('\r', false) => {}
            (c, _) => current.push(c),
        }
    }
    if !current.is_empty() || !line.is_empty() {
        line.push(current);
        lines.push(line);
    }
    lines
}

/// The separator a CSV text uses: `;` when its first line, outside quotes, has more of them than
/// of `,`, as files saved where the comma is the decimal mark do; `,` otherwise. A tab wins over
/// both when the first line has more tabs.
pub fn separator_of(text: &str) -> char {
    let (mut commas, mut semicolons, mut tabs, mut quoted) = (0_usize, 0_usize, 0_usize, false);
    for c in text.strip_prefix('\u{feff}').unwrap_or(text).chars() {
        match (c, quoted) {
            ('"', _) => quoted = !quoted,
            ('\n', false) => break,
            (',', false) => commas += 1,
            (';', false) => semicolons += 1,
            ('\t', false) => tabs += 1,
            _ => {}
        }
    }
    if tabs > commas.max(semicolons) {
        '\t'
    } else if semicolons > commas {
        ';'
    } else {
        ','
    }
}

/// The cells of a CSV text with the separator it uses, from [`separator_of`].
pub fn read_any(text: &str) -> Vec<Vec<String>> {
    read(text, separator_of(text))
}

/// Text from the bytes of a CSV file: UTF-8, with or without a byte order mark, or else Latin-1
/// (Windows-1252), the way older spreadsheets in Portuguese save it.
pub fn text_of(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => bytes.iter().map(|&b| windows_1252(b)).collect(),
    }
}

/// A byte of Windows-1252, which is Latin-1 apart from a few printable signs in 0x80..0xA0.
fn windows_1252(byte: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
        '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    match byte {
        0x80..=0x9f => HIGH[usize::from(byte - 0x80)],
        _ => char::from(byte),
    }
}

#[cfg(test)]
mod tests {
    use super::{push_field, read, read_any, separator_of, text_of};

    fn field(text: &str, separator: char) -> String {
        let mut out = String::new();
        push_field(&mut out, text, separator);
        out
    }

    #[test]
    fn quotes_only_what_needs_it() {
        assert_eq!(field("plain", ','), "plain");
        assert_eq!(field("a,b", ','), "\"a,b\"");
        assert_eq!(field("a,b", ';'), "a,b");
        assert_eq!(field("a;b", ';'), "\"a;b\"");
        assert_eq!(field("line\nbreak", ','), "\"line\nbreak\"");
        assert_eq!(field("cr\r", ','), "\"cr\r\"");
        assert_eq!(field("say \"hi\"", ','), "\"say \"\"hi\"\"\"");
        assert_eq!(field("", ','), "");
    }

    #[test]
    fn reads_quotes_line_breaks_and_both_separators() {
        let text = "a,\"b, \"\"c\"\"\",\"d\ne\"\r\nf,,\n";
        assert_eq!(read(text, ','), vec![vec!["a", "b, \"c\"", "d\ne"], vec!["f", "", ""]]);
        assert_eq!(read_any("\u{feff}nome;valor\nAna;1,5\n"), vec![vec!["nome", "valor"], vec!["Ana", "1,5"]]);
        assert_eq!(read_any("x"), vec![vec!["x"]]);
        assert!(read_any("").is_empty());
    }

    #[test]
    fn a_separator_inside_quotes_does_not_count() {
        assert_eq!(separator_of("\"a;b;c\",d\n"), ',');
        assert_eq!(separator_of("a;b,c;d\n1,2,3,4,5"), ';');
        assert_eq!(separator_of("a\tb\tc"), '\t');
    }

    #[test]
    fn what_it_writes_it_reads_back() {
        let cells = ["plain", "a;b", "say \"hi\"", "two\nlines", ""];
        let mut line = String::new();
        for (at, cell) in cells.iter().enumerate() {
            if at > 0 {
                line.push(';');
            }
            push_field(&mut line, cell, ';');
        }
        assert_eq!(read(&line, ';'), vec![cells.to_vec()]);
    }

    #[test]
    fn bytes_become_text_in_utf8_or_latin1() {
        assert_eq!(text_of(b"\xEF\xBB\xBFS\xC3\xA3o"), "São");
        assert_eq!(text_of(b"S\xE3o \x80"), "São €");
    }
}
