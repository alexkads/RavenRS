//! Between what records hold and what controls show.

use ravenrs_app::{AttachmentStore, EntityInfo, FieldInfo, FieldKind, Tint, Value};
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

/// A record keeps a file as text: its name on the first line and after it its content, or the key
/// its content is under when a store keeps it, which `@` tells apart from base64.
pub(crate) fn value_of_file(
    file: Option<&PickedFile>,
    store: Option<&dyn AttachmentStore>,
) -> Result<Value, String> {
    let Some(file) = file else { return Ok(Value::Null) };
    Ok(Value::Text(match store {
        Some(store) => format!("{}\n@{}", file.name, store.put(&file.name, &file.bytes)?),
        None => format!("{}\n{}", file.name, encode(&file.bytes)),
    }))
}

/// A file from what a record keeps.
pub(crate) fn file_of(value: &Value, store: Option<&dyn AttachmentStore>) -> Option<PickedFile> {
    let Value::Text(text) = value else { return None };
    let (name, content) = text.split_once('\n')?;
    let bytes = match content.strip_prefix('@') {
        Some(key) => store?.get(key).ok()??,
        None => decode(content)?,
    };
    Some(PickedFile::new(name, bytes))
}

/// The name of a file a record keeps, without reading its content.
pub(crate) fn file_name(value: &Value) -> Option<String> {
    let Value::Text(text) = value else { return None };
    text.split_once('\n').map(|(name, _)| name.to_owned())
}

/// What a `display_format` says around the value, and what it says about the value.
fn format_parts(format: &str) -> Option<(&str, &str, &str)> {
    let start = format.find('{')?;
    let end = start + format[start..].find('}')?;
    Some((&format[..start], format[start + 1..end].trim_start_matches(':'), &format[end + 1..]))
}

/// The decimals of `.2`.
fn decimals_of(spec: &str) -> Option<usize> {
    spec.strip_prefix('.')?.parse().ok()
}

/// A moment by a pattern such as `dd/MM/yyyy HH:mm`.
fn moment_text(moment: &Moment, pattern: &str) -> String {
    let date = Date::from_days(moment.days);
    let seconds = i64::from(moment.seconds).rem_euclid(SECONDS_IN_A_DAY);
    pattern
        .replace("yyyy", &format!("{:04}", date.year()))
        .replace("MM", &format!("{:02}", date.month()))
        .replace("dd", &format!("{:02}", date.day()))
        .replace("HH", &format!("{:02}", seconds / 3600))
        .replace("mm", &format!("{:02}", (seconds % 3600) / 60))
        .replace("ss", &format!("{:02}", seconds % 60))
}

/// A value as its field's `display_format` asks.
fn formatted(field: &FieldInfo, value: &Value) -> Option<String> {
    let (before, spec, after) = format_parts(field.display_format.as_deref()?)?;
    let middle = match value {
        Value::Null => return Some(String::new()),
        Value::Date(moment) if !spec.is_empty() => moment_text(moment, spec),
        Value::Number(number) => match decimals_of(spec) {
            Some(decimals) => {
                let number = if matches!(field.kind, FieldKind::Money) { number / 100.0 } else { *number };
                format!("{number:.decimals$}")
            }
            None => plain_text(field, value),
        },
        _ => plain_text(field, value),
    };
    Some(format!("{before}{middle}{after}"))
}

/// The prefix, decimals and suffix a `display_format` gives a number.
fn number_shape(field: &FieldInfo) -> Option<(String, usize, String)> {
    let (before, spec, after) = format_parts(field.display_format.as_deref()?)?;
    Some((before.to_owned(), decimals_of(spec).unwrap_or(0), after.to_owned()))
}

/// A value as words, with what its field knows about it.
pub(crate) fn display(field: &FieldInfo, value: &Value) -> String {
    formatted(field, value).unwrap_or_else(|| plain_text(field, value))
}

