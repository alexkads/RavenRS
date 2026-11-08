//! Runs a subset of Rust read with `syn`: what small UI examples use, with the host's types
//! reached through a [`Registry`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use proc_macro2::Span;
use ravenrs_reactive::State;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{BinOp, Expr, Lit, Pat, Stmt, Token, UnOp};

use crate::registry::{HostFn, Registry, use_registry};
use crate::value::{Record, Value, short};

/// Something wrong at a place of the program: its line and column (both from 1), how many
/// characters it covers, and what is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveError {
    /// The line, from 1.
    pub line: usize,
    /// The column of the first character, from 1.
    pub column: usize,
    /// How many characters it covers, at least 1.
    pub length: usize,
    /// What is wrong.
    pub message: String,
}

impl LiveError {
    pub(crate) fn at(span: Span, message: impl Into<String>) -> Self {
        let (start, end) = (span.start(), span.end());
        let length = if end.line == start.line { end.column.saturating_sub(start.column).max(1) } else { 1 };
        Self { line: start.line.max(1), column: start.column + 1, length, message: message.into() }
    }
}

impl LiveError {
    /// An error with no place in the code, such as one the host found.
    pub fn nowhere(message: impl Into<String>) -> Self {
        Self { line: 1, column: 1, length: 1, message: message.into() }
    }
}

impl fmt::Display for LiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for LiveError {}

/// How evaluation leaves an expression early.
enum Flow {
    Error(LiveError),
    Return(Value),
    Break,
    Continue,
}

impl From<LiveError> for Flow {
    fn from(error: LiveError) -> Self {
        Self::Error(error)
    }
}

type Eval<T = Value> = Result<T, Flow>;

/// Variables in view, with the scope around them.
struct Scope {
    vars: RefCell<HashMap<String, Value>>,
    parent: Option<Rc<Scope>>,
}

impl Scope {
    fn root() -> Rc<Self> {
        Rc::new(Self { vars: RefCell::new(HashMap::new()), parent: None })
    }

    fn child(parent: &Rc<Self>) -> Rc<Self> {
        Rc::new(Self { vars: RefCell::new(HashMap::new()), parent: Some(parent.clone()) })
    }

    fn get(&self, name: &str) -> Option<Value> {
        match self.vars.borrow().get(name) {
            Some(value) => Some(value.clone()),
            None => self.parent.as_ref()?.get(name),
        }
    }

    fn set(&self, name: &str, value: Value) -> bool {
        if let Some(slot) = self.vars.borrow_mut().get_mut(name) {
            *slot = value;
            return true;
        }
        self.parent.as_ref().is_some_and(|parent| parent.set(name, value))
    }

    fn define(&self, name: &str, value: Value) {
        self.vars.borrow_mut().insert(name.to_owned(), value);
    }
}

/// A function of the program: a free one, or a method of a struct it declares.
pub(crate) struct Function {
    sig: syn::Signature,
    block: syn::Block,
}

/// The shape of a struct the program declares.
#[derive(Clone)]
enum Shape {
    Unit,
    Tuple(usize),
    Named(Vec<String>),
}

/// What a whole program shares: the host, its functions, constants, structs and methods. The
/// items written inside a function join it when that function runs.
pub(crate) struct Program {
    registry: Rc<Registry>,
    functions: RefCell<HashMap<String, Rc<Function>>>,
    constants: RefCell<HashMap<String, Rc<Expr>>>,
    structs: RefCell<HashMap<String, Shape>>,
    methods: RefCell<HashMap<(String, String), Rc<Function>>>,
}

impl Program {
    pub(crate) fn method(&self, ty: &str, name: &str) -> Option<Rc<Function>> {
        self.methods.borrow().get(&(ty.to_owned(), name.to_owned())).cloned()
    }

    fn function(&self, name: &str) -> Option<Rc<Function>> {
        self.functions.borrow().get(name).cloned()
    }

    /// Takes in an item: a function, constant, struct or `impl` block; `use` lines are read
    /// and left alone.
    fn declare(&self, item: &syn::Item) -> Result<(), LiveError> {
        match item {
            syn::Item::Fn(function) => {
                let made = Function { sig: function.sig.clone(), block: (*function.block).clone() };
                self.functions.borrow_mut().insert(function.sig.ident.to_string(), Rc::new(made));
            }
            syn::Item::Const(constant) => {
                self.constants
                    .borrow_mut()
                    .insert(constant.ident.to_string(), Rc::new((*constant.expr).clone()));
            }
            syn::Item::Static(constant) => {
                self.constants
                    .borrow_mut()
                    .insert(constant.ident.to_string(), Rc::new((*constant.expr).clone()));
            }
            syn::Item::Struct(declared) => {
                if !declared.generics.params.is_empty() {
                    return Err(LiveError::at(
                        declared.generics.span(),
                        "a struct of the playground takes no generics",
                    ));
                }
                let shape = match &declared.fields {
                    syn::Fields::Unit => Shape::Unit,
                    syn::Fields::Unnamed(fields) => Shape::Tuple(fields.unnamed.len()),
                    syn::Fields::Named(fields) => Shape::Named(
                        fields
                            .named
                            .iter()
                            .filter_map(|f| f.ident.as_ref().map(ToString::to_string))
                            .collect(),
                    ),
                };
                self.structs.borrow_mut().insert(declared.ident.to_string(), shape);
            }
            syn::Item::Impl(block) => {
                let syn::Type::Path(path) = &*block.self_ty else {
                    return Err(LiveError::at(
                        block.self_ty.span(),
                        "an `impl` here is for a struct of the program",
                    ));
                };
                let ty = path.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
                for item in &block.items {
                    if let syn::ImplItem::Fn(method) = item {
                        let made = Function { sig: method.sig.clone(), block: method.block.clone() };
                        self.methods
                            .borrow_mut()
                            .insert((ty.clone(), method.sig.ident.to_string()), Rc::new(made));
                    }
                }
            }
            syn::Item::Use(_) => {}
            other => {
                return Err(LiveError::at(
                    other.span(),
                    "the playground reads functions, constants, structs, `impl` blocks and `use` only",
                ));
            }
        }
        Ok(())
    }
}

/// Calls a function of the program with these arguments; a method gets its receiver first.
pub(crate) fn call_function(
    program: &Rc<Program>,
    function: &Function,
    arguments: Vec<Value>,
) -> Result<Value, LiveError> {
    use_registry(program.registry.clone());
    let runner = Runner { program: program.clone() };
    runner.call_fn(function, arguments, function.sig.ident.span())
}

/// What a closure runs: its own code, or a function named where a closure goes, as
/// `.map(Some)`, `Text::bound(&s, Clone::clone)` or `.validate(b, validate::email)`.
enum Body {
    Code(Box<Code>),
    Path(Vec<String>),
    Host(HostFn),
}

/// A closure written in the program: its parameters, body and the variables it sees.
struct Code {
    params: Vec<Pat>,
    body: Expr,
    scope: Rc<Scope>,
}

/// A closure of the interpreted program, callable later by the host.
pub struct Closure {
    body: Body,
    program: Rc<Program>,
    span: Span,
}

impl fmt::Debug for Closure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Closure").finish_non_exhaustive()
    }
}

impl Closure {
    /// Calls it with these arguments.
    pub fn call(&self, arguments: Vec<Value>) -> Result<Value, LiveError> {
        use_registry(self.program.registry.clone());
        let runner = Runner { program: self.program.clone() };
        let (params, body, scope) = match &self.body {
            Body::Code(code) => (&code.params, &code.body, &code.scope),
            Body::Path(names) => {
                return match runner.call_path(names, arguments, self.span) {
                    Ok(value) | Err(Flow::Return(value)) => Ok(value),
                    Err(flow) => Err(self.flow_error(flow)),
                };
            }
            Body::Host(f) => return f(arguments).map_err(|e| self.error(e)),
        };
        let scope = Scope::child(scope);
        if arguments.len() != params.len() {
            return Err(self.error(format!(
                "this closure takes {} arguments, and got {}",
                params.len(),
                arguments.len()
            )));
        }
        for (pattern, value) in params.iter().zip(arguments) {
            runner.bind(pattern, value, &scope).map_err(|flow| self.flow_error(flow))?;
        }
        match runner.expr(body, &scope) {
            Ok(value) | Err(Flow::Return(value)) => Ok(value),
            Err(flow) => Err(self.flow_error(flow)),
        }
    }

    /// An error placed at the closure.
    pub fn error(&self, message: impl Into<String>) -> LiveError {
        LiveError::at(self.span, message)
    }

    fn flow_error(&self, flow: Flow) -> LiveError {
        match flow {
            Flow::Error(error) => error,
            _ => self.error("break or continue outside a loop"),
        }
    }
}

