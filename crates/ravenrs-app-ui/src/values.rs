//! Between what records hold and what controls show.

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Tint, Value};
use ravenrs_criteria::DateTime as Moment;
use ravenrs_report::base64::{decode, encode};
use ravenrs_style::Semantic;
use ravenrs_widgets::grid::Value as Cell;
use ravenrs_widgets::{Date, DateTime, GridColumn, NumberFormat, PickedFile, Time};

use crate::session::Words;

const SECONDS_IN_A_DAY: i64 = 86_400;

/// A record's date as the day people pick.
pub(crate) fn date_of(value: &Value) -> Option<Date> {
    match value {
        Value::Date(moment) => Some(Date::from_days(moment.days)),
        _ => None,
    }
}

pub(crate) fn value_of_date(date: Option<Date>) -> Value {
    date.map_or(Value::Null, |date| Value::Date(Moment { days: date.to_days(), seconds: 0 }))
}

pub(crate) fn date_time_of(value: &Value) -> Option<DateTime> {
    let Value::Date(moment) = value else { return None };
    let seconds = i64::from(moment.seconds).rem_euclid(SECONDS_IN_A_DAY);
    let time = Time::new((seconds / 3600) as u8, ((seconds % 3600) / 60) as u8)?;
    Some(DateTime { date: Date::from_days(moment.days), time })
}

pub(crate) fn value_of_date_time(moment: Option<DateTime>) -> Value {
    moment.map_or(Value::Null, |moment| {
        let seconds = i64::from(moment.time.hour()) * 3600 + i64::from(moment.time.minute()) * 60;
        Value::Date(Moment { days: moment.date.to_days(), seconds: seconds as _ })
    })
}

/// A record keeps a file as text, its name on the first line and its content after it.
pub(crate) fn value_of_file(file: Option<&PickedFile>) -> Value {
    file.map_or(Value::Null, |file| Value::Text(format!("{}\n{}", file.name, encode(&file.bytes))))
}

/// A file from what a record keeps.
pub(crate) fn file_of(value: &Value) -> Option<PickedFile> {
    let Value::Text(text) = value else { return None };
    let (name, content) = text.split_once('\n')?;
    Some(PickedFile::new(name, decode(content)?))
}

/// A value as words, with what its field knows about it.
pub(crate) fn display(field: &FieldInfo, value: &Value) -> String {
    match (&field.kind, value) {
        (_, Value::Null) => String::new(),
        // Its name, and never its content.
        (FieldKind::Image | FieldKind::File, value) => file_of(value).map_or_else(String::new, |f| f.name),
        (FieldKind::Enum(options), Value::Text(code)) => options
            .iter()
            .find(|option| option.code == *code)
            .map_or_else(|| code.clone(), |option| option.caption.clone()),
        (FieldKind::Date, value) => date_of(value).map_or_else(String::new, |date| date.to_string()),
        (_, Value::Text(text)) => text.clone(),
        (_, Value::Number(number)) if number.fract() == 0.0 => format!("{}", *number as i64),
        (_, Value::Number(number)) => number.to_string(),
        (_, Value::Bool(on)) => on.to_string(),
        (_, other) => format!("{other:?}"),
    }
}

/// The cell of a list. Money shows in whole units, though it is kept in cents.
pub(crate) fn cell_of(field: &FieldInfo, value: &Value) -> Cell {
    match (&field.kind, value) {
        (_, Value::Null) => Cell::Empty,
        (FieldKind::Money, Value::Number(cents)) => Cell::Number(cents / 100.0),
        (FieldKind::Date, value) => date_of(value).map_or(Cell::Empty, Cell::Date),
        (FieldKind::Enum(_) | FieldKind::DateTime | FieldKind::Image | FieldKind::File, value) => {
            Cell::Text(display(field, value))
        }
        (_, Value::Number(number)) => Cell::Number(*number),
        (_, Value::Bool(on)) => Cell::Bool(*on),
        (_, Value::Text(text)) => Cell::Text(text.clone()),
        (_, other) => Cell::Text(format!("{other:?}")),
    }
}

