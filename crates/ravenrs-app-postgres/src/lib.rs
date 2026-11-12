//! Records in PostgreSQL, as an [`ObjectSpace`](ravenrs_app::ObjectSpace). What every SQL database
//! has in common is in `ravenrs-app-sql`, and this says how PostgreSQL does it.
//!
//! The types of a table that came first are met where they are: a value goes in as what its column
//! asks for, and comes out as what a field can hold.

use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;

use bytes::{BufMut, BytesMut};
use postgres::Client;
use postgres::error::SqlState;
use postgres::types::{FromSql, IsNull, ToSql, Type, to_sql_checked};
use ravenrs_app::{FieldKind, Value};
use ravenrs_app_sql::{SqlBackend, SqlBuilder, SqlCell, SqlDatabase, SqlError, SqlObjectSpace, SqlRows};
pub use ravenrs_app_sql::{SqlWords, TableMap};
use ravenrs_criteria::{DateTime, Dialect};

type Boxed = Box<dyn Error + Sync + Send>;

const SECONDS_A_DAY: i64 = 86_400;
const MICROS: i64 = 1_000_000;

/// PostgreSQL counts from the first day of 2000.
fn its_epoch() -> i64 {
    DateTime::from_ymd(2000, 1, 1).map_or(0, |date| date.days)
}

/// A value on its way in, as what its column asks for.
#[derive(Debug)]
struct Bound<'a>(&'a Value);

fn number_of(value: &Value) -> Result<f64, Boxed> {
    match value {
        Value::Number(number) => Ok(*number),
        Value::Bool(on) => Ok(f64::from(u8::from(*on))),
        Value::Text(text) => Ok(text.trim().parse()?),
        other => Err(format!("{other:?} is not a number").into()),
    }
}

fn date_of(value: &Value) -> Result<DateTime, Boxed> {
    match value {
        Value::Date(moment) => Ok(*moment),
        Value::Text(text) => DateTime::parse(text).ok_or_else(|| format!("{text} is not a date").into()),
        other => Err(format!("{other:?} is not a date").into()),
    }
}

fn text_of(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        Value::Date(moment) => moment.to_string(),
        Value::Bool(on) => on.to_string(),
        Value::Number(number) if number.fract() == 0.0 && number.abs() < 9e15 => (*number as i64).to_string(),
        Value::Number(number) => number.to_string(),
        _ => String::new(),
    }
}

/// A number in the form PostgreSQL has for `numeric`: groups of four digits around the point.
fn write_numeric(number: f64, out: &mut BytesMut) -> Result<(), Boxed> {
    if !number.is_finite() {
        return Err("a number has to be finite".into());
    }
    let text = format!("{:.6}", number.abs());
    let (whole, part) = text.split_once('.').unwrap_or((&text, ""));
    let mut digits: Vec<i16> = Vec::new();
    let lead = whole.len() % 4;
    let mut groups: Vec<String> = Vec::new();
    if lead > 0 {
        groups.push(whole[..lead].to_owned());
    }
    groups
        .extend(whole.as_bytes()[lead..].chunks(4).map(|chunk| String::from_utf8_lossy(chunk).into_owned()));
    let weight = groups.len() as i16 - 1;
    for group in &groups {
        digits.push(group.parse()?);
    }
    let padded = format!("{part:0<8}");
    digits.push(padded[..4].parse()?);
    digits.push(padded[4..8].parse()?);
    out.put_i16(digits.len() as i16);
    out.put_i16(weight);
    out.put_u16(if number < 0.0 { 0x4000 } else { 0 });
    out.put_i16(6);
    for digit in digits {
        out.put_i16(digit);
    }
    Ok(())
}

fn read_numeric(raw: &[u8]) -> Result<f64, Boxed> {
    let word = |at: usize| -> Result<i16, Boxed> {
        let bytes = raw.get(at..at + 2).ok_or("a numeric is cut short")?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    };
    let (count, weight, sign) = (word(0)? as usize, i32::from(word(2)?), word(4)? as u16);
    let mut total = 0.0;
    for at in 0..count {
        total += f64::from(word(8 + at * 2)?) * 10_000_f64.powi(weight - at as i32);
    }
    Ok(if sign == 0x4000 { -total } else { total })
}

