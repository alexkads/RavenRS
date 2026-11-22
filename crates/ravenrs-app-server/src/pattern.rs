//! A rule's regular expression, as an OpenAPI `pattern` for clients to check before they send.
//!
//! The rules read patterns with `regex_lite`, while OpenAPI reads them as ECMA-262, with or without
//! the `u` flag depending on the client. A pattern goes out only when every one of those readings
//! takes exactly what the rule takes. Otherwise a client would turn down what the server accepts,
//! which is worse than not knowing the rule: the server still checks every write.
//!
//! `regex_lite` steps character by character, and so does ECMA-262 with `u`; without it, a
//! character outside the Basic Multilingual Plane is two halves. So whatever can match "any
//! character" (`.`, a negated class, `\D`, `\S`, `\W`) goes out as either a pair of halves or one
//! character that is no half, which is one character in both readings. JSON carries no lone half,
//! as the server refuses one.
//!
//! What still drifts between them stays out: flags such as `(?i)`; word boundaries, Unicode
//! classes, nested classes and set operations; and escapes that `u` refuses. The Perl classes are
//! ASCII in `regex_lite`, so they go out spelled out.

/// One character outside the Basic Multilingual Plane, as the two halves a reading without `u` sees.
const PAIR: &str = r"[\uD800-\uDBFF][\uDC00-\uDFFF]";

/// The halves, which no character of the Basic Multilingual Plane is.
const HALVES: &str = r"\uD800-\uDFFF";

/// Any character but those in a negated class, one character whatever the reading.
fn any_but(class: &str) -> String {
    format!("(?:{PAIR}|[^{HALVES}{class}])")
}

/// Any character at all, one character whatever the reading, and as `regex_lite` would read it
/// once the halves are taken out.
fn any() -> String {
    format!(r"(?:{PAIR}|[^{HALVES}\n]|\n)")
}

/// Whether every alternative at the top of a translated pattern starts at the start of the text.
fn anchored(translated: &str) -> bool {
    let (mut depth, mut class, mut escaped) = (0usize, false, false);
    let mut alternatives = vec![String::new()];
    for c in translated.chars() {
        let top = depth == 0 && !class && !escaped;
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '[' if !class => class = true,
            ']' if class => class = false,
            '(' if !class => depth += 1,
            ')' if !class => depth = depth.saturating_sub(1),
            '|' if top => {
                alternatives.push(String::new());
                continue;
            }
            _ => {}
        }
        if let Some(last) = alternatives.last_mut() {
            last.push(c);
        }
    }
    alternatives.iter().all(|alternative| alternative.starts_with('^'))
}

/// The pattern so that it is found anywhere in the text, as JSON Schema means it, also by a client
/// that only tries it at the start, as Python's `re.match` does. What already starts there stays
/// as it is; the rest is let in after as little as needed of anything.
pub(crate) fn anywhere(translated: &str) -> String {
    if anchored(translated) { translated.to_owned() } else { format!("{}*?(?:{translated})", any()) }
}

/// What `\d`, `\s` and `\w` hold in `regex_lite`, inside a class.
fn perl_class(escape: char) -> Option<&'static str> {
    match escape {
        'd' => Some("0-9"),
        's' => Some(r"\t\n\v\f\r "),
        'w' => Some("0-9A-Za-z_"),
        _ => None,
    }
}

/// Whether ECMA-262 reads `\` and this as the character itself, with and without `u`.
fn escapes_itself(c: char) -> bool {
    matches!(c, '^' | '$' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '/')
}

/// What an escape outside a class is, or none if it cannot be said the same way.
fn escape_outside(escape: char) -> Option<String> {
    if let Some(class) = perl_class(escape) {
        return Some(format!("[{class}]"));
    }
    if let Some(class) = perl_class(escape.to_ascii_lowercase()).filter(|_| escape.is_ascii_uppercase()) {
        return Some(any_but(class));
    }
    match escape {
        'n' | 't' | 'r' | 'f' | 'v' => Some(format!("\\{escape}")),
        c if escapes_itself(c) => Some(format!("\\{c}")),
        // `regex_lite` takes `\<` and `\>` as word boundaries.
        '<' | '>' => None,
        // Other punctuation is itself in `regex_lite`; `u` refuses the escape, so it goes bare.
        c if c.is_ascii_punctuation() || c == ' ' => Some(c.to_string()),
        _ => None,
    }
}

