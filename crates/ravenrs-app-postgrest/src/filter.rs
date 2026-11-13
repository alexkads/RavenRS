//! A filter in the words PostgREST reads: `and=(name.eq."Alfa",seats.gt.1)`.
//!
//! What its grammar cannot say (arithmetic, date functions, a calculated field, a path through a
//! relation) is [`Untranslatable`], and the space reads the records and filters them in memory, as
//! the SQL spaces do with what SQL cannot say. Nothing here knows about HTTP or the space.

use std::collections::BTreeMap;

use ravenrs_criteria::{BinaryOp, Criteria, Function, GroupOp, UnaryOp, Value};

/// The filter cannot be said to PostgREST.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Untranslatable;

type Said = Result<String, Untranslatable>;

/// Where each field the database can filter on is: its column. A field that is not here (a
/// calculated one, a collection) makes the filter untranslatable.
pub(crate) type Columns = BTreeMap<String, String>;

/// A value as PostgREST reads it in a filter. Text always goes in double quotes, so a comma, a
/// point or a parenthesis in it is not taken for the grammar.
fn literal(value: &Value) -> Said {
    match value {
        Value::Bool(on) => Ok(on.to_string()),
        Value::Number(number) if number.is_finite() => {
            if number.fract() == 0.0 && number.abs() < 9e15 {
                Ok((*number as i64).to_string())
            } else {
                Ok(number.to_string())
            }
        }
        Value::Text(text) => Ok(quoted(text)),
        Value::Date(moment) => Ok(quoted(&moment.to_string())),
        _ => Err(Untranslatable),
    }
}

fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A `Like` pattern: `%` is `*` here, and `_` stays one character.
fn pattern(text: &str) -> Said {
    if text.contains('*') {
        // There is no way to say a literal `*` in a PostgREST pattern.
        return Err(Untranslatable);
    }
    Ok(quoted(&text.replace('%', "*")))
}

fn column<'a>(criteria: &Criteria, columns: &'a Columns) -> Result<&'a str, Untranslatable> {
    match criteria {
        Criteria::Property(path) if path.len() == 1 => {
            columns.get(&path[0]).map(String::as_str).ok_or(Untranslatable)
        }
        _ => Err(Untranslatable),
    }
}

fn comparison(op: BinaryOp) -> Option<(&'static str, &'static str)> {
    // The operator, and the one it turns into when the value is on the left.
    Some(match op {
        BinaryOp::Eq => ("eq", "eq"),
        BinaryOp::Ne => ("neq", "neq"),
        BinaryOp::Lt => ("lt", "gt"),
        BinaryOp::Le => ("lte", "gte"),
        BinaryOp::Gt => ("gt", "lt"),
        BinaryOp::Ge => ("gte", "lte"),
        _ => return None,
    })
}