impl ToSql for Bound<'_> {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, Boxed> {
        if matches!(self.0, Value::Null) {
            return Ok(IsNull::Yes);
        }
        match *ty {
            Type::BOOL => {
                let on = match self.0 {
                    Value::Bool(on) => *on,
                    other => number_of(other)? != 0.0,
                };
                out.put_u8(u8::from(on));
            }
            Type::INT2 => out.put_i16(number_of(self.0)? as i16),
            Type::INT4 => out.put_i32(number_of(self.0)? as i32),
            Type::INT8 => out.put_i64(number_of(self.0)? as i64),
            Type::FLOAT4 => out.put_f32(number_of(self.0)? as f32),
            Type::FLOAT8 => out.put_f64(number_of(self.0)?),
            Type::NUMERIC => write_numeric(number_of(self.0)?, out)?,
            Type::DATE => out.put_i32((date_of(self.0)?.days - its_epoch()) as i32),
            Type::TIMESTAMP | Type::TIMESTAMPTZ => {
                let moment = date_of(self.0)?;
                let days = moment.days - its_epoch();
                out.put_i64((days * SECONDS_A_DAY + i64::from(moment.seconds)) * MICROS);
            }
            Type::UUID => {
                let text = text_of(self.0).replace('-', "");
                if text.len() != 32 {
                    return Err("not a uuid".into());
                }
                for at in (0..32).step_by(2) {
                    out.put_u8(u8::from_str_radix(&text[at..at + 2], 16)?);
                }
            }
            _ => out.put_slice(text_of(self.0).as_bytes()),
        }
        Ok(IsNull::No)
    }

    fn accepts(_: &Type) -> bool {
        true
    }

    to_sql_checked!();
}

/// What a column holds on its way out.
struct Cell(SqlCell);

impl<'a> FromSql<'a> for Cell {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Boxed> {
        let cell = match *ty {
            Type::BOOL => SqlCell::Bool(bool::from_sql(ty, raw)?),
            Type::INT2 => SqlCell::Integer(i64::from(i16::from_sql(ty, raw)?)),
            Type::INT4 => SqlCell::Integer(i64::from(i32::from_sql(ty, raw)?)),
            Type::INT8 => SqlCell::Integer(i64::from_sql(ty, raw)?),
            Type::FLOAT4 => SqlCell::Real(f64::from(f32::from_sql(ty, raw)?)),
            Type::FLOAT8 => SqlCell::Real(f64::from_sql(ty, raw)?),
            Type::NUMERIC => {
                let number = read_numeric(raw)?;
                if number.fract() == 0.0 && number.abs() < 9e15 {
                    SqlCell::Integer(number as i64)
                } else {
                    SqlCell::Real(number)
                }
            }
            Type::DATE => {
                let days = i64::from(i32::from_sql(&Type::INT4, raw)?);
                SqlCell::Date(DateTime { days: days + its_epoch(), seconds: 0 })
            }
            Type::TIMESTAMP | Type::TIMESTAMPTZ => {
                let total = i64::from_sql(&Type::INT8, raw)?.div_euclid(MICROS);
                SqlCell::Date(DateTime {
                    days: total.div_euclid(SECONDS_A_DAY) + its_epoch(),
                    seconds: total.rem_euclid(SECONDS_A_DAY) as u32,
                })
            }
            Type::UUID => {
                let hex: String = raw.iter().map(|byte| format!("{byte:02x}")).collect();
                let part = |from: usize, to: usize| hex.get(from..to).unwrap_or("");
                SqlCell::Text(format!(
                    "{}-{}-{}-{}-{}",
                    part(0, 8),
                    part(8, 12),
                    part(12, 16),
                    part(16, 20),
                    part(20, 32)
                ))
            }
            // Whatever else has no field to show it, as with a blob in SQLite.
            Type::BYTEA => SqlCell::Null,
            _ => std::str::from_utf8(raw).map_or(SqlCell::Null, |text| SqlCell::Text(text.to_owned())),
        };
        Ok(Self(cell))
    }

    fn from_sql_null(_: &Type) -> Result<Self, Boxed> {
        Ok(Self(SqlCell::Null))
    }

    fn accepts(_: &Type) -> bool {
        true
    }
}

fn sql_error(error: &postgres::Error) -> SqlError {
    let Some(database) = error.as_db_error() else { return SqlError::other(error) };
    let code = database.code();
    if *code == SqlState::UNIQUE_VIOLATION {
        // "Key (name)=(x) already exists." The value stays out of it.
        let column = database
            .detail()
            .and_then(|detail| detail.strip_prefix("Key ("))
            .and_then(|rest| rest.split(')').next())
            .and_then(|columns| columns.split(", ").next())
            .map(|column| column.trim_matches('"').to_owned());
        return SqlError::Unique(column);
    }
    if *code == SqlState::FOREIGN_KEY_VIOLATION {
        return SqlError::ForeignKey;
    }
    if code.code().starts_with("23") {
        return SqlError::Constraint;
    }
    SqlError::Other(database.message().to_owned())
}