/// Runs the function `entry` of `source`, or its first function, with no arguments.
///
/// ```
/// let registry = std::rc::Rc::new(ravenrs_live::Registry::new());
/// let value = ravenrs_live::run(
///     "fn total() -> i64 { (1..=4).map(|n| n * n).sum() }",
///     None,
///     registry,
/// )
/// .map_err(|e| e[0].to_string())?;
/// assert_eq!(value, ravenrs_live::Value::Int(30));
/// # Ok::<(), String>(())
/// ```
pub fn run(source: &str, entry: Option<&str>, registry: Rc<Registry>) -> Result<Value, Vec<LiveError>> {
    let file = syn::parse_file(source).map_err(|e| {
        e.into_iter()
            .map(|e| LiveError::at(e.span(), format!("this is not Rust the playground reads: {e}")))
            .collect::<Vec<_>>()
    })?;
    let program = Rc::new(Program {
        registry: registry.clone(),
        functions: RefCell::default(),
        constants: RefCell::default(),
        structs: RefCell::default(),
        methods: RefCell::default(),
    });
    let mut first = None;
    for item in &file.items {
        if let syn::Item::Fn(function) = item {
            first.get_or_insert_with(|| function.sig.ident.to_string());
        }
        program.declare(item).map_err(|e| vec![e])?;
    }
    let entry = entry.map(str::to_owned).or(first).ok_or_else(|| {
        vec![LiveError::at(
            Span::call_site(),
            "there is no function to run: write `fn example() -> impl View { .. }`",
        )]
    })?;
    use_registry(registry);
    let runner = Runner { program };
    let function = runner
        .program
        .function(&entry)
        .ok_or_else(|| vec![LiveError::at(Span::call_site(), format!("there is no function `{entry}`"))])?;
    runner.call_fn(&function, Vec::new(), function.sig.ident.span()).map_err(|e| vec![e])
}

struct Runner {
    program: Rc<Program>,
}

fn fail<T>(span: Span, message: impl Into<String>) -> Eval<T> {
    Err(Flow::Error(LiveError::at(span, message)))
}

fn number(value: &Value) -> Option<f64> {
    match value.read() {
        Value::Int(n) => Some(n as f64),
        Value::Float(n) => Some(n),
        _ => None,
    }
}

/// The names of a path, without the standard library's modules in front: `std::rc::Rc::new`
/// reads as `Rc::new`.
fn path_names(path: &syn::Path) -> Vec<String> {
    let names: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    let skip = names
        .iter()
        .take_while(|n| {
            matches!(
                n.as_str(),
                "std" | "core" | "alloc" | "collections" | "rc" | "cell" | "sync" | "f32" | "f64"
            ) && names.len() > 2
        })
        .count();
    names[skip.min(names.len().saturating_sub(2))..].to_vec()
}

impl Runner {
    fn registry(&self) -> &Registry {
        &self.program.registry
    }

    fn call_fn(&self, function: &Function, arguments: Vec<Value>, span: Span) -> Result<Value, LiveError> {
        let scope = Scope::root();
        let params: Vec<&syn::FnArg> = function.sig.inputs.iter().collect();
        if params.len() != arguments.len() {
            return Err(LiveError::at(
                span,
                format!("`{}` takes {} arguments", function.sig.ident, params.len()),
            ));
        }
        for (param, value) in params.into_iter().zip(arguments) {
            match param {
                syn::FnArg::Typed(typed) => {
                    self.bind(&typed.pat, value, &scope).map_err(|f| self.flow_error(f, span))?
                }
                syn::FnArg::Receiver(_) => scope.define("self", value),
            }
        }
        match self.block(&function.block, &scope) {
            Ok(value) | Err(Flow::Return(value)) => Ok(value),
            Err(flow) => Err(self.flow_error(flow, span)),
        }
    }

    fn flow_error(&self, flow: Flow, span: Span) -> LiveError {
        match flow {
            Flow::Error(error) => error,
            _ => LiveError::at(span, "break or continue outside a loop"),
        }
    }

    fn block(&self, block: &syn::Block, scope: &Rc<Scope>) -> Eval {
        let scope = Scope::child(scope);
        let mut last = Value::Unit;
        for statement in &block.stmts {
            last = match statement {
                Stmt::Local(local) => {
                    let value = match &local.init {
                        Some(init) => self.expr(&init.expr, &scope)?,
                        None => Value::Unit,
                    };
                    match local.init.as_ref().and_then(|init| init.diverge.as_ref()) {
                        // `let Ok(x) = .. else { .. };`: the `else` must leave.
                        Some((_, otherwise)) => {
                            if !self.matches(&local.pat, &value, &scope)? {
                                self.expr(otherwise, &scope)?;
                                return fail(
                                    otherwise.span(),
                                    "the `else` of a `let` has to leave: return, break or panic",
                                );
                            }
                        }
                        None => self.bind(&local.pat, value, &scope)?,
                    }
                    Value::Unit
                }
                Stmt::Expr(expr, semicolon) => {
                    let value = self.expr(expr, &scope)?;
                    if semicolon.is_some() { Value::Unit } else { value }
                }
                Stmt::Item(item) => {
                    match item {
                        // Named inside a function, they are seen only there.
                        syn::Item::Fn(function) => {
                            let name = function.sig.ident.to_string();
                            self.program.declare(item)?;
                            scope.define(&name, self.path_closure(vec![name.clone()], function.span()));
                        }
                        syn::Item::Const(constant) => {
                            let value = self.expr(&constant.expr, &scope)?;
                            scope.define(&constant.ident.to_string(), value);
                        }
                        other => self.program.declare(other)?,
                    }
                    Value::Unit
                }
                Stmt::Macro(mac) => {
                    let value = self.mac(&mac.mac, &scope)?;
                    if mac.semi_token.is_some() { Value::Unit } else { value }
                }
            };
        }
        Ok(last)
    }

    /// Binds a pattern to a value, as `let` and closure parameters do.
    fn bind(&self, pattern: &Pat, value: Value, scope: &Rc<Scope>) -> Eval<()> {
        if self.matches(pattern, &value, scope)? {
            Ok(())
        } else {
            fail(pattern.span(), format!("a {} does not fit this pattern", value.kind()))
        }
    }

    /// Whether a value fits a pattern, binding its names when it does.
    fn matches(&self, pattern: &Pat, value: &Value, scope: &Rc<Scope>) -> Eval<bool> {
        Ok(match pattern {
            Pat::Ident(ident) => {
                let name = ident.ident.to_string();
                // A path-like name such as `None` is a constant, not a new variable.
                if name == "None" {
                    return Ok(matches!(value.read(), Value::Option(None)));
                }
                if let Some((_, sub)) = &ident.subpat
                    && !self.matches(sub, value, scope)?
                {
                    return Ok(false);
                }
                scope.define(&name, value.clone());
                true
            }
            Pat::Wild(_) | Pat::Rest(_) => true,
            Pat::Type(typed) => self.matches(&typed.pat, value, scope)?,
            Pat::Reference(reference) => self.matches(&reference.pat, value, scope)?,
            Pat::Paren(inner) => self.matches(&inner.pat, value, scope)?,
            Pat::Tuple(tuple) => {
                let items = match value.read() {
                    Value::Tuple(items) => items,
                    _ => return Ok(false),
                };
                if items.len() != tuple.elems.len() {
                    return Ok(false);
                }
                for (pattern, item) in tuple.elems.iter().zip(items) {
                    if !self.matches(pattern, &item, scope)? {
                        return Ok(false);
                    }
                }
                true
            }
            Pat::Slice(slice) => {
                let items = value.items().map_err(|e| Flow::Error(LiveError::at(slice.span(), e)))?;
                if items.len() != slice.elems.len() {
                    return Ok(false);
                }
                for (pattern, item) in slice.elems.iter().zip(items) {
                    if !self.matches(pattern, &item, scope)? {
                        return Ok(false);
                    }
                }
                true
            }
            Pat::Lit(literal) => self.literal(&literal.lit)? == value.read(),
            Pat::Struct(pattern) => {
                let name = path_names(&pattern.path).last().cloned().unwrap_or_default();
                let Value::Record(record) = value.read() else { return Ok(false) };
                if record.name != name {
                    return Ok(false);
                }
                for field in &pattern.fields {
                    let key = match &field.member {
                        syn::Member::Named(ident) => ident.to_string(),
                        syn::Member::Unnamed(index) => index.index.to_string(),
                    };
                    let Some(inner) = record.field(&key) else { return Ok(false) };
                    if !self.matches(&field.pat, &inner, scope)? {
                        return Ok(false);
                    }
                }
                true
            }
            Pat::Range(range) => {
                let Some(n) = number(value) else { return Ok(false) };
                let low = match &range.start {
                    Some(start) => number(&self.expr(start, scope)?).unwrap_or(f64::MIN),
                    None => f64::MIN,
                };
                let high = match &range.end {
                    Some(end) => number(&self.expr(end, scope)?).unwrap_or(f64::MAX),
                    None => f64::MAX,
                };
                let closed = matches!(range.limits, syn::RangeLimits::Closed(_));
                n >= low && (if closed { n <= high } else { n < high })
            }
            Pat::Or(or) => {
                for case in &or.cases {
                    if self.matches(case, value, scope)? {
                        return Ok(true);
                    }
                }
                false
            }
            Pat::TupleStruct(tuple) => {
                let name = path_names(&tuple.path).last().cloned().unwrap_or_default();
                let inner = match (name.as_str(), value.read()) {
                    ("Some", Value::Option(Some(inner))) => *inner,
                    ("Ok", Value::Result(Ok(inner))) | ("Err", Value::Result(Err(inner))) => *inner,
                    ("Some" | "Ok" | "Err", _) => return Ok(false),
                    (_, Value::Record(record)) if record.name == name => {
                        let fields: Vec<Value> =
                            record.fields.borrow().iter().map(|(_, v)| v.clone()).collect();
                        for (pattern, field) in tuple.elems.iter().zip(fields) {
                            if !self.matches(pattern, &field, scope)? {
                                return Ok(false);
                            }
                        }
                        return Ok(true);
                    }
                    _ => {
                        return fail(
                            tuple.span(),
                            "only `Some(..)`, `Ok(..)`, `Err(..)` and the program's structs are read in patterns of this kind",
                        );
                    }
                };
                match tuple.elems.first() {
                    Some(pattern) => self.matches(pattern, &inner, scope)?,
                    None => true,
                }
            }
            Pat::Path(path) => {
                let names = path_names(&path.path);
                let expected = self.path_value(&names, path.span())?;
                expected == value.read() || same_host(&expected, value)
            }
            other => return fail(other.span(), "this pattern is not read here"),
        })
    }