/// The column of a list.
pub(crate) fn column_of(field: &FieldInfo, caption: &str, words: &Words) -> GridColumn {
    let number = |decimals: usize, prefix: &str| NumberFormat {
        decimals,
        prefix: prefix.to_owned(),
        thousands: words.thousands,
        decimal: words.decimal,
        ..NumberFormat::default()
    };
    let column = match &field.kind {
        FieldKind::Integer => GridColumn::number(caption).number_format(number(0, "")),
        FieldKind::Decimal(places) => {
            GridColumn::number(caption).number_format(number(usize::from(*places), ""))
        }
        FieldKind::Money => GridColumn::number(caption).number_format(number(2, &words.money_prefix)),
        FieldKind::Boolean => GridColumn::boolean(caption),
        FieldKind::Date => GridColumn::date(caption),
        _ => GridColumn::text(caption),
    };
    column.field(field.name.clone())
}

/// The color of a tint, in the theme.
pub(crate) fn semantic_of(tint: Tint) -> Semantic {
    match tint {
        Tint::Danger => Semantic::Danger,
        Tint::Warning => Semantic::Warning,
        Tint::Success => Semantic::Success,
        Tint::Accent => Semantic::Accent,
        _ => Semantic::InkMuted,
    }
}

/// Text that identifies a key, the same as the grid gives its cell.
pub(crate) fn key_text(entity: &EntityInfo, key: &Value) -> String {
    match entity.key_field() {
        Some(field) => cell_of(field, key).key_text(),
        None => String::new(),
    }
}

/// A key back from its text.
pub(crate) fn key_from_text(entity: &EntityInfo, text: &str) -> Value {
    match entity.key_field().map(|field| &field.kind) {
        Some(FieldKind::Integer | FieldKind::Decimal(_)) => text.parse().map_or(Value::Null, Value::Number),
        _ => Value::Text(text.to_owned()),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn dates_go_there_and_back() {
        let date = Date::new(2026, 11, 2).expect("a date");
        assert_eq!(date_of(&value_of_date(Some(date))), Some(date));
        assert_eq!(value_of_date(None), Value::Null);
        let moment = DateTime { date, time: Time::new(9, 30).expect("a time") };
        assert_eq!(date_time_of(&value_of_date_time(Some(moment))), Some(moment));
    }

    #[test]
    fn money_shows_in_whole_units_and_enums_by_caption() {
        let money = FieldInfo::new("m", FieldKind::Money);
        assert_eq!(cell_of(&money, &Value::Number(1250.0)), Cell::Number(12.5));
        let kind = FieldInfo::new("k", FieldKind::Enum(vec![ravenrs_app::EnumOption::new("a", "Ativo")]));
        assert_eq!(cell_of(&kind, &Value::Text("a".into())), Cell::Text("Ativo".into()));
        assert_eq!(display(&kind, &Value::Text("z".into())), "z", "an old code still shows");
    }

    #[test]
    fn files_go_there_and_back_and_show_by_name() {
        let file = PickedFile::new("nota.pdf", vec![1_u8, 2, 250]);
        let kept = value_of_file(Some(&file));
        let back = file_of(&kept).expect("a file");
        assert_eq!(back.name, "nota.pdf");
        assert_eq!(&*back.bytes, &[1_u8, 2, 250]);
        assert_eq!(value_of_file(None), Value::Null);
        assert_eq!(file_of(&Value::Text("not a file".into())).map(|f| f.name), None);
        assert_eq!(display(&FieldInfo::new("f", FieldKind::File), &kept), "nota.pdf");
    }

    #[test]
    fn keys_survive_being_text() {
        let numbered = EntityInfo::new("A").field(FieldInfo::new("id", FieldKind::Integer).key());
        let text = key_text(&numbered, &Value::Number(12.0));
        assert_eq!(key_from_text(&numbered, &text), Value::Number(12.0));
        let coded = EntityInfo::new("B").field(FieldInfo::new("c", FieldKind::Text).typed_key());
        assert_eq!(key_from_text(&coded, "x1"), Value::Text("x1".into()));
    }
}
