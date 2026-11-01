//! The criteria tree. It is public on purpose: programs build, inspect and
//! rewrite criteria (security rules, saved filters), and printing a tree gives
//! text that parses back to the same tree.

use std::fmt;

use crate::functions::{Aggregate, Function};
use crate::value::Value;

/// A parsed expression or filter.
///
/// ```
/// use ravenrs_criteria::Criteria;
///
/// let c = Criteria::parse("[Price] * [Qty] > 100 And [Active]").expect("valid");
/// assert_eq!(c.to_string(), "[Price] * [Qty] > 100 And [Active]");
/// assert_eq!(c.properties(), ["Price", "Qty", "Active"]);
/// ```
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Criteria {
    /// A literal.
    Value(Value),
    /// A field path, such as `Customer.Name`; a leading `^` reads the parent row.
    Property(Vec<String>),
    /// A parameter, `?Name`.
    Parameter(String),
    /// A unary operator.
    Unary(UnaryOp, Box<Criteria>),
    /// A binary operator.
    Binary(BinaryOp, Box<Criteria>, Box<Criteria>),
    /// `And` or `Or` over two or more operands.
    Group(GroupOp, Vec<Criteria>),
    /// `value In (a, b, c)`.
    In(Box<Criteria>, Vec<Criteria>),
    /// `value Between(low, high)`.
    Between(Box<Criteria>, Box<Criteria>, Box<Criteria>),
    /// `value Is Null`.
    IsNull(Box<Criteria>),
    /// A built-in function.
    Function(Function, Vec<Criteria>),
    /// A function resolved by name when evaluated (see [`crate::Environment::register`]).
    Custom(String, Vec<Criteria>),
    /// An aggregate over the current scope, or over a nested collection.
    Aggregate(Box<AggregateCall>),
}

/// An aggregate call.
#[derive(Clone, Debug, PartialEq)]
pub struct AggregateCall {
    /// The nested collection, or `None` for the rows in scope.
    pub collection: Option<Vec<String>>,
    /// `[Items][[Qty] > 0]`: only rows where this holds.
    pub condition: Option<Criteria>,
    /// The function.
    pub function: Aggregate,
    /// Its arguments, evaluated per row.
    pub args: Vec<Criteria>,
}

/// Unary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnaryOp {
    /// Logical `Not`.
    Not,
    /// Arithmetic `-`.
    Minus,
    /// Bitwise `~`.
    BitNot,
}

/// Binary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[allow(missing_docs, reason = "the operators speak for themselves")]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// `Like`, with `%` and `_` wildcards.
    Like,
    BitAnd,
    BitOr,
    BitXor,
}

/// `And` or `Or`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroupOp {
    /// All operands hold.
    And,
    /// Any operand holds.
    Or,
}

impl BinaryOp {
    fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Rem => "%",
            Self::Eq => "=",
            Self::Ne => "<>",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Like => "Like",
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
        }
    }

    fn precedence(self) -> u8 {
        match self {
            Self::BitOr => 4,
            Self::BitXor => 5,
            Self::BitAnd => 6,
            Self::Eq | Self::Ne | Self::Lt | Self::Le | Self::Gt | Self::Ge | Self::Like => 7,
            Self::Add | Self::Sub => 8,
            Self::Mul | Self::Div | Self::Rem => 9,
        }
    }
}

impl std::ops::Not for Criteria {
    type Output = Self;

    /// `Not self`.
    fn not(self) -> Self {
        Self::Unary(UnaryOp::Not, Box::new(self))
    }
}

impl Criteria {
    /// Parses `source`; see the crate documentation for the syntax.
    pub fn parse(source: &str) -> Result<Self, crate::ParseError> {
        crate::parse::parse(source)
    }

    /// A field.
    pub fn property(path: &str) -> Self {
        Self::Property(path.split('.').map(str::to_owned).collect())
    }

    /// A literal.
    pub fn value(value: impl Into<Value>) -> Self {
        Self::Value(value.into())
    }

    /// `self = other`.
    pub fn equals(self, other: Self) -> Self {
        Self::Binary(BinaryOp::Eq, Box::new(self), Box::new(other))
    }

    /// `self And other`, flattening nested `And`s.
    pub fn and(self, other: Self) -> Self {
        Self::group(GroupOp::And, self, other)
    }

    /// `self Or other`, flattening nested `Or`s.
    pub fn or(self, other: Self) -> Self {
        Self::group(GroupOp::Or, self, other)
    }

    fn group(op: GroupOp, a: Self, b: Self) -> Self {
        let mut items = Vec::new();
        for c in [a, b] {
            match c {
                Self::Group(o, inner) if o == op => items.extend(inner),
                other => items.push(other),
            }
        }
        Self::Group(op, items)
    }

