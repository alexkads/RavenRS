//! Expression evaluation against rows of JSON data. This interprets the
//! parsed tree of the report expression language; it never runs host code.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use serde_json::Value as Json;

use super::{Agg, BinOp, Expr, Func, Node};
use crate::format::{Culture, format_composite};
use crate::value::{DateTime, Value};

/// What every evaluation in one report run shares: parameters, calculated
/// fields, culture, the clock, and caches.
pub(crate) struct Env {
    pub params: HashMap<String, Value>,
    pub calculated: HashMap<String, Expr>,
    pub culture: Culture,
    pub now: DateTime,
    /// Running sums by (expression, scope): the last row index and the total so far.
    running: RefCell<HashMap<(usize, usize), (usize, f64)>>,
    depth: Cell<u32>,
}

impl Env {
    pub fn new(
        params: HashMap<String, Value>,
        calculated: HashMap<String, Expr>,
        culture: Culture,
        now: DateTime,
    ) -> Self {
        Self { params, calculated, culture, now, running: RefCell::new(HashMap::new()), depth: Cell::new(0) }
    }
}

/// Where an expression is evaluated: the current row, its master row, the rows
/// aggregates run over, and the page.
#[derive(Clone, Copy)]
pub(crate) struct Ctx<'a> {
    pub env: &'a Env,
    pub row: Option<&'a Json>,
    pub parent: Option<&'a Json>,
    /// The rows aggregates run over (a group, a page, the report).
    pub scope: &'a [&'a Json],
    /// The current row's index in `scope`, for running aggregates.
    pub position: Option<usize>,
    /// Page number and page count, inside page bands.
    pub page: Option<(usize, usize)>,
}

impl<'a> Ctx<'a> {
    pub fn eval(&self, expr: &Expr) -> Value {
        self.node(&expr.0)
    }

    fn field(&self, path: &[String]) -> Value {
        let (base, path) = match path.split_first() {
            Some((first, rest)) if first == "^" => (self.parent, rest),
            _ => (self.row, path),
        };
        if path.len() == 1
            && let Some(calc) = self.env.calculated.get(&path[0])
        {
            // Calculated fields may use each other; a cycle yields Null.
            if self.env.depth.get() > 32 {
                return Value::Null;
            }
            self.env.depth.set(self.env.depth.get() + 1);
            let v = self.eval(calc);
            self.env.depth.set(self.env.depth.get() - 1);
            return v;
        }
        lookup(base, path).map_or(Value::Null, Value::from_json)
    }

    fn node(&self, n: &Node) -> Value {
        match n {
            Node::Lit(v) => v.clone(),
            Node::Field(path) => self.field(path),
            Node::Param(name) => self.env.params.get(name).cloned().unwrap_or_default(),
            Node::Neg(a) => match self.node(a) {
                Value::Null => Value::Null,
                v => v.as_number().map_or(Value::Null, |x| Value::Number(-x)),
            },
            Node::Not(a) => Value::Bool(!self.node(a).is_true()),
            Node::And(a, b) => Value::Bool(self.node(a).is_true() && self.node(b).is_true()),
            Node::Or(a, b) => Value::Bool(self.node(a).is_true() || self.node(b).is_true()),
            Node::Bin(op, a, b) => binary(*op, self.node(a), self.node(b)),
            Node::In(a, items) => {
                let v = self.node(a);
                Value::Bool(items.iter().any(|i| v.loose_eq(&self.node(i))))
            }
            Node::Between(a, lo, hi) => {
                let v = self.node(a);
                if v.is_null() {
                    return Value::Bool(false);
                }
                Value::Bool(v.compare(&self.node(lo)).is_ge() && v.compare(&self.node(hi)).is_le())
            }
            Node::IsNull(a, negated) => Value::Bool(self.node(a).is_null() != *negated),
            Node::Call(f, args) => self.call(*f, args),
            Node::Agg(agg, arg) => {
                self.aggregate(*agg, arg.as_deref(), self.scope, n as *const Node as usize)
            }
            Node::Collection(path, agg, arg) => {
                let Some(Json::Array(items)) = lookup(self.row, path) else {
                    return if *agg == Agg::Count { Value::Number(0.0) } else { Value::Null };
                };
                let rows: Vec<&Json> = items.iter().collect();
                let inner = Ctx { parent: self.row, scope: &rows, position: None, ..*self };
                inner.aggregate(*agg, arg.as_deref(), &rows, n as *const Node as usize)
            }
        }
    }

