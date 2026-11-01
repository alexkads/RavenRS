//! In-memory evaluation over any kind of row. This interprets the parsed tree
//! of the criteria language; it never runs host code, except the functions a
//! program registers on purpose.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::ast::{AggregateCall, BinaryOp, Criteria, GroupOp, UnaryOp};
use crate::format::{Culture, format_composite};
use crate::functions::{Aggregate, Function};
use crate::value::{DateTime, Value};

/// A row expressions can read: a JSON object, a grid row, an entity.
pub trait Record: Sized {
    /// The value at a field path; `None` when there is no such field.
    fn field(&self, path: &[String]) -> Option<Value>;

    /// The rows of a nested collection; `None` when there is no such collection.
    fn collection(&self, path: &[String]) -> Option<Vec<&Self>>;
}

#[cfg(feature = "json")]
fn json_at<'j>(base: &'j serde_json::Value, path: &[String]) -> Option<&'j serde_json::Value> {
    let mut cur = base;
    for seg in path {
        cur = match cur {
            serde_json::Value::Object(map) => map.get(seg)?,
            serde_json::Value::Array(items) => items.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

#[cfg(feature = "json")]
impl Record for serde_json::Value {
    fn field(&self, path: &[String]) -> Option<Value> {
        json_at(self, path).map(Value::from_json)
    }

    fn collection(&self, path: &[String]) -> Option<Vec<&Self>> {
        match json_at(self, path)? {
            serde_json::Value::Array(items) => Some(items.iter().collect()),
            _ => None,
        }
    }
}

impl Record for HashMap<String, Value> {
    fn field(&self, path: &[String]) -> Option<Value> {
        self.get(&path.join(".")).cloned()
    }

    fn collection(&self, _path: &[String]) -> Option<Vec<&Self>> {
        None
    }
}

/// A function a program adds to the language.
pub type CustomFunction = Rc<dyn Fn(&[Value]) -> Result<Value, String>>;

/// What every evaluation shares: parameters, calculated fields, custom
/// functions, the culture and the clock. Keep one per run; it is not thread-safe.
pub struct Environment {
    parameters: HashMap<String, Value>,
    calculated: HashMap<String, Criteria>,
    functions: HashMap<String, CustomFunction>,
    culture: Culture,
    now: DateTime,
    /// Running sums by (expression, scope): the last row index and the total so far.
    running: RefCell<HashMap<(usize, usize), (usize, f64)>>,
    depth: Cell<u32>,
    random: Cell<u64>,
    error: RefCell<Option<EvalError>>,
}

impl std::fmt::Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Environment").field("culture", &self.culture.name).finish_non_exhaustive()
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    /// An environment in US English, on the system clock.
    pub fn new() -> Self {
        let now = DateTime::now();
        Self {
            parameters: HashMap::new(),
            calculated: HashMap::new(),
            functions: HashMap::new(),
            culture: Culture::en_us(),
            now,
            running: RefCell::new(HashMap::new()),
            depth: Cell::new(0),
            random: Cell::new(
                (now.days as u64).wrapping_mul(86_400) ^ u64::from(now.seconds) ^ 0x9e37_79b9_7f4a_7c15,
            ),
            error: RefCell::new(None),
        }
    }

    /// Sets the culture for formats.
    pub fn culture(mut self, culture: Culture) -> Self {
        self.culture = culture;
        self
    }

    /// The culture in effect, for formatting outside expressions.
    pub fn formats(&self) -> &Culture {
        &self.culture
    }

    /// Fixes the clock (`Today()`, `Now()`), for tests and reproducible runs.
    pub fn at(mut self, now: DateTime) -> Self {
        self.now = now;
        self
    }

    /// Sets a parameter, read as `?name`.
    pub fn parameter(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.parameters.insert(name.into(), value.into());
        self
    }

    /// Adds a calculated field, read as `[name]`.
    pub fn calculated(mut self, name: impl Into<String>, expression: Criteria) -> Self {
        self.calculated.insert(name.into(), expression);
        self
    }

    /// Adds a function to the language; names are case-insensitive.
    ///
    /// ```
    /// use ravenrs_criteria::{Criteria, Environment, Scope, Value};
    ///
    /// let env = Environment::new().register("Double", |args| {
    ///     Ok(Value::Number(args[0].as_number().unwrap_or(0.0) * 2.0))
    /// });
    /// let c = Criteria::parse("double(21)").expect("valid");
    /// let row = serde_json::json!({});
    /// assert_eq!(Scope::new(&env).row(&row).evaluate(&c), Ok(Value::Number(42.0)));
    /// ```
    pub fn register(
        mut self,
        name: &str,
        function: impl Fn(&[Value]) -> Result<Value, String> + 'static,
    ) -> Self {
        self.functions.insert(name.to_lowercase(), Rc::new(function));
        self
    }

    /// Whether a custom function with this name is registered.
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(&name.to_lowercase())
    }

    fn fail(&self, error: EvalError) {
        self.error.borrow_mut().get_or_insert(error);
    }
}