    /// Calls `f` on this node and everything under it.
    pub fn walk(&self, f: &mut dyn FnMut(&Criteria)) {
        f(self);
        match self {
            Self::Value(_) | Self::Property(_) | Self::Parameter(_) => {}
            Self::Unary(_, a) | Self::IsNull(a) => a.walk(f),
            Self::Binary(_, a, b) => {
                a.walk(f);
                b.walk(f);
            }
            Self::Group(_, items) | Self::Function(_, items) | Self::Custom(_, items) => {
                items.iter().for_each(|c| c.walk(f));
            }
            Self::In(a, items) => {
                a.walk(f);
                items.iter().for_each(|c| c.walk(f));
            }
            Self::Between(a, b, c) => {
                a.walk(f);
                b.walk(f);
                c.walk(f);
            }
            Self::Aggregate(call) => {
                if let Some(c) = &call.condition {
                    c.walk(f);
                }
                call.args.iter().for_each(|c| c.walk(f));
            }
        }
    }

    /// The field paths used, in order, without repeats.
    pub fn properties(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.walk(&mut |c| {
            if let Self::Property(p) = c {
                let path = p.join(".");
                if !out.contains(&path) {
                    out.push(path);
                }
            }
        });
        out
    }

    /// The parameter names used, in order, without repeats.
    pub fn parameters(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.walk(&mut |c| {
            if let Self::Parameter(p) = c
                && !out.contains(p)
            {
                out.push(p.clone());
            }
        });
        out
    }

    /// True when any node satisfies `test`.
    pub fn any(&self, test: &dyn Fn(&Criteria) -> bool) -> bool {
        let mut found = false;
        self.walk(&mut |c| found |= test(c));
        found
    }

    /// Binding strength when printed: higher binds tighter.
    fn precedence(&self) -> u8 {
        match self {
            Self::Group(GroupOp::Or, _) => 1,
            Self::Group(GroupOp::And, _) => 2,
            Self::Unary(UnaryOp::Not, _) => 3,
            Self::Binary(op, ..) => op.precedence(),
            Self::In(..) | Self::Between(..) | Self::IsNull(_) => 7,
            Self::Unary(..) => 10,
            _ => 11,
        }
    }

    fn write_child(&self, f: &mut fmt::Formatter<'_>, min: u8) -> fmt::Result {
        if self.precedence() < min { write!(f, "({self})") } else { write!(f, "{self}") }
    }
}

fn write_list(f: &mut fmt::Formatter<'_>, items: &[Criteria]) -> fmt::Result {
    for (i, c) in items.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{c}")?;
    }
    Ok(())
}

impl fmt::Display for Criteria {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(v) => match v {
                Value::Null => f.write_str("Null"),
                Value::Bool(true) => f.write_str("True"),
                Value::Bool(false) => f.write_str("False"),
                Value::Number(n) if *n < 0.0 => write!(f, "({})", crate::value::format_plain(*n)),
                Value::Number(n) => f.write_str(&crate::value::format_plain(*n)),
                Value::Text(s) => write!(f, "'{}'", s.replace('\'', "''")),
                Value::Date(d) => write!(f, "#{d}#"),
            },
            Self::Property(p) => write!(f, "[{}]", p.join(".")),
            Self::Parameter(p) => write!(f, "?{p}"),
            Self::Unary(UnaryOp::Not, a) => {
                f.write_str("Not ")?;
                a.write_child(f, 3)
            }
            Self::Unary(UnaryOp::Minus, a) => {
                f.write_str("-")?;
                a.write_child(f, 10)
            }
            Self::Unary(UnaryOp::BitNot, a) => {
                f.write_str("~")?;
                a.write_child(f, 10)
            }
            Self::Binary(op, a, b) => {
                let p = op.precedence();
                a.write_child(f, p)?;
                write!(f, " {} ", op.symbol())?;
                // Operators associate left, so a right operand of equal strength needs parentheses.
                b.write_child(f, p + 1)
            }
            Self::Group(op, items) => {
                let (word, p) = if *op == GroupOp::And { (" And ", 3) } else { (" Or ", 2) };
                for (i, c) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_str(word)?;
                    }
                    c.write_child(f, p)?;
                }
                Ok(())
            }
            Self::In(a, items) => {
                a.write_child(f, 8)?;
                f.write_str(" In (")?;
                write_list(f, items)?;
                f.write_str(")")
            }
            Self::Between(a, lo, hi) => {
                a.write_child(f, 8)?;
                write!(f, " Between({lo}, {hi})")
            }
            Self::IsNull(a) => {
                a.write_child(f, 8)?;
                f.write_str(" Is Null")
            }
            Self::Function(func, args) => {
                write!(f, "{}(", func.name())?;
                write_list(f, args)?;
                f.write_str(")")
            }
            Self::Custom(name, args) => {
                write!(f, "{name}(")?;
                write_list(f, args)?;
                f.write_str(")")
            }
            Self::Aggregate(call) => {
                if let Some(path) = &call.collection {
                    write!(f, "[{}]", path.join("."))?;
                    match &call.condition {
                        Some(c) => write!(f, "[{c}].")?,
                        None => f.write_str("[].")?,
                    }
                }
                write!(f, "{}(", call.function.name())?;
                write_list(f, &call.args)?;
                f.write_str(")")
            }
        }
    }
}