    fn aggregate(&self, agg: Agg, arg: Option<&Node>, rows: &[&Json], key: usize) -> Value {
        let value_at = |i: usize| -> Value {
            match arg {
                Some(a) => Ctx { row: Some(rows[i]), position: Some(i), ..*self }.node(a),
                None => Value::Bool(true),
            }
        };
        match agg {
            Agg::Count => Value::Number(if arg.is_none() {
                rows.len()
            } else {
                (0..rows.len()).filter(|&i| !value_at(i).is_null()).count()
            } as f64),
            Agg::Sum => Value::Number((0..rows.len()).filter_map(|i| value_at(i).as_number()).sum()),
            Agg::Avg => {
                let nums: Vec<f64> = (0..rows.len()).filter_map(|i| value_at(i).as_number()).collect();
                if nums.is_empty() {
                    Value::Null
                } else {
                    Value::Number(nums.iter().sum::<f64>() / nums.len() as f64)
                }
            }
            Agg::Min | Agg::Max => (0..rows.len())
                .map(value_at)
                .filter(|v| !v.is_null())
                .reduce(|a, b| {
                    let take_b = if agg == Agg::Min { b.compare(&a).is_lt() } else { b.compare(&a).is_gt() };
                    if take_b { b } else { a }
                })
                .unwrap_or_default(),
            Agg::DistinctCount => {
                let set: HashSet<String> = (0..rows.len())
                    .map(value_at)
                    .filter(|v| !v.is_null())
                    .map(|v| format!("{v:?}"))
                    .collect();
                Value::Number(set.len() as f64)
            }
            Agg::First => {
                if rows.is_empty() {
                    Value::Null
                } else {
                    value_at(0)
                }
            }
            Agg::Last => {
                if rows.is_empty() {
                    Value::Null
                } else {
                    value_at(rows.len() - 1)
                }
            }
            Agg::RunningSum => {
                // Up to and including the current row; incremental across rows.
                let Some(upto) = self.position.filter(|p| *p < rows.len()) else {
                    return self.aggregate(Agg::Sum, arg, rows, key);
                };
                let cache_key = (key, rows.as_ptr() as usize);
                let cached = self.env.running.borrow().get(&cache_key).copied();
                let total = match cached {
                    Some((last, acc)) if last + 1 == upto => acc + value_at(upto).as_number().unwrap_or(0.0),
                    Some((last, acc)) if last == upto => acc,
                    _ => (0..=upto).filter_map(|i| value_at(i).as_number()).sum(),
                };
                self.env.running.borrow_mut().insert(cache_key, (upto, total));
                Value::Number(total)
            }
        }
    }