/// How many times, as `{n}`, `{n,}` or `{n,m}`, starting at `at` on the `{`, and where it ends.
fn repetition(chars: &[char], at: usize) -> Option<(String, usize)> {
    let close = chars[at..].iter().position(|c| *c == '}')? + at;
    let inside: String = chars[at + 1..close].iter().collect();
    let (low, high) = inside.split_once(',').map_or((inside.as_str(), None), |(l, h)| (l, Some(h)));
    let digits = |text: &str| !text.is_empty() && text.chars().all(|c| c.is_ascii_digit());
    (digits(low) && high.is_none_or(|high| high.is_empty() || digits(high)))
        .then(|| (format!("{{{inside}}}"), close))
}

/// The pattern as ECMA-262 takes it, or none when some client could read it otherwise.
pub(crate) fn ecma(pattern: &str) -> Option<String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len() + 8);
    // Where the class being read began, past its `[` or `[^`, and whether it is negated.
    let mut class: Option<usize> = None;
    let mut negated = false;
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if u32::from(c) > 0xFFFF {
            return None;
        }
        match (class, c) {
            (Some(start), ']') if at > start => {
                out.push(']');
                if negated {
                    out.push(')');
                }
                class = None;
            }
            // `[]a]` holds `]` for `regex_lite`, and nothing at all for ECMA-262.
            (Some(_), ']' | '[') => return None,
            (Some(_), '&' | '~' | '-') if chars.get(at + 1) == Some(&c) => return None,
            (Some(start), '\\') => {
                at += 1;
                let escape = *chars.get(at)?;
                if let Some(held) = perl_class(escape) {
                    // A class is no end of a range.
                    let before = at >= start + 2 && chars[at - 2] == '-';
                    if before || chars.get(at + 1) == Some(&'-') {
                        return None;
                    }
                    out.push_str(held);
                } else {
                    match escape {
                        'n' | 't' | 'r' | 'f' | 'v' => out.extend(['\\', escape]),
                        e if escapes_itself(e) || e == '-' => out.extend(['\\', e]),
                        '<' | '>' => return None,
                        e if e.is_ascii_punctuation() || e == ' ' => out.push(e),
                        _ => return None,
                    }
                }
            }
            (Some(_), '/') => out.push_str(r"\/"),
            (Some(_), other) => out.push(other),
            (None, '\\') => {
                at += 1;
                out.push_str(&escape_outside(*chars.get(at)?)?);
            }
            (None, '.') => out.push_str(&any_but(r"\n")),
            (None, ']' | '}') => return None,
            (None, '[') => {
                negated = chars.get(at + 1) == Some(&'^');
                if negated {
                    // The halves go first, so nothing in the class can make a range of them.
                    out.push_str(&format!("(?:{PAIR}|[^{HALVES}"));
                    at += 1;
                } else {
                    out.push('[');
                }
                class = Some(at + 1);
            }
            (None, '(') if chars.get(at + 1) == Some(&'?') => {
                if chars.get(at + 2) != Some(&':') {
                    return None;
                }
                out.push_str("(?:");
                at += 2;
            }
            (None, '{') => {
                let (counted, end) = repetition(&chars, at)?;
                out.push_str(&counted);
                at = end;
            }
            (None, '/') => out.push_str(r"\/"),
            (None, other) => out.push(other),
        }
        at += 1;
    }
    (class.is_none() && !out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use regex_lite::Regex;

    use super::{HALVES, PAIR, anywhere, ecma};

    /// The translation as `regex_lite` can read it, which knows no halves: what is left once the
    /// pair and the halves come out is what the rest of the translation made of the pattern.
    fn without_halves(translated: &str) -> String {
        translated.replace(&format!("{PAIR}|"), "").replace(HALVES, "")
    }

    #[test]
    fn what_reads_the_same_everywhere_goes_out() {
        assert_eq!(ecma("^[A-Z]{2}[0-9]+$").as_deref(), Some("^[A-Z]{2}[0-9]+$"));
        assert_eq!(ecma(r"^\d{5}-\d{3}$").as_deref(), Some("^[0-9]{5}-[0-9]{3}$"));
        assert_eq!(ecma(r"^[\w.-]+$").as_deref(), Some("^[0-9A-Za-z_.-]+$"));
        assert_eq!(ecma(r"^[\w\-]+$").as_deref(), Some(r"^[0-9A-Za-z_\-]+$"));
        assert_eq!(ecma(r"a\sb").as_deref(), Some(r"a[\t\n\v\f\r ]b"));
        assert_eq!(ecma("^(?:ab|cd){1,3}$").as_deref(), Some("^(?:ab|cd){1,3}$"));
        assert_eq!(ecma(r"^\(\d+\)$").as_deref(), Some(r"^\([0-9]+\)$"));
        assert_eq!(ecma(r"a\@b\-c").as_deref(), Some("a@b-c"), "u refuses \\@ and \\- outside a class");
        assert_eq!(ecma("a/b").as_deref(), Some(r"a\/b"));
        assert_eq!(ecma("^[À-ÿ]+$").as_deref(), Some("^[À-ÿ]+$"));
    }

    #[test]
    fn what_can_be_anywhere_is_let_in_after_anything() {
        assert_eq!(anywhere("^[A-Z]+$"), "^[A-Z]+$", "it already starts at the start");
        assert_eq!(anywhere("^a|^b"), "^a|^b");
        let any = format!(r"(?:{PAIR}|[^{HALVES}\n]|\n)");
        assert_eq!(anywhere("[0-9]"), format!("{any}*?(?:[0-9])"));
        assert_eq!(anywhere("^a|b"), format!("{any}*?(?:^a|b)"), "one alternative is enough");
        assert_eq!(anywhere(r"\^a"), format!(r"{any}*?(?:\^a)"), "an escaped ^ is a character");
        assert_eq!(anywhere("[|]"), format!("{any}*?(?:[|])"), "a | in a class is a character");
    }

    #[test]
    fn any_character_is_one_character_in_every_reading() {
        let pair = r"[\uD800-\uDBFF][\uDC00-\uDFFF]";
        assert_eq!(ecma("^.+$").unwrap(), format!(r"^(?:{pair}|[^\uD800-\uDFFF\n])+$"));
        assert_eq!(ecma("[^@]+").unwrap(), format!(r"(?:{pair}|[^\uD800-\uDFFF@])+"));
        assert_eq!(ecma(r"[^\d@]").unwrap(), format!(r"(?:{pair}|[^\uD800-\uDFFF0-9@])"));
        assert_eq!(ecma(r"\D").unwrap(), format!(r"(?:{pair}|[^\uD800-\uDFFF0-9])"));
        assert_eq!(ecma(r"^\S{1,3}$").unwrap(), format!(r"^(?:{pair}|[^\uD800-\uDFFF\t\n\v\f\r ]){{1,3}}$"));
    }

    #[test]
    fn what_some_client_could_read_otherwise_stays_out() {
        for pattern in [
            "",
            r"\bword\b",
            r"[\D]",
            r"[^\S]",
            "[^]a]",
            r"\p{L}",
            "(?i)abc",
            "(?P<n>a)",
            "(?=a)",
            "[[:alpha:]]",
            "[a-z&&[^x]]",
            "[a--b]",
            "[]a]",
            r"[a-\d]",
            r"[\d-z]",
            r"\<a\>",
            r"\x41",
            r"\u{41}",
            r"(a)\1",
            "a{",
            "a{x}",
            "a}",
            "[abc",
            "😀",
            "\\",
        ] {
            assert_eq!(ecma(pattern), None, "{pattern:?}");
        }
    }

    #[test]
    fn the_translation_means_what_the_rule_means() {
        let inputs = [
            "",
            " ",
            "\u{85}",
            "AB12",
            "ABC-1234",
            "😀",
            "a😀b",
            "12345-678",
            "x_y.z-",
            "a b",
            "a\tb",
            "(42)",
            "a@b-c",
            "a/b",
            "ção",
            "abcdab",
            "😀😀😀",
            "a\nb",
            "@😀@",
        ];
        for pattern in [
            "^[A-Z]{2}[0-9]+$",
            r"^\d{5}-\d{3}$",
            r"^[\w.-]+$",
            r"^[\w\-]+$",
            r"a\sb",
            "^(?:ab|cd){1,3}$",
            r"^\(\d+\)$",
            r"a\@b\-c",
            "a/b",
            "^[À-ÿ]+$",
            "^.+$",
            "^.{1,2}$",
            "a.b",
            "^[^@]+@[^@]+$",
            r"^[^\d@]+$",
            r"^\D+$",
            r"^\S{1,3}$",
            r"^\W$",
            r"\d",
            "@",
            "^a|b$",
            "(?:^x)|y",
            r"[^\s]$",
        ] {
            let translated = anywhere(&ecma(pattern).unwrap());
            let readable = without_halves(&translated);
            let (rule, sent) = (Regex::new(pattern).unwrap(), Regex::new(&readable).unwrap());
            // Read only at the start, as Python's `re.match` reads it, it still means the same.
            let at_start = Regex::new(&format!("^(?:{readable})")).unwrap();
            for input in inputs {
                assert_eq!(
                    rule.is_match(input),
                    sent.is_match(input),
                    "{pattern:?} as {readable:?} on {input:?}"
                );
                assert_eq!(
                    rule.is_match(input),
                    at_start.is_match(input),
                    "{pattern:?} at the start on {input:?}"
                );
            }
        }
    }
}
