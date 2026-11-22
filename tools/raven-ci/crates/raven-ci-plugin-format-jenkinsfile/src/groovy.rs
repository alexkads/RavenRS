//! Reading the Groovy of a declarative `Jenkinsfile` into a tree of calls and blocks.
//!
//! The declarative pipeline is a small part of Groovy: calls with or without parentheses, named
//! arguments, closures written as blocks, strings of four kinds and assignments in `environment`.
//! This reads that part and nothing more. Every node keeps its line and column. What it cannot read
//! (an operator, a `def`, Groovy code) becomes a [`Problem`] at its place, and reading goes on
//! after it, so one odd line does not hide the rest of the file.

use std::fmt;

/// A place in the file: line and column, counting from 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct At {
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for At {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// A piece of a string: text, or a Groovy interpolation `${…}` / `$name` with its expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Text(String),
    Interpolation(String),
}

/// A Groovy string, cut into its pieces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GString {
    pub parts: Vec<Part>,
}

impl GString {
    /// The text when it has no interpolation.
    pub fn plain(&self) -> Option<String> {
        let mut text = String::new();
        for part in &self.parts {
            match part {
                Part::Text(piece) => text.push_str(piece),
                Part::Interpolation(_) => return None,
            }
        }
        Some(text)
    }
}

/// A value: what an argument or an assignment holds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Str(GString),
    Number(String),
    Bool(bool),
    Null,
    /// A name or a path of names: `any`, `scm`, `params.E2E`.
    Path(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
    /// A call used as a value: `credentials('id')`, `logRotator(numToKeepStr: '10')`.
    Call(Box<Stmt>),
}

impl Value {
    /// The text of a string with no interpolation, or of a number.
    pub fn text(&self) -> Option<String> {
        match self {
            Self::Str(text) => text.plain(),
            Self::Number(number) => Some(number.clone()),
            _ => None,
        }
    }
}

/// An argument: positional, or named (`time: 30`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Arg {
    pub name: Option<String>,
    pub value: Value,
    pub at: At,
}

/// A statement: a call with its arguments and block, or an assignment `NAME = value`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stmt {
    pub name: String,
    pub at: At,
    pub args: Vec<Arg>,
    /// What follows `=`, for an assignment.
    pub assigned: Option<Value>,
    pub block: Option<Block>,
}

impl Stmt {
    /// The argument named `name`.
    pub fn named(&self, name: &str) -> Option<&Arg> {
        self.args.iter().find(|arg| arg.name.as_deref() == Some(name))
    }

    /// The positional arguments.
    pub fn positional(&self) -> impl Iterator<Item = &Arg> {
        self.args.iter().filter(|arg| arg.name.is_none())
    }

    /// The statements of its block; none when it has no block.
    pub fn body(&self) -> &[Stmt] {
        self.block.as_ref().map_or(&[], |block| block.stmts.as_slice())
    }
}

/// The inside of `{ … }`. A block that is Groovy code (`script`, `expression`) is kept as its
/// source text, not read.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Block {
    pub stmts: Vec<Stmt>,
    pub source: Option<String>,
}

/// What could not be read, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Problem {
    pub at: At,
    pub message: String,
}

/// The blocks kept as source, unread: Groovy code inside the declarative pipeline.
const CODE_BLOCKS: &[&str] = &["script", "expression"];

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Str(GString),
    Number(String),
    Sym(char),
}

#[derive(Clone, Debug)]
struct Token {
    tok: Tok,
    at: At,
    /// A line ends between the token before and this one.
    after_newline: bool,
    start: usize,
    end: usize,
}

struct Lexer<'s> {
    text: &'s str,
    chars: Vec<(usize, char)>,
    pos: usize,
    line: usize,
    column: usize,
}

