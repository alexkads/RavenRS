//! CSV fields (RFC 4180), written the same way by every engine that exports CSV.

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

#[cfg(test)]
mod tests {
    use super::push_field;

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
}
