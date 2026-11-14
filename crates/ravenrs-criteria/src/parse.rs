//! Lexer and recursive-descent parser.

use crate::ast::{AggregateCall, BinaryOp, Criteria, GroupOp, UnaryOp};
use crate::functions::{Aggregate, Function};
use crate::value::{DateTime, Value};

/// A syntax error, with the byte offset where it was found.
///
/// `message` is in English. An interface in another language translates `code`,
/// filling `{0}`, `{1}`… with `args`, the way [`ParseError::CODES`] lists them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// What is wrong, in English.
    pub message: String,
    /// Byte offset in the source.
    pub offset: usize,
    /// The code and the arguments, boxed: the parser recurses with this in every `Result`,
    /// so it stays as small as a message and an offset.
    detail: Box<(&'static str, Vec<String>)>,
}

impl ParseError {
    /// Every error code, with its English message. `{0}`, `{1}`… stand for `args`.
    pub const CODES: &'static [(&'static str, &'static str)] = &[
        ("unterminated_comment", "unterminated comment"),
        ("invalid_number", "invalid number"),
        ("unterminated_text", "unterminated text"),
        ("unterminated_date", "unterminated date"),
        ("invalid_date", "invalid date"),
        ("unterminated_brace", "unterminated '{'"),
        ("missing_bracket", "missing ']'"),
        ("unexpected_char", "unexpected '{0}'"),
        ("expected_assignment", "expected `field = expression`"),
        ("expected_field_name", "expected the name of a field before `=`"),
        ("unexpected", "unexpected {0}"),
        ("unexpected_end", "unexpected end of expression"),
        ("expected_found", "expected {0}, found {1}"),
        ("expected_end", "expected {0}, found end of expression"),
        ("too_deep", "expression is nested too deeply"),
        ("expected_null", "expected 'Null' after 'Is'"),
        ("expected_comma", "expected ',' or ')', found {0}"),
        ("expected_comma_end", "expected ',' or ')', found end of expression"),
        ("empty_field", "empty field '[]'"),
        ("invalid_field", "invalid field '[{0}]'"),
        ("expected_aggregate", "expected an aggregate after '].'"),
        ("unknown_aggregate", "unknown aggregate '{0}'"),
        ("unknown_name", "unknown name '{0}' (fields go in brackets: [{0}])"),
        ("wrong_arity", "wrong number of arguments ({0}) for {1}"),
    ];

    /// An error `code` at `offset`, its English message filled with `args`.
    fn new(code: &'static str, args: Vec<String>, offset: usize) -> Self {
        let template = Self::CODES.iter().find(|(c, _)| *c == code).map_or(code, |(_, t)| t);
        Self { message: fill(template, &args), offset, detail: Box::new((code, args)) }
    }

    /// Which error it is, one of [`ParseError::CODES`].
    pub fn code(&self) -> &'static str {
        self.detail.0
    }

    /// What goes in the `{0}`, `{1}`… of the message: a name, a token, a count.
    pub fn args(&self) -> &[String] {
        &self.detail.1
    }

    /// The message in another language: `template` is what `code` says there, with
    /// `{0}`, `{1}`… where the arguments go.
    pub fn message_in(&self, template: &str) -> String {
        fill(template, self.args())
    }
}

/// `template` with `{0}`, `{1}`… replaced by `args`.
fn fill(template: &str, args: &[String]) -> String {
    let mut out = template.to_owned();
    for (i, arg) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), arg);
    }
    out
}

/// "unexpected X" for the token `t`, with the end of the text as its own code.
fn unexpected(t: &Tok, offset: usize) -> ParseError {
    match t {
        Tok::End => ParseError::new("unexpected_end", Vec::new(), offset),
        t => ParseError::new("unexpected", vec![describe(t)], offset),
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (at {})", self.message, self.offset)
    }
}

impl std::error::Error for ParseError {}

/// Deepest nesting accepted, so hostile input cannot overflow the stack.
const MAX_DEPTH: u32 = 128;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Date(DateTime),
    Ident(String),
    /// `[...]`, with its text and the text's offset; an empty one marks a collection.
    Field(String, usize),
    Param(String),
    LParen,
    RParen,
    Comma,
    Dot,
    Op(&'static str),
    End,
}