impl<'s> Lexer<'s> {
    fn new(text: &'s str) -> Self {
        Self { text, chars: text.char_indices().collect(), pos: 0, line: 1, column: 1 }
    }

    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.pos + ahead).map(|(_, c)| *c)
    }

    fn offset(&self) -> usize {
        self.chars.get(self.pos).map_or(self.text.len(), |(i, _)| *i)
    }

    fn at(&self) -> At {
        At { line: self.line, column: self.column }
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek(0)?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    fn starts_with(&self, s: &str) -> bool {
        s.chars().enumerate().all(|(i, c)| self.peek(i) == Some(c))
    }

    fn tokens(mut self) -> Result<Vec<Token>, Problem> {
        let mut tokens = Vec::new();
        let mut newline = true;
        loop {
            // Blanks and comments.
            match self.peek(0) {
                None => return Ok(tokens),
                Some('\n') => {
                    newline = true;
                    self.bump();
                    continue;
                }
                Some(c) if c.is_whitespace() => {
                    self.bump();
                    continue;
                }
                Some('/') if self.peek(1) == Some('/') => {
                    while self.peek(0).is_some_and(|c| c != '\n') {
                        self.bump();
                    }
                    continue;
                }
                Some('/') if self.peek(1) == Some('*') => {
                    let at = self.at();
                    self.bump();
                    self.bump();
                    loop {
                        if self.peek(0).is_none() {
                            return Err(Problem { at, message: "o comentário /* não fecha".to_owned() });
                        }
                        if self.starts_with("*/") {
                            self.bump();
                            self.bump();
                            break;
                        }
                        self.bump();
                    }
                    continue;
                }
                // A line that goes on in the next one.
                Some('\\') if self.peek(1) == Some('\n') => {
                    self.bump();
                    self.bump();
                    continue;
                }
                Some(_) => {}
            }
            let at = self.at();
            let start = self.offset();
            let c = self.peek(0).unwrap_or_default();
            let tok = if c == '\'' || c == '"' {
                Tok::Str(self.string()?)
            } else if c.is_ascii_digit() {
                let mut number = String::new();
                while let Some(d) = self.peek(0).filter(|d| d.is_ascii_alphanumeric() || *d == '.' || *d == '_') {
                    number.push(d);
                    self.bump();
                }
                Tok::Number(number)
            } else if c.is_alphabetic() || c == '_' || c == '$' {
                let mut name = String::new();
                while let Some(d) = self.peek(0).filter(|d| d.is_alphanumeric() || *d == '_' || *d == '$') {
                    name.push(d);
                    self.bump();
                }
                Tok::Ident(name)
            } else {
                self.bump();
                Tok::Sym(c)
            };
            tokens.push(Token { tok, at, after_newline: newline, start, end: self.offset() });
            newline = false;
        }
    }

    /// A string of any of the four kinds, at its opening quote.
    fn string(&mut self) -> Result<GString, Problem> {
        let at = self.at();
        let quote = self.peek(0).unwrap_or('\'');
        let triple = self.peek(1) == Some(quote) && self.peek(2) == Some(quote);
        let width = if triple { 3 } else { 1 };
        for _ in 0..width {
            self.bump();
        }
        let interpolates = quote == '"';
        let mut parts = Vec::new();
        let mut text = String::new();
        loop {
            let Some(c) = self.peek(0) else {
                let kind = if triple { format!("{0}{0}{0}", quote) } else { quote.to_string() };
                return Err(Problem { at, message: format!("o texto aberto com {kind} não fecha") });
            };
            if c == quote && (!triple || (self.peek(1) == Some(quote) && self.peek(2) == Some(quote))) {
                for _ in 0..width {
                    self.bump();
                }
                break;
            }
            if c == '\n' && !triple {
                return Err(Problem { at, message: format!("o texto aberto com {quote} não fecha na linha") });
            }
            if c == '\\' {
                self.bump();
                let escaped = self.bump().unwrap_or('\\');
                match escaped {
                    'n' => text.push('\n'),
                    't' => text.push('\t'),
                    'r' => text.push('\r'),
                    'b' => text.push('\u{8}'),
                    'f' => text.push('\u{c}'),
                    '\n' => {}
                    'u' => {
                        let hex: String = (0..4).filter_map(|_| self.bump()).collect();
                        match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                            Some(ch) => text.push(ch),
                            None => {
                                return Err(Problem { at: self.at(), message: format!("\\u{hex} não é um caractere") });
                            }
                        }
                    }
                    other => text.push(other),
                }
                continue;
            }
            if interpolates && c == '$' {
                if self.peek(1) == Some('{') {
                    self.bump();
                    self.bump();
                    let mut depth = 1;
                    let mut expression = String::new();
                    loop {
                        let Some(d) = self.bump() else {
                            return Err(Problem { at, message: "a interpolação ${ não fecha".to_owned() });
                        };
                        match d {
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        expression.push(d);
                    }
                    if !text.is_empty() {
                        parts.push(Part::Text(std::mem::take(&mut text)));
                    }
                    parts.push(Part::Interpolation(expression.trim().to_owned()));
                    continue;
                }
                if self.peek(1).is_some_and(|d| d.is_alphabetic() || d == '_') {
                    self.bump();
                    let mut expression = String::new();
                    loop {
                        match self.peek(0) {
                            Some(d) if d.is_alphanumeric() || d == '_' => expression.push(d),
                            Some('.') if self.peek(1).is_some_and(|d| d.is_alphabetic() || d == '_') => {
                                expression.push('.');
                            }
                            _ => break,
                        }
                        self.bump();
                    }
                    if !text.is_empty() {
                        parts.push(Part::Text(std::mem::take(&mut text)));
                    }
                    parts.push(Part::Interpolation(expression));
                    continue;
                }
            }
            text.push(c);
            self.bump();
        }
        if !text.is_empty() || parts.is_empty() {
            parts.push(Part::Text(text));
        }
        Ok(GString { parts })
    }
}