    fn literal(&self, literal: &Lit) -> Eval {
        Ok(match literal {
            Lit::Int(n) => {
                if matches!(n.suffix(), "f32" | "f64") {
                    Value::Float(
                        n.base10_parse().map_err(|e| Flow::Error(LiveError::at(n.span(), e.to_string())))?,
                    )
                } else {
                    Value::Int(
                        n.base10_parse().map_err(|e| Flow::Error(LiveError::at(n.span(), e.to_string())))?,
                    )
                }
            }
            Lit::Float(n) => Value::Float(
                n.base10_parse().map_err(|e| Flow::Error(LiveError::at(n.span(), e.to_string())))?,
            ),
            Lit::Str(s) if !s.suffix().is_empty() => return fail(s.span(), "a text literal takes no suffix"),
            Lit::Str(s) => Value::Str(s.value()),
            Lit::Bool(b) => Value::Bool(b.value),
            Lit::Char(c) => Value::Char(c.value()),
            Lit::Byte(b) => Value::Int(i64::from(b.value())),
            other => return fail(other.span(), "this kind of literal is not read here"),
        })
    }

    /// `Type::NAME` or a constant of the program.
    fn path_value(&self, names: &[String], span: Span) -> Eval {
        match names {
            [name] => {
                if name == "None" {
                    return Ok(Value::Option(None));
                }
                let constant = self.program.constants.borrow().get(name).cloned();
                if let Some(expr) = constant {
                    return self.expr(&expr, &Scope::root());
                }
                let shape = self.program.structs.borrow().get(name).cloned();
                match shape {
                    Some(Shape::Unit) => return Ok(self.record(name, Vec::new())),
                    Some(Shape::Tuple(_)) => return Ok(self.path_closure(names.to_vec(), span)),
                    _ => {}
                }
                if self.program.function(name).is_some()
                    || matches!(name.as_str(), "Some" | "Ok" | "Err")
                    || self.registry().find_free(name).is_some()
                {
                    return Ok(self.path_closure(names.to_vec(), span));
                }
                fail(span, format!("there is no `{name}` here"))
            }
            [.., ty, name] => {
                if let Some(value) = self.registry().find_constant(ty, name) {
                    return Ok(value);
                }
                if let Some(value) = builtin_constant(ty, name) {
                    return Ok(value);
                }
                // A function named where a closure goes, as `Clone::clone` or `validate::email`.
                if let Some(f) = self.registry().find_function(ty, name) {
                    return Ok(Value::Closure(Rc::new(Closure {
                        body: Body::Host(f),
                        program: self.program.clone(),
                        span,
                    })));
                }
                if self.program.method(ty, name).is_some()
                    || self.registry().find_method(ty, name).is_some()
                    || matches!(
                        ty.as_str(),
                        "Clone"
                            | "ToString"
                            | "ToOwned"
                            | "Into"
                            | "From"
                            | "Vec"
                            | "String"
                            | "Option"
                            | "Box"
                            | "Rc"
                            | "Default"
                    )
                {
                    return Ok(self.path_closure(names.to_vec(), span));
                }
                if self.registry().knows_type(ty) {
                    fail(span, format!("`{ty}` has no `{name}` the playground knows"))
                } else {
                    fail(span, format!("the playground does not know `{ty}` yet"))
                }
            }
            [] => fail(span, "an empty path"),
        }
    }

    fn path_closure(&self, names: Vec<String>, span: Span) -> Value {
        Value::Closure(Rc::new(Closure { body: Body::Path(names), program: self.program.clone(), span }))
    }

    fn record(&self, name: &str, fields: Vec<(String, Value)>) -> Value {
        Value::Record(Rc::new(Record {
            name: name.to_owned(),
            fields: RefCell::new(fields),
            program: Some(self.program.clone()),
        }))
    }

    /// `Name { a, b: 2, ..base }`, for a struct of the program or one the host offers.
    fn struct_literal(&self, literal: &syn::ExprStruct, scope: &Rc<Scope>) -> Eval {
        let names = path_names(&literal.path);
        let name = names.last().cloned().unwrap_or_default();
        let mut fields = Vec::new();
        for field in &literal.fields {
            let key = match &field.member {
                syn::Member::Named(ident) => ident.to_string(),
                syn::Member::Unnamed(index) => index.index.to_string(),
            };
            fields.push((key, self.expr(&field.expr, scope)?));
        }
        let base = match &literal.rest {
            Some(rest) => Some(self.expr(rest, scope)?),
            None => None,
        };
        let shape = self.program.structs.borrow().get(&name).cloned();
        if let Some(shape) = shape {
            let Shape::Named(order) = shape else {
                return fail(literal.span(), format!("`{name}` has no named fields"));
            };
            let base = match base.map(|b| b.read()) {
                Some(Value::Record(record)) => record.fields.borrow().clone(),
                Some(other) => {
                    return fail(literal.span(), format!("`..` takes a {name}, not a {}", other.kind()));
                }
                None => Vec::new(),
            };
            let mut made = Vec::new();
            for key in order {
                let value = match fields.iter().position(|(k, _)| *k == key) {
                    Some(at) => fields.remove(at).1,
                    None => match base.iter().find(|(k, _)| *k == key) {
                        Some((_, v)) => v.clone(),
                        None => {
                            return fail(literal.span(), format!("the field `{key}` of `{name}` is missing"));
                        }
                    },
                };
                made.push((key, value));
            }
            if let Some((extra, _)) = fields.first() {
                return fail(literal.span(), format!("`{name}` has no field `{extra}`"));
            }
            return Ok(self.record(&name, made));
        }
        match self.registry().find_record(&name) {
            Some(build) => build(fields, base).map_err(|e| Flow::Error(LiveError::at(literal.span(), e))),
            None => fail(
                literal.span(),
                format!(
                    "the playground does not build `{}` from its fields yet",
                    short(&quote_path(&literal.path))
                ),
            ),
        }
    }

    /// `value.field`, of a tuple, a struct of the program or a host value.
    fn field(&self, value: &Value, member: &syn::Member, span: Span) -> Eval {
        let key = match member {
            syn::Member::Named(ident) => ident.to_string(),
            syn::Member::Unnamed(index) => index.index.to_string(),
        };
        match value.read() {
            Value::Tuple(items) => {
                let at: usize = key.parse().unwrap_or(usize::MAX);
                items.get(at).cloned().ok_or(()).or_else(|()| fail(span, "the tuple is shorter"))
            }
            Value::Record(record) => record
                .field(&key)
                .ok_or(())
                .or_else(|()| fail(span, format!("a {} has no field `{key}`", record.name))),
            Value::Host(host) => match self.registry().find_field(host.type_name(), &key) {
                Some(get) => get(&Value::Host(host)).map_err(|e| Flow::Error(LiveError::at(span, e))),
                None => fail(
                    span,
                    format!("the playground cannot read the field `{key}` of a {}", host.type_name()),
                ),
            },
            other => fail(span, format!("a {} has no fields the playground reads", other.kind())),
        }
    }