fn lex(src: &str, base: usize) -> Result<Vec<(Tok, usize)>, ParseError> {
    let mut out = Vec::new();
    let mut pos = 0;
    let err = |code: &'static str, args: Vec<String>, at: usize| ParseError::new(code, args, base + at);
    loop {
        let rest = &src[pos..];
        let trimmed = rest.trim_start();
        pos += rest.len() - trimmed.len();
        if trimmed.starts_with("/*") {
            let Some(end) = trimmed.find("*/") else {
                return Err(err("unterminated_comment", Vec::new(), pos));
            };
            pos += end + 2;
            continue;
        }
        let start = pos;
        let Some(c) = trimmed.chars().next() else {
            out.push((Tok::End, base + start));
            return Ok(out);
        };
        let tok = match c {
            '0'..='9' => {
                let digits = trimmed
                    .char_indices()
                    .find(|(i, ch)| {
                        !(ch.is_ascii_digit()
                            || (*ch == '.' && trimmed[i + 1..].starts_with(|n: char| n.is_ascii_digit())))
                    })
                    .map_or(trimmed.len(), |(i, _)| i);
                let n = trimmed[..digits].parse().map_err(|_| err("invalid_number", Vec::new(), pos))?;
                pos += digits;
                // Type suffixes (12m, 1.5f...) do not change the value here.
                if let Some(s) = src[pos..].chars().next()
                    && matches!(s.to_ascii_lowercase(), 'm' | 'f' | 's' | 'b' | 'l' | 'u')
                    && !src[pos + 1..].starts_with(|n: char| n.is_alphanumeric() || n == '_')
                {
                    pos += 1;
                }
                Tok::Num(n)
            }
            '\'' => {
                let mut s = String::new();
                let mut chars = trimmed[1..].char_indices();
                let mut end = None;
                while let Some((i, ch)) = chars.next() {
                    if ch == '\'' {
                        if trimmed[1 + i + 1..].starts_with('\'') {
                            s.push('\'');
                            chars.next();
                            continue;
                        }
                        end = Some(1 + i + 1);
                        break;
                    }
                    s.push(ch);
                }
                let Some(end) = end else { return Err(err("unterminated_text", Vec::new(), pos)) };
                pos += end;
                Tok::Str(s)
            }
            '#' => {
                let Some(close) = trimmed[1..].find('#') else {
                    return Err(err("unterminated_date", Vec::new(), pos));
                };
                let d = DateTime::parse(&trimmed[1..1 + close])
                    .ok_or_else(|| err("invalid_date", Vec::new(), pos))?;
                pos += close + 2;
                Tok::Date(d)
            }
            '{' => {
                let Some(close) = trimmed.find('}') else {
                    return Err(err("unterminated_brace", Vec::new(), pos));
                };
                let guid = trimmed[1..close].to_owned();
                pos += close + 1;
                Tok::Str(guid)
            }
            '[' => {
                // Brackets nest in `[Items][[Qty] > 0]`, and quoted text may hold brackets.
                let (mut depth, mut quoted, mut close) = (0, false, None);
                for (i, ch) in trimmed.char_indices() {
                    match ch {
                        '\'' => quoted = !quoted,
                        '[' if !quoted => depth += 1,
                        ']' if !quoted => {
                            depth -= 1;
                            if depth == 0 {
                                close = Some(i);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let Some(close) = close else { return Err(err("missing_bracket", Vec::new(), pos)) };
                let inner = trimmed[1..close].to_owned();
                pos += close + 1;
                Tok::Field(inner, base + start + 1)
            }
            '?' => {
                let len = 1 + trimmed[1..]
                    .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                    .unwrap_or(trimmed.len() - 1);
                pos += len;
                // A lone `?` is the null constant.
                Tok::Param(trimmed[1..len].to_owned())
            }
            '(' | ')' | ',' | '.' => {
                pos += 1;
                match c {
                    '(' => Tok::LParen,
                    ')' => Tok::RParen,
                    ',' => Tok::Comma,
                    _ => Tok::Dot,
                }
            }
            c if c.is_alphabetic() || c == '_' => {
                let len =
                    trimmed.find(|ch: char| !(ch.is_alphanumeric() || ch == '_')).unwrap_or(trimmed.len());
                pos += len;
                Tok::Ident(trimmed[..len].to_owned())
            }
            _ => {
                const OPS: [&str; 19] = [
                    "<>", "!=", "==", "<=", ">=", "&&", "||", "+", "-", "*", "/", "%", "=", "<", ">", "!",
                    "&", "|", "^",
                ];
                if let Some(op) = OPS.iter().find(|op| trimmed.starts_with(**op)) {
                    pos += op.len();
                    Tok::Op(op)
                } else if c == '~' {
                    pos += 1;
                    Tok::Op("~")
                } else {
                    return Err(err("unexpected_char", vec![c.to_string()], pos));
                }
            }
        };
        out.push((tok, base + start));
    }
}

/// Splits `field = expression; ...` into its fields and parsed expressions. A field is a name or
/// `[name]`; `;` and `=` inside quotes or brackets belong to the expression.
pub(crate) fn parse_assignments(source: &str) -> Result<Vec<(String, Criteria)>, ParseError> {
    let mut found = Vec::new();
    let mut start = 0;
    for end in top_level(source, b';').into_iter().chain([source.len()]) {
        let piece = &source[start..end];
        if !piece.trim().is_empty() {
            let at = |offset: usize| start + offset;
            let Some(&equals) = top_level(piece, b'=').first() else {
                let first = piece.len() - piece.trim_start().len();
                return Err(ParseError::new("expected_assignment", Vec::new(), at(first)));
            };
            let name = piece[..equals].trim();
            let name = name.strip_prefix('[').and_then(|n| n.strip_suffix(']')).unwrap_or(name).trim();
            let simple = name.starts_with(|c: char| c.is_alphabetic() || c == '_')
                && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
            if !simple {
                let first = piece.len() - piece.trim_start().len();
                return Err(ParseError::new("expected_field_name", Vec::new(), at(first)));
            }
            let text = &piece[equals + 1..];
            let expression =
                parse(text).map_err(|e| ParseError { offset: at(equals + 1 + e.offset), ..e })?;
            found.push((name.to_owned(), expression));
        }
        start = end + 1;
    }
    Ok(found)
}

/// Where `wanted` stands outside quotes and brackets; an `=` next to `<`, `>`, `!` or `=` is part
/// of an operator.
fn top_level(source: &str, wanted: u8) -> Vec<usize> {
    let bytes = source.as_bytes();
    let (mut quoted, mut brackets, mut found) = (false, 0usize, Vec::new());
    for (i, &byte) in bytes.iter().enumerate() {
        match byte {
            b'\'' => quoted = !quoted,
            _ if quoted => {}
            b'[' => brackets += 1,
            b']' => brackets = brackets.saturating_sub(1),
            _ if brackets > 0 => {}
            b'=' if wanted == b'=' => {
                let before = i.checked_sub(1).map(|j| bytes[j]);
                let after = bytes.get(i + 1).copied();
                let operator = matches!(before, Some(b'<' | b'>' | b'!' | b'=')) || after == Some(b'=');
                if !operator {
                    found.push(i);
                }
            }
            _ if byte == wanted => found.push(i),
            _ => {}
        }
    }
    found
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    i: usize,
    depth: u32,
}

/// Parses an expression.
pub fn parse(source: &str) -> Result<Criteria, ParseError> {
    parse_at(source, 0, 0)
}

/// `depth` is how deep it already is, so a condition inside a field does not start counting over.
fn parse_at(source: &str, base: usize, depth: u32) -> Result<Criteria, ParseError> {
    let toks = lex(source, base)?;
    let mut p = Parser { toks, i: 0, depth };
    let node = p.or()?;
    match p.peek() {
        Tok::End => Ok(node),
        other => Err(unexpected(other, p.offset())),
    }
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::End => "end of expression".into(),
        Tok::Ident(s) => format!("'{s}'"),
        Tok::Op(o) => format!("'{o}'"),
        Tok::Field(f, _) => format!("'[{f}]'"),
        Tok::RParen => "')'".into(),
        Tok::LParen => "'('".into(),
        Tok::Comma => "','".into(),
        Tok::Dot => "'.'".into(),
        Tok::Num(n) => format!("'{n}'"),
        Tok::Str(s) => format!("'{s}'"),
        Tok::Date(d) => format!("'{d}'"),
        Tok::Param(p) => format!("'?{p}'"),
    }
}

fn bin(op: BinaryOp, a: Criteria, b: Criteria) -> Criteria {
    Criteria::Binary(op, Box::new(a), Box::new(b))
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.i].0
    }

    fn offset(&self) -> usize {
        self.toks[self.i].1
    }

    fn err(&self, code: &'static str) -> ParseError {
        ParseError::new(code, Vec::new(), self.offset())
    }

    fn bump(&mut self) -> Tok {
        let t = self.toks[self.i].0.clone();
        if self.i + 1 < self.toks.len() {
            self.i += 1;
        }
        t
    }

    fn is_word(&self, word: &str) -> bool {
        matches!(self.peek(), Tok::Ident(s) if s.eq_ignore_ascii_case(word))
    }

    fn is_op(&self, ops: &[&str]) -> Option<&'static str> {
        match self.peek() {
            Tok::Op(o) if ops.contains(o) => Some(o),
            _ => None,
        }
    }

    fn expect(&mut self, t: &Tok) -> Result<(), ParseError> {
        if self.peek() == t {
            self.bump();
            Ok(())
        } else {
            Err(match self.peek() {
                Tok::End => ParseError::new("expected_end", vec![describe(t)], self.offset()),
                found => ParseError::new("expected_found", vec![describe(t), describe(found)], self.offset()),
            })
        }
    }

    fn nested(&mut self, source: &str, at: usize) -> Result<Criteria, ParseError> {
        self.enter()?;
        let parsed = parse_at(source, at, self.depth);
        self.depth -= 1;
        parsed
    }

    fn enter(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("too_deep"));
        }
        Ok(())
    }

    fn or(&mut self) -> Result<Criteria, ParseError> {
        let first = self.and()?;
        let mut items = vec![first];
        while self.is_word("or") || self.is_op(&["||"]).is_some() {
            self.bump();
            items.push(self.and()?);
        }
        Ok(if items.len() == 1 { items.remove(0) } else { Criteria::Group(GroupOp::Or, items) })
    }

    fn and(&mut self) -> Result<Criteria, ParseError> {
        let first = self.not()?;
        let mut items = vec![first];
        while self.is_word("and") || self.is_op(&["&&"]).is_some() {
            self.bump();
            items.push(self.not()?);
        }
        Ok(if items.len() == 1 { items.remove(0) } else { Criteria::Group(GroupOp::And, items) })
    }

    fn not(&mut self) -> Result<Criteria, ParseError> {
        if self.is_word("not") || self.is_op(&["!"]).is_some() {
            self.bump();
            self.enter()?;
            let inner = self.not()?;
            self.depth -= 1;
            return Ok(Criteria::Unary(UnaryOp::Not, Box::new(inner)));
        }
        self.bit_or()
    }

    fn bit_or(&mut self) -> Result<Criteria, ParseError> {
        let mut left = self.bit_xor()?;
        while self.is_op(&["|"]).is_some() {
            self.bump();
            left = bin(BinaryOp::BitOr, left, self.bit_xor()?);
        }
        Ok(left)
    }

    fn bit_xor(&mut self) -> Result<Criteria, ParseError> {
        let mut left = self.bit_and()?;
        while self.is_op(&["^"]).is_some() {
            self.bump();
            left = bin(BinaryOp::BitXor, left, self.bit_and()?);
        }
        Ok(left)
    }

    fn bit_and(&mut self) -> Result<Criteria, ParseError> {
        let mut left = self.comparison()?;
        while self.is_op(&["&"]).is_some() {
            self.bump();
            left = bin(BinaryOp::BitAnd, left, self.comparison()?);
        }
        Ok(left)
    }

    fn comparison(&mut self) -> Result<Criteria, ParseError> {
        let mut left = self.additive()?;
        loop {
            if let Some(op) = self.is_op(&["=", "==", "<>", "!=", "<", "<=", ">", ">="]) {
                self.bump();
                let op = match op {
                    "=" | "==" => BinaryOp::Eq,
                    "<>" | "!=" => BinaryOp::Ne,
                    "<" => BinaryOp::Lt,
                    "<=" => BinaryOp::Le,
                    ">" => BinaryOp::Gt,
                    _ => BinaryOp::Ge,
                };
                left = bin(op, left, self.additive()?);
                continue;
            }
            // `[A] Not In (...)`, `[A] Not Between(...)`, `[A] Not Like '...'`.
            let negated = self.is_word("not")
                && matches!(self.toks.get(self.i + 1), Some((Tok::Ident(w), _))
                    if ["in", "between", "like"].iter().any(|k| w.eq_ignore_ascii_case(k)));
            if negated {
                self.bump();
            }
            let wrap = |c: Criteria| if negated { Criteria::Unary(UnaryOp::Not, Box::new(c)) } else { c };
            if self.is_word("like") {
                self.bump();
                let pattern = self.additive()?;
                left = wrap(bin(BinaryOp::Like, left, pattern));
            } else if self.is_word("in") {
                self.bump();
                self.expect(&Tok::LParen)?;
                let items = self.args()?;
                left = wrap(Criteria::In(Box::new(left), items));
            } else if self.is_word("between") {
                self.bump();
                self.expect(&Tok::LParen)?;
                self.enter()?;
                let lo = self.or()?;
                self.expect(&Tok::Comma)?;
                let hi = self.or()?;
                self.depth -= 1;
                self.expect(&Tok::RParen)?;
                left = wrap(Criteria::Between(Box::new(left), Box::new(lo), Box::new(hi)));
            } else if self.is_word("is") {
                self.bump();
                let is_not = self.is_word("not");
                if is_not {
                    self.bump();
                }
                if !self.is_word("null") {
                    return Err(self.err("expected_null"));
                }
                self.bump();
                let c = Criteria::IsNull(Box::new(left));
                left = if is_not { Criteria::Unary(UnaryOp::Not, Box::new(c)) } else { c };
            } else {
                return Ok(left);
            }
        }
    }

    fn additive(&mut self) -> Result<Criteria, ParseError> {
        let mut left = self.multiplicative()?;
        while let Some(op) = self.is_op(&["+", "-"]) {
            self.bump();
            let op = if op == "+" { BinaryOp::Add } else { BinaryOp::Sub };
            left = bin(op, left, self.multiplicative()?);
        }
        Ok(left)
    }

    fn multiplicative(&mut self) -> Result<Criteria, ParseError> {
        let mut left = self.unary()?;
        while let Some(op) = self.is_op(&["*", "/", "%"]) {
            self.bump();
            let op = match op {
                "*" => BinaryOp::Mul,
                "/" => BinaryOp::Div,
                _ => BinaryOp::Rem,
            };
            left = bin(op, left, self.unary()?);
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Criteria, ParseError> {
        if let Some(op) = self.is_op(&["-", "+", "~"]) {
            self.bump();
            self.enter()?;
            let inner = self.unary()?;
            self.depth -= 1;
            return Ok(match op {
                "-" => Criteria::Unary(UnaryOp::Minus, Box::new(inner)),
                "~" => Criteria::Unary(UnaryOp::BitNot, Box::new(inner)),
                _ => inner,
            });
        }
        self.primary()
    }

    /// Arguments up to and including the closing parenthesis.
    fn args(&mut self) -> Result<Vec<Criteria>, ParseError> {
        let mut args = Vec::new();
        if *self.peek() == Tok::RParen {
            self.bump();
            return Ok(args);
        }
        self.enter()?;
        loop {
            args.push(self.or()?);
            let at = self.offset();
            match self.bump() {
                Tok::Comma => {}
                Tok::RParen => {
                    self.depth -= 1;
                    return Ok(args);
                }
                other => {
                    return Err(match other {
                        Tok::End => ParseError::new("expected_comma_end", Vec::new(), at),
                        other => ParseError::new("expected_comma", vec![describe(&other)], at),
                    });
                }
            }
        }
    }

    fn primary(&mut self) -> Result<Criteria, ParseError> {
        let at = self.offset();
        match self.bump() {
            Tok::Num(n) => Ok(Criteria::Value(Value::Number(n))),
            Tok::Str(s) => Ok(Criteria::Value(Value::Text(s))),
            Tok::Date(d) => Ok(Criteria::Value(Value::Date(d))),
            Tok::Param(p) if p.is_empty() => Ok(Criteria::Value(Value::Null)),
            Tok::Param(p) => Ok(Criteria::Parameter(p)),
            Tok::LParen => {
                self.enter()?;
                let inner = self.or()?;
                self.depth -= 1;
                self.expect(&Tok::RParen)?;
                Ok(inner)
            }
            Tok::Field(name, _) if name.trim().is_empty() => {
                Err(ParseError::new("empty_field", Vec::new(), at))
            }
            Tok::Field(name, _) => self.field(&name, at),
            Tok::Ident(word) => self.word(&word, at),
            other => Err(unexpected(&other, at)),
        }
    }

    fn field(&mut self, name: &str, at: usize) -> Result<Criteria, ParseError> {
        let path: Vec<String> = name.split('.').map(|s| s.trim().to_owned()).collect();
        if path.iter().any(String::is_empty) {
            return Err(ParseError::new("invalid_field", vec![name.to_owned()], at));
        }
        // `[Parameters.Name]` is the older spelling of `?Name`.
        if path.len() == 2 && path[0].eq_ignore_ascii_case("parameters") {
            return Ok(Criteria::Parameter(path[1].clone()));
        }
        // `[Items][].Sum(...)` or `[Items][[Qty] > 0].Sum(...)`: an aggregate over a collection.
        if let Tok::Field(inner, inner_at) = self.peek().clone() {
            self.bump();
            let condition = if inner.trim().is_empty() { None } else { Some(self.nested(&inner, inner_at)?) };
            self.expect(&Tok::Dot)?;
            let fat = self.offset();
            let Tok::Ident(f) = self.bump() else {
                return Err(ParseError::new("expected_aggregate", Vec::new(), fat));
            };
            let function = Aggregate::from_name(&f)
                .ok_or_else(|| ParseError::new("unknown_aggregate", vec![f.clone()], fat))?;
            self.expect(&Tok::LParen)?;
            let args = self.args()?;
            check_aggregate(function, args.len(), fat)?;
            return Ok(Criteria::Aggregate(Box::new(AggregateCall {
                collection: Some(path),
                condition,
                function,
                args,
            })));
        }
        Ok(Criteria::Property(path))
    }

    fn word(&mut self, word: &str, at: usize) -> Result<Criteria, ParseError> {
        let lower = word.to_lowercase();
        match lower.as_str() {
            "true" => return Ok(Criteria::Value(Value::Bool(true))),
            "false" => return Ok(Criteria::Value(Value::Bool(false))),
            "null" => return Ok(Criteria::Value(Value::Null)),
            _ => {}
        }
        if *self.peek() != Tok::LParen {
            return Err(ParseError::new("unknown_name", vec![word.to_owned()], at));
        }
        self.bump();
        let args = self.args()?;
        // Min and Max with two arguments compare; with one they aggregate.
        let scalar = Function::from_name(word)
            .filter(|f| !matches!(f, Function::Min2 | Function::Max2) || args.len() == 2);
        if let Some(f) = scalar {
            let (min, max) = f.arity();
            if args.len() < min || args.len() > max || (f == Function::Iif && args.len() % 2 == 0) {
                return Err(ParseError::new(
                    "wrong_arity",
                    vec![args.len().to_string(), f.info().signature.to_owned()],
                    at,
                ));
            }
            return Ok(Criteria::Function(f, args));
        }
        if let Some(function) = Aggregate::from_name(word) {
            check_aggregate(function, args.len(), at)?;
            return Ok(Criteria::Aggregate(Box::new(AggregateCall {
                collection: None,
                condition: None,
                function,
                args,
            })));
        }
        // Resolved when evaluated, against the functions the environment registers.
        Ok(Criteria::Custom(word.to_owned(), args))
    }
}

fn check_aggregate(function: Aggregate, args: usize, at: usize) -> Result<(), ParseError> {
    let (min, max) = function.arity();
    if args < min || args > max {
        return Err(ParseError::new(
            "wrong_arity",
            vec![args.to_string(), function.info().signature.to_owned()],
            at,
        ));
    }
    Ok(())
}