struct Parser<'s> {
    text: &'s str,
    tokens: Vec<Token>,
    pos: usize,
    problems: Vec<Problem>,
}

type Step<T> = Result<T, Problem>;

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn is_sym(&self, c: char) -> bool {
        matches!(self.peek(), Some(Token { tok: Tok::Sym(s), .. }) if *s == c)
    }

    fn here(&self) -> At {
        self.peek().or_else(|| self.tokens.last()).map_or(At { line: 1, column: 1 }, |t| t.at)
    }

    fn fail<T>(&self, message: impl Into<String>) -> Step<T> {
        Err(Problem { at: self.here(), message: message.into() })
    }

    fn expect(&mut self, c: char) -> Step<()> {
        if self.is_sym(c) {
            self.pos += 1;
            Ok(())
        } else {
            self.fail(format!("esperava {c} aqui"))
        }
    }

    /// The statements up to the `}` that closes the block (or the end of the file at the top).
    fn stmts(&mut self, top: bool) -> Vec<Stmt> {
        let mut stmts = Vec::new();
        loop {
            while self.is_sym(';') {
                self.pos += 1;
            }
            match self.peek() {
                None => {
                    if !top {
                        let at = self.tokens.last().map_or_else(At::default, |t| t.at);
                        self.problems.push(Problem { at, message: "falta fechar um bloco com }".to_owned() });
                    }
                    return stmts;
                }
                Some(Token { tok: Tok::Sym('}'), at, .. }) => {
                    if top {
                        let at = *at;
                        self.problems.push(Problem { at, message: "este } não fecha nenhum bloco".to_owned() });
                        self.pos += 1;
                        continue;
                    }
                    return stmts;
                }
                Some(_) => {}
            }
            let start = self.pos;
            match self.stmt() {
                Ok(stmt) => stmts.push(stmt),
                Err(problem) => {
                    self.problems.push(problem);
                    self.recover(start);
                }
            }
        }
    }

    /// Skips what is left of a statement that could not be read: up to the next line at the same
    /// depth, or to the `}` of the block it is in.
    fn recover(&mut self, start: usize) {
        self.pos = start + 1;
        let mut depth = 0_i32;
        // Count what opened from the start of the statement.
        for token in &self.tokens[start..self.pos] {
            depth += opens(&token.tok);
        }
        while let Some(token) = self.tokens.get(self.pos) {
            if depth <= 0 && token.after_newline {
                return;
            }
            if depth <= 0 && token.tok == Tok::Sym('}') {
                return;
            }
            depth += opens(&token.tok);
            self.pos += 1;
        }
    }

    fn stmt(&mut self) -> Step<Stmt> {
        let Some(token) = self.peek().cloned() else { return self.fail("o arquivo acabou") };
        let name = match &token.tok {
            Tok::Ident(name) => name.clone(),
            Tok::Sym('@') => return self.fail("anotações como @Library não são do pipeline declarativo"),
            _ => return self.fail("aqui esperava o nome de um bloco ou de um passo"),
        };
        self.pos += 1;
        let mut stmt = Stmt { name, at: token.at, args: Vec::new(), assigned: None, block: None };
        if self.is_sym('=') {
            self.pos += 1;
            stmt.assigned = Some(self.value()?);
            return self.end_of(stmt);
        }
        if self.is_sym('(') {
            self.pos += 1;
            stmt.args = self.args(')')?;
            self.expect(')')?;
        } else if self.starts_bare_argument() {
            stmt.args = self.bare_args()?;
        }
        if self.is_sym('{') {
            stmt.block = Some(self.block(&stmt.name)?);
        }
        self.end_of(stmt)
    }

    /// The statement ends at a new line, a `;` or the `}` of its block.
    fn end_of(&self, stmt: Stmt) -> Step<Stmt> {
        match self.peek() {
            None => Ok(stmt),
            Some(token) if token.after_newline => Ok(stmt),
            Some(Token { tok: Tok::Sym(';' | '}'), .. }) => Ok(stmt),
            Some(Token { tok, .. }) => self.fail(format!("não entendi {} depois de {}", shown(tok), stmt.name)),
        }
    }

    fn block(&mut self, owner: &str) -> Step<Block> {
        let open = self.pos;
        self.expect('{')?;
        if CODE_BLOCKS.contains(&owner) {
            // Groovy code: kept as text, read by nobody.
            let mut depth = 1;
            while let Some(token) = self.tokens.get(self.pos) {
                depth += opens(&token.tok);
                self.pos += 1;
                if depth == 0 {
                    let from = self.tokens[open].end;
                    let to = self.tokens[self.pos - 1].start;
                    let source = self.text[from..to].trim().to_owned();
                    return Ok(Block { stmts: Vec::new(), source: Some(source) });
                }
            }
            return Err(Problem { at: self.tokens[open].at, message: format!("o bloco {owner} não fecha") });
        }
        let stmts = self.stmts(false);
        self.expect('}')?;
        Ok(Block { stmts, source: None })
    }

    fn starts_bare_argument(&self) -> bool {
        match self.peek() {
            Some(token) if token.after_newline => false,
            Some(Token { tok: Tok::Str(_) | Tok::Number(_) | Tok::Ident(_) | Tok::Sym('[' | '-'), .. }) => true,
            _ => false,
        }
    }

    /// Arguments with no parentheses: up to the end of the line, going on after a comma.
    fn bare_args(&mut self) -> Step<Vec<Arg>> {
        let mut args = vec![self.arg()?];
        while self.is_sym(',') {
            self.pos += 1;
            args.push(self.arg()?);
        }
        Ok(args)
    }

    /// Arguments up to `close`, which is not taken.
    fn args(&mut self, close: char) -> Step<Vec<Arg>> {
        let mut args = Vec::new();
        while !self.is_sym(close) {
            args.push(self.arg()?);
            if self.is_sym(',') {
                self.pos += 1;
            } else if !self.is_sym(close) {
                return self.fail(format!("esperava , ou {close} aqui"));
            }
        }
        Ok(args)
    }

    fn arg(&mut self) -> Step<Arg> {
        let at = self.here();
        let named = matches!(
            (self.tokens.get(self.pos), self.tokens.get(self.pos + 1)),
            (Some(Token { tok: Tok::Ident(_) | Tok::Str(_), .. }), Some(Token { tok: Tok::Sym(':'), .. }))
        );
        let name = if named {
            let name = match &self.tokens[self.pos].tok {
                Tok::Ident(name) => name.clone(),
                Tok::Str(text) => text.plain().unwrap_or_default(),
                _ => String::new(),
            };
            self.pos += 2;
            Some(name)
        } else {
            None
        };
        Ok(Arg { name, value: self.value()?, at })
    }

    fn value(&mut self) -> Step<Value> {
        let Some(token) = self.peek().cloned() else { return self.fail("faltou um valor") };
        self.pos += 1;
        let value = match token.tok {
            Tok::Str(text) => Value::Str(text),
            Tok::Number(number) => Value::Number(number),
            Tok::Sym('-') => match self.peek().map(|t| t.tok.clone()) {
                Some(Tok::Number(number)) => {
                    self.pos += 1;
                    Value::Number(format!("-{number}"))
                }
                _ => return self.fail("esperava um número depois de -"),
            },
            Tok::Sym('[') => self.list()?,
            Tok::Ident(name) => match name.as_str() {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                "null" => Value::Null,
                _ => {
                    let mut path = name;
                    while self.is_sym('.') {
                        self.pos += 1;
                        match self.peek().map(|t| t.tok.clone()) {
                            Some(Tok::Ident(part)) => {
                                self.pos += 1;
                                path.push('.');
                                path.push_str(&part);
                            }
                            _ => return self.fail("esperava um nome depois do ponto"),
                        }
                    }
                    if self.is_sym('(') {
                        self.pos += 1;
                        let args = self.args(')')?;
                        self.expect(')')?;
                        let mut call = Stmt { name: path, at: token.at, args, assigned: None, block: None };
                        if self.is_sym('{') {
                            call.block = Some(self.block(&call.name)?);
                        }
                        Value::Call(Box::new(call))
                    } else {
                        Value::Path(path)
                    }
                }
            },
            other => return Err(Problem { at: token.at, message: format!("não entendi {}", shown(&other)) }),
        };
        // An operator after a value is Groovy code, not a declarative value.
        if let Some(Token { tok: Tok::Sym(c), after_newline: false, .. }) = self.peek()
            && matches!(c, '+' | '*' | '/' | '?' | '=' | '!' | '<' | '>' | '&' | '|' | '.' | '%')
        {
            return self.fail(format!("o operador {c} é código Groovy, que o conversor não lê"));
        }
        Ok(value)
    }

    /// `[a, b]`, `[k: v]` or `[:]`, after the `[`.
    fn list(&mut self) -> Step<Value> {
        if self.is_sym(':') {
            self.pos += 1;
            self.expect(']')?;
            return Ok(Value::Map(Vec::new()));
        }
        let args = self.args(']')?;
        self.expect(']')?;
        if !args.is_empty() && args.iter().all(|arg| arg.name.is_some()) {
            Ok(Value::Map(args.into_iter().map(|arg| (arg.name.unwrap_or_default(), arg.value)).collect()))
        } else if args.iter().any(|arg| arg.name.is_some()) {
            Err(Problem { at: args[0].at, message: "a lista mistura itens com nome e sem nome".to_owned() })
        } else {
            Ok(Value::List(args.into_iter().map(|arg| arg.value).collect()))
        }
    }
}