fn plain_text(field: &FieldInfo, value: &Value) -> String {
    match (&field.kind, value) {
        (_, Value::Null) => String::new(),
        (FieldKind::Tags(options), Value::Text(codes)) => codes
            .split(',')
            .map(|code| {
                options
                    .iter()
                    .find(|option| option.code == code)
                    .map_or(code, |option| option.caption.as_str())
            })
            .collect::<Vec<_>>()
            .join(", "),
        // Its name, and never its content.
        (FieldKind::Image | FieldKind::File, value) => file_name(value).unwrap_or_default(),
        (FieldKind::Enum(options), Value::Text(code)) => options
            .iter()
            .find(|option| option.code == *code)
            .map_or_else(|| code.clone(), |option| option.caption.clone()),
        (FieldKind::Date, value) => date_of(value).map_or_else(String::new, |date| date.to_string()),
        // Year first, so a column of these sorts as it reads.
        (FieldKind::DateTime, Value::Date(when)) => {
            let (year, month, day) = when.ymd();
            let (hour, minute) = (when.seconds / 3600, when.seconds % 3600 / 60);
            format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
        }
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
        (_, Value::Number(number)) => Cell::Number(*number),
        // A pattern of its own gives up on sorting as a date, to show as it was asked.
        (FieldKind::Date, value) if field.display_format.is_some() => Cell::Text(display(field, value)),
        (FieldKind::Date, value) => date_of(value).map_or(Cell::Empty, Cell::Date),
        (
            FieldKind::Enum(_)
            | FieldKind::Tags(_)
            | FieldKind::DateTime
            | FieldKind::Image
            | FieldKind::File,
            value,
        ) => Cell::Text(display(field, value)),
        (_, Value::Text(_)) if field.display_format.is_some() => Cell::Text(display(field, value)),
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
        FieldKind::Integer | FieldKind::Decimal(_) | FieldKind::Money if field.display_format.is_some() => {
            let (prefix, decimals, suffix) = number_shape(field).unwrap_or_default();
            GridColumn::number(caption).number_format(NumberFormat { suffix, ..number(decimals, &prefix) })
        }
        FieldKind::Integer => GridColumn::number(caption).number_format(number(0, "")),
        FieldKind::Decimal(places) => {
            GridColumn::number(caption).number_format(number(usize::from(*places), ""))
        }
        FieldKind::Money => GridColumn::number(caption).number_format(number(2, &words.money_prefix)),
        FieldKind::Boolean => {
            let (yes, no) = (words.yes.clone(), words.no.clone());
            GridColumn::boolean(caption).format(move |value| match value {
                ravenrs_widgets::grid::Value::Bool(true) => yes.clone(),
                ravenrs_widgets::grid::Value::Bool(false) => no.clone(),
                _ => String::new(),
            })
        }
        FieldKind::Date => GridColumn::date(caption),
        _ => GridColumn::text(caption),
    };
    column.field(field.name.clone())
}

/// Whether a field can be changed right on a list.
pub(crate) fn editable_in_list(field: &FieldInfo) -> bool {
    !field.is_key
        && !field.read_only
        && field.calculated.is_none()
        && matches!(
            field.kind,
            FieldKind::Text
                | FieldKind::Integer
                | FieldKind::Decimal(_)
                | FieldKind::Money
                | FieldKind::Boolean
                | FieldKind::Date
                | FieldKind::Enum(_)
        )
}

