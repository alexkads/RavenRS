//! Lexer and recursive-descent parser.

use super::{Agg, BinOp, Expr, ExprError, Func, Node};
use crate::value::{DateTime, Value};

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Date(DateTime),
    Ident(String),
    /// `[Name]`; an empty `[]` marks a collection.
    Field(String),
    Param(String),
    LParen,
    RParen,
    Comma,
    Dot,
    Op(&'static str),
    End,
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
}

impl Lexer<'_> {
    fn err(&self, message: impl Into<String>) -> ExprError {
        ExprError { message: message.into(), offset: self.pos }
    }

    fn tokens(mut self) -> Result<Vec<(Tok, usize)>, ExprError> {
        let mut out = Vec::new();
        loop {
            let rest = &self.src[self.pos..];
            let trimmed = rest.trim_start();
            self.pos += rest.len() - trimmed.len();
            let start = self.pos;
            let Some(c) = trimmed.chars().next() else {
                out.push((Tok::End, start));
                return Ok(out);
            };
            let tok = match c {
                '0'..='9' => {
                    let len = trimmed
                        .char_indices()
                        .find(|(i, ch)| {
                            !(ch.is_ascii_digit()
                                || (*ch == '.' && trimmed[i + 1..].starts_with(|n: char| n.is_ascii_digit())))
                        })
                        .map_or(trimmed.len(), |(i, _)| i);
                    let n = trimmed[..len].parse().map_err(|_| self.err("invalid number"))?;
                    self.pos += len;
                    Tok::Num(n)
                }
                '\'' | '"' => {
                    let mut s = String::new();
                    let mut chars = trimmed[1..].char_indices();
                    let mut end = None;
                    while let Some((i, ch)) = chars.next() {
                        if ch == c {
                            if trimmed[1 + i + 1..].starts_with(c) {
                                s.push(c);
                                chars.next();
                                continue;
                            }
                            end = Some(1 + i + 1);
                            break;
                        }
                        s.push(ch);
                    }
                    let Some(end) = end else { return Err(self.err("unterminated text")) };
                    self.pos += end;
                    Tok::Str(s)
                }
                '#' => {
                    let Some(close) = trimmed[1..].find('#') else {
                        return Err(self.err("unterminated date"));
                    };
                    let d =
                        DateTime::parse(&trimmed[1..1 + close]).ok_or_else(|| self.err("invalid date"))?;
                    self.pos += close + 2;
                    Tok::Date(d)
                }
                '[' => {
                    let Some(close) = trimmed.find(']') else { return Err(self.err("missing ']'")) };
                    let name = trimmed[1..close].trim().to_owned();
                    self.pos += close + 1;
                    Tok::Field(name)
                }
                '?' => {
                    let len = 1 + trimmed[1..]
                        .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                        .unwrap_or(trimmed.len() - 1);
                    if len == 1 {
                        return Err(self.err("missing parameter name after '?'"));
                    }
                    self.pos += len;
                    Tok::Param(trimmed[1..len].to_owned())
                }
                '(' | ')' | ',' | '.' => {
                    self.pos += 1;
                    match c {
                        '(' => Tok::LParen,
                        ')' => Tok::RParen,
                        ',' => Tok::Comma,
                        _ => Tok::Dot,
                    }
                }
                c if c.is_alphabetic() || c == '_' => {
                    let len = trimmed
                        .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                        .unwrap_or(trimmed.len());
                    self.pos += len;
                    Tok::Ident(trimmed[..len].to_owned())
                }
                _ => {
                    const OPS: [&str; 18] = [
                        "<>", "!=", "==", "<=", ">=", "&&", "||", "+", "-", "*", "/", "%", "=", "<", ">",
                        "!", "&", "|",
                    ];
                    let Some(op) = OPS.iter().find(|op| trimmed.starts_with(**op)) else {
                        return Err(self.err(format!("unexpected '{c}'")));
                    };
                    self.pos += op.len();
                    Tok::Op(op)
                }
            };
            out.push((tok, start));
        }
    }
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    i: usize,
}