fn opens(tok: &Tok) -> i32 {
    match tok {
        Tok::Sym('{' | '(' | '[') => 1,
        Tok::Sym('}' | ')' | ']') => -1,
        _ => 0,
    }
}

fn shown(tok: &Tok) -> String {
    match tok {
        Tok::Ident(name) => name.clone(),
        Tok::Str(_) => "um texto".to_owned(),
        Tok::Number(number) => number.clone(),
        Tok::Sym(c) => c.to_string(),
    }
}

/// The statements of `text`, and what could not be read.
pub(crate) fn parse(text: &str) -> (Vec<Stmt>, Vec<Problem>) {
    match Lexer::new(text).tokens() {
        Err(problem) => (Vec::new(), vec![problem]),
        Ok(tokens) => {
            let mut parser = Parser { text, tokens, pos: 0, problems: Vec::new() };
            let stmts = parser.stmts(true);
            (stmts, parser.problems)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str) -> Stmt {
        let (mut stmts, problems) = parse(text);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(stmts.len(), 1);
        stmts.remove(0)
    }

    #[test]
    fn calls_with_and_without_parentheses_keep_their_named_arguments_and_place() {
        let stmt = one("stage('Build') {\n  sh 'make'\n  timeout(time: 30, unit: 'MINUTES') { echo \"oi\" }\n}");
        assert_eq!(stmt.name, "stage");
        assert_eq!(stmt.positional().next().and_then(|a| a.value.text()).as_deref(), Some("Build"));
        let body = stmt.body();
        assert_eq!(body[0].name, "sh");
        assert_eq!(body[0].at, At { line: 2, column: 3 });
        assert_eq!(body[1].named("time").map(|a| a.value.clone()), Some(Value::Number("30".into())));
        assert_eq!(body[1].body()[0].name, "echo");
    }

    #[test]
    fn strings_of_the_four_kinds_with_escapes_and_interpolation() {
        let stmt = one(r#"x 'a\'b', "c\$d ${params.V} $env.E", '''l1
l2''', """t ${X}"""
"#);
        let values: Vec<Value> = stmt.args.into_iter().map(|a| a.value).collect();
        assert_eq!(values[0].text().as_deref(), Some("a'b"));
        let Value::Str(second) = &values[1] else { panic!("a string") };
        assert_eq!(
            second.parts,
            vec![
                Part::Text("c$d ".into()),
                Part::Interpolation("params.V".into()),
                Part::Text(" ".into()),
                Part::Interpolation("env.E".into()),
            ]
        );
        assert_eq!(values[2].text().as_deref(), Some("l1\nl2"));
        assert!(values[3].text().is_none());
    }

    #[test]
    fn script_and_expression_blocks_are_kept_as_text() {
        let stmt = one("script {\n  def x = readFile('a') + 'b'\n  if (x) { echo x }\n}");
        let block = stmt.block.expect("a block");
        assert!(block.stmts.is_empty());
        assert!(block.source.unwrap_or_default().starts_with("def x"));
    }

    #[test]
    fn a_line_it_cannot_read_is_a_problem_and_the_rest_is_read() {
        let (stmts, problems) = parse("steps {\n  sh 'a' + b\n  sh 'c'\n}\n");
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].at.line, 2);
        assert_eq!(stmts[0].body().len(), 1);
        assert_eq!(stmts[0].body()[0].args[0].value.text().as_deref(), Some("c"));
    }

    #[test]
    fn a_string_or_block_that_does_not_close_says_where_it_opened() {
        let (_, problems) = parse("sh 'abc\nnext");
        assert_eq!(problems[0].at.line, 1);
        let (_, problems) = parse("pipeline {\n  stages {\n");
        assert!(problems.iter().any(|p| p.message.contains("fechar")));
    }

    #[test]
    fn assignments_lists_maps_and_calls_as_values() {
        let (stmts, problems) = parse(
            "A = 'x'\nB = credentials('id')\nbuildDiscarder(logRotator(numToKeepStr: '10'))\nchoice(choices: ['a', 'b'])\n",
        );
        assert!(problems.is_empty(), "{problems:?}");
        assert!(matches!(stmts[0].assigned, Some(Value::Str(_))));
        assert!(matches!(&stmts[1].assigned, Some(Value::Call(call)) if call.name == "credentials"));
        assert!(matches!(&stmts[2].args[0].value, Value::Call(call) if call.named("numToKeepStr").is_some()));
        assert!(matches!(&stmts[3].args[0].value, Value::List(items) if items.len() == 2));
    }
}
