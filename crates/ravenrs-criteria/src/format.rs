//! Culture-aware formatting with .NET-style format strings, as report
//! designers know them: `{0:C2}`, `{0:N0}`, `{0:dd/MM/yyyy}`, `{0:P1}`.

use crate::value::{DateTime, Value};

/// Number, currency and date conventions of a language and region.
#[derive(Clone, Debug, PartialEq)]
pub struct Culture {
    /// The culture name, e.g. `pt-BR`.
    pub name: &'static str,
    /// Decimal separator.
    pub decimal: char,
    /// Thousands separator.
    pub group: char,
    /// Currency symbol.
    pub currency: &'static str,
    /// Currency symbol before the number, with a space between them when true.
    pub currency_space: bool,
    /// Short date pattern (`d`).
    pub short_date: &'static str,
    /// Long date pattern (`D`).
    pub long_date: &'static str,
    /// Short time pattern (`t`).
    pub short_time: &'static str,
    /// Month names, January first.
    pub months: [&'static str; 12],
    /// Day names, Sunday first.
    pub days: [&'static str; 7],
    /// Words for true and false.
    pub yes_no: [&'static str; 2],
}

const PT_BR: Culture = Culture {
    name: "pt-BR",
    decimal: ',',
    group: '.',
    currency: "R$",
    currency_space: true,
    short_date: "dd/MM/yyyy",
    long_date: "dddd, d 'de' MMMM 'de' yyyy",
    short_time: "HH:mm",
    months: [
        "janeiro",
        "fevereiro",
        "março",
        "abril",
        "maio",
        "junho",
        "julho",
        "agosto",
        "setembro",
        "outubro",
        "novembro",
        "dezembro",
    ],
    days: [
        "domingo",
        "segunda-feira",
        "terça-feira",
        "quarta-feira",
        "quinta-feira",
        "sexta-feira",
        "sábado",
    ],
    yes_no: ["Sim", "Não"],
};

const EN_US: Culture = Culture {
    name: "en-US",
    decimal: '.',
    group: ',',
    currency: "$",
    currency_space: false,
    short_date: "M/d/yyyy",
    long_date: "dddd, MMMM d, yyyy",
    short_time: "h:mm tt",
    months: [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ],
    days: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
    yes_no: ["Yes", "No"],
};

impl Culture {
    /// Brazilian Portuguese.
    pub fn pt_br() -> Self {
        PT_BR
    }

    /// US English.
    pub fn en_us() -> Self {
        EN_US
    }

    /// The culture for a name such as `pt-BR` or `en`, falling back to `en-US`.
    pub fn named(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "pt-br" | "pt" | "pt_br" => PT_BR,
            _ => EN_US,
        }
    }

    /// `1234567.891` with `decimals` places and thousands separators: `1.234.567,89`.
    pub fn number(&self, n: f64, decimals: usize, grouping: bool) -> String {
        NumberStyle {
            decimals,
            group: grouping.then_some(self.group),
            decimal: self.decimal,
            prefix: "",
            suffix: "",
        }
        .format(n)
    }

    /// A currency amount: `R$ 1.234,50` or `$1,234.50`; negatives as `-R$ 1,00`.
    pub fn currency(&self, n: f64, decimals: usize) -> String {
        let space = if self.currency_space { "\u{a0}" } else { "" };
        NumberStyle {
            decimals,
            group: Some(self.group),
            decimal: self.decimal,
            prefix: &format!("{}{space}", self.currency),
            suffix: "",
        }
        .format(n)
    }
}

/// How to write a number: decimal places, separators and text around it. This is the one place
/// that lays out digits, for [`Culture`] and for anything with separators of its own.
///
/// ```
/// use ravenrs_criteria::NumberStyle;
///
/// let style = NumberStyle { decimals: 2, group: Some('.'), decimal: ',', prefix: "R$ ", suffix: "" };
/// assert_eq!(style.format(-1234.5), "-R$ 1.234,50");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberStyle<'a> {
    /// Digits after the decimal separator.
    pub decimals: usize,
    /// Thousands separator, if digits are grouped.
    pub group: Option<char>,
    /// Decimal separator.
    pub decimal: char,
    /// Text before the number, after the sign (e.g. a currency symbol).
    pub prefix: &'a str,
    /// Text after the number (e.g. `%`).
    pub suffix: &'a str,
}

impl NumberStyle<'_> {
    /// Writes `n`, rounded to the style's decimals. A value that rounds to zero has no sign.
    pub fn format(&self, n: f64) -> String {
        let fixed = format!("{:.*}", self.decimals, n.abs());
        let (int, frac) = fixed.split_once('.').unwrap_or((&fixed, ""));
        let mut out =
            String::with_capacity(fixed.len() + int.len() / 3 + self.prefix.len() + self.suffix.len());
        if n < 0.0 && fixed.chars().any(|c| c.is_ascii_digit() && c != '0') {
            out.push('-');
        }
        out.push_str(self.prefix);
        for (i, c) in int.chars().enumerate() {
            if let Some(group) = self.group
                && i > 0
                && (int.len() - i) % 3 == 0
            {
                out.push(group);
            }
            out.push(c);
        }
        if !frac.is_empty() {
            out.push(self.decimal);
            out.push_str(frac);
        }
        out.push_str(self.suffix);
        out
    }
}