/// A problem while evaluating. Type mismatches are not errors: they give
/// `Null`, as a missing value would, so a report still prints.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EvalError {
    /// A function that is neither built in nor registered.
    UnknownFunction(String),
    /// A registered function reported a failure.
    FunctionFailed {
        /// The function.
        name: String,
        /// What it reported.
        message: String,
    },
    /// Calculated fields that depend on each other in a cycle.
    Cycle(String),
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFunction(name) => write!(f, "unknown function '{name}'"),
            Self::FunctionFailed { name, message } => write!(f, "function '{name}' failed: {message}"),
            Self::Cycle(name) => write!(f, "calculated field '{name}' depends on itself"),
        }
    }
}

impl std::error::Error for EvalError {}

/// Where an expression is evaluated: the row, its parent, the rows aggregates
/// run over, and the page.
pub struct Scope<'a, R> {
    env: &'a Environment,
    row: Option<&'a R>,
    parent: Option<&'a R>,
    rows: &'a [&'a R],
    position: Option<usize>,
    page: Option<(usize, usize)>,
}

impl<R> std::fmt::Debug for Scope<'_, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scope")
            .field("rows", &self.rows.len())
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

impl<R> Clone for Scope<'_, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<R> Copy for Scope<'_, R> {}

impl<'a, R: Record> Scope<'a, R> {
    /// A scope with no row.
    pub fn new(env: &'a Environment) -> Self {
        Self { env, row: None, parent: None, rows: &[], position: None, page: None }
    }

    /// The current row.
    pub fn row(mut self, row: &'a R) -> Self {
        self.row = Some(row);
        self
    }

    /// Sets or clears the current row.
    pub fn maybe_row(mut self, row: Option<&'a R>) -> Self {
        self.row = row;
        self
    }

    /// The parent row, read as `[^.Field]`.
    pub fn parent(mut self, parent: Option<&'a R>) -> Self {
        self.parent = parent;
        self
    }

