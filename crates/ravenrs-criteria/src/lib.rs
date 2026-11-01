//! A criteria and expression language, independent of any UI: the same
//! expressions filter a grid, compute a report total, guard a security rule or
//! become a SQL `WHERE`.
//!
//! * [`Criteria`] is a public tree. Parse it from text, build it in code, walk
//!   it, and print it back to text that parses to the same tree.
//! * [`Scope`] evaluates it in memory over any [`Record`]: JSON, a map, or your
//!   own rows.
//! * [`validate`] checks it against a [`Schema`] before it runs.
//! * [`to_sql`] translates it to parameterized SQL.
//! * [`functions::catalog`] lists every function, for expression editors.
//!
//! # Syntax
//!
//! * Fields and parameters: `[Price]`, `[Customer.Name]`, `[^.Number]` (parent row), `?MinTotal`.
//! * Literals: `12.5`, `'text'` (`''` escapes a quote), `#2026-11-01#`, `True`, `False`, `Null`, `{guid}`.
//! * Operators:
//!   * Arithmetic: `+ - * / %`; `+` joins text.
//!   * Comparison: `= <> != < <= > >=`, `Like '%a_'`.
//!   * Logic: `And Or Not` (or `&& || !`).
//!   * Bits: `& | ^ ~`.
//!   * Tests: `In (a, b)`, `Between(a, b)`, `Is Null`, `Is Not Null`, `Not In (...)`.
//! * Functions: `Iif(...)`, `Upper(...)`, `AddMonths(...)` and others, in any case.
//! * Aggregates over the rows in scope: `Sum([Total])`, `Count()`, also `sumSum(...)`.
//! * Aggregates over a nested collection, optionally filtered:
//!   `[Items][].Sum([Qty] * [Price])`, `[Items][[Qty] > 0].Count()`.
//! * Comments: `/* like this */`.
//!
//! ```
//! use ravenrs_criteria::{Criteria, Environment, Scope, Value};
//! use serde_json::json;
//!
//! let order = json!({ "Status": "open", "Items": [{ "Qty": 2, "Price": 5.0 }, { "Qty": 0, "Price": 9.0 }] });
//! let total = Criteria::parse("[Items][[Qty] > 0].Sum([Qty] * [Price])").expect("valid");
//! let env = Environment::new();
//! assert_eq!(Scope::new(&env).row(&order).evaluate(&total), Ok(Value::Number(10.0)));
//!
//! let filter = Criteria::parse("[Status] = ?Wanted").expect("valid");
//! let env = Environment::new().parameter("Wanted", "open");
//! assert!(Scope::new(&env).row(&order).matches(&filter));
//! ```

mod ast;
mod eval;
mod format;
pub mod functions;
mod parse;
mod sql;
mod validate;
mod value;

pub use ast::{AggregateCall, BinaryOp, Criteria, GroupOp, UnaryOp};
pub use eval::{CustomFunction, Environment, EvalError, Record, Scope};
pub use format::{Culture, NumberStyle, format_composite, format_value};
pub use functions::{Aggregate, Category, Function, FunctionInfo};
pub use parse::ParseError;
pub use sql::{Dialect, SqlError, SqlParam, SqlQuery, to_sql};
pub use validate::{FieldType, Problem, Schema, validate};
pub use value::{DateTime, Value};
