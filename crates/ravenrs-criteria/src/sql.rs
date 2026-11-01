//! Translates criteria to SQL for the database to run, as object-relational
//! mappers do. Values always travel as bound parameters and identifiers are
//! always quoted, so data can never change the shape of the query. What SQL
//! cannot express comes back as [`SqlError`]; evaluate that part in memory.

use crate::ast::{BinaryOp, Criteria, GroupOp, UnaryOp};
use crate::functions::{Aggregate, Function};
use crate::value::Value;

/// A database's SQL flavor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Dialect {
    /// PostgreSQL: `"name"`, `$1`.
    Postgres,
    /// MySQL and MariaDB: `` `name` ``, `?`.
    MySql,
    /// Microsoft SQL Server: `[name]`, `@p1`.
    SqlServer,
    /// SQLite: `"name"`, `?1`.
    Sqlite,
}

/// A bound parameter of a translated query.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SqlParam {
    /// A literal from the expression.
    Value(Value),
    /// A criteria parameter (`?Name`) for the caller to supply.
    Named(String),
}

/// Translated SQL: text with placeholders, and what to bind to them, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct SqlQuery {
    /// The SQL text.
    pub text: String,
    /// The values for the placeholders, in order.
    pub params: Vec<SqlParam>,
}

/// Why something cannot run in the database.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SqlError {
    /// This function or construct has no SQL translation here.
    Unsupported(String),
}

impl std::fmt::Display for SqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "{what} cannot be translated to SQL"),
        }
    }
}

impl std::error::Error for SqlError {}

/// Translates `criteria` to a SQL expression for a `WHERE` or a `SELECT` list.
///
/// ```
/// use ravenrs_criteria::{Criteria, Dialect, SqlParam, Value, to_sql};
///
/// let c = Criteria::parse("[Total] > 100 And [Name] Like ?Pattern").expect("valid");
/// let q = to_sql(&c, Dialect::Postgres).expect("translatable");
/// assert_eq!(q.text, r#""Total" > $1 AND "Name" LIKE $2"#);
/// assert_eq!(q.params, [SqlParam::Value(Value::Number(100.0)), SqlParam::Named("Pattern".into())]);
/// ```
pub fn to_sql(criteria: &Criteria, dialect: Dialect) -> Result<SqlQuery, SqlError> {
    let mut w = Writer { dialect, params: Vec::new() };
    let text = w.node(criteria)?;
    Ok(SqlQuery { text, params: w.params })
}

struct Writer {
    dialect: Dialect,
    params: Vec<SqlParam>,
}

impl Writer {
    fn bind(&mut self, p: SqlParam) -> String {
        self.params.push(p);
        let n = self.params.len();
        match self.dialect {
            Dialect::Postgres => format!("${n}"),
            Dialect::MySql => "?".to_owned(),
            Dialect::SqlServer => format!("@p{n}"),
            Dialect::Sqlite => format!("?{n}"),
        }
    }

    fn ident(&self, name: &str) -> String {
        match self.dialect {
            Dialect::Postgres | Dialect::Sqlite => format!("\"{}\"", name.replace('"', "\"\"")),
            Dialect::MySql => format!("`{}`", name.replace('`', "``")),
            Dialect::SqlServer => format!("[{}]", name.replace(']', "]]")),
        }
    }

    /// A child, in parentheses unless it is a simple term.
    fn term(&mut self, c: &Criteria) -> Result<String, SqlError> {
        let s = self.node(c)?;
        Ok(match c {
            Criteria::Value(_)
            | Criteria::Property(_)
            | Criteria::Parameter(_)
            | Criteria::Function(..)
            | Criteria::Aggregate(_) => s,
            _ => format!("({s})"),
        })
    }