/// Formats `value` with a .NET-style format specifier (`C2`, `N0`,
/// `dd/MM/yyyy`, `#,##0.00`...). An empty specifier gives the plain text.
pub fn format_value(value: &Value, spec: &str, culture: &Culture) -> String {
    let spec = spec.trim();
    match value {
        Value::Null => String::new(),
        Value::Number(n) => format_number(*n, spec, culture),
        Value::Date(d) => format_date(*d, spec, culture),
        Value::Bool(b) if spec.is_empty() => culture.yes_no[usize::from(!*b)].to_owned(),
        Value::Text(s) if !spec.is_empty() => {
            // Text that looks like a number or a date takes number and date formats.
            if let Some(d) = DateTime::parse(s).filter(|_| is_date_spec(spec)) {
                format_date(d, spec, culture)
            } else if let Some(n) = s.trim().parse::<f64>().ok().filter(|_| !is_date_spec(spec)) {
                format_number(n, spec, culture)
            } else {
                s.clone()
            }
        }
        other => other.to_text(),
    }
}

fn is_date_spec(spec: &str) -> bool {
    matches!(spec, "d" | "D" | "t" | "T" | "g" | "G" | "f" | "F" | "M" | "m" | "Y" | "y")
        || spec.contains("yy")
        || spec.contains("MM")
        || spec.contains("dd")
        || spec.contains("HH")
        || spec.contains("mm")
}

/// Formats a composite string: `{0}`, `{1:C2}`, `{{` and `}}` for braces.
pub fn format_composite(pattern: &str, args: &[Value], culture: &Culture) -> String {
    let mut out = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut inner = String::new();
                for c in chars.by_ref() {
                    if c == '}' {
                        break;
                    }
                    inner.push(c);
                }
                let (index, spec) = inner.split_once(':').unwrap_or((&inner, ""));
                let index = index.split(',').next().unwrap_or_default().trim();
                match index.parse::<usize>().ok().and_then(|i| args.get(i)) {
                    Some(v) => out.push_str(&format_value(v, spec, culture)),
                    None => {
                        out.push('{');
                        out.push_str(&inner);
                        out.push('}');
                    }
                }
            }
            c => out.push(c),
        }
    }
    out
}

fn format_number(n: f64, spec: &str, culture: &Culture) -> String {
    if spec.is_empty() || spec.eq_ignore_ascii_case("g") {
        let plain = crate::value::format_plain(n);
        return plain.replace('.', &culture.decimal.to_string());
    }
    let mut chars = spec.chars();
    let kind = chars.next().unwrap_or('G');
    let digits: Option<usize> = chars.as_str().parse().ok();
    let standard = chars.as_str().is_empty() || digits.is_some();
    if standard {
        match kind.to_ascii_uppercase() {
            'C' => return culture.currency(n, digits.unwrap_or(2)),
            'N' => return culture.number(n, digits.unwrap_or(2), true),
            'F' => return culture.number(n, digits.unwrap_or(2), false),
            'P' => return format!("{}%", culture.number(n * 100.0, digits.unwrap_or(2), true)),
            'D' => {
                let s = format!("{:0width$}", (n.round() as i64).abs(), width = digits.unwrap_or(1));
                return if n < 0.0 { format!("-{s}") } else { s };
            }
            'X' => {
                let s = format!("{:0width$X}", n.round() as i64, width = digits.unwrap_or(1));
                return if kind == 'x' { s.to_lowercase() } else { s };
            }
            _ => {}
        }
    }
    format_custom_number(n, spec, culture)
}

