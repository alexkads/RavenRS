//! Advanced filters: an expression the user types becomes a parameterized SQL
//! `WHERE` through `ravenrs-criteria`, so typed values never reach the SQL text.

use ravenrs_criteria::{Criteria, Dialect, FieldType, Schema, SqlParam, Value, to_sql, validate};
use rusqlite::types::Value as SqlValue;

/// A filter ready to run against the `ledger` view.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledFilter {
    /// SQL text with `?1`, `?2`… placeholders.
    pub sql: String,
    /// The values for the placeholders, in order.
    pub params: Vec<SqlValue>,
}

/// The fields of the `ledger` view, which is what a filter may refer to.
pub fn schema() -> Schema {
    Schema::new()
        .field("Data", FieldType::Text)
        .field("Descrição", FieldType::Text)
        .field("Valor", FieldType::Number)
        .field("Tipo", FieldType::Text)
        .field("Conta", FieldType::Text)
        .field("Categoria", FieldType::Text)
        .field("Membro", FieldType::Text)
}

/// Checks and translates an expression. Blank text means no filter.
pub fn compile(text: &str) -> Result<Option<CompiledFilter>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let criteria = Criteria::parse(text).map_err(|e| format!("Expressão inválida: {e}"))?;
    if let Some(problem) = validate(&criteria, &schema()).first() {
        return Err(format!("Expressão inválida: {problem}"));
    }
    let query =
        to_sql(&criteria, Dialect::Sqlite).map_err(|e| format!("Este filtro não roda no banco: {e}"))?;
    let params = query.params.iter().map(bind).collect::<Result<_, _>>()?;
    Ok(Some(CompiledFilter { sql: query.text, params }))
}

fn bind(param: &SqlParam) -> Result<SqlValue, String> {
    match param {
        SqlParam::Value(Value::Null) => Ok(SqlValue::Null),
        SqlParam::Value(Value::Bool(b)) => Ok(SqlValue::Integer(i64::from(*b))),
        SqlParam::Value(Value::Number(n)) => Ok(SqlValue::Real(*n)),
        SqlParam::Value(Value::Text(t)) => Ok(SqlValue::Text(t.clone())),
        SqlParam::Value(Value::Date(d)) => {
            let (year, month, day) = d.ymd();
            Ok(SqlValue::Text(format!("{year:04}-{month:02}-{day:02}")))
        }
        _ => Err("Este filtro usa um valor que o banco não entende.".to_owned()),
    }
}