    fn expr(&self, expr: &Expr, scope: &Rc<Scope>) -> Eval {
        match expr {
            Expr::Lit(literal) => self.literal(&literal.lit),
            Expr::Path(path) => {
                let names = path_names(&path.path);
                if let [name] = names.as_slice()
                    && let Some(value) = scope.get(name)
                {
                    return Ok(value);
                }
                self.path_value(&names, path.span())
            }
            Expr::Paren(inner) => self.expr(&inner.expr, scope),
            Expr::Group(inner) => self.expr(&inner.expr, scope),
            Expr::Block(block) => self.block(&block.block, scope),
            Expr::Reference(reference) => self.expr(&reference.expr, scope),
            Expr::Tuple(tuple) => {
                if tuple.elems.is_empty() {
                    return Ok(Value::Unit);
                }
                Ok(Value::Tuple(self.all(tuple.elems.iter(), scope)?))
            }
            Expr::Array(array) => Ok(Value::List(self.all(array.elems.iter(), scope)?)),
            Expr::Repeat(repeat) => {
                let item = self.expr(&repeat.expr, scope)?;
                let count = self.int(&repeat.len, scope)?;
                Ok(Value::List(vec![item; usize::try_from(count).unwrap_or(0)]))
            }
            Expr::Range(range) => {
                let start = match &range.start {
                    Some(start) => self.expr(start, scope)?.read(),
                    None => Value::Int(0),
                };
                let Some(end) = &range.end else { return fail(range.span(), "a range needs an end here") };
                let end_value = self.expr(end, scope)?.read();
                if matches!(start, Value::Float(_)) || matches!(end_value, Value::Float(_)) {
                    let (Some(a), Some(b)) = (number(&start), number(&end_value)) else {
                        return fail(range.span(), "a range takes numbers");
                    };
                    return Ok(Value::FloatRange(a, b));
                }
                let (Value::Int(start), Value::Int(end)) = (start, end_value) else {
                    return fail(range.span(), "a range takes numbers");
                };
                let end = if matches!(range.limits, syn::RangeLimits::Closed(_)) { end + 1 } else { end };
                Ok(Value::Range(start, end))
            }
            Expr::Unary(unary) => {
                let value = self.expr(&unary.expr, scope)?;
                match unary.op {
                    UnOp::Neg(_) => match value.read() {
                        Value::Int(n) => Ok(Value::Int(-n)),
                        Value::Float(n) => Ok(Value::Float(-n)),
                        other => fail(unary.span(), format!("cannot negate a {}", other.kind())),
                    },
                    UnOp::Not(_) => match value.read() {
                        Value::Bool(b) => Ok(Value::Bool(!b)),
                        other => fail(unary.span(), format!("cannot negate a {}", other.kind())),
                    },
                    UnOp::Deref(_) => Ok(value.read()),
                    _ => fail(unary.span(), "this operator is not read here"),
                }
            }
            Expr::Binary(binary) => self.binary(binary, scope),
            Expr::Assign(assign) => {
                let value = self.expr(&assign.right, scope)?;
                self.assign(&assign.left, value, scope)?;
                Ok(Value::Unit)
            }
            Expr::Cast(cast) => {
                let value = self.expr(&cast.expr, scope)?;
                let target = match &*cast.ty {
                    syn::Type::Path(path) => {
                        path.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default()
                    }
                    _ => String::new(),
                };
                let as_number = number(&value);
                Ok(match (target.as_str(), as_number, value.read()) {
                    ("f32" | "f64", Some(n), _) => Value::Float(n),
                    (_, Some(n), _) if target.starts_with('i') || target.starts_with('u') => {
                        Value::Int(n as i64)
                    }
                    (_, _, Value::Char(c)) => Value::Int(i64::from(u32::from(c))),
                    (_, _, Value::Bool(b)) => Value::Int(i64::from(b)),
                    // `as AnyView` and the like only name the type; the value is the same.
                    _ if !matches!(target.as_str(), "f32" | "f64")
                        && !target.starts_with('i')
                        && !target.starts_with('u') =>
                    {
                        value
                    }
                    _ => return fail(cast.span(), format!("cannot cast a {} to {target}", value.kind())),
                })
            }
            Expr::If(branch) => {
                let inner = Scope::child(scope);
                let taken = match &*branch.cond {
                    Expr::Let(binding) => {
                        let value = self.expr(&binding.expr, scope)?;
                        self.matches(&binding.pat, &value, &inner)?
                    }
                    condition => match self.expr(condition, scope)?.read() {
                        Value::Bool(b) => b,
                        other => {
                            return fail(
                                condition.span(),
                                format!("a condition is a bool, not a {}", other.kind()),
                            );
                        }
                    },
                };
                if taken {
                    self.block(&branch.then_branch, &inner)
                } else if let Some((_, otherwise)) = &branch.else_branch {
                    self.expr(otherwise, scope)
                } else {
                    Ok(Value::Unit)
                }
            }
            Expr::Match(choice) => {
                let value = self.expr(&choice.expr, scope)?;
                for arm in &choice.arms {
                    let inner = Scope::child(scope);
                    if !self.matches(&arm.pat, &value, &inner)? {
                        continue;
                    }
                    if let Some((_, guard)) = &arm.guard
                        && self.expr(guard, &inner)?.read() != Value::Bool(true)
                    {
                        continue;
                    }
                    return self.expr(&arm.body, &inner);
                }
                fail(choice.span(), format!("no arm fits {:?}", value))
            }
            Expr::ForLoop(each) => {
                let items = self
                    .expr(&each.expr, scope)?
                    .items()
                    .map_err(|e| Flow::Error(LiveError::at(each.expr.span(), e)))?;
                for item in items {
                    let inner = Scope::child(scope);
                    self.bind(&each.pat, item, &inner)?;
                    match self.block(&each.body, &inner) {
                        Ok(_) | Err(Flow::Continue) => {}
                        Err(Flow::Break) => break,
                        Err(other) => return Err(other),
                    }
                }
                Ok(Value::Unit)
            }
            Expr::While(each) => {
                let mut rounds = 0u32;
                loop {
                    rounds += 1;
                    if rounds > 100_000 {
                        return fail(each.span(), "this loop ran 100 000 times: the playground stops it");
                    }
                    if self.expr(&each.cond, scope)?.read() != Value::Bool(true) {
                        break;
                    }
                    match self.block(&each.body, scope) {
                        Ok(_) | Err(Flow::Continue) => {}
                        Err(Flow::Break) => break,
                        Err(other) => return Err(other),
                    }
                }
                Ok(Value::Unit)
            }
            Expr::Break(_) => Err(Flow::Break),
            Expr::Continue(_) => Err(Flow::Continue),
            Expr::Return(back) => {
                let value = match &back.expr {
                    Some(value) => self.expr(value, scope)?,
                    None => Value::Unit,
                };
                Err(Flow::Return(value))
            }
            Expr::Closure(closure) => Ok(Value::Closure(Rc::new(Closure {
                body: Body::Code(Box::new(Code {
                    params: closure.inputs.iter().cloned().collect(),
                    body: (*closure.body).clone(),
                    scope: scope.clone(),
                })),
                program: self.program.clone(),
                span: closure.span(),
            }))),
            Expr::Field(field) => {
                let value = self.expr(&field.base, scope)?;
                self.field(&value, &field.member, field.span())
            }
            Expr::Try(tried) => match self.expr(&tried.expr, scope)?.read() {
                Value::Option(Some(inner)) | Value::Result(Ok(inner)) => Ok(*inner),
                none @ (Value::Option(None) | Value::Result(Err(_))) => Err(Flow::Return(none)),
                other => {
                    fail(tried.span(), format!("`?` takes an Option or a Result, not a {}", other.kind()))
                }
            },
            Expr::Index(index) => {
                let items = self
                    .expr(&index.expr, scope)?
                    .items()
                    .map_err(|e| Flow::Error(LiveError::at(index.span(), e)))?;
                let at = self.int(&index.index, scope)?;
                usize::try_from(at)
                    .ok()
                    .and_then(|at| items.get(at).cloned())
                    .ok_or(())
                    .or_else(|()| fail(index.span(), format!("index {at} is past the end ({})", items.len())))
            }
            Expr::Macro(mac) => self.mac(&mac.mac, scope),
            Expr::Call(call) => self.call(call, scope),
            Expr::MethodCall(call) => self.method(call, scope),
            Expr::Struct(literal) => self.struct_literal(literal, scope),
            Expr::Let(binding) => {
                let value = self.expr(&binding.expr, scope)?;
                Ok(Value::Bool(self.matches(&binding.pat, &value, scope)?))
            }
            other => fail(other.span(), "this expression is not read by the playground yet"),
        }
    }