/// Custom numeric patterns: `#,##0.00`, `0.0%`, `000`, with literal text around
/// them and an optional `;` section for negatives.
fn format_custom_number(n: f64, spec: &str, culture: &Culture) -> String {
    let (spec, n) = match spec.split_once(';') {
        Some((_, negative)) if n < 0.0 => (negative, n.abs()),
        Some((positive, _)) => (positive, n),
        None => (spec, n),
    };
    let is_digit = |c: char| matches!(c, '0' | '#' | ',' | '.');
    let start = spec.find(is_digit);
    let end = spec.rfind(is_digit).map(|i| i + 1);
    let (Some(start), Some(end)) = (start, end) else { return spec.to_owned() };
    let (prefix, pattern, suffix) = (&spec[..start], &spec[start..end], &spec[end..]);
    let percent = prefix.contains('%') || suffix.contains('%');
    let value = if percent { n * 100.0 } else { n };
    let (int_pat, frac_pat) = pattern.split_once('.').unwrap_or((pattern, ""));
    let min_frac = frac_pat.chars().filter(|c| *c == '0').count();
    let max_frac = frac_pat.chars().filter(|c| matches!(c, '0' | '#')).count();
    let min_int = int_pat.chars().filter(|c| *c == '0').count();
    let mut body = culture.number(value, max_frac, int_pat.contains(','));
    // Drop optional (`#`) trailing decimals that are zero.
    if max_frac > min_frac
        && let Some(dot) = body.rfind(culture.decimal)
    {
        let mut keep = body.len();
        while keep > dot + 1 + min_frac && body[..keep].ends_with('0') {
            keep -= 1;
        }
        body.truncate(keep);
        if body.ends_with(culture.decimal) {
            body.pop();
        }
    }
    // Pad the integer part to the required zeros.
    let negative = body.starts_with('-');
    let digits_start = usize::from(negative);
    let int_len = body[digits_start..].find(culture.decimal).unwrap_or(body.len() - digits_start);
    let int_digits = body[digits_start..digits_start + int_len].chars().filter(char::is_ascii_digit).count();
    if int_digits < min_int {
        body.insert_str(digits_start, &"0".repeat(min_int - int_digits));
    }
    if min_int == 0 && body[digits_start..].starts_with(&format!("0{}", culture.decimal)) {
        body.remove(digits_start);
    }
    format!("{}{body}{}", unquote(prefix), unquote(suffix))
}

fn unquote(s: &str) -> String {
    s.replace(['\'', '"'], "")
}

fn format_date(d: DateTime, spec: &str, culture: &Culture) -> String {
    let pattern = match spec {
        "" | "G" => format!(
            "{} {}",
            culture.short_date,
            if culture.name == "pt-BR" { "HH:mm:ss" } else { "h:mm:ss tt" }
        ),
        "d" => culture.short_date.to_owned(),
        "D" => culture.long_date.to_owned(),
        "t" => culture.short_time.to_owned(),
        "T" => (if culture.name == "pt-BR" { "HH:mm:ss" } else { "h:mm:ss tt" }).to_owned(),
        "g" => format!("{} {}", culture.short_date, culture.short_time),
        "M" | "m" => (if culture.name == "pt-BR" { "d 'de' MMMM" } else { "MMMM d" }).to_owned(),
        "Y" | "y" => (if culture.name == "pt-BR" { "MMMM 'de' yyyy" } else { "MMMM yyyy" }).to_owned(),
        custom => custom.to_owned(),
    };
    // A date without a time shows only the date part for the general format.
    let pattern = if spec.is_empty() && d.seconds == 0 { culture.short_date.to_owned() } else { pattern };
    let (year, month, day) = d.ymd();
    let (hour, minute, second) = d.hms();
    let mut out = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' || c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != c {
                out.push(chars[i]);
                i += 1;
            }
            i += 1;
            continue;
        }
        let mut run = 1;
        while i + run < chars.len() && chars[i + run] == c {
            run += 1;
        }
        let text = match (c, run) {
            ('y', 1) => (year % 100).to_string(),
            ('y', 2) => format!("{:02}", year % 100),
            ('y', _) => format!("{year:0run$}"),
            ('M', 1) => month.to_string(),
            ('M', 2) => format!("{month:02}"),
            ('M', 3) => abbreviate(culture.months[(month - 1) as usize]),
            ('M', _) => culture.months[(month - 1) as usize].to_owned(),
            ('d', 1) => day.to_string(),
            ('d', 2) => format!("{day:02}"),
            ('d', 3) => abbreviate(culture.days[d.weekday() as usize]),
            ('d', _) => culture.days[d.weekday() as usize].to_owned(),
            ('H', 1) => hour.to_string(),
            ('H', _) => format!("{hour:02}"),
            ('h', n) => {
                let h12 = if hour % 12 == 0 { 12 } else { hour % 12 };
                if n == 1 { h12.to_string() } else { format!("{h12:02}") }
            }
            ('m', 1) => minute.to_string(),
            ('m', _) => format!("{minute:02}"),
            ('s', 1) => second.to_string(),
            ('s', _) => format!("{second:02}"),
            ('t', _) => (if hour < 12 { "AM" } else { "PM" }).to_owned(),
            (c, n) => c.to_string().repeat(n),
        };
        out.push_str(&text);
        i += run;
    }
    out
}