/// What a cell edited on a list is, in what the record holds; nothing for what cannot be read.
pub(crate) fn value_of_cell(field: &FieldInfo, cell: &Cell) -> Option<Value> {
    Some(match (&field.kind, cell) {
        (_, Cell::Empty) => Value::Null,
        (FieldKind::Money, Cell::Number(units)) => Value::Number((units * 100.0).round()),
        (FieldKind::Integer, Cell::Number(number)) => Value::Number(number.trunc()),
        (_, Cell::Number(number)) => Value::Number(*number),
        (_, Cell::Bool(on)) => Value::Bool(*on),
        (FieldKind::Date, Cell::Date(date)) => value_of_date(Some(*date)),
        (FieldKind::Enum(options), Cell::Text(text)) => {
            let found = options.iter().find(|option| option.caption == *text || option.code == *text)?;
            Value::Text(found.code.clone())
        }
        (FieldKind::Text | FieldKind::LongText, Cell::Text(text)) => {
            if text.is_empty() {
                Value::Null
            } else {
                Value::Text(text.clone())
            }
        }
        _ => return None,
    })
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
        let kept = value_of_file(Some(&file), None).expect("kept");
        let back = file_of(&kept, None).expect("a file");
        assert_eq!(back.name, "nota.pdf");
        assert_eq!(&*back.bytes, &[1_u8, 2, 250]);
        assert_eq!(value_of_file(None, None), Ok(Value::Null));
        assert_eq!(file_of(&Value::Text("not a file".into()), None).map(|f| f.name), None);
        assert_eq!(display(&FieldInfo::new("f", FieldKind::File), &kept), "nota.pdf");
    }

    #[test]
    fn a_store_keeps_the_content_out_of_the_record() {
        let store = ravenrs_app::MemoryAttachmentStore::default();
        let file = PickedFile::new("nota.pdf", vec![9_u8; 2000]);
        let kept = value_of_file(Some(&file), Some(&store)).expect("kept");
        let Value::Text(text) = &kept else { panic!("text") };
        assert!(text.len() < 40, "only a key: {text}");
        assert_eq!(file_of(&kept, Some(&store)).expect("a file").bytes.len(), 2000);
        assert_eq!(display(&FieldInfo::new("f", FieldKind::File), &kept), "nota.pdf");
    }

    #[test]
    fn a_display_format_says_how_a_value_shows() {
        let percent = FieldInfo::new("p", FieldKind::Integer).display_format("{} %");
        assert_eq!(display(&percent, &Value::Number(60.0)), "60 %");
        assert_eq!(display(&percent, &Value::Null), "");
        let fine = FieldInfo::new("n", FieldKind::Decimal(9)).display_format("{:.4}");
        assert_eq!(display(&fine, &Value::Number(0.123_456_789)), "0.1235");
        assert_eq!(cell_of(&fine, &Value::Number(1.5)), Cell::Number(1.5), "it still sorts as a number");
        let when = FieldInfo::new("w", FieldKind::DateTime).display_format("{:dd/MM/yyyy HH:mm}");
        let date = Date::new(2026, 11, 3).expect("a date");
        let moment = Value::Date(Moment { days: date.to_days(), seconds: 9 * 3600 + 5 * 60 });
        assert_eq!(display(&when, &moment), "03/11/2026 09:05");
        let site = FieldInfo::new("s", FieldKind::Text).display_format("https://{}");
        assert_eq!(display(&site, &Value::Text("a.b".into())), "https://a.b");
        assert_eq!(cell_of(&site, &Value::Text("a.b".into())), Cell::Text("https://a.b".into()));
    }

    #[test]
    fn tags_show_by_their_captions() {
        let options =
            vec![ravenrs_app::EnumOption::new("a", "Um"), ravenrs_app::EnumOption::new("b", "Dois")];
        let tags = FieldInfo::new("t", FieldKind::Tags(options));
        assert_eq!(display(&tags, &Value::Text("a,b".into())), "Um, Dois");
    }

    #[test]
    fn what_is_typed_on_a_list_becomes_what_a_record_holds() {
        let money = FieldInfo::new("m", FieldKind::Money);
        assert_eq!(value_of_cell(&money, &Cell::Number(12.5)), Some(Value::Number(1250.0)));
        let kind = FieldInfo::new("k", FieldKind::Enum(vec![ravenrs_app::EnumOption::new("a", "Ativo")]));
        assert_eq!(value_of_cell(&kind, &Cell::Text("Ativo".into())), Some(Value::Text("a".into())));
        assert_eq!(value_of_cell(&kind, &Cell::Text("Nada".into())), None);
        let text = FieldInfo::new("t", FieldKind::Text);
        assert_eq!(value_of_cell(&text, &Cell::Text(String::new())), Some(Value::Null));
        assert!(!editable_in_list(&FieldInfo::new("id", FieldKind::Integer).key()));
        assert!(editable_in_list(&money));
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
