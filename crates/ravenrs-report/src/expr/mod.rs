//! The expression language, close to DevExpress Criteria:
//!
//! * Fields and parameters: `[Price] * [Quantity]`, `[Customer.Name]`, `[^.Number]` (master row), `?MinTotal`.
//! * Literals: `12.5`, `'text'` (`''` escapes a quote), `#2026-11-01#`, `True`, `False`, `Null`.
//! * Operators:
//!   * Arithmetic: `+ - * / %`; `+` joins text.
//!   * Comparison: `= <> != < <= > >=`.
//!   * Logic: `And Or Not` (or `&& || !`).
//!   * Tests: `In (a, b)`, `Between(a, b)`, `Is Null`, `Is Not Null`.
//! * Functions:
//!   * Logic: `Iif(cond, a, cond2, b, ..., else)`, `IsNull(x)`, `IsNull(x, fallback)`, `IsNullOrEmpty`.
//!   * Text: `Concat`, `Upper`, `Lower`, `Trim`, `Len`, `Substring`, `Replace`, `Contains`,
//!     `StartsWith`, `EndsWith`, `PadLeft`, `PadRight`, `ToStr`, `ToNumber`.
//!   * Math: `Abs`, `Round(x, digits)`, `Floor`, `Ceiling`, `Power`, `Sqrt`, `Min(a, b)`, `Max(a, b)`.
//!   * Dates: `Today`, `Now`, `AddDays`, `AddMonths`, `AddYears`, `GetYear`, `GetMonth`,
//!     `GetDay`, `GetDayOfWeek`, `DateDiffDay`.
//!   * Formatting and pages: `FormatString('{0:C2}', x)`, and `PageNumber()` and
//!     `PageCount()` in page headers and footers.
//! * Aggregates over the current scope (group, page or report): `Sum([Total])`,
//!   `Count()`, `Avg`, `Min`, `Max`, `DistinctCount`, `First`, `Last`, `RunningSum`.
//! * Aggregates over a nested collection: `[Items][].Sum([Qty] * [Price])`, `[Items][].Count()`.

mod eval;
mod parse;

pub(crate) use eval::{Ctx, Env, lookup};
pub use parse::parse;

use crate::value::Value;

/// A parsed expression.
#[derive(Clone, Debug, PartialEq)]
pub struct Expr(pub(crate) Node);

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Node {
    Lit(Value),
    /// A field path; a leading `^` segment reads the master row.
    Field(Vec<String>),
    Param(String),
    Neg(Box<Node>),
    Not(Box<Node>),
    Bin(BinOp, Box<Node>, Box<Node>),
    And(Box<Node>, Box<Node>),
    Or(Box<Node>, Box<Node>),
    In(Box<Node>, Vec<Node>),
    Between(Box<Node>, Box<Node>, Box<Node>),
    IsNull(Box<Node>, bool),
    Call(Func, Vec<Node>),
    /// An aggregate over the current scope; the argument is evaluated per row.
    Agg(Agg, Option<Box<Node>>),
    /// An aggregate over the collection at a field path of the current row.
    Collection(Vec<String>, Agg, Option<Box<Node>>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BinOp {
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
}

/// Aggregate functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Agg {
    Sum,
    Count,
    Avg,
    Min,
    Max,
    DistinctCount,
    First,
    Last,
    RunningSum,
}

impl Agg {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "sum" => Self::Sum,
            "count" => Self::Count,
            "avg" => Self::Avg,
            "min" => Self::Min,
            "max" => Self::Max,
            "distinctcount" => Self::DistinctCount,
            "first" => Self::First,
            "last" => Self::Last,
            "runningsum" => Self::RunningSum,
            _ => return None,
        })
    }
}

/// Scalar functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Func {
    Iif,
    IsNull,
    IsNullOrEmpty,
    Concat,
    Upper,
    Lower,
    Trim,
    Len,
    Substring,
    Replace,
    Contains,
    StartsWith,
    EndsWith,
    PadLeft,
    PadRight,
    ToStr,
    ToNumber,
    Abs,
    Round,
    Floor,
    Ceiling,
    Power,
    Sqrt,
    Min2,
    Max2,
    Today,
    Now,
    AddDays,
    AddMonths,
    AddYears,
    GetYear,
    GetMonth,
    GetDay,
    GetDayOfWeek,
    DateDiffDay,
    FormatString,
    PageNumber,
    PageCount,
}