fn abbreviate(name: &str) -> String {
    name.chars().take(3).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(v: impl Into<Value>, spec: &str) -> String {
        format_value(&v.into(), spec, &Culture::pt_br())
    }

    fn en(v: impl Into<Value>, spec: &str) -> String {
        format_value(&v.into(), spec, &Culture::en_us())
    }

    #[test]
    fn standard_number_formats() {
        assert_eq!(pt(1234.5, "C2"), "R$\u{a0}1.234,50");
        assert_eq!(en(1234.5, "C"), "$1,234.50");
        assert_eq!(pt(-1234.5, "C2"), "-R$\u{a0}1.234,50");
        assert_eq!(pt(1234567.891, "N2"), "1.234.567,89");
        assert_eq!(en(1234567.891, "N0"), "1,234,568");
        assert_eq!(pt(0.1234, "P1"), "12,3%");
        assert_eq!(en(42.0, "D5"), "00042");
        assert_eq!(pt(1.23456, "F3"), "1,235");
        assert_eq!(pt(-0.001, "N2"), "0,00", "no negative zero");
        assert_eq!(pt(2.5, ""), "2,5");
    }

    #[test]
    fn custom_number_patterns() {
        assert_eq!(en(1234.5, "#,##0.00"), "1,234.50");
        assert_eq!(en(1234.5, "#,##0.##"), "1,234.5");
        assert_eq!(en(7.0, "000"), "007");
        assert_eq!(en(0.256, "0.0%"), "25.6%");
        assert_eq!(en(0.5, "#.##"), ".5");
        assert_eq!(en(-5.0, "0;(0)"), "(5)");
        assert_eq!(pt(12.0, "'Qtd: '0"), "Qtd: 12");
    }

    #[test]
    fn date_formats() {
        let d = Value::Date(DateTime::parse("2026-11-01T09:05:07").expect("valid"));
        assert_eq!(pt(d.clone(), "d"), "01/11/2026");
        assert_eq!(en(d.clone(), "d"), "11/1/2026");
        assert_eq!(pt(d.clone(), "D"), "domingo, 1 de novembro de 2026");
        assert_eq!(en(d.clone(), "D"), "Sunday, November 1, 2026");
        assert_eq!(pt(d.clone(), "dd/MM/yy HH:mm"), "01/11/26 09:05");
        assert_eq!(en(d.clone(), "MMM d, h:mm tt"), "Nov 1, 9:05 AM");
        assert_eq!(pt(d, "MMMM 'de' yyyy"), "novembro de 2026");
        assert_eq!(pt("2026-03-15", "dd/MM/yyyy"), "15/03/2026", "ISO text formats as a date");
    }

    #[test]
    fn composite_strings() {
        let c = Culture::pt_br();
        let s = format_composite("Página {0} de {1}", &[Value::from(2_i64), Value::from(9_i64)], &c);
        assert_eq!(s, "Página 2 de 9");
        assert_eq!(format_composite("{0:C2} {{x}}", &[Value::from(5.0)], &c), "R$\u{a0}5,00 {x}");
        assert_eq!(format_composite("{3}", &[], &c), "{3}", "missing arguments stay visible");
        assert_eq!(pt(true, ""), "Sim");
    }
}