    fn all<'a>(&self, exprs: impl Iterator<Item = &'a Expr>, scope: &Rc<Scope>) -> Eval<Vec<Value>> {
        exprs.map(|e| self.expr(e, scope)).collect()
    }

    fn int(&self, expr: &Expr, scope: &Rc<Scope>) -> Eval<i64> {
        match self.expr(expr, scope)?.read() {
            Value::Int(n) => Ok(n),
            Value::Float(n) => Ok(n as i64),
            other => fail(expr.span(), format!("expected a number, found {}", other.kind())),
        }
    }

    fn assign(&self, target: &Expr, value: Value, scope: &Rc<Scope>) -> Eval<()> {
        match target {
            Expr::Path(path) => {
                let names = path_names(&path.path);
                let [name] = names.as_slice() else {
                    return fail(path.span(), "only variables can be assigned");
                };
                match scope.get(name) {
                    Some(Value::Place(place)) => *place.borrow_mut() = value,
                    Some(_) => {
                        scope.set(name, value);
                    }
                    None => return fail(path.span(), format!("there is no `{name}` here")),
                }
                Ok(())
            }
            Expr::Unary(unary) if matches!(unary.op, UnOp::Deref(_)) => {
                match self.expr(&unary.expr, scope)? {
                    Value::Place(place) => {
                        *place.borrow_mut() = value;
                        Ok(())
                    }
                    other => fail(unary.span(), format!("a {} cannot be changed through `*`", other.kind())),
                }
            }
            Expr::Paren(inner) => self.assign(&inner.expr, value, scope),
            Expr::Field(field) => {
                let key = match &field.member {
                    syn::Member::Named(ident) => ident.to_string(),
                    syn::Member::Unnamed(index) => index.index.to_string(),
                };
                match self.expr(&field.base, scope)?.read() {
                    Value::Record(record) => {
                        let mut fields = record.fields.borrow_mut();
                        match fields.iter_mut().find(|(k, _)| *k == key) {
                            Some((_, slot)) => *slot = value,
                            None => {
                                return fail(field.span(), format!("a {} has no field `{key}`", record.name));
                            }
                        }
                        Ok(())
                    }
                    Value::Host(host) => match self.registry().find_set_field(host.type_name(), &key) {
                        Some(set) => set(&Value::Host(host), value)
                            .map_err(|e| Flow::Error(LiveError::at(field.span(), e))),
                        None => fail(
                            field.span(),
                            format!(
                                "the playground cannot change the field `{key}` of a {}",
                                host.type_name()
                            ),
                        ),
                    },
                    other => fail(field.span(), format!("a {} has no fields to change", other.kind())),
                }
            }
            other => fail(other.span(), "only variables, fields and `*place` can be assigned here"),
        }
    }

    fn binary(&self, binary: &syn::ExprBinary, scope: &Rc<Scope>) -> Eval {
        let span = binary.span();
        match binary.op {
            BinOp::And(_) | BinOp::Or(_) => {
                let left = self.expr(&binary.left, scope)?.read() == Value::Bool(true);
                let and = matches!(binary.op, BinOp::And(_));
                if and != left {
                    return Ok(Value::Bool(left));
                }
                let right = self.expr(&binary.right, scope)?;
                return match right.read() {
                    Value::Bool(b) => Ok(Value::Bool(b)),
                    other => fail(span, format!("expected a bool, found {}", other.kind())),
                };
            }
            _ => {}
        }
        let left = self.expr(&binary.left, scope)?.read();
        let right = self.expr(&binary.right, scope)?.read();
        let compound = match binary.op {
            BinOp::AddAssign(_) => Some('+'),
            BinOp::SubAssign(_) => Some('-'),
            BinOp::MulAssign(_) => Some('*'),
            BinOp::DivAssign(_) => Some('/'),
            BinOp::RemAssign(_) => Some('%'),
            _ => None,
        };
        if let Some(op) = compound {
            let value = arithmetic(op, &left, &right).map_err(|e| Flow::Error(LiveError::at(span, e)))?;
            self.assign(&binary.left, value, scope)?;
            return Ok(Value::Unit);
        }
        let result = match binary.op {
            BinOp::Add(_) => arithmetic('+', &left, &right),
            BinOp::Sub(_) => arithmetic('-', &left, &right),
            BinOp::Mul(_) => arithmetic('*', &left, &right),
            BinOp::Div(_) => arithmetic('/', &left, &right),
            BinOp::Rem(_) => arithmetic('%', &left, &right),
            BinOp::Eq(_) => Ok(Value::Bool(left == right)),
            BinOp::Ne(_) => Ok(Value::Bool(left != right)),
            BinOp::Lt(_) | BinOp::Le(_) | BinOp::Gt(_) | BinOp::Ge(_) => {
                let order = compare(&left, &right)
                    .ok_or_else(|| format!("cannot compare a {} with a {}", left.kind(), right.kind()));
                order.map(|order| {
                    Value::Bool(match binary.op {
                        BinOp::Lt(_) => order.is_lt(),
                        BinOp::Le(_) => order.is_le(),
                        BinOp::Gt(_) => order.is_gt(),
                        _ => order.is_ge(),
                    })
                })
            }
            BinOp::BitAnd(_) | BinOp::BitOr(_) | BinOp::BitXor(_) => match (&left, &right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(match binary.op {
                    BinOp::BitAnd(_) => a & b,
                    BinOp::BitOr(_) => a | b,
                    _ => a ^ b,
                })),
                (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(match binary.op {
                    BinOp::BitAnd(_) => a & b,
                    BinOp::BitOr(_) => a | b,
                    _ => a ^ b,
                })),
                _ => Err("bit operators take integers or bools".to_owned()),
            },
            _ => Err("this operator is not read here".to_owned()),
        };
        result.map_err(|e| Flow::Error(LiveError::at(span, e)))
    }

    fn mac(&self, mac: &syn::Macro, scope: &Rc<Scope>) -> Eval {
        let name = mac.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
        let span = mac.span();
        let args = || {
            mac.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
                .map_err(|e| Flow::Error(LiveError::at(e.span(), e.to_string())))
        };
        match name.as_str() {
            "vec" => {
                // `vec![x; n]` or `vec![a, b, c]`.
                let repeat = mac.parse_body_with(|input: syn::parse::ParseStream<'_>| {
                    let item: Expr = input.parse()?;
                    input.parse::<Token![;]>()?;
                    let count: Expr = input.parse()?;
                    Ok((item, count))
                });
                if let Ok((item, count)) = repeat {
                    let item = self.expr(&item, scope)?;
                    let count = self.int(&count, scope)?;
                    return Ok(Value::List(vec![item; usize::try_from(count).unwrap_or(0)]));
                }
                let items = args()?;
                Ok(Value::List(self.all(items.iter(), scope)?))
            }
            "format" | "println" | "eprintln" | "print" | "eprint" | "panic" | "assert" | "debug_assert" => {
                let items: Vec<Expr> = args()?.into_iter().collect();
                if matches!(name.as_str(), "assert" | "debug_assert") {
                    let holds = match items.first() {
                        Some(condition) => self.expr(condition, scope)?.read() == Value::Bool(true),
                        None => true,
                    };
                    return if holds { Ok(Value::Unit) } else { fail(span, "an assert failed") };
                }
                let Some((first, rest)) = items.split_first() else {
                    if matches!(name.as_str(), "println" | "eprintln") {
                        crate::value::print("\n");
                        return Ok(Value::Unit);
                    }
                    return Ok(Value::Str(String::new()));
                };
                let Expr::Lit(syn::ExprLit { lit: Lit::Str(template), .. }) = first else {
                    return fail(first.span(), "format! starts with a string literal");
                };
                let values = self.all(rest.iter(), scope)?;
                let text = format_text(&template.value(), &values, scope)
                    .map_err(|e| Flow::Error(LiveError::at(template.span(), e)))?;
                match name.as_str() {
                    "format" => Ok(Value::Str(text)),
                    "panic" => fail(span, text),
                    "println" | "eprintln" => {
                        crate::value::print(&format!("{text}\n"));
                        Ok(Value::Unit)
                    }
                    _ => {
                        crate::value::print(&text);
                        Ok(Value::Unit)
                    }
                }
            }
            "unreachable" | "todo" | "unimplemented" => fail(span, format!("`{name}!` was reached")),
            "matches" => {
                let (value, pattern) = mac
                    .parse_body_with(|input: syn::parse::ParseStream<'_>| {
                        let value: Expr = input.parse()?;
                        input.parse::<Token![,]>()?;
                        let pattern = Pat::parse_multi_with_leading_vert(input)?;
                        Ok((value, pattern))
                    })
                    .map_err(|e| Flow::Error(LiveError::at(e.span(), e.to_string())))?;
                let value = self.expr(&value, scope)?;
                Ok(Value::Bool(self.matches(&pattern, &value, &Scope::child(scope))?))
            }
            _ => fail(span, format!("the playground does not read the macro `{name}!`")),
        }
    }

    fn call(&self, call: &syn::ExprCall, scope: &Rc<Scope>) -> Eval {
        let span = call.span();
        let Expr::Path(path) = &*call.func else {
            let callee = self.expr(&call.func, scope)?;
            let arguments = self.all(call.args.iter(), scope)?;
            return self.call_value(&callee, arguments, span);
        };
        let names = path_names(&path.path);
        if let [name] = names.as_slice()
            && let Some(callee) = scope.get(name)
        {
            let arguments = self.all(call.args.iter(), scope)?;
            return self.call_value(&callee, arguments, span);
        }
        let arguments = self.all(call.args.iter(), scope)?;
        self.call_path(&names, arguments, span)
    }

    /// Calls what a path names: a function of the program or of the host, a constructor the
    /// interpreter knows, or a method written as `Type::method(value, ..)`.
    fn call_path(&self, names: &[String], arguments: Vec<Value>, span: Span) -> Eval {
        let first = || arguments.first().cloned().unwrap_or(Value::Unit);
        match names.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
            ["Some"] => Ok(Value::Option(Some(Box::new(first())))),
            ["Ok"] => Ok(Value::Result(Ok(Box::new(first())))),
            ["Err"] => Ok(Value::Result(Err(Box::new(first())))),
            ["Box", "new"]
            | ["Rc", "new"]
            | ["Arc", "new"]
            | ["String", "from"]
            | ["Cell", "new"]
            | ["From", "from"]
            | ["Into", "into"] => Ok(first()),
            ["Vec", "new"]
            | ["Vec", "with_capacity"]
            | ["BTreeSet" | "HashSet" | "BTreeMap" | "HashMap", "new"] => Ok(Value::List(Vec::new())),
            ["BTreeSet" | "HashSet" | "Vec", "from" | "from_iter"] => {
                let mut items = first().items().map_err(|e| Flow::Error(LiveError::at(span, e)))?;
                if names[0].ends_with("Set") {
                    let mut kept: Vec<Value> = Vec::new();
                    for item in items.drain(..) {
                        if !kept.contains(&item) {
                            kept.push(item);
                        }
                    }
                    kept.sort_by(|a, b| compare(a, b).unwrap_or(std::cmp::Ordering::Equal));
                    items = kept;
                }
                Ok(Value::List(items))
            }
            ["f32" | "f64", "from"] => {
                number(&first()).map(Value::Float).ok_or(()).or_else(|()| fail(span, "a number goes here"))
            }
            [ty, "from" | "try_from"] if ty.starts_with('i') || ty.starts_with('u') => number(&first())
                .map(|n| Value::Int(n as i64))
                .ok_or(())
                .or_else(|()| fail(span, "a number goes here")),
            ["String", "new"] => Ok(Value::Str(String::new())),
            ["State", "new"] | ["RefCell", "new"] => Ok(Value::State(State::new(first()))),
            ["Default", "default"] => fail(span, "say the value instead of `Default::default()` here"),
            [name] => {
                let shape = self.program.structs.borrow().get(*name).cloned();
                if let Some(Shape::Tuple(count)) = shape {
                    if arguments.len() != count {
                        return fail(span, format!("`{name}` takes {count} values"));
                    }
                    let fields = arguments.into_iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect();
                    return Ok(self.record(name, fields));
                }
                if let Some(function) = self.program.function(name) {
                    return self.call_fn(&function, arguments, span).map_err(Flow::Error);
                }
                if let Some(host) = self.registry().find_free(name) {
                    return host(arguments).map_err(|e| Flow::Error(LiveError::at(span, e)));
                }
                fail(span, format!("there is no function `{name}` here"))
            }
            [.., ty, name] => {
                if let Some(method) = self.program.method(ty, name) {
                    return self.call_fn(&method, arguments, span).map_err(Flow::Error);
                }
                if let Some(host) = self.registry().find_function(ty, name) {
                    return host(arguments).map_err(|e| Flow::Error(LiveError::at(span, e)));
                }
                // `Type::method(value, ..)` is `value.method(..)`, as `Clone::clone` passed to a map.
                let is_method = self.registry().find_method(ty, name).is_some()
                    || matches!(
                        *ty,
                        "Clone" | "ToString" | "ToOwned" | "Into" | "Option" | "String" | "Vec" | "str"
                    );
                if is_method && !arguments.is_empty() {
                    let mut arguments = arguments;
                    let receiver = arguments.remove(0);
                    return self.call_method(receiver, name, arguments, span);
                }
                if self.registry().knows_type(ty) {
                    fail(span, format!("`{ty}::{name}` is not one the playground knows yet"))
                } else {
                    fail(span, format!("the playground does not know `{ty}` yet"))
                }
            }
            [] => fail(span, "an empty path"),
        }
    }

    fn call_value(&self, callee: &Value, arguments: Vec<Value>, span: Span) -> Eval {
        match callee.read() {
            Value::Closure(closure) => closure.call(arguments).map_err(Flow::Error),
            other => fail(span, format!("a {} cannot be called", other.kind())),
        }
    }

    fn closure_arg(&self, value: &Value, span: Span) -> Eval<Rc<Closure>> {
        match value.read() {
            Value::Closure(closure) => Ok(closure),
            other => fail(span, format!("expected a closure, found {}", other.kind())),
        }
    }

    fn method(&self, call: &syn::ExprMethodCall, scope: &Rc<Scope>) -> Eval {
        let span = call.method.span();
        let name = call.method.to_string();
        let receiver = self.expr(&call.receiver, scope)?;
        let arguments = self.all(call.args.iter(), scope)?;
        self.call_method(receiver, &name, arguments, span)
    }

    fn call_method(&self, receiver: Value, name: &str, arguments: Vec<Value>, span: Span) -> Eval {
        let name = name.to_owned();
        let err = |e: String| Flow::Error(LiveError::at(span, e));
        // `Place`s hold what a closure may change; most methods read them.
        if let Value::Place(place) = &receiver
            && name == "push"
        {
            let mut list = place.borrow_mut();
            if let Value::List(items) = &mut *list {
                items.extend(arguments);
                return Ok(Value::Unit);
            }
        }
        let value = receiver.read();
        if let Value::Record(record) = &value {
            if let Some(method) = self.program.method(&record.name, &name) {
                let mut all = vec![value.clone()];
                all.extend(arguments);
                return self.call_fn(&method, all, span).map_err(Flow::Error);
            }
            if matches!(name.as_str(), "clone" | "to_owned") {
                let fields = record.fields.borrow().clone();
                return Ok(self.record(&record.name, fields));
            }
            return fail(span, format!("a {} has no method `{name}`", record.name));
        }
        if let Value::Binding(binding) = &value {
            return match name.as_str() {
                "get" => Ok(binding.get()),
                "set" => {
                    binding.set(arguments.into_iter().next().unwrap_or(Value::Unit));
                    Ok(Value::Unit)
                }
                "update" => {
                    let closure = self.closure_arg(arguments.first().unwrap_or(&Value::Unit), span)?;
                    let place = Rc::new(RefCell::new(fresh(binding.get())));
                    closure.call(vec![Value::Place(place.clone())]).map_err(Flow::Error)?;
                    let changed = place.borrow().clone();
                    binding.set(changed);
                    Ok(Value::Unit)
                }
                "map" => self.lens(binding.clone(), &arguments, span),
                "clone" | "binding" => Ok(value.clone()),
                _ => fail(span, format!("a Binding has no method `{name}` the playground knows")),
            };
        }
        if let Value::Host(host) = &value {
            // Methods every view has, such as `.padding(..)`, are kept under `View`.
            let found = self
                .registry()
                .find_method_for(host.type_name(), &name, host.held_type())
                .or_else(|| self.registry().find_method("View", &name));
            if let Some(f) = found {
                let mut all = vec![value.clone()];
                all.extend(arguments);
                return f(all).map_err(err);
            }
            return match name.as_str() {
                "clone" | "to_owned" => Ok(host.duplicate().map_or(value.clone(), Value::Host)),
                "into" => Ok(value),
                _ => {
                    fail(span, format!("`{}` has no method `{name}` the playground knows", host.type_name()))
                }
            };
        }
        if let Value::State(state) = &value {
            return match name.as_str() {
                "get" => Ok(state.get()),
                "set" | "set_if_changed" => {
                    state.set(arguments.into_iter().next().unwrap_or(Value::Unit));
                    Ok(Value::Unit)
                }
                "update" => {
                    let closure = self.closure_arg(arguments.first().unwrap_or(&Value::Unit), span)?;
                    let place = Rc::new(RefCell::new(fresh(state.get())));
                    closure.call(vec![Value::Place(place.clone())]).map_err(Flow::Error)?;
                    let changed = place.borrow().clone();
                    state.set(changed);
                    Ok(Value::Unit)
                }
                "with" => {
                    let closure = self.closure_arg(arguments.first().unwrap_or(&Value::Unit), span)?;
                    closure.call(vec![state.get()]).map_err(Flow::Error)
                }
                "map" => self.lens(state.binding(), &arguments, span),
                "clone" | "binding" => Ok(value),
                _ => fail(span, format!("a State has no method `{name}` the playground knows")),
            };
        }
        builtin_method(self, &name, value, arguments, span)
    }
}