    fn node(&mut self, c: &Criteria) -> Result<String, SqlError> {
        Ok(match c {
            Criteria::Value(Value::Null) => "NULL".to_owned(),
            Criteria::Value(v) => self.bind(SqlParam::Value(v.clone())),
            Criteria::Parameter(p) => self.bind(SqlParam::Named(p.clone())),
            Criteria::Property(path) => {
                if path.first().is_some_and(|p| p == "^") {
                    return Err(SqlError::Unsupported("a parent row reference".into()));
                }
                path.iter().map(|p| self.ident(p)).collect::<Vec<_>>().join(".")
            }
            Criteria::Unary(UnaryOp::Not, a) => format!("NOT {}", self.term(a)?),
            Criteria::Unary(UnaryOp::Minus, a) => format!("-{}", self.term(a)?),
            Criteria::Unary(UnaryOp::BitNot, a) => format!("~{}", self.term(a)?),
            Criteria::Binary(BinaryOp::Eq, a, b) if **b == Criteria::Value(Value::Null) => {
                format!("{} IS NULL", self.term(a)?)
            }
            Criteria::Binary(BinaryOp::Ne, a, b) if **b == Criteria::Value(Value::Null) => {
                format!("{} IS NOT NULL", self.term(a)?)
            }
            Criteria::Binary(op, a, b) => {
                let symbol = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Rem => "%",
                    BinaryOp::Eq => "=",
                    BinaryOp::Ne => "<>",
                    BinaryOp::Lt => "<",
                    BinaryOp::Le => "<=",
                    BinaryOp::Gt => ">",
                    BinaryOp::Ge => ">=",
                    BinaryOp::Like => "LIKE",
                    BinaryOp::BitAnd => "&",
                    BinaryOp::BitOr => "|",
                    BinaryOp::BitXor => "^",
                };
                // `+` in criteria also joins text, which SQL cannot tell from the types here.
                format!("{} {symbol} {}", self.term(a)?, self.term(b)?)
            }
            Criteria::Group(op, items) => {
                let word = if *op == GroupOp::And { " AND " } else { " OR " };
                let parts = items
                    .iter()
                    .map(|i| match i {
                        // Comparisons bind tighter than AND and OR.
                        Criteria::Binary(
                            BinaryOp::Eq
                            | BinaryOp::Ne
                            | BinaryOp::Lt
                            | BinaryOp::Le
                            | BinaryOp::Gt
                            | BinaryOp::Ge
                            | BinaryOp::Like,
                            ..,
                        )
                        | Criteria::In(..)
                        | Criteria::Between(..)
                        | Criteria::IsNull(_) => self.node(i),
                        other => self.term(other),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                parts.join(word)
            }
            Criteria::In(a, items) => {
                let value = self.term(a)?;
                let list = items.iter().map(|i| self.node(i)).collect::<Result<Vec<_>, _>>()?;
                format!("{value} IN ({})", list.join(", "))
            }
            Criteria::Between(a, lo, hi) => {
                let (a, lo, hi) = (self.term(a)?, self.term(lo)?, self.term(hi)?);
                format!("{a} BETWEEN {lo} AND {hi}")
            }
            Criteria::IsNull(a) => format!("{} IS NULL", self.term(a)?),
            Criteria::Function(f, args) => self.function(*f, args)?,
            Criteria::Aggregate(call) => {
                if call.collection.is_some() || call.condition.is_some() {
                    return Err(SqlError::Unsupported("an aggregate over a nested collection".into()));
                }
                let name = match call.function {
                    Aggregate::Sum => "SUM",
                    Aggregate::Count => "COUNT",
                    Aggregate::Avg => "AVG",
                    Aggregate::Min => "MIN",
                    Aggregate::Max => "MAX",
                    other => return Err(SqlError::Unsupported(format!("the aggregate {}", other.name()))),
                };
                match call.args.first() {
                    Some(a) => format!("{name}({})", self.node(a)?),
                    None => format!("{name}(*)"),
                }
            }
            Criteria::Custom(name, _) => {
                return Err(SqlError::Unsupported(format!("the custom function {name}")));
            }
        })
    }

    fn call(&mut self, name: &str, args: &[Criteria]) -> Result<String, SqlError> {
        let list = args.iter().map(|a| self.node(a)).collect::<Result<Vec<_>, _>>()?;
        Ok(format!("{name}({})", list.join(", ")))
    }

    /// `text LIKE pattern`, with the part bound as data and its wildcards escaped.
    fn like(&mut self, args: &[Criteria], before: bool, after: bool) -> Result<String, SqlError> {
        let text = self.term(&args[0])?;
        let Criteria::Value(Value::Text(part)) = &args[1] else {
            return Err(SqlError::Unsupported("a text search with a computed pattern".into()));
        };
        let escaped = part.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
        let pattern = format!("{}{escaped}{}", if before { "%" } else { "" }, if after { "%" } else { "" });
        let bound = self.bind(SqlParam::Value(Value::Text(pattern)));
        Ok(format!("{text} LIKE {bound} ESCAPE '\\'"))
    }

    fn date_part(&mut self, part: &str, arg: &Criteria) -> Result<String, SqlError> {
        let v = self.node(arg)?;
        Ok(match self.dialect {
            Dialect::Postgres => format!("EXTRACT({part} FROM {v})"),
            Dialect::MySql => format!("{part}({v})"),
            Dialect::SqlServer => format!("DATEPART({part}, {v})"),
            Dialect::Sqlite => {
                let code = match part {
                    "YEAR" => "%Y",
                    "MONTH" => "%m",
                    _ => "%d",
                };
                format!("CAST(strftime('{code}', {v}) AS INTEGER)")
            }
        })
    }

    fn function(&mut self, f: Function, args: &[Criteria]) -> Result<String, SqlError> {
        use Function as F;
        Ok(match f {
            F::Upper => self.call("UPPER", args)?,
            F::Lower => self.call("LOWER", args)?,
            F::Trim => self.call("TRIM", args)?,
            F::Abs => self.call("ABS", args)?,
            F::Round => self.call("ROUND", args)?,
            F::Floor => self.call("FLOOR", args)?,
            F::Ceiling => self.call("CEILING", args)?,
            F::Power => self.call("POWER", args)?,
            F::Sqrt | F::Sqr => self.call("SQRT", args)?,
            F::Replace => self.call("REPLACE", args)?,
            F::Concat => match self.dialect {
                Dialect::Sqlite => {
                    let parts = args.iter().map(|a| self.term(a)).collect::<Result<Vec<_>, _>>()?;
                    parts.join(" || ")
                }
                _ => self.call("CONCAT", args)?,
            },
            F::Len => match self.dialect {
                Dialect::SqlServer => self.call("LEN", args)?,
                Dialect::MySql => self.call("CHAR_LENGTH", args)?,
                _ => self.call("LENGTH", args)?,
            },
            F::IsNull if args.len() == 2 => self.call("COALESCE", args)?,
            F::IsNull => format!("{} IS NULL", self.term(&args[0])?),
            F::IsNullOrEmpty => {
                let a = self.term(&args[0])?;
                format!("({a} IS NULL OR {a} = '')")
            }
            F::Iif => {
                let mut s = String::from("CASE");
                let mut i = 0;
                while i + 1 < args.len() {
                    let (cond, value) = (self.node(&args[i])?, self.node(&args[i + 1])?);
                    s.push_str(&format!(" WHEN {cond} THEN {value}"));
                    i += 2;
                }
                let last = self.node(&args[args.len() - 1])?;
                format!("{s} ELSE {last} END")
            }
            F::Contains => self.like(args, true, true)?,
            F::StartsWith => self.like(args, false, true)?,
            F::EndsWith => self.like(args, true, false)?,
            F::GetYear => self.date_part("YEAR", &args[0])?,
            F::GetMonth => self.date_part("MONTH", &args[0])?,
            F::GetDay => self.date_part("DAY", &args[0])?,
            other => return Err(SqlError::Unsupported(format!("the function {}", other.name()))),
        })
    }
}
