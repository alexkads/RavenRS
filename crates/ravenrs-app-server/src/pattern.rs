//! A rule's regular expression, as an OpenAPI `pattern` for clients to check before they send.
//!
//! The rules read patterns with `regex_lite`, while OpenAPI reads them as ECMA-262, with or without
//! the `u` flag depending on the client. A pattern goes out only when every one of those readings
//! takes exactly what the rule takes. Otherwise a client would turn down what the server accepts,
//! which is worse than not knowing the rule: the server still checks every write.
//!
//! So anything whose meaning drifts between them stays out: `.` and negated classes, which step
//! over a character outside the Basic Multilingual Plane differently without `u`; flags such as
//! `(?i)`; word boundaries, Unicode classes, nested classes and set operations; and escapes that
//! `u` refuses. The Perl classes are ASCII in `regex_lite`, so they go out spelled out.

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
    // Where the class being read began, past its `[`.
    let mut class: Option<usize> = None;
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if u32::from(c) > 0xFFFF {
            return None;
        }
        match (class, c) {
            (Some(start), ']') if at > start => {
                out.push(']');
                class = None;
            }
            // `[]a]` holds `]` for `regex_lite`, and nothing at all for ECMA-262.
            (Some(_), ']' | '[') => return None,
            (Some(start), '^') if at == start => return None,
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
            (None, '.' | ']' | '}') => return None,
            (None, '[') => {
                out.push('[');
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

    use super::ecma;

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
    fn what_some_client_could_read_otherwise_stays_out() {
        for pattern in [
            "",
            "^.+$",
            "[^@]+",
            r"\D",
            r"\S+",
            r"\bword\b",
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
        ] {
            let translated = ecma(pattern).unwrap();
            let (rule, sent) = (Regex::new(pattern).unwrap(), Regex::new(&translated).unwrap());
            for input in inputs {
                assert_eq!(
                    rule.is_match(input),
                    sent.is_match(input),
                    "{pattern:?} as {translated:?} on {input:?}"
                );
            }
        }
    }
}