/// A copy of a value whose host objects are copied too, so changing it through a place does
/// not change the original before it is set back.
fn fresh(value: Value) -> Value {
    match value {
        Value::Host(host) => Value::Host(host.duplicate().unwrap_or(host)),
        Value::Tuple(items) => Value::Tuple(items.into_iter().map(fresh).collect()),
        Value::List(items) => Value::List(items.into_iter().map(fresh).collect()),
        Value::Option(Some(inner)) => Value::Option(Some(Box::new(fresh(*inner)))),
        other => other,
    }
}

impl Runner {
    /// `binding.map(|whole| part, |whole, part| ..)`: a binding to a part of another.
    fn lens(&self, whole: ravenrs_reactive::Binding<Value>, arguments: &[Value], span: Span) -> Eval {
        let get = self.closure_arg(arguments.first().unwrap_or(&Value::Unit), span)?;
        let set = self.closure_arg(arguments.get(1).unwrap_or(&Value::Unit), span)?;
        let reader = whole.clone();
        let binding = ravenrs_reactive::Binding::new(
            move || match get.call(vec![reader.get()]) {
                Ok(part) => part,
                Err(problem) => {
                    crate::value::report(problem);
                    Value::Unit
                }
            },
            move |part: Value| {
                let place = Rc::new(RefCell::new(fresh(whole.get())));
                match set.call(vec![Value::Place(place.clone()), part]) {
                    Ok(_) => whole.set(place.borrow().clone()),
                    Err(problem) => crate::value::report(problem),
                }
            },
        );
        Ok(Value::Binding(binding))
    }
}

fn quote_path(path: &syn::Path) -> String {
    path_names(path).join("::")
}

fn same_host(expected: &Value, value: &Value) -> bool {
    match (expected, value.read()) {
        (Value::Host(a), Value::Host(b)) => {
            a.type_name() == b.type_name() && format!("{a:?}") == format!("{b:?}")
        }
        _ => false,
    }
}

impl fmt::Debug for crate::value::Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.type_name())
    }
}

fn builtin_constant(ty: &str, name: &str) -> Option<Value> {
    Some(match (ty, name) {
        ("f32" | "f64", "MAX") => Value::Float(f64::MAX),
        ("f32" | "f64", "MIN") => Value::Float(f64::MIN),
        ("f32" | "f64", "INFINITY") => Value::Float(f64::INFINITY),
        ("f32" | "f64", "NEG_INFINITY") => Value::Float(f64::NEG_INFINITY),
        ("f32", "EPSILON") => Value::Float(f64::from(f32::EPSILON)),
        ("consts", "PI") => Value::Float(std::f64::consts::PI),
        ("consts", "TAU") => Value::Float(std::f64::consts::TAU),
        ("u8", "MAX") => Value::Int(i64::from(u8::MAX)),
        ("u32", "MAX") => Value::Int(i64::from(u32::MAX)),
        ("i32", "MAX") => Value::Int(i64::from(i32::MAX)),
        ("usize" | "u64" | "i64", "MAX") => Value::Int(i64::MAX),
        _ => return None,
    })
}

fn arithmetic(op: char, left: &Value, right: &Value) -> Result<Value, String> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(match op {
            '+' => a.wrapping_add(*b),
            '-' => a.wrapping_sub(*b),
            '*' => a.wrapping_mul(*b),
            '/' if *b == 0 => return Err("division by zero".into()),
            '/' => a / b,
            '%' if *b == 0 => return Err("division by zero".into()),
            _ => a % b,
        })),
        (Value::Str(a), Value::Str(b)) if op == '+' => Ok(Value::Str(format!("{a}{b}"))),
        (Value::Str(a), Value::Char(b)) if op == '+' => Ok(Value::Str(format!("{a}{b}"))),
        _ => match (number(left), number(right)) {
            (Some(a), Some(b)) => Ok(Value::Float(match op {
                '+' => a + b,
                '-' => a - b,
                '*' => a * b,
                '/' => a / b,
                _ => a % b,
            })),
            _ => Err(format!("cannot use `{op}` on a {} and a {}", left.kind(), right.kind())),
        },
    }
}