    /// The rows aggregates run over (a group, a page, a whole list).
    pub fn rows(mut self, rows: &'a [&'a R]) -> Self {
        self.rows = rows;
        self
    }

    /// The current row's index in [`Self::rows`], for running aggregates.
    pub fn position(mut self, position: Option<usize>) -> Self {
        self.position = position;
        self
    }

    /// The page number and page count, for `PageNumber()` and `PageCount()`.
    pub fn page(mut self, number: usize, count: usize) -> Self {
        self.page = Some((number, count));
        self
    }

    /// Evaluates `criteria`.
    pub fn evaluate(&self, criteria: &Criteria) -> Result<Value, EvalError> {
        let value = self.node(criteria);
        match self.env.error.borrow_mut().take() {
            Some(e) => Err(e),
            None => Ok(value),
        }
    }

    /// Evaluates a filter: true when it holds; errors count as false.
    pub fn matches(&self, criteria: &Criteria) -> bool {
        self.evaluate(criteria).is_ok_and(|v| v.is_true())
    }

    fn field(&self, path: &[String]) -> Value {
        let (base, path) = match path.split_first() {
            Some((first, rest)) if first == "^" => (self.parent, rest),
            _ => (self.row, path),
        };
        if path.len() == 1
            && let Some(calc) = self.env.calculated.get(&path[0])
        {
            if self.env.depth.get() > 32 {
                self.env.fail(EvalError::Cycle(path[0].clone()));
                return Value::Null;
            }
            self.env.depth.set(self.env.depth.get() + 1);
            let v = self.node(calc);
            self.env.depth.set(self.env.depth.get() - 1);
            return v;
        }
        base.and_then(|b| b.field(path)).unwrap_or_default()
    }

    fn node(&self, n: &Criteria) -> Value {
        match n {
            Criteria::Value(v) => v.clone(),
            Criteria::Property(path) => self.field(path),
            Criteria::Parameter(name) => self.env.parameters.get(name).cloned().unwrap_or_default(),
            Criteria::Unary(op, a) => {
                let v = self.node(a);
                match op {
                    UnaryOp::Not => Value::Bool(!v.is_true()),
                    UnaryOp::Minus => v.as_number().map_or(Value::Null, |x| Value::Number(-x)),
                    UnaryOp::BitNot => {
                        v.as_number().map_or(Value::Null, |x| Value::Number(!(x as i64) as f64))
                    }
                }
            }
            Criteria::Group(GroupOp::And, items) => Value::Bool(items.iter().all(|c| self.node(c).is_true())),
            Criteria::Group(GroupOp::Or, items) => Value::Bool(items.iter().any(|c| self.node(c).is_true())),
            Criteria::Binary(op, a, b) => binary(*op, self.node(a), self.node(b)),
            Criteria::In(a, items) => {
                let v = self.node(a);
                Value::Bool(items.iter().any(|i| v.loose_eq(&self.node(i))))
            }
            Criteria::Between(a, lo, hi) => {
                let v = self.node(a);
                if v.is_null() {
                    return Value::Bool(false);
                }
                Value::Bool(v.compare(&self.node(lo)).is_ge() && v.compare(&self.node(hi)).is_le())
            }
            Criteria::IsNull(a) => Value::Bool(self.node(a).is_null()),
            Criteria::Function(f, args) => self.call(*f, args),
            Criteria::Custom(name, args) => {
                let Some(function) = self.env.functions.get(&name.to_lowercase()).cloned() else {
                    self.env.fail(EvalError::UnknownFunction(name.clone()));
                    return Value::Null;
                };
                let values: Vec<Value> = args.iter().map(|a| self.node(a)).collect();
                function(&values).unwrap_or_else(|message| {
                    self.env.fail(EvalError::FunctionFailed { name: name.clone(), message });
                    Value::Null
                })
            }
            Criteria::Aggregate(call) => self.aggregate_call(call),
        }
    }

    fn aggregate_call(&self, call: &AggregateCall) -> Value {
        let key = std::ptr::from_ref(call) as usize;
        match &call.collection {
            None => {
                let filtered: Vec<&R>;
                let rows: &[&R] = match &call.condition {
                    Some(cond) => {
                        filtered = self
                            .rows
                            .iter()
                            .copied()
                            .filter(|r| self.at_row(r, None).node(cond).is_true())
                            .collect();
                        &filtered
                    }
                    None => self.rows,
                };
                self.aggregate(call, rows, key)
            }
            Some(path) => {
                let base = match path.split_first() {
                    Some((first, rest)) if first == "^" => self.parent.and_then(|p| p.collection(rest)),
                    _ => self.row.and_then(|r| r.collection(path)),
                };
                let Some(items) = base else {
                    return match call.function {
                        Aggregate::Count | Aggregate::DistinctCount => Value::Number(0.0),
                        Aggregate::Exists => Value::Bool(false),
                        _ => Value::Null,
                    };
                };
                // Inside the collection, `^` reads the row that owns it.
                let inner = Scope { parent: self.row, position: None, ..*self };
                let rows: Vec<&R> = match &call.condition {
                    Some(cond) => {
                        items.into_iter().filter(|r| inner.at_row(r, None).node(cond).is_true()).collect()
                    }
                    None => items,
                };
                // The rows live only for this call, so they stay out of the scope's lifetime.
                aggregate_over(self.env, call, &rows, self.row, None, self.page, key)
            }
        }
    }

    fn at_row(&self, row: &'a R, position: Option<usize>) -> Self {
        Scope { row: Some(row), position, ..*self }
    }

    fn aggregate(&self, call: &AggregateCall, rows: &[&R], key: usize) -> Value {
        aggregate_over(
            self.env,
            call,
            rows,
            self.parent,
            self.position.filter(|_| rows.len() == self.rows.len()),
            self.page,
            key,
        )
    }

    #[allow(clippy::too_many_lines, reason = "one arm per function is the clearest form")]
    fn call(&self, f: Function, args: &[Criteria]) -> Value {
        use Function as F;
        let a = |i: usize| args.get(i).map(|n| self.node(n)).unwrap_or_default();
        let text = |i: usize| a(i).to_text();
        let num = |i: usize| a(i).as_number();
        let date = |i: usize| a(i).as_date();
        let math = |g: fn(f64) -> f64| num(0).map_or(Value::Null, |x| Value::Number(g(x)));
        let part = |g: fn(DateTime) -> i64| date(0).map_or(Value::Null, |d| Value::Number(g(d) as f64));
        let add = |unit: i64| match (date(0), num(1)) {
            (Some(d), Some(n)) => Value::Date(add_seconds(d, n as i64 * unit)),
            _ => Value::Null,
        };
        let diff = |g: fn(DateTime, DateTime) -> i64| match (date(0), date(1)) {
            (Some(x), Some(y)) => Value::Number(g(x, y) as f64),
            _ => Value::Null,
        };
        let now = self.env.now;
        let today = DateTime { seconds: 0, ..now };
        let month_start = |d: DateTime| {
            let (y, m, _) = d.ymd();
            DateTime::from_ymd(y, m, 1).unwrap_or(d)
        };
        let year_start = |d: DateTime| DateTime::from_ymd(d.ymd().0, 1, 1).unwrap_or(d);
        let week_start = |d: DateTime| DateTime { seconds: 0, ..d.add_days(-i64::from(d.weekday())) };
        let month_is = |m: u32| Value::Bool(date(0).is_some_and(|d| d.ymd().1 == m));
        let in_days = |from: DateTime, to: DateTime| {
            Value::Bool(date(0).is_some_and(|d| d.days >= from.days && d.days < to.days))
        };
        match f {
            F::Iif => {
                let mut i = 0;
                while i + 1 < args.len() {
                    if a(i).is_true() {
                        return a(i + 1);
                    }
                    i += 2;
                }
                a(args.len() - 1)
            }
            F::IsNull if args.len() == 1 => Value::Bool(a(0).is_null()),
            F::IsNull => match a(0) {
                Value::Null => a(1),
                v => v,
            },
            F::IsNullOrEmpty => Value::Bool(match a(0) {
                Value::Null => true,
                Value::Text(s) => s.is_empty(),
                _ => false,
            }),
            F::Ascii => text(0).chars().next().map_or(Value::Null, |c| Value::Number(f64::from(c as u32))),
            F::Char => num(0)
                .and_then(|n| char::from_u32(n as u32))
                .map_or(Value::Null, |c| Value::Text(c.to_string())),
            F::CharIndex => {
                let (needle, hay) = (text(0), text(1));
                let start = num(2).unwrap_or(0.0).max(0.0) as usize;
                let skip: usize = hay.chars().take(start).map(char::len_utf8).sum();
                let found = hay[skip..].find(&needle).map(|b| start + hay[skip..skip + b].chars().count());
                Value::Number(found.map_or(-1.0, |i| i as f64))
            }
            F::Concat => Value::Text((0..args.len()).map(text).collect()),
            F::Contains => Value::Bool(text(0).contains(&text(1))),
            F::EndsWith => Value::Bool(text(0).ends_with(&text(1))),
            F::Insert => {
                let mut chars: Vec<char> = text(0).chars().collect();
                let at = (num(1).unwrap_or(0.0).max(0.0) as usize).min(chars.len());
                chars.splice(at..at, text(2).chars());
                Value::Text(chars.into_iter().collect())
            }
            F::Len => Value::Number(text(0).chars().count() as f64),
            F::Lower => Value::Text(text(0).to_lowercase()),
            F::NewLine => Value::Text("\n".into()),
            F::PadLeft | F::PadRight => {
                let s = text(0);
                let width = num(1).unwrap_or(0.0).max(0.0) as usize;
                let fill = text(2).chars().next().unwrap_or(' ');
                let pad: String =
                    std::iter::repeat_n(fill, width.saturating_sub(s.chars().count())).collect();
                Value::Text(if f == F::PadLeft { pad + &s } else { s + &pad })
            }
            F::Remove => {
                let chars: Vec<char> = text(0).chars().collect();
                let start = (num(1).unwrap_or(0.0).max(0.0) as usize).min(chars.len());
                let count = num(2).map_or(chars.len() - start, |n| n.max(0.0) as usize);
                let end = start.saturating_add(count).min(chars.len());
                Value::Text(chars[..start].iter().chain(&chars[end..]).collect())
            }
            F::Replace => Value::Text(text(0).replace(&text(1), &text(2))),
            F::Reverse => Value::Text(text(0).chars().rev().collect()),
            F::StartsWith => Value::Bool(text(0).starts_with(&text(1))),
            F::Substring => {
                let start = num(1).unwrap_or(0.0).max(0.0) as usize;
                let count = num(2).map_or(usize::MAX, |n| n.max(0.0) as usize);
                Value::Text(text(0).chars().skip(start).take(count).collect())
            }
            F::ToStr => Value::Text(text(0)),
            F::Trim => Value::Text(text(0).trim().to_owned()),
            F::Upper => Value::Text(text(0).to_uppercase()),
            F::Abs => math(f64::abs),
            F::Acos => math(f64::acos),
            F::Asin => math(f64::asin),
            F::Atn => math(f64::atan),
            F::Atn2 => match (num(0), num(1)) {
                (Some(y), Some(x)) => Value::Number(y.atan2(x)),
                _ => Value::Null,
            },
            F::BigMul => match (num(0), num(1)) {
                (Some(x), Some(y)) => Value::Number(x * y),
                _ => Value::Null,
            },
            F::Ceiling => math(f64::ceil),
            F::Cos => math(f64::cos),
            F::Cosh => math(f64::cosh),
            F::Exp => math(f64::exp),
            F::Floor => math(f64::floor),
            F::Log => match (num(0).filter(|x| *x > 0.0), num(1)) {
                (Some(x), Some(base)) => Value::Number(x.log(base)),
                (Some(x), None) => Value::Number(x.ln()),
                _ => Value::Null,
            },
            F::Log10 => num(0).filter(|x| *x > 0.0).map_or(Value::Null, |x| Value::Number(x.log10())),
            F::Max2 | F::Min2 => {
                let (x, y) = (a(0), a(1));
                let first = if f == F::Min2 { x.compare(&y).is_le() } else { x.compare(&y).is_ge() };
                if first { x } else { y }
            }
            F::Power => match (num(0), num(1)) {
                (Some(x), Some(y)) => Value::Number(x.powf(y)),
                _ => Value::Null,
            },
            F::Rnd => {
                let mut x = self.env.random.get();
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                self.env.random.set(x);
                Value::Number((x >> 11) as f64 / (1u64 << 53) as f64)
            }
            // Half away from zero, as accountants expect.
            F::Round => num(0).map_or(Value::Null, |x| {
                let p = 10f64.powi(num(1).unwrap_or(0.0) as i32);
                Value::Number((x * p).round() / p)
            }),
            F::Sign => num(0).map_or(Value::Null, |x| Value::Number(if x == 0.0 { 0.0 } else { x.signum() })),
            F::Sin => math(f64::sin),
            F::Sinh => math(f64::sinh),
            F::Sqr | F::Sqrt => num(0).filter(|x| *x >= 0.0).map_or(Value::Null, |x| Value::Number(x.sqrt())),
            F::Tan => math(f64::tan),
            F::Tanh => math(f64::tanh),
            F::ToDecimal | F::ToDouble | F::ToFloat | F::ToNumber => {
                num(0).map_or(Value::Null, Value::Number)
            }
            F::ToInt | F::ToLong => math(f64::trunc),
            F::AddDays => add(86_400),
            F::AddHours => add(3600),
            F::AddMinutes => add(60),
            F::AddSeconds => add(1),
            F::AddMonths | F::AddYears => match (date(0), num(1)) {
                (Some(d), Some(n)) => {
                    Value::Date(d.add_months(n as i64 * if f == F::AddYears { 12 } else { 1 }))
                }
                _ => Value::Null,
            },
            F::DateDiffDay => diff(|x, y| y.days - x.days),
            F::DateDiffHour => diff(|x, y| (total_seconds(y) - total_seconds(x)) / 3600),
            F::DateDiffMinute => diff(|x, y| (total_seconds(y) - total_seconds(x)) / 60),
            F::DateDiffSecond => diff(|x, y| total_seconds(y) - total_seconds(x)),
            F::DateDiffMonth => diff(|x, y| {
                let (xy, xm, _) = x.ymd();
                let (yy, ym, _) = y.ymd();
                (i64::from(yy) - i64::from(xy)) * 12 + i64::from(ym) - i64::from(xm)
            }),
            F::DateDiffYear => diff(|x, y| i64::from(y.ymd().0) - i64::from(x.ymd().0)),
            F::DateTimeFromParts => {
                let n = |i: usize, default: f64| num(i).unwrap_or(default);
                let d = DateTime::from_ymd(n(0, 0.0) as i32, n(1, 0.0) as u32, n(2, 0.0) as u32);
                d.map_or(Value::Null, |d| {
                    Value::Date(DateTime {
                        seconds: (n(3, 0.0) * 3600.0 + n(4, 0.0) * 60.0 + n(5, 0.0)) as u32,
                        ..d
                    })
                })
            }
            F::GetDate => date(0).map_or(Value::Null, |d| Value::Date(DateTime { seconds: 0, ..d })),
            F::GetDay => part(|d| i64::from(d.ymd().2)),
            F::GetDayOfWeek => part(|d| i64::from(d.weekday())),
            F::GetDayOfYear => {
                part(|d| d.days - DateTime::from_ymd(d.ymd().0, 1, 1).map_or(d.days, |s| s.days) + 1)
            }
            F::GetHour => part(|d| i64::from(d.hms().0)),
            F::GetMinute => part(|d| i64::from(d.hms().1)),
            F::GetMonth => part(|d| i64::from(d.ymd().1)),
            F::GetSecond => part(|d| i64::from(d.hms().2)),
            F::GetYear => part(|d| i64::from(d.ymd().0)),
            F::Now | F::UtcNow | F::LocalDateTimeNow => Value::Date(now),
            F::Today | F::LocalDateTimeToday => Value::Date(today),
            F::LocalDateTimeTomorrow => Value::Date(today.add_days(1)),
            F::LocalDateTimeYesterday => Value::Date(today.add_days(-1)),
            F::LocalDateTimeDayAfterTomorrow => Value::Date(today.add_days(2)),
            F::LocalDateTimeThisWeek => Value::Date(week_start(now)),
            F::LocalDateTimeLastWeek => Value::Date(week_start(now).add_days(-7)),
            F::LocalDateTimeNextWeek => Value::Date(week_start(now).add_days(7)),
            F::LocalDateTimeTwoWeeksAway => Value::Date(week_start(now).add_days(14)),
            F::LocalDateTimeThisMonth => Value::Date(month_start(now)),
            F::LocalDateTimeLastMonth => Value::Date(month_start(now).add_months(-1)),
            F::LocalDateTimeNextMonth => Value::Date(month_start(now).add_months(1)),
            F::LocalDateTimeTwoMonthsAway => Value::Date(month_start(now).add_months(2)),
            F::LocalDateTimeThisYear => Value::Date(year_start(now)),
            F::LocalDateTimeLastYear => Value::Date(year_start(now).add_months(-12)),
            F::LocalDateTimeNextYear => Value::Date(year_start(now).add_months(12)),
            F::LocalDateTimeTwoYearsAway => Value::Date(year_start(now).add_months(24)),
            F::LocalDateTimeYearBeforeToday => Value::Date(today.add_months(-12)),
            F::InDateRange => match (date(0), date(1), date(2)) {
                (Some(d), Some(lo), Some(hi)) => Value::Bool(d.days >= lo.days && d.days <= hi.days),
                _ => Value::Bool(false),
            },
            F::InTimeRange | F::OutOfTimeRange => match (date(0), date(1), date(2)) {
                (Some(d), Some(lo), Some(hi)) => {
                    let (t, lo, hi) = (d.seconds, lo.seconds, hi.seconds);
                    // A range whose end comes before its start spans midnight.
                    let inside = if lo <= hi { t >= lo && t <= hi } else { t >= lo || t <= hi };
                    Value::Bool(inside == (f == F::InTimeRange))
                }
                _ => Value::Bool(false),
            },
            F::IsSameDay => match date(0) {
                Some(d) => Value::Bool((1..args.len()).any(|i| date(i).is_some_and(|o| o.days == d.days))),
                None => Value::Bool(false),
            },
            F::IsThisWeek => in_days(week_start(now), week_start(now).add_days(7)),
            F::IsThisMonth => in_days(month_start(now), month_start(now).add_months(1)),
            F::IsLastMonth => in_days(month_start(now).add_months(-1), month_start(now)),
            F::IsNextMonth => in_days(month_start(now).add_months(1), month_start(now).add_months(2)),
            F::IsThisYear => in_days(year_start(now), year_start(now).add_months(12)),
            F::IsLastYear => in_days(year_start(now).add_months(-12), year_start(now)),
            F::IsNextYear => in_days(year_start(now).add_months(12), year_start(now).add_months(24)),
            F::IsYearToDate => in_days(year_start(now), today.add_days(1)),
            F::IsJanuary => month_is(1),
            F::IsFebruary => month_is(2),
            F::IsMarch => month_is(3),
            F::IsApril => month_is(4),
            F::IsMay => month_is(5),
            F::IsJune => month_is(6),
            F::IsJuly => month_is(7),
            F::IsAugust => month_is(8),
            F::IsSeptember => month_is(9),
            F::IsOctober => month_is(10),
            F::IsNovember => month_is(11),
            F::IsDecember => month_is(12),
            F::FormatString => {
                let rest: Vec<Value> = (1..args.len()).map(a).collect();
                Value::Text(format_composite(&text(0), &rest, &self.env.culture))
            }
            F::Rgb => color(&[num(0), num(1), num(2)]),
            F::Argb => color(&[num(1), num(2), num(3), num(0)]),
            F::PageNumber => self.page.map_or(Value::Null, |(n, _)| Value::Number(n as f64)),
            F::PageCount => self.page.map_or(Value::Null, |(_, c)| Value::Number(c as f64)),
            F::CurrentRowIndexInGroup => self.position.map_or(Value::Null, |p| Value::Number(p as f64)),
            F::PrevRowColumnValue | F::NextRowColumnValue => {
                let Some(p) = self.position else { return Value::Null };
                let at = if f == F::PrevRowColumnValue { p.checked_sub(1) } else { p.checked_add(1) };
                match at.and_then(|i| self.rows.get(i)) {
                    Some(row) => self.at_row(row, at).node(&args[0]),
                    None => Value::Null,
                }
            }
        }
    }
}