    fn call(&self, f: Func, args: &[Node]) -> Value {
        let a = |i: usize| args.get(i).map(|n| self.node(n)).unwrap_or_default();
        let text = |i: usize| a(i).to_text();
        let num = |i: usize| a(i).as_number();
        let date = |i: usize| a(i).as_date();
        let culture = &self.env.culture;
        match f {
            Func::Iif => {
                let mut i = 0;
                while i + 1 < args.len() {
                    if a(i).is_true() {
                        return a(i + 1);
                    }
                    i += 2;
                }
                a(args.len() - 1)
            }
            Func::IsNull if args.len() == 1 => Value::Bool(a(0).is_null()),
            Func::IsNull => match a(0) {
                Value::Null => a(1),
                v => v,
            },
            Func::IsNullOrEmpty => Value::Bool(match a(0) {
                Value::Null => true,
                Value::Text(s) => s.is_empty(),
                _ => false,
            }),
            Func::Concat => Value::Text((0..args.len()).map(text).collect()),
            Func::Upper => Value::Text(text(0).to_uppercase()),
            Func::Lower => Value::Text(text(0).to_lowercase()),
            Func::Trim => Value::Text(text(0).trim().to_owned()),
            Func::Len => Value::Number(text(0).chars().count() as f64),
            Func::Substring => {
                let s = text(0);
                let start = num(1).unwrap_or(0.0).max(0.0) as usize;
                let len = num(2).map_or(usize::MAX, |n| n.max(0.0) as usize);
                Value::Text(s.chars().skip(start).take(len).collect())
            }
            Func::Replace => Value::Text(text(0).replace(&text(1), &text(2))),
            Func::Contains => Value::Bool(text(0).contains(&text(1))),
            Func::StartsWith => Value::Bool(text(0).starts_with(&text(1))),
            Func::EndsWith => Value::Bool(text(0).ends_with(&text(1))),
            Func::PadLeft | Func::PadRight => {
                let s = text(0);
                let width = num(1).unwrap_or(0.0).max(0.0) as usize;
                let fill = text(2).chars().next().unwrap_or(' ');
                let pad: String =
                    std::iter::repeat_n(fill, width.saturating_sub(s.chars().count())).collect();
                Value::Text(if f == Func::PadLeft { pad + &s } else { s + &pad })
            }
            Func::ToStr => Value::Text(text(0)),
            Func::ToNumber => num(0).map_or(Value::Null, Value::Number),
            Func::Abs => num(0).map_or(Value::Null, |x| Value::Number(x.abs())),
            Func::Round => num(0).map_or(Value::Null, |x| {
                let p = 10f64.powi(num(1).unwrap_or(0.0) as i32);
                // Half away from zero, as accountants expect.
                Value::Number((x * p).round() / p)
            }),
            Func::Floor => num(0).map_or(Value::Null, |x| Value::Number(x.floor())),
            Func::Ceiling => num(0).map_or(Value::Null, |x| Value::Number(x.ceil())),
            Func::Power => match (num(0), num(1)) {
                (Some(x), Some(y)) => Value::Number(x.powf(y)),
                _ => Value::Null,
            },
            Func::Sqrt => num(0).filter(|x| *x >= 0.0).map_or(Value::Null, |x| Value::Number(x.sqrt())),
            Func::Min2 | Func::Max2 => {
                let (x, y) = (a(0), a(1));
                let first = if f == Func::Min2 { x.compare(&y).is_le() } else { x.compare(&y).is_ge() };
                if first { x } else { y }
            }
            Func::Today => Value::Date(DateTime { seconds: 0, ..self.env.now }),
            Func::Now => Value::Date(self.env.now),
            Func::AddDays => match (date(0), num(1)) {
                (Some(d), Some(n)) => Value::Date(d.add_days(n as i64)),
                _ => Value::Null,
            },
            Func::AddMonths | Func::AddYears => match (date(0), num(1)) {
                (Some(d), Some(n)) => {
                    Value::Date(d.add_months(n as i64 * if f == Func::AddYears { 12 } else { 1 }))
                }
                _ => Value::Null,
            },
            Func::GetYear => date(0).map_or(Value::Null, |d| Value::Number(f64::from(d.ymd().0))),
            Func::GetMonth => date(0).map_or(Value::Null, |d| Value::Number(f64::from(d.ymd().1))),
            Func::GetDay => date(0).map_or(Value::Null, |d| Value::Number(f64::from(d.ymd().2))),
            Func::GetDayOfWeek => date(0).map_or(Value::Null, |d| Value::Number(f64::from(d.weekday()))),
            Func::DateDiffDay => match (date(0), date(1)) {
                (Some(x), Some(y)) => Value::Number((y.days - x.days) as f64),
                _ => Value::Null,
            },
            Func::FormatString => {
                let rest: Vec<Value> = (1..args.len()).map(a).collect();
                Value::Text(format_composite(&text(0), &rest, culture))
            }
            Func::PageNumber => self.page.map_or(Value::Null, |(n, _)| Value::Number(n as f64)),
            Func::PageCount => self.page.map_or(Value::Null, |(_, c)| Value::Number(c as f64)),
        }
    }
}