fn compare(left: &Value, right: &Value) -> Option<std::cmp::Ordering> {
    match (left, right) {
        (Value::Str(a), Value::Str(b)) => Some(a.cmp(b)),
        (Value::Char(a), Value::Char(b)) => Some(a.cmp(b)),
        (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
        (Value::Tuple(a), Value::Tuple(b)) | (Value::List(a), Value::List(b)) => {
            for (x, y) in a.iter().zip(b) {
                match compare(x, y)? {
                    std::cmp::Ordering::Equal => {}
                    other => return Some(other),
                }
            }
            Some(a.len().cmp(&b.len()))
        }
        _ => number(left)?.partial_cmp(&number(right)?),
    }
}

/// How a value is written by `{}`.
fn display(value: &Value) -> String {
    match value.read() {
        Value::Str(s) => s,
        Value::Int(n) => n.to_string(),
        Value::Float(n) => n.to_string(),
        Value::Char(c) => c.to_string(),
        Value::Bool(b) => b.to_string(),
        other => format!("{other:?}"),
    }
}

/// `format!`'s rules for `{}`, `{:?}`, `{name}`, `{0}`, `{:.2}`, `{:>5}` and `{{`.
fn format_text(template: &str, values: &[Value], scope: &Rc<Scope>) -> Result<String, String> {
    let mut out = String::new();
    let mut next = 0;
    let mut chars = template.chars().peekable();
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
                let mut inside = String::new();
                for c in chars.by_ref() {
                    if c == '}' {
                        break;
                    }
                    inside.push(c);
                }
                let (name, spec) = inside.split_once(':').unwrap_or((&inside, ""));
                let value = if name.is_empty() {
                    next += 1;
                    values.get(next - 1).cloned().ok_or("more {} than values")?
                } else if let Ok(index) = name.parse::<usize>() {
                    values.get(index).cloned().ok_or("no value at that position")?
                } else {
                    scope.get(name).ok_or_else(|| format!("there is no `{name}` here"))?
                };
                out.push_str(&formatted(&value, spec));
            }
            other => out.push(other),
        }
    }
    Ok(out)
}

fn formatted(value: &Value, spec: &str) -> String {
    let debug = spec.ends_with('?');
    let spec = spec.trim_end_matches('?');
    let (head, precision) = match spec.split_once('.') {
        Some((head, digits)) => (head, digits.parse::<usize>().ok()),
        None => (spec, None),
    };
    let mut text = match (precision, number(value)) {
        (Some(places), Some(n)) => format!("{n:.places$}"),
        _ if debug => format!("{:?}", value.read()),
        _ => display(value),
    };
    let (align, width) = match head.chars().next() {
        Some(a @ ('<' | '>' | '^')) => (Some(a), head[1..].trim_start_matches('0').parse::<usize>().ok()),
        _ => (None, head.trim_start_matches('0').parse::<usize>().ok()),
    };
    if let Some(width) = width {
        let pad = width.saturating_sub(text.chars().count());
        let zero = head.starts_with('0') && number(value).is_some();
        text = match align {
            Some('<') => format!("{text}{}", " ".repeat(pad)),
            Some('^') => format!("{}{text}{}", " ".repeat(pad / 2), " ".repeat(pad - pad / 2)),
            _ if zero => format!("{}{text}", "0".repeat(pad)),
            _ if number(value).is_some() || align == Some('>') => format!("{}{text}", " ".repeat(pad)),
            _ => format!("{text}{}", " ".repeat(pad)),
        };
    }
    text
}

/// Methods of numbers, text, lists, ranges and options, iterators included, eagerly.
fn builtin_method(runner: &Runner, name: &str, value: Value, arguments: Vec<Value>, span: Span) -> Eval {
    let err = |e: String| Flow::Error(LiveError::at(span, e));
    let arg = |i: usize| arguments.get(i).cloned().unwrap_or(Value::Unit);
    let closure = |i: usize| runner.closure_arg(&arg(i), span);
    let call = |f: &Rc<Closure>, args: Vec<Value>| f.call(args).map_err(Flow::Error);
    let truthy = |v: Value| v.read() == Value::Bool(true);
    // Shared by every kind.
    match name {
        "clone"
        | "to_owned"
        | "into"
        | "as_ref"
        | "as_str"
        | "borrow"
        | "iter"
        | "into_iter"
        | "iter_mut"
        | "cloned"
        | "copied"
        | "by_ref"
        | "as_slice"
        | "to_vec"
        | "unwrap_or_default_kind" => {
            return Ok(match value {
                Value::Range(a, b) if name != "clone" => Value::List((a..b).map(Value::Int).collect()),
                other => other,
            });
        }
        "to_string" => return Ok(Value::Str(display(&value))),
        _ => {}
    }
    if name == "contains"
        && let Value::Range(..) | Value::FloatRange(..) = value
    {
        let (low, high, closed) = match value {
            Value::FloatRange(a, b) => (a, b, false),
            Value::Range(a, b) => (a as f64, b as f64, false),
            _ => (0.0, 0.0, false),
        };
        let n = number(&arg(0)).ok_or(()).or_else(|()| fail(span, "`contains` takes a number here"))?;
        return Ok(Value::Bool(n >= low && (n < high || (closed && n <= high))));
    }
    if let Value::Result(result) = &value {
        let (ok, error) = match result {
            Ok(v) => (Some((**v).clone()), None),
            Err(e) => (None, Some((**e).clone())),
        };
        let wrap_ok = |v: Value| Value::Result(Ok(Box::new(v)));
        return match name {
            "is_ok" => Ok(Value::Bool(ok.is_some())),
            "is_err" => Ok(Value::Bool(error.is_some())),
            "ok" => Ok(Value::Option(ok.map(Box::new))),
            "err" => Ok(Value::Option(error.map(Box::new))),
            "unwrap" | "expect" => {
                ok.ok_or(()).or_else(|()| fail(span, format!("unwrap on Err({:?})", error)))
            }
            "unwrap_or" => Ok(ok.unwrap_or_else(|| arg(0))),
            "unwrap_or_else" => match ok {
                Some(v) => Ok(v),
                None => call(&closure(0)?, vec![error.unwrap_or(Value::Unit)]),
            },
            "map" => match ok {
                Some(v) => Ok(wrap_ok(call(&closure(0)?, vec![v])?)),
                None => Ok(value.clone()),
            },
            "map_err" => match error {
                Some(e) => Ok(Value::Result(Err(Box::new(call(&closure(0)?, vec![e])?)))),
                None => Ok(value.clone()),
            },
            "and_then" => match ok {
                Some(v) => call(&closure(0)?, vec![v]),
                None => Ok(value.clone()),
            },
            _ => fail(span, format!("a Result has no method `{name}` the playground knows")),
        };
    }
    match &value {
        Value::Int(_) | Value::Float(_) => {
            let n = number(&value).unwrap_or_default();
            let other = number(&arg(0));
            let float = |f: f64| Ok(Value::Float(f));
            let keep = |f: f64| {
                Ok(if matches!(value, Value::Int(_)) && f.fract() == 0.0 {
                    Value::Int(f as i64)
                } else {
                    Value::Float(f)
                })
            };
            return match name {
                "abs" => keep(n.abs()),
                "min" => keep(n.min(other.unwrap_or(n))),
                "max" => keep(n.max(other.unwrap_or(n))),
                "clamp" => keep(n.clamp(other.unwrap_or(n), number(&arg(1)).unwrap_or(n))),
                "pow" | "powi" => keep(n.powi(other.unwrap_or(1.0) as i32)),
                "powf" => float(n.powf(other.unwrap_or(1.0))),
                "sqrt" => float(n.sqrt()),
                "sin" => float(n.sin()),
                "cos" => float(n.cos()),
                "tan" => float(n.tan()),
                "atan2" => float(n.atan2(other.unwrap_or(1.0))),
                "exp" => float(n.exp()),
                "ln" => float(n.ln()),
                "floor" => float(n.floor()),
                "ceil" => float(n.ceil()),
                "round" => float(n.round()),
                "trunc" => float(n.trunc()),
                "fract" => float(n.fract()),
                "signum" => keep(n.signum()),
                "to_radians" => float(n.to_radians()),
                "to_degrees" => float(n.to_degrees()),
                "rem_euclid" => keep(n.rem_euclid(other.unwrap_or(1.0))),
                "is_finite" => Ok(Value::Bool(n.is_finite())),
                "saturating_sub" => keep((n - other.unwrap_or(0.0)).max(0.0)),
                "saturating_add" | "wrapping_add" => keep(n + other.unwrap_or(0.0)),
                "is_even" => Ok(Value::Bool(n as i64 % 2 == 0)),
                _ => fail(span, format!("a number has no method `{name}` the playground knows")),
            };
        }
        Value::Str(text) => {
            let pattern = match arg(0).read() {
                Value::Str(s) => s,
                Value::Char(c) => c.to_string(),
                _ => String::new(),
            };
            return match name {
                "len" => Ok(Value::Int(text.len() as i64)),
                "is_empty" => Ok(Value::Bool(text.is_empty())),
                "to_uppercase" => Ok(Value::Str(text.to_uppercase())),
                "to_lowercase" => Ok(Value::Str(text.to_lowercase())),
                "trim" => Ok(Value::Str(text.trim().to_owned())),
                "as_bytes" | "bytes" => {
                    Ok(Value::List(text.bytes().map(|b| Value::Int(i64::from(b))).collect()))
                }
                "strip_prefix" => {
                    Ok(Value::Option(text.strip_prefix(&pattern).map(|t| Box::new(Value::Str(t.to_owned())))))
                }
                "strip_suffix" => {
                    Ok(Value::Option(text.strip_suffix(&pattern).map(|t| Box::new(Value::Str(t.to_owned())))))
                }
                "split_once" => Ok(Value::Option(text.split_once(&pattern).map(|(a, b)| {
                    Box::new(Value::Tuple(vec![Value::Str(a.to_owned()), Value::Str(b.to_owned())]))
                }))),
                "trim_start" => Ok(Value::Str(text.trim_start().to_owned())),
                "trim_end" => Ok(Value::Str(text.trim_end().to_owned())),
                "chars" => Ok(Value::List(text.chars().map(Value::Char).collect())),
                "contains" => Ok(Value::Bool(text.contains(&pattern))),
                "starts_with" => Ok(Value::Bool(text.starts_with(&pattern))),
                "ends_with" => Ok(Value::Bool(text.ends_with(&pattern))),
                "repeat" => Ok(Value::Str(
                    text.repeat(usize::try_from(number(&arg(0)).unwrap_or(0.0) as i64).unwrap_or(0)),
                )),
                "split" => Ok(Value::List(text.split(&pattern).map(|s| Value::Str(s.to_owned())).collect())),
                "lines" => Ok(Value::List(text.lines().map(|s| Value::Str(s.to_owned())).collect())),
                "replace" => Ok(Value::Str(text.replace(&pattern, &display(&arg(1))))),
                "parse" => Ok(match text.trim().parse::<i64>() {
                    Ok(n) => Value::Int(n),
                    Err(_) => text.trim().parse::<f64>().map(Value::Float).map_err(|e| err(e.to_string()))?,
                }),
                _ => fail(span, format!("text has no method `{name}` the playground knows")),
            };
        }
        Value::Char(c) => {
            return match name {
                "to_ascii_uppercase" => Ok(Value::Char(c.to_ascii_uppercase())),
                "to_ascii_lowercase" => Ok(Value::Char(c.to_ascii_lowercase())),
                "is_alphabetic" => Ok(Value::Bool(c.is_alphabetic())),
                "is_numeric" => Ok(Value::Bool(c.is_numeric())),
                _ => fail(span, format!("a char has no method `{name}` the playground knows")),
            };
        }
        Value::Option(option) => {
            let inner = option.as_deref().cloned();
            return match name {
                "is_some" => Ok(Value::Bool(inner.is_some())),
                "is_none" => Ok(Value::Bool(inner.is_none())),
                "unwrap" | "expect" => inner.ok_or(()).or_else(|()| fail(span, "unwrap on None")),
                "unwrap_or" => Ok(inner.unwrap_or_else(|| arg(0))),
                "unwrap_or_else" => match inner {
                    Some(v) => Ok(v),
                    None => call(&closure(0)?, Vec::new()),
                },
                "map" => match inner {
                    Some(v) => Ok(Value::Option(Some(Box::new(call(&closure(0)?, vec![v])?)))),
                    None => Ok(Value::Option(None)),
                },
                "and_then" => match inner {
                    Some(v) => call(&closure(0)?, vec![v]),
                    None => Ok(Value::Option(None)),
                },
                "map_or" => match inner {
                    Some(v) => call(&closure(1)?, vec![v]),
                    None => Ok(arg(0)),
                },
                "filter" => match inner {
                    Some(v) if truthy(call(&closure(0)?, vec![v.clone()])?) => {
                        Ok(Value::Option(Some(Box::new(v))))
                    }
                    _ => Ok(Value::Option(None)),
                },
                "or" => Ok(if inner.is_some() { value.clone() } else { arg(0) }),
                "ok_or" => Ok(Value::Result(match inner {
                    Some(v) => Ok(Box::new(v)),
                    None => Err(Box::new(arg(0))),
                })),
                "unwrap_or_default" => Ok(inner.unwrap_or(Value::Unit)),
                _ => list_method(runner, name, value.items().map_err(err)?, &arguments, span),
            };
        }
        _ => {}
    }
    match value.items() {
        Ok(items) => list_method(runner, name, items, &arguments, span),
        Err(_) => fail(span, format!("a {} has no method `{name}` the playground knows", value.kind())),
    }
}