/// Parses an expression.
pub fn parse(source: &str) -> Result<Expr, ExprError> {
    let toks = Lexer { src: source, pos: 0 }.tokens()?;
    let mut p = Parser { toks, i: 0 };
    let node = p.or()?;
    match p.peek() {
        Tok::End => Ok(Expr(node)),
        other => Err(p.err(format!("unexpected {}", describe(other)))),
    }
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::End => "end of expression".into(),
        Tok::Ident(s) => format!("'{s}'"),
        Tok::Op(o) => format!("'{o}'"),
        Tok::Field(f) => format!("'[{f}]'"),
        Tok::RParen => "')'".into(),
        Tok::LParen => "'('".into(),
        Tok::Comma => "','".into(),
        Tok::Dot => "'.'".into(),
        other => format!("{other:?}"),
    }
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.i].0
    }

    fn offset(&self) -> usize {
        self.toks[self.i].1
    }

    fn err(&self, message: impl Into<String>) -> ExprError {
        ExprError { message: message.into(), offset: self.offset() }
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

    fn expect(&mut self, t: &Tok) -> Result<(), ExprError> {
        if self.peek() == t {
            self.bump();
            Ok(())
        } else {
            Err(self.err(format!("expected {}, found {}", describe(t), describe(self.peek()))))
        }
    }

    fn or(&mut self) -> Result<Node, ExprError> {
        let mut left = self.and()?;
        while self.is_word("or") || self.is_op(&["||", "|"]).is_some() {
            self.bump();
            left = Node::Or(Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Node, ExprError> {
        let mut left = self.not()?;
        while self.is_word("and") || self.is_op(&["&&", "&"]).is_some() {
            self.bump();
            left = Node::And(Box::new(left), Box::new(self.not()?));
        }
        Ok(left)
    }

    fn not(&mut self) -> Result<Node, ExprError> {
        if self.is_word("not") || self.is_op(&["!"]).is_some() {
            self.bump();
            return Ok(Node::Not(Box::new(self.not()?)));
        }
        self.comparison()
    }

    fn comparison(&mut self) -> Result<Node, ExprError> {
        let left = self.additive()?;
        if let Some(op) = self.is_op(&["=", "==", "<>", "!=", "<", "<=", ">", ">="]) {
            self.bump();
            let op = match op {
                "=" | "==" => BinOp::Eq,
                "<>" | "!=" => BinOp::Ne,
                "<" => BinOp::Lt,
                "<=" => BinOp::Le,
                ">" => BinOp::Gt,
                _ => BinOp::Ge,
            };
            return Ok(Node::Bin(op, Box::new(left), Box::new(self.additive()?)));
        }
        if self.is_word("is") {
            self.bump();
            let negated = self.is_word("not");
            if negated {
                self.bump();
            }
            if !self.is_word("null") {
                return Err(self.err("expected 'Null' after 'Is'"));
            }
            self.bump();
            return Ok(Node::IsNull(Box::new(left), negated));
        }
        if self.is_word("in") {
            self.bump();
            self.expect(&Tok::LParen)?;
            let items = self.args()?;
            return Ok(Node::In(Box::new(left), items));
        }
        if self.is_word("between") {
            self.bump();
            self.expect(&Tok::LParen)?;
            let lo = self.or()?;
            self.expect(&Tok::Comma)?;
            let hi = self.or()?;
            self.expect(&Tok::RParen)?;
            return Ok(Node::Between(Box::new(left), Box::new(lo), Box::new(hi)));
        }
        Ok(left)
    }

    fn additive(&mut self) -> Result<Node, ExprError> {
        let mut left = self.multiplicative()?;
        while let Some(op) = self.is_op(&["+", "-"]) {
            self.bump();
            let op = if op == "+" { BinOp::Add } else { BinOp::Sub };
            left = Node::Bin(op, Box::new(left), Box::new(self.multiplicative()?));
        }
        Ok(left)
    }

    fn multiplicative(&mut self) -> Result<Node, ExprError> {
        let mut left = self.unary()?;
        while let Some(op) = self.is_op(&["*", "/", "%"]) {
            self.bump();
            let op = match op {
                "*" => BinOp::Mul,
                "/" => BinOp::Div,
                _ => BinOp::Rem,
            };
            left = Node::Bin(op, Box::new(left), Box::new(self.unary()?));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Node, ExprError> {
        if self.is_op(&["-"]).is_some() {
            self.bump();
            return Ok(Node::Neg(Box::new(self.unary()?)));
        }
        if self.is_op(&["+"]).is_some() {
            self.bump();
            return self.unary();
        }
        self.primary()
    }

    /// Arguments up to and including the closing parenthesis.
    fn args(&mut self) -> Result<Vec<Node>, ExprError> {
        let mut args = Vec::new();
        if *self.peek() == Tok::RParen {
            self.bump();
            return Ok(args);
        }
        loop {
            args.push(self.or()?);
            match self.bump() {
                Tok::Comma => {}
                Tok::RParen => return Ok(args),
                other => return Err(self.err(format!("expected ',' or ')', found {}", describe(&other)))),
            }
        }
    }

    fn primary(&mut self) -> Result<Node, ExprError> {
        let at = self.offset();
        match self.bump() {
            Tok::Num(n) => Ok(Node::Lit(Value::Number(n))),
            Tok::Str(s) => Ok(Node::Lit(Value::Text(s))),
            Tok::Date(d) => Ok(Node::Lit(Value::Date(d))),
            Tok::Param(p) => Ok(Node::Param(p)),
            Tok::LParen => {
                let inner = self.or()?;
                self.expect(&Tok::RParen)?;
                Ok(inner)
            }
            Tok::Field(name) if name.is_empty() => {
                Err(ExprError { message: "empty field '[]'".into(), offset: at })
            }
            Tok::Field(name) => {
                let path: Vec<String> = name.split('.').map(|s| s.trim().to_owned()).collect();
                if path.iter().any(String::is_empty) {
                    return Err(ExprError { message: format!("invalid field '[{name}]'"), offset: at });
                }
                // `[Items][].Sum(...)`: an aggregate over a nested collection.
                if *self.peek() == Tok::Field(String::new()) {
                    self.bump();
                    self.expect(&Tok::Dot)?;
                    let fat = self.offset();
                    let Tok::Ident(f) = self.bump() else {
                        return Err(ExprError {
                            message: "expected an aggregate after '[].'".into(),
                            offset: fat,
                        });
                    };
                    let agg = Agg::from_name(&f.to_lowercase()).ok_or_else(|| ExprError {
                        message: format!("unknown aggregate '{f}'"),
                        offset: fat,
                    })?;
                    self.expect(&Tok::LParen)?;
                    let arg = self.aggregate_arg(agg, fat)?;
                    return Ok(Node::Collection(path, agg, arg));
                }
                Ok(Node::Field(path))
            }
            Tok::Ident(word) => {
                let lower = word.to_lowercase();
                match lower.as_str() {
                    "true" => return Ok(Node::Lit(Value::Bool(true))),
                    "false" => return Ok(Node::Lit(Value::Bool(false))),
                    "null" => return Ok(Node::Lit(Value::Null)),
                    _ => {}
                }
                if *self.peek() != Tok::LParen {
                    return Err(ExprError {
                        message: format!("unknown name '{word}' (fields go in brackets: [{word}])"),
                        offset: at,
                    });
                }
                self.bump();
                // Min and Max with one argument (or none) are aggregates; with two, they compare.
                if let Some(agg) = Agg::from_name(&lower) {
                    let save = self.i;
                    let probe = self.args()?;
                    if !(matches!(agg, Agg::Min | Agg::Max) && probe.len() == 2) {
                        self.i = save;
                        let arg = self.aggregate_arg(agg, at)?;
                        return Ok(Node::Agg(agg, arg));
                    }
                    let f = Func::resolve(&lower, probe.len())
                        .map_err(|m| ExprError { message: m, offset: at })?;
                    return Ok(Node::Call(f, probe));
                }
                let args = self.args()?;
                let f =
                    Func::resolve(&lower, args.len()).map_err(|m| ExprError { message: m, offset: at })?;
                Ok(Node::Call(f, args))
            }
            other => Err(ExprError { message: format!("unexpected {}", describe(&other)), offset: at }),
        }
    }

    /// The argument of an aggregate, after its opening parenthesis.
    fn aggregate_arg(&mut self, agg: Agg, at: usize) -> Result<Option<Box<Node>>, ExprError> {
        let args = self.args()?;
        match (agg, args.len()) {
            (Agg::Count, 0) => Ok(None),
            (_, 1) => Ok(args.into_iter().next().map(Box::new)),
            _ => Err(ExprError {
                message: "aggregates take one argument (Count takes none or one)".into(),
                offset: at,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str) -> Node {
        parse(s).unwrap_or_else(|e| panic!("{s}: {e}")).0
    }

    fn err(s: &str) -> ExprError {
        parse(s).expect_err(s)
    }

    #[test]
    fn precedence_and_literals() {
        assert_eq!(
            ok("1 + 2 * 3"),
            Node::Bin(
                BinOp::Add,
                Box::new(Node::Lit(Value::Number(1.0))),
                Box::new(Node::Bin(
                    BinOp::Mul,
                    Box::new(Node::Lit(Value::Number(2.0))),
                    Box::new(Node::Lit(Value::Number(3.0)))
                ))
            )
        );
        assert_eq!(ok("'it''s'"), Node::Lit(Value::Text("it's".into())));
        assert!(matches!(ok("#2026-11-01#"), Node::Lit(Value::Date(_))));
        assert!(matches!(ok("[A] > 1 And Not [B] Or ?P"), Node::Or(..)));
        assert_eq!(ok("[Customer.Name]"), Node::Field(vec!["Customer".into(), "Name".into()]));
        assert_eq!(ok("[^.Number]"), Node::Field(vec!["^".into(), "Number".into()]));
    }

    #[test]
    fn functions_aggregates_and_collections() {
        assert!(matches!(ok("Iif([X] > 0, 'a', 'b')"), Node::Call(Func::Iif, a) if a.len() == 3));
        assert!(matches!(ok("Sum([Total])"), Node::Agg(Agg::Sum, Some(_))));
        assert!(matches!(ok("count()"), Node::Agg(Agg::Count, None)));
        assert!(matches!(ok("Max([A], [B])"), Node::Call(Func::Max2, _)));
        assert!(matches!(ok("Max([A])"), Node::Agg(Agg::Max, Some(_))));
        assert!(
            matches!(ok("[Items][].Sum([Qty] * [Price])"), Node::Collection(p, Agg::Sum, Some(_)) if p == ["Items"])
        );
        assert!(matches!(ok("[X] In (1, 2, 3)"), Node::In(_, v) if v.len() == 3));
        assert!(matches!(ok("[X] Between(1, 9)"), Node::Between(..)));
        assert!(matches!(ok("[X] Is Not Null"), Node::IsNull(_, true)));
        assert!(
            Expr::parse("FormatString('{0} of {1}', PageNumber(), PageCount())").expect("valid").uses_page()
        );
    }

    #[test]
    fn errors_point_at_the_problem() {
        assert_eq!(err("[A] +").message, "unexpected end of expression");
        assert_eq!(err("Total * 2").offset, 0);
        assert!(err("Total * 2").message.contains("[Total]"));
        assert!(err("Foo(1)").message.contains("unknown function"));
        assert!(err("Iif(1, 2)").message.contains("wrong number"));
        assert_eq!(err("[A] = 'x").message, "unterminated text");
        assert_eq!(err("(1 + 2").message, "expected ')', found end of expression");
        assert_eq!(err("[A] @ 2").offset, 4);
    }
}