impl Func {
    /// The function for a lowercase name and argument count, with its arity check.
    fn resolve(name: &str, args: usize) -> Result<Self, String> {
        let (f, ok) = match name {
            "iif" => (Self::Iif, args >= 3 && args % 2 == 1),
            "isnull" => (Self::IsNull, (1..=2).contains(&args)),
            "isnullorempty" => (Self::IsNullOrEmpty, args == 1),
            "concat" => (Self::Concat, args >= 1),
            "upper" => (Self::Upper, args == 1),
            "lower" => (Self::Lower, args == 1),
            "trim" => (Self::Trim, args == 1),
            "len" => (Self::Len, args == 1),
            "substring" => (Self::Substring, (2..=3).contains(&args)),
            "replace" => (Self::Replace, args == 3),
            "contains" => (Self::Contains, args == 2),
            "startswith" => (Self::StartsWith, args == 2),
            "endswith" => (Self::EndsWith, args == 2),
            "padleft" => (Self::PadLeft, (2..=3).contains(&args)),
            "padright" => (Self::PadRight, (2..=3).contains(&args)),
            "tostr" | "tostring" => (Self::ToStr, args == 1),
            "tonumber" | "todecimal" | "todouble" => (Self::ToNumber, args == 1),
            "abs" => (Self::Abs, args == 1),
            "round" => (Self::Round, (1..=2).contains(&args)),
            "floor" => (Self::Floor, args == 1),
            "ceiling" => (Self::Ceiling, args == 1),
            "power" => (Self::Power, args == 2),
            "sqrt" => (Self::Sqrt, args == 1),
            "min" => (Self::Min2, args == 2),
            "max" => (Self::Max2, args == 2),
            "today" => (Self::Today, args == 0),
            "now" => (Self::Now, args == 0),
            "adddays" => (Self::AddDays, args == 2),
            "addmonths" => (Self::AddMonths, args == 2),
            "addyears" => (Self::AddYears, args == 2),
            "getyear" => (Self::GetYear, args == 1),
            "getmonth" => (Self::GetMonth, args == 1),
            "getday" => (Self::GetDay, args == 1),
            "getdayofweek" => (Self::GetDayOfWeek, args == 1),
            "datediffday" => (Self::DateDiffDay, args == 2),
            "formatstring" => (Self::FormatString, args >= 1),
            "pagenumber" => (Self::PageNumber, args == 0),
            "pagecount" => (Self::PageCount, args == 0),
            _ => return Err(format!("unknown function '{name}'")),
        };
        if ok { Ok(f) } else { Err(format!("wrong number of arguments ({args}) for '{name}'")) }
    }
}

impl Expr {
    /// Parses `source`; see the module documentation for the syntax.
    pub fn parse(source: &str) -> Result<Self, ExprError> {
        parse(source)
    }

    /// True when the expression uses `PageNumber()` or `PageCount()`.
    pub fn uses_page(&self) -> bool {
        fn walk(n: &Node) -> bool {
            match n {
                Node::Call(Func::PageNumber | Func::PageCount, _) => true,
                Node::Call(_, args) | Node::In(_, args) => args.iter().any(walk),
                Node::Neg(a) | Node::Not(a) | Node::IsNull(a, _) => walk(a),
                Node::Bin(_, a, b) | Node::And(a, b) | Node::Or(a, b) => walk(a) || walk(b),
                Node::Between(a, b, c) => walk(a) || walk(b) || walk(c),
                Node::Agg(_, a) | Node::Collection(_, _, a) => a.as_deref().is_some_and(walk),
                _ => false,
            }
        }
        walk(&self.0)
    }
}

/// A syntax error in an expression, with the byte offset where it was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExprError {
    /// What is wrong.
    pub message: String,
    /// Byte offset in the source.
    pub offset: usize,
}

impl std::fmt::Display for ExprError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (at {})", self.message, self.offset)
    }
}

impl std::error::Error for ExprError {}
