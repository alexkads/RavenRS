//! Lexer and recursive-descent parser.

use crate::ast::{AggregateCall, BinaryOp, Criteria, GroupOp, UnaryOp};
use crate::functions::{Aggregate, Function};
use crate::value::{DateTime, Value};

/// A syntax error, with the byte offset where it was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// What is wrong.
    pub message: String,
    /// Byte offset in the source.
    pub offset: usize,
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
    let err = |message: String, at: usize| ParseError { message, offset: base + at };
    loop {
        let rest = &src[pos..];
        let trimmed = rest.trim_start();
        pos += rest.len() - trimmed.len();
        if trimmed.starts_with("/*") {
            let Some(end) = trimmed.find("*/") else { return Err(err("unterminated comment".into(), pos)) };
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
                let n = trimmed[..digits].parse().map_err(|_| err("invalid number".into(), pos))?;
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
                let Some(end) = end else { return Err(err("unterminated text".into(), pos)) };
                pos += end;
                Tok::Str(s)
            }
            '#' => {
                let Some(close) = trimmed[1..].find('#') else {
                    return Err(err("unterminated date".into(), pos));
                };
                let d =
                    DateTime::parse(&trimmed[1..1 + close]).ok_or_else(|| err("invalid date".into(), pos))?;
                pos += close + 2;
                Tok::Date(d)
            }
            '{' => {
                let Some(close) = trimmed.find('}') else { return Err(err("unterminated '{'".into(), pos)) };
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
                let Some(close) = close else { return Err(err("missing ']'".into(), pos)) };
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
                    return Err(err(format!("unexpected '{c}'"), pos));
                }
            }
        };
        out.push((tok, base + start));
    }
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    i: usize,
    depth: u32,
}

/// Parses an expression.
pub fn parse(source: &str) -> Result<Criteria, ParseError> {
    parse_at(source, 0)
}

fn parse_at(source: &str, base: usize) -> Result<Criteria, ParseError> {
    let toks = lex(source, base)?;
    let mut p = Parser { toks, i: 0, depth: 0 };
    let node = p.or()?;
    match p.peek() {
        Tok::End => Ok(node),
        other => Err(p.err(format!("unexpected {}", describe(other)))),
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

    fn err(&self, message: impl Into<String>) -> ParseError {
        ParseError { message: message.into(), offset: self.offset() }
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
            Err(self.err(format!("expected {}, found {}", describe(t), describe(self.peek()))))
        }
    }

    fn enter(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("expression is nested too deeply"));
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
                let lo = self.or()?;
                self.expect(&Tok::Comma)?;
                let hi = self.or()?;
                self.expect(&Tok::RParen)?;
                left = wrap(Criteria::Between(Box::new(left), Box::new(lo), Box::new(hi)));
            } else if self.is_word("is") {
                self.bump();
                let is_not = self.is_word("not");
                if is_not {
                    self.bump();
                }
                if !self.is_word("null") {
                    return Err(self.err("expected 'Null' after 'Is'"));
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
                    return Err(ParseError {
                        message: format!("expected ',' or ')', found {}", describe(&other)),
                        offset: at,
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
                Err(ParseError { message: "empty field '[]'".into(), offset: at })
            }
            Tok::Field(name, _) => self.field(&name, at),
            Tok::Ident(word) => self.word(&word, at),
            other => Err(ParseError { message: format!("unexpected {}", describe(&other)), offset: at }),
        }
    }

    fn field(&mut self, name: &str, at: usize) -> Result<Criteria, ParseError> {
        let path: Vec<String> = name.split('.').map(|s| s.trim().to_owned()).collect();
        if path.iter().any(String::is_empty) {
            return Err(ParseError { message: format!("invalid field '[{name}]'"), offset: at });
        }
        // `[Parameters.Name]` is the older spelling of `?Name`.
        if path.len() == 2 && path[0].eq_ignore_ascii_case("parameters") {
            return Ok(Criteria::Parameter(path[1].clone()));
        }
        // `[Items][].Sum(...)` or `[Items][[Qty] > 0].Sum(...)`: an aggregate over a collection.
        if let Tok::Field(inner, inner_at) = self.peek().clone() {
            self.bump();
            let condition = if inner.trim().is_empty() { None } else { Some(parse_at(&inner, inner_at)?) };
            self.expect(&Tok::Dot)?;
            let fat = self.offset();
            let Tok::Ident(f) = self.bump() else {
                return Err(ParseError { message: "expected an aggregate after '].'".into(), offset: fat });
            };
            let function = Aggregate::from_name(&f)
                .ok_or_else(|| ParseError { message: format!("unknown aggregate '{f}'"), offset: fat })?;
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
            return Err(ParseError {
                message: format!("unknown name '{word}' (fields go in brackets: [{word}])"),
                offset: at,
            });
        }
        self.bump();
        let args = self.args()?;
        // Min and Max with two arguments compare; with one they aggregate.
        let scalar = Function::from_name(word)
            .filter(|f| !matches!(f, Function::Min2 | Function::Max2) || args.len() == 2);
        if let Some(f) = scalar {
            let (min, max) = f.arity();
            if args.len() < min || args.len() > max || (f == Function::Iif && args.len() % 2 == 0) {
                return Err(ParseError {
                    message: format!("wrong number of arguments ({}) for {}", args.len(), f.info().signature),
                    offset: at,
                });
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
        return Err(ParseError {
            message: format!("wrong number of arguments ({args}) for {}", function.info().signature),
            offset: at,
        });
    }
    Ok(())
}