/// A connection to PostgreSQL.
pub struct PostgresBackend {
    client: RefCell<Client>,
    statements: RefCell<HashMap<String, postgres::Statement>>,
}

impl std::fmt::Debug for PostgresBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresBackend").finish_non_exhaustive()
    }
}

impl From<Client> for PostgresBackend {
    fn from(client: Client) -> Self {
        Self { client: RefCell::new(client), statements: RefCell::default() }
    }
}

impl PostgresBackend {
    fn statement(&self, sql: &str) -> Result<postgres::Statement, SqlError> {
        if let Some(found) = self.statements.borrow().get(sql) {
            return Ok(found.clone());
        }
        let made = self.client.borrow_mut().prepare(sql).map_err(|e| sql_error(&e))?;
        self.statements.borrow_mut().insert(sql.to_owned(), made.clone());
        Ok(made)
    }
}

impl SqlBackend for PostgresBackend {
    fn dialect(&self) -> Dialect {
        Dialect::Postgres
    }

    fn column_type(&self, kind: &FieldKind) -> &'static str {
        match kind {
            FieldKind::Integer | FieldKind::Money => "BIGINT",
            FieldKind::Decimal(_) => "DOUBLE PRECISION",
            FieldKind::Boolean => "BOOLEAN",
            _ => "TEXT",
        }
    }

    fn slot(&self, at: usize) -> String {
        format!("${at}")
    }

    fn offset_only(&self, offset: usize) -> String {
        format!(" OFFSET {offset}")
    }

    fn columns_of(&self, table: &str) -> Result<Vec<String>, SqlError> {
        let sql = "SELECT column_name::text FROM information_schema.columns \
                   WHERE table_schema = current_schema() AND table_name = $1";
        let rows = self.client.borrow_mut().query(sql, &[&table]).map_err(|e| sql_error(&e))?;
        rows.iter().map(|row| row.try_get(0).map_err(|e| sql_error(&e))).collect()
    }

    fn execute(&self, sql: &str, values: &[Value]) -> Result<u64, SqlError> {
        let statement = self.statement(sql)?;
        let bound: Vec<Bound<'_>> = values.iter().map(Bound).collect();
        let parameters: Vec<&(dyn ToSql + Sync)> =
            bound.iter().map(|value| value as &(dyn ToSql + Sync)).collect();
        self.client.borrow_mut().execute(&statement, &parameters).map_err(|e| sql_error(&e))
    }

    fn batch(&self, sql: &str) -> Result<(), SqlError> {
        // What changes the schema can leave statements that no longer fit.
        if !matches!(sql, "BEGIN" | "COMMIT" | "ROLLBACK") {
            self.statements.borrow_mut().clear();
        }
        self.client.borrow_mut().batch_execute(sql).map_err(|e| sql_error(&e))
    }

    fn query(&self, sql: &str, values: &[Value]) -> Result<SqlRows, SqlError> {
        let statement = self.statement(sql)?;
        let names: Vec<String> = statement.columns().iter().map(|column| column.name().to_owned()).collect();
        let bound: Vec<Bound<'_>> = values.iter().map(Bound).collect();
        let parameters: Vec<&(dyn ToSql + Sync)> =
            bound.iter().map(|value| value as &(dyn ToSql + Sync)).collect();
        let rows = self.client.borrow_mut().query(&statement, &parameters).map_err(|e| sql_error(&e))?;
        let mut read = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut cells = Vec::with_capacity(names.len());
            for at in 0..names.len() {
                let cell: Cell = row.try_get(at).map_err(|e| sql_error(&e))?;
                cells.push(cell.0);
            }
            read.push(cells);
        }
        Ok(SqlRows::new(names, read))
    }
}

/// Records in PostgreSQL. Any number of spaces share its connection.
pub type PostgresDatabase = SqlDatabase<PostgresBackend>;
/// Puts a database together, one entity at a time.
pub type PostgresBuilder = SqlBuilder<PostgresBackend>;
/// A unit of work over a [`PostgresDatabase`]. A commit is one transaction.
pub type PostgresObjectSpace = SqlObjectSpace<PostgresBackend>;