fn list_method(runner: &Runner, name: &str, items: Vec<Value>, arguments: &[Value], span: Span) -> Eval {
    let arg = |i: usize| arguments.get(i).cloned().unwrap_or(Value::Unit);
    let closure = |i: usize| runner.closure_arg(&arg(i), span);
    let call = |f: &Rc<Closure>, args: Vec<Value>| f.call(args).map_err(Flow::Error);
    let truthy = |v: Value| v.read() == Value::Bool(true);
    let count = |i: usize| usize::try_from(number(&arg(i)).unwrap_or(0.0) as i64).unwrap_or(0);
    let some = |v: Option<Value>| Value::Option(v.map(Box::new));
    Ok(match name {
        "map" => {
            let f = closure(0)?;
            Value::List(items.into_iter().map(|v| call(&f, vec![v])).collect::<Eval<Vec<_>>>()?)
        }
        "for_each" => {
            let f = closure(0)?;
            for item in items {
                call(&f, vec![item])?;
            }
            Value::Unit
        }
        "filter" => {
            let f = closure(0)?;
            let mut kept = Vec::new();
            for item in items {
                if truthy(call(&f, vec![item.clone()])?) {
                    kept.push(item);
                }
            }
            Value::List(kept)
        }
        "filter_map" | "flat_map" | "flatten" => {
            let f = if name == "flatten" { None } else { Some(closure(0)?) };
            let mut kept = Vec::new();
            for item in items {
                let out = match &f {
                    Some(f) => call(f, vec![item])?,
                    None => item,
                };
                kept.extend(out.items().map_err(|e| Flow::Error(LiveError::at(span, e)))?);
            }
            Value::List(kept)
        }
        "enumerate" => Value::List(
            items.into_iter().enumerate().map(|(i, v)| Value::Tuple(vec![Value::Int(i as i64), v])).collect(),
        ),
        "rev" => Value::List(items.into_iter().rev().collect()),
        "take" => Value::List(items.into_iter().take(count(0)).collect()),
        "skip" => Value::List(items.into_iter().skip(count(0)).collect()),
        "step_by" => Value::List(items.into_iter().step_by(count(0).max(1)).collect()),
        "chain" => {
            let mut all = items;
            all.extend(arg(0).items().map_err(|e| Flow::Error(LiveError::at(span, e)))?);
            Value::List(all)
        }
        "zip" => {
            let other = arg(0).items().map_err(|e| Flow::Error(LiveError::at(span, e)))?;
            Value::List(items.into_iter().zip(other).map(|(a, b)| Value::Tuple(vec![a, b])).collect())
        }
        "collect" | "to_vec" => Value::List(items),
        "len" | "count" => Value::Int(items.len() as i64),
        "is_empty" => Value::Bool(items.is_empty()),
        "first" | "next" => some(items.into_iter().next()),
        "last" => some(items.into_iter().last()),
        "get" | "nth" => some(items.into_iter().nth(count(0))),
        "contains" => Value::Bool(items.contains(&arg(0))),
        "sum" | "product" => {
            let product = name == "product";
            let mut total = if product { Value::Int(1) } else { Value::Int(0) };
            for item in items {
                total = arithmetic(if product { '*' } else { '+' }, &total, &item.read())
                    .map_err(|e| Flow::Error(LiveError::at(span, e)))?;
            }
            total
        }
        "min" | "max" => {
            let mut best: Option<Value> = None;
            for item in items {
                let better = match &best {
                    None => true,
                    Some(current) => compare(&item, current)
                        .is_some_and(|o| if name == "min" { o.is_lt() } else { o.is_gt() }),
                };
                if better {
                    best = Some(item);
                }
            }
            some(best)
        }
        "any" | "all" => {
            let f = closure(0)?;
            let any = name == "any";
            let mut answer = !any;
            for item in items {
                if truthy(call(&f, vec![item])?) == any {
                    answer = any;
                    break;
                }
            }
            Value::Bool(answer)
        }
        "find" | "position" => {
            let f = closure(0)?;
            let mut found = None;
            for (i, item) in items.into_iter().enumerate() {
                if truthy(call(&f, vec![item.clone()])?) {
                    found = Some(if name == "find" { item } else { Value::Int(i as i64) });
                    break;
                }
            }
            some(found)
        }
        "fold" => {
            let f = closure(1)?;
            let mut acc = arg(0);
            for item in items {
                acc = call(&f, vec![acc, item])?;
            }
            acc
        }
        "join" | "concat" => {
            let separator = if name == "join" { display(&arg(0)) } else { String::new() };
            Value::Str(items.iter().map(display).collect::<Vec<_>>().join(&separator))
        }
        "unwrap_or_default" => some(items.into_iter().next()),
        _ => return fail(span, format!("a list has no method `{name}` the playground knows")),
    })
}