/// One condition, or a group, as it goes inside `and(…)`: `seats.gt.1`, `or(…)`, `not.and(…)`.
fn condition(criteria: &Criteria, columns: &Columns) -> Said {
    match criteria {
        Criteria::Group(op, parts) => group(*op, parts, columns),
        Criteria::Unary(UnaryOp::Not, inner) => negated(inner, columns),
        Criteria::Binary(BinaryOp::Like, left, right) => {
            let Criteria::Value(Value::Text(text)) = right.as_ref() else { return Err(Untranslatable) };
            Ok(format!("{}.like.{}", column(left, columns)?, pattern(text)?))
        }
        Criteria::Binary(op, left, right) => {
            let (forward, backward) = comparison(*op).ok_or(Untranslatable)?;
            let (field, op, value) = match (left.as_ref(), right.as_ref()) {
                (field, Criteria::Value(value)) => (field, forward, value),
                (Criteria::Value(value), field) => (field, backward, value),
                _ => return Err(Untranslatable),
            };
            let field = column(field, columns)?;
            match (op, value) {
                ("eq", Value::Null) => Ok(format!("{field}.is.null")),
                ("neq", Value::Null) => Ok(format!("{field}.not.is.null")),
                (_, Value::Null) => Err(Untranslatable),
                _ => Ok(format!("{field}.{op}.{}", literal(value)?)),
            }
        }
        Criteria::In(field, values) => {
            let field = column(field, columns)?;
            let listed = values
                .iter()
                .map(|value| match value {
                    Criteria::Value(value) => literal(value),
                    _ => Err(Untranslatable),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if listed.is_empty() {
                return Err(Untranslatable);
            }
            Ok(format!("{field}.in.({})", listed.join(",")))
        }
        Criteria::Between(field, low, high) => {
            let field = column(field, columns)?;
            let (Criteria::Value(low), Criteria::Value(high)) = (low.as_ref(), high.as_ref()) else {
                return Err(Untranslatable);
            };
            Ok(format!("and({field}.gte.{},{field}.lte.{})", literal(low)?, literal(high)?))
        }
        Criteria::IsNull(field) => Ok(format!("{}.is.null", column(field, columns)?)),
        Criteria::Function(function, arguments) => text_function(*function, arguments, columns),
        // A boolean field alone, as in `[Active]`.
        Criteria::Property(_) => Ok(format!("{}.is.true", column(criteria, columns)?)),
        _ => Err(Untranslatable),
    }
}

fn text_function(function: Function, arguments: &[Criteria], columns: &Columns) -> Said {
    let [field, Criteria::Value(Value::Text(part))] = arguments else { return Err(Untranslatable) };
    if part.contains(['*', '%', '_']) {
        return Err(Untranslatable);
    }
    let field = column(field, columns)?;
    let wanted = match function {
        Function::StartsWith => format!("{part}*"),
        Function::EndsWith => format!("*{part}"),
        Function::Contains => format!("*{part}*"),
        _ => return Err(Untranslatable),
    };
    Ok(format!("{field}.like.{}", quoted(&wanted)))
}

fn group(op: GroupOp, parts: &[Criteria], columns: &Columns) -> Said {
    let said = parts.iter().map(|part| condition(part, columns)).collect::<Result<Vec<_>, _>>()?;
    let name = match op {
        GroupOp::And => "and",
        GroupOp::Or => "or",
        _ => return Err(Untranslatable),
    };
    Ok(format!("{name}({})", said.join(",")))
}

fn negated(inner: &Criteria, columns: &Columns) -> Said {
    let said = condition(inner, columns)?;
    // A group takes `not.` in front; a condition takes it after the column.
    if said.starts_with("and(") || said.starts_with("or(") {
        return Ok(format!("not.{said}"));
    }
    match said.split_once('.') {
        Some((field, rest)) if !rest.starts_with("not.") => Ok(format!("{field}.not.{rest}")),
        Some((field, rest)) => Ok(format!("{field}.{}", &rest["not.".len()..])),
        None => Err(Untranslatable),
    }
}

/// The filter as the value of an `and` query parameter: `(name.eq."Alfa",seats.gt.1)`.
pub(crate) fn translate(criteria: &Criteria, columns: &Columns) -> Said {
    let said = condition(criteria, columns)?;
    Ok(match said.strip_prefix("and(") {
        Some(inside) => format!("({inside}"),
        None => format!("({said})"),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn columns() -> Columns {
        [("name", "name"), ("seats", "seats"), ("rate", "rate"), ("active", "is_active"), ("day", "day")]
            .into_iter()
            .map(|(field, column)| (field.to_owned(), column.to_owned()))
            .collect()
    }

    fn said(text: &str) -> Result<String, Untranslatable> {
        translate(&Criteria::parse(text).unwrap(), &columns())
    }

    #[test]
    fn comparisons_go_with_the_column_and_the_value() {
        assert_eq!(said("[seats] > 1").unwrap(), "(seats.gt.1)");
        assert_eq!(said("[seats] >= 1").unwrap(), "(seats.gte.1)");
        assert_eq!(said("[seats] < 1.5").unwrap(), "(seats.lt.1.5)");
        assert_eq!(said("[seats] <= 1").unwrap(), "(seats.lte.1)");
        assert_eq!(said("[seats] <> 2").unwrap(), "(seats.neq.2)");
        assert_eq!(said("[name] = 'Alfa'").unwrap(), "(name.eq.\"Alfa\")");
        assert_eq!(said("3 < [seats]").unwrap(), "(seats.gt.3)", "the value on the left turns the operator");
        assert_eq!(said("[active] = True").unwrap(), "(is_active.eq.true)", "the column, not the field");
        assert_eq!(said("[active]").unwrap(), "(is_active.is.true)");
        assert_eq!(said("[day] = #2026-11-13#").unwrap(), "(day.eq.\"2026-11-13\")");
    }

    #[test]
    fn groups_lists_ranges_and_nulls() {
        assert_eq!(said("[seats] > 1 And [active] = True").unwrap(), "(seats.gt.1,is_active.eq.true)");
        assert_eq!(
            said("[seats] > 1 And ([name] = 'A' Or [name] = 'B')").unwrap(),
            "(seats.gt.1,or(name.eq.\"A\",name.eq.\"B\"))"
        );
        assert_eq!(said("[name] = 'A' Or [name] = 'B'").unwrap(), "(or(name.eq.\"A\",name.eq.\"B\"))");
        assert_eq!(said("[name] In ('Alfa', 'Gama')").unwrap(), "(name.in.(\"Alfa\",\"Gama\"))");
        assert_eq!(said("[seats] Between(2, 3)").unwrap(), "(seats.gte.2,seats.lte.3)");
        assert_eq!(said("[name] Is Null").unwrap(), "(name.is.null)");
        assert_eq!(said("Not [name] Is Null").unwrap(), "(name.not.is.null)");
        assert_eq!(said("Not ([seats] = 1 Or [seats] = 2)").unwrap(), "(not.or(seats.eq.1,seats.eq.2))");
        assert_eq!(said("Not [seats] > 1").unwrap(), "(seats.not.gt.1)");
    }

    #[test]
    fn text_matches_keep_their_meaning() {
        assert_eq!(said("[name] Like 'B%'").unwrap(), "(name.like.\"B*\")");
        assert_eq!(said("[name] Like 'B_ta'").unwrap(), "(name.like.\"B_ta\")");
        assert_eq!(said("StartsWith([name], 'Al')").unwrap(), "(name.like.\"Al*\")");
        assert_eq!(said("EndsWith([name], 'fa')").unwrap(), "(name.like.\"*fa\")");
        assert_eq!(said("Contains([name], 'lf')").unwrap(), "(name.like.\"*lf*\")");
    }

    #[test]
    fn what_looks_like_the_grammar_inside_text_stays_text() {
        assert_eq!(said("[name] = 'a,b.(c)'").unwrap(), "(name.eq.\"a,b.(c)\")");
        assert_eq!(said("[name] = 'say \"hi\"'").unwrap(), "(name.eq.\"say \\\"hi\\\"\")");
        assert_eq!(said("[name] = 'back\\slash'").unwrap(), "(name.eq.\"back\\\\slash\")");
    }

    #[test]
    fn what_postgrest_cannot_say_goes_to_memory() {
        assert_eq!(said("[seats] + 1 > 2"), Err(Untranslatable), "arithmetic");
        assert_eq!(said("[twice] > 2"), Err(Untranslatable), "a field with no column");
        assert_eq!(said("GetYear([day]) = 2026"), Err(Untranslatable), "a date function");
        assert_eq!(said("[seats] = [rate]"), Err(Untranslatable), "two fields");
        assert_eq!(said("[name] Like 'a*b'"), Err(Untranslatable), "a literal star");
        assert_eq!(said("Contains([name], '50%')"), Err(Untranslatable), "a wildcard inside the part");
        assert_eq!(said("[seats] > 1 And [seats] + 1 > 2"), Err(Untranslatable), "one part is enough");
    }
}