fn total_seconds(d: DateTime) -> i64 {
    d.days * 86_400 + i64::from(d.seconds)
}

fn add_seconds(d: DateTime, n: i64) -> DateTime {
    let t = total_seconds(d) + n;
    DateTime { days: t.div_euclid(86_400), seconds: t.rem_euclid(86_400) as u32 }
}

fn color(parts: &[Option<f64>]) -> Value {
    let mut s = String::from("#");
    for p in parts {
        let Some(p) = p else { return Value::Null };
        s.push_str(&format!("{:02x}", p.clamp(0.0, 255.0) as u8));
    }
    Value::Text(s)
}

fn variance(values: &[f64], population: bool) -> Option<f64> {
    let n = values.len() as f64;
    let denominator = if population { n } else { n - 1.0 };
    if values.is_empty() || denominator <= 0.0 {
        return None;
    }
    let mean = values.iter().sum::<f64>() / n;
    Some(values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / denominator)
}

fn number(v: Option<f64>) -> Value {
    v.map_or(Value::Null, Value::Number)
}

/// Runs an aggregate over `rows`. `parent` is what `^` reads inside.
#[allow(clippy::too_many_lines, reason = "one arm per aggregate is the clearest form")]
fn aggregate_over<R: Record>(
    env: &Environment,
    call: &AggregateCall,
    rows: &[&R],
    parent: Option<&R>,
    position: Option<usize>,
    page: Option<(usize, usize)>,
    key: usize,
) -> Value {
    let scope_at = |i: usize| Scope { env, row: Some(rows[i]), parent, rows, position: Some(i), page };
    let arg = |i: usize, n: usize| -> Value {
        match call.args.get(n) {
            Some(a) => scope_at(i).node(a),
            None => Value::Bool(true),
        }
    };
    let values = || -> Vec<Value> { (0..rows.len()).map(|i| arg(i, 0)).collect() };
    let numbers = || -> Vec<f64> { values().iter().filter_map(Value::as_number).collect() };
    let distinct = || -> Vec<f64> {
        let mut seen = HashSet::new();
        numbers().into_iter().filter(|n| seen.insert(n.to_bits())).collect()
    };
    use Aggregate as A;
    match call.function {
        A::Count if call.args.is_empty() => Value::Number(rows.len() as f64),
        A::Count => Value::Number(values().iter().filter(|v| !v.is_null()).count() as f64),
        A::Sum => Value::Number(numbers().iter().sum()),
        A::Avg => {
            let n = numbers();
            if n.is_empty() { Value::Null } else { Value::Number(n.iter().sum::<f64>() / n.len() as f64) }
        }
        A::Min | A::Max => values()
            .into_iter()
            .filter(|v| !v.is_null())
            .reduce(|a, b| {
                let take_b =
                    if call.function == A::Min { b.compare(&a).is_lt() } else { b.compare(&a).is_gt() };
                if take_b { b } else { a }
            })
            .unwrap_or_default(),
        A::Exists if call.args.is_empty() => Value::Bool(!rows.is_empty()),
        A::Exists => Value::Bool((0..rows.len()).any(|i| arg(i, 0).is_true())),
        A::Single => match rows.len() {
            1 if call.args.is_empty() => Value::Bool(true),
            1 => arg(0, 0),
            _ => Value::Null,
        },
        A::Join => {
            let sep = call.args.get(1).map_or_else(
                || ", ".to_owned(),
                |c| Scope { env, row: None, parent, rows, position: None, page }.node(c).to_text(),
            );
            Value::Text(
                values().iter().filter(|v| !v.is_null()).map(Value::to_text).collect::<Vec<_>>().join(&sep),
            )
        }
        A::First => {
            if rows.is_empty() {
                Value::Null
            } else {
                arg(0, 0)
            }
        }
        A::Last => {
            if rows.is_empty() {
                Value::Null
            } else {
                arg(rows.len() - 1, 0)
            }
        }
        A::Median => {
            let mut n = numbers();
            if n.is_empty() {
                return Value::Null;
            }
            n.sort_by(f64::total_cmp);
            let mid = n.len() / 2;
            Value::Number(if n.len() % 2 == 1 { n[mid] } else { (n[mid - 1] + n[mid]) / 2.0 })
        }
        A::Var => number(variance(&numbers(), false)),
        A::VarP => number(variance(&numbers(), true)),
        A::StdDev => number(variance(&numbers(), false).map(f64::sqrt)),
        A::StdDevP => number(variance(&numbers(), true).map(f64::sqrt)),
        A::DistinctSum => Value::Number(distinct().iter().sum()),
        A::DistinctCount => {
            let set: HashSet<String> =
                values().iter().filter(|v| !v.is_null()).map(|v| format!("{v:?}")).collect();
            Value::Number(set.len() as f64)
        }
        A::DistinctAvg => {
            let n = distinct();
            if n.is_empty() { Value::Null } else { Value::Number(n.iter().sum::<f64>() / n.len() as f64) }
        }
        A::DistinctVar => number(variance(&distinct(), false)),
        A::DistinctVarP => number(variance(&distinct(), true)),
        A::DistinctStdDev => number(variance(&distinct(), false).map(f64::sqrt)),
        A::DistinctStdDevP => number(variance(&distinct(), true).map(f64::sqrt)),
        A::WeightedAvg => {
            let (mut total, mut weight) = (0.0, 0.0);
            for i in 0..rows.len() {
                if let (Some(v), Some(w)) = (arg(i, 0).as_number(), arg(i, 1).as_number()) {
                    total += v * w;
                    weight += w;
                }
            }
            if weight == 0.0 { Value::Null } else { Value::Number(total / weight) }
        }
        A::RecordNumber => position.map_or(Value::Null, |p| Value::Number(p as f64 + 1.0)),
        A::Percentage => {
            let total: f64 = numbers().iter().sum();
            match position.filter(|p| *p < rows.len()) {
                Some(p) if total != 0.0 => number(arg(p, 0).as_number().map(|v| v / total)),
                _ => Value::Null,
            }
        }
        A::RunningSum => {
            // Up to and including the current row; incremental across rows.
            let Some(upto) = position.filter(|p| *p < rows.len()) else {
                return Value::Number(numbers().iter().sum());
            };
            let cache_key = (key, rows.as_ptr() as usize);
            let cached = env.running.borrow().get(&cache_key).copied();
            let total = match cached {
                Some((last, acc)) if last + 1 == upto => acc + arg(upto, 0).as_number().unwrap_or(0.0),
                Some((last, acc)) if last == upto => acc,
                _ => (0..=upto).filter_map(|i| arg(i, 0).as_number()).sum(),
            };
            env.running.borrow_mut().insert(cache_key, (upto, total));
            Value::Number(total)
        }
    }
}