/// The JSON value at `path` under `base`.
pub(crate) fn lookup<'j>(base: Option<&'j Json>, path: &[String]) -> Option<&'j Json> {
    let mut cur = base?;
    for seg in path {
        cur = match cur {
            Json::Object(map) => map.get(seg)?,
            Json::Array(items) => items.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

fn binary(op: BinOp, x: Value, y: Value) -> Value {
    use std::cmp::Ordering;
    let cmp = |o: fn(Ordering) -> bool| {
        if x.is_null() || y.is_null() {
            // Comparisons with Null are false, except Null = Null.
            return Value::Bool(
                op == BinOp::Eq && x.is_null() && y.is_null()
                    || op == BinOp::Ne && x.is_null() != y.is_null(),
            );
        }
        Value::Bool(o(x.compare(&y)))
    };
    match op {
        BinOp::Eq => {
            if x.is_null() || y.is_null() {
                return Value::Bool(x.is_null() && y.is_null());
            }
            Value::Bool(x.loose_eq(&y))
        }
        BinOp::Ne => match binary(BinOp::Eq, x, y) {
            Value::Bool(b) => Value::Bool(!b),
            v => v,
        },
        BinOp::Lt => cmp(Ordering::is_lt),
        BinOp::Le => cmp(Ordering::is_le),
        BinOp::Gt => cmp(Ordering::is_gt),
        BinOp::Ge => cmp(Ordering::is_ge),
        BinOp::Add => match (&x, &y) {
            (Value::Null, _) | (_, Value::Null) => Value::Null,
            // Text joins: 'Nº ' + [Number].
            (Value::Text(_), _) | (_, Value::Text(_)) => Value::Text(x.to_text() + &y.to_text()),
            (Value::Date(d), _) => y.as_number().map_or(Value::Null, |n| Value::Date(d.add_days(n as i64))),
            _ => arith(&x, &y, |a, b| a + b),
        },
        BinOp::Sub => match (&x, &y) {
            (Value::Date(a), Value::Date(b)) => Value::Number((a.days - b.days) as f64),
            (Value::Date(d), _) => {
                y.as_number().map_or(Value::Null, |n| Value::Date(d.add_days(-(n as i64))))
            }
            _ => arith(&x, &y, |a, b| a - b),
        },
        BinOp::Mul => arith(&x, &y, |a, b| a * b),
        // Division by zero gives Null rather than infinity, so totals stay printable.
        BinOp::Div => match y.as_number() {
            Some(d) if d != 0.0 => arith(&x, &y, |a, b| a / b),
            _ => Value::Null,
        },
        BinOp::Rem => match y.as_number() {
            Some(d) if d != 0.0 => arith(&x, &y, |a, b| a % b),
            _ => Value::Null,
        },
    }
}

fn arith(x: &Value, y: &Value, f: impl Fn(f64, f64) -> f64) -> Value {
    match (x.as_number(), y.as_number()) {
        (Some(a), Some(b)) => Value::Number(f(a, b)),
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::parse;
    use serde_json::json;

    fn env() -> Env {
        let mut params = HashMap::new();
        params.insert("Min".to_owned(), Value::Number(10.0));
        let mut calculated = HashMap::new();
        calculated.insert("Total".to_owned(), parse("[Qty] * [Price]").expect("valid"));
        Env::new(params, calculated, Culture::pt_br(), DateTime::parse("2026-11-01T10:00").expect("valid"))
    }

    fn eval_in(env: &Env, row: &Json, scope: &[&Json], src: &str) -> Value {
        let ctx = Ctx { env, row: Some(row), parent: None, scope, position: None, page: Some((2, 7)) };
        ctx.eval(&parse(src).unwrap_or_else(|e| panic!("{src}: {e}")))
    }

    #[test]
    fn scalars_operators_and_functions() {
        let env = env();
        let row = json!({ "Qty": 3, "Price": 2.5, "Name": "Ana", "Due": "2026-11-10", "Customer": { "City": "Recife" } });
        let e = |s: &str| eval_in(&env, &row, &[], s);
        assert_eq!(e("[Qty] * [Price]"), Value::Number(7.5));
        assert_eq!(e("[Total] + 1"), Value::Number(8.5), "calculated fields");
        assert_eq!(e("[Customer.City]"), Value::from("Recife"));
        assert_eq!(e("'Olá, ' + [Name]"), Value::from("Olá, Ana"));
        assert_eq!(e("[Total] > ?Min"), Value::Bool(false));
        assert_eq!(e("Iif([Qty] > 5, 'big', [Qty] > 2, 'medium', 'small')"), Value::from("medium"));
        assert_eq!(e("[Missing] = Null"), Value::Bool(true));
        assert_eq!(e("[Missing] > 1"), Value::Bool(false));
        assert_eq!(e("IsNull([Missing], 'n/a')"), Value::from("n/a"));
        assert_eq!(e("1 / 0"), Value::Null);
        assert_eq!(e("Round(2.345, 2)"), Value::Number(2.35));
        assert_eq!(e("Upper(Substring([Name], 1))"), Value::from("NA"));
        assert_eq!(e("PadLeft('7', 3, '0')"), Value::from("007"));
        assert_eq!(e("DateDiffDay(Today(), [Due])"), Value::Number(9.0));
        assert_eq!(e("GetMonth(AddMonths([Due], 3))"), Value::Number(2.0));
        assert_eq!(
            e("FormatString('{0:C2} em {1:dd/MM}', [Total], [Due])"),
            Value::from("R$\u{a0}7,50 em 10/11")
        );
        assert_eq!(
            e("FormatString('Página {0} de {1}', PageNumber(), PageCount())"),
            Value::from("Página 2 de 7")
        );
        assert_eq!(e("[Qty] In (1, 3)"), Value::Bool(true));
        assert_eq!(e("[Price] Between(2, 3)"), Value::Bool(true));
        assert_eq!(e("1 = '1'"), Value::Bool(false));
    }

    #[test]
    fn aggregates_over_scopes_and_collections() {
        let env = env();
        let rows = [
            json!({"Qty": 1, "Price": 10, "Region": "N"}),
            json!({"Qty": 2, "Price": 5, "Region": "S"}),
            json!({"Qty": 4, "Price": 1, "Region": "N"}),
        ];
        let scope: Vec<&Json> = rows.iter().collect();
        let e = |s: &str| eval_in(&env, &rows[0], &scope, s);
        assert_eq!(e("Sum([Total])"), Value::Number(24.0));
        assert_eq!(e("Count()"), Value::Number(3.0));
        assert_eq!(e("Avg([Qty])"), Value::Number(7.0 / 3.0));
        assert_eq!(e("Max([Price])"), Value::Number(10.0));
        assert_eq!(e("DistinctCount([Region])"), Value::Number(2.0));
        assert_eq!(e("Last([Region])"), Value::from("N"));
        let invoice = json!({"Items": [{"Qty": 2, "Price": 3}, {"Qty": 1, "Price": 4}], "Discount": 1});
        let e = |s: &str| eval_in(&env, &invoice, &[], s);
        assert_eq!(e("[Items][].Sum([Qty] * [Price]) - [Discount]"), Value::Number(9.0));
        assert_eq!(e("[Items][].Count()"), Value::Number(2.0));
        assert_eq!(e("[Items][].Sum([Qty] * [^.Discount])"), Value::Number(3.0), "^ reads the master row");
        assert_eq!(e("[Nothing][].Count()"), Value::Number(0.0));
    }

    #[test]
    fn running_sum_is_incremental_and_correct() {
        let env = env();
        let rows: Vec<Json> = (1..=1000).map(|i| json!({ "V": i })).collect();
        let scope: Vec<&Json> = rows.iter().collect();
        let expr = parse("RunningSum([V])").expect("valid");
        for (i, row) in rows.iter().enumerate() {
            let ctx =
                Ctx { env: &env, row: Some(row), parent: None, scope: &scope, position: Some(i), page: None };
            let n = (i + 1) as f64;
            assert_eq!(ctx.eval(&expr), Value::Number(n * (n + 1.0) / 2.0));
        }
    }
}