/// `%` matches any run of characters, `_` any one character; case-sensitive.
fn like(text: &str, pattern: &str) -> bool {
    let (t, p): (Vec<char>, Vec<char>) = (text.chars().collect(), pattern.chars().collect());
    let (mut ti, mut pi, mut star, mut mark) = (0, 0, None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '_' || (p[pi] != '%' && p[pi] == t[ti])) {
            ti += 1;
            pi += 1;
        } else if pi < p.len() && p[pi] == '%' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '%')
}

fn binary(op: BinaryOp, x: Value, y: Value) -> Value {
    use std::cmp::Ordering;
    let cmp = |o: fn(Ordering) -> bool| {
        // Comparisons with Null are false.
        if x.is_null() || y.is_null() { Value::Bool(false) } else { Value::Bool(o(x.compare(&y))) }
    };
    match op {
        BinaryOp::Eq => {
            if x.is_null() || y.is_null() {
                return Value::Bool(x.is_null() && y.is_null());
            }
            Value::Bool(x.loose_eq(&y))
        }
        BinaryOp::Ne => match binary(BinaryOp::Eq, x, y) {
            Value::Bool(b) => Value::Bool(!b),
            v => v,
        },
        BinaryOp::Lt => cmp(Ordering::is_lt),
        BinaryOp::Le => cmp(Ordering::is_le),
        BinaryOp::Gt => cmp(Ordering::is_gt),
        BinaryOp::Ge => cmp(Ordering::is_ge),
        BinaryOp::Like => {
            if x.is_null() || y.is_null() {
                Value::Bool(false)
            } else {
                Value::Bool(like(&x.to_text(), &y.to_text()))
            }
        }
        BinaryOp::Add => match (&x, &y) {
            (Value::Null, _) | (_, Value::Null) => Value::Null,
            // Text joins: 'Nº ' + [Number].
            (Value::Text(_), _) | (_, Value::Text(_)) => Value::Text(x.to_text() + &y.to_text()),
            (Value::Date(d), _) => y.as_number().map_or(Value::Null, |n| Value::Date(d.add_days(n as i64))),
            _ => arith(&x, &y, |a, b| a + b),
        },
        BinaryOp::Sub => match (&x, &y) {
            (Value::Date(a), Value::Date(b)) => Value::Number((a.days - b.days) as f64),
            (Value::Date(d), _) => {
                y.as_number().map_or(Value::Null, |n| Value::Date(d.add_days(-(n as i64))))
            }
            _ => arith(&x, &y, |a, b| a - b),
        },
        BinaryOp::Mul => arith(&x, &y, |a, b| a * b),
        // Division by zero gives Null rather than infinity, so totals stay printable.
        BinaryOp::Div => match y.as_number() {
            Some(d) if d != 0.0 => arith(&x, &y, |a, b| a / b),
            _ => Value::Null,
        },
        BinaryOp::Rem => match y.as_number() {
            Some(d) if d != 0.0 => arith(&x, &y, |a, b| a % b),
            _ => Value::Null,
        },
        BinaryOp::BitAnd => arith(&x, &y, |a, b| ((a as i64) & (b as i64)) as f64),
        BinaryOp::BitOr => arith(&x, &y, |a, b| ((a as i64) | (b as i64)) as f64),
        BinaryOp::BitXor => arith(&x, &y, |a, b| ((a as i64) ^ (b as i64)) as f64),
    }
}

fn arith(x: &Value, y: &Value, f: impl Fn(f64, f64) -> f64) -> Value {
    match (x.as_number(), y.as_number()) {
        (Some(a), Some(b)) => Value::Number(f(a, b)),
        _ => Value::Null,
    }
}
