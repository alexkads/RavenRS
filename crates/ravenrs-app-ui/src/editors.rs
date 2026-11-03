//! A control for each kind of field.

use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Object, Query, Value};
use ravenrs_core::Color;
use ravenrs_criteria::Criteria;
use ravenrs_reactive::{Binding, State};
use ravenrs_widgets::{
    AnyView, Checkbox, ColorPicker, Date, DatePicker, DateTimePicker, LookupField, LookupItem, NumberField,
    SegmentedControl, Select, TextArea, TextField,
};

use crate::session::Session;
use crate::values::{
    date_of, date_time_of, display, key_from_text, key_text, value_of_date, value_of_date_time,
};

/// What a control is being asked for.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct EditorRequest {
    /// The field it edits.
    pub field: FieldInfo,
    /// The value, one state per field, so typing does not touch the whole record.
    pub value: State<Value>,
    /// What people read as its name.
    pub caption: String,
    /// It shows but does not change.
    pub read_only: bool,
    /// The application it lives in.
    pub session: Session,
}

type EditorFactory = Rc<dyn Fn(&EditorRequest) -> AnyView>;

/// Which control edits which kind of field, by name. The model's `Editor` setting
/// picks one, so a field can change its control without touching code.
#[derive(Clone, Default)]
pub struct EditorRegistry(BTreeMap<String, EditorFactory>);

impl std::fmt::Debug for EditorRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.keys()).finish()
    }
}

impl EditorRegistry {
    /// Registers a control by name, replacing one of the same name. The name of a
    /// kind of field, such as `Text`, replaces the built-in for it.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        factory: impl Fn(&EditorRequest) -> AnyView + 'static,
    ) {
        self.0.insert(name.into(), Rc::new(factory));
    }

    /// The control for a request, by the name given or else by the kind of its field.
    pub(crate) fn editor(&self, name: &str, request: &EditorRequest) -> AnyView {
        match (self.0.get(name), name, &request.field.kind) {
            (Some(factory), ..) => factory(request),
            (None, "Segmented", FieldKind::Enum(options)) => segmented_editor(request, options),
            _ => built_in(request),
        }
    }
}

fn number_binding(value: &State<Value>) -> Binding<Option<f64>> {
    let (read, write) = (value.clone(), value.clone());
    Binding::new(
        move || if let Value::Number(number) = read.get() { Some(number) } else { None },
        move |number: Option<f64>| write.set(number.map_or(Value::Null, Value::Number)),
    )
}

fn cents_binding(value: &State<Value>) -> Binding<Option<i64>> {
    let (read, write) = (value.clone(), value.clone());
    Binding::new(
        move || if let Value::Number(cents) = read.get() { Some(cents.round() as i64) } else { None },
        move |cents: Option<i64>| write.set(cents.map_or(Value::Null, |c| Value::Number(c as f64))),
    )
}

fn text_binding(value: &State<Value>) -> Binding<String> {
    let (read, write) = (value.clone(), value.clone());
    Binding::new(
        move || if let Value::Text(text) = read.get() { text } else { String::new() },
        move |text: String| write.set(if text.is_empty() { Value::Null } else { Value::Text(text) }),
    )
}

fn today(session: &Session) -> Date {
    session.today()
}

fn segmented_editor(request: &EditorRequest, options: &[ravenrs_app::EnumOption]) -> AnyView {
    let captions: Vec<String> = options.iter().map(|o| o.caption.clone()).collect();
    if request.read_only {
        return enum_editor(request, options);
    }
    Box::new(SegmentedControl::new(option_index(request, options), captions).label(request.caption.clone()))
}

fn enum_editor(request: &EditorRequest, options: &[ravenrs_app::EnumOption]) -> AnyView {
    let captions: Vec<String> = options.iter().map(|o| o.caption.clone()).collect();
    Box::new(
        Select::new(option_index(request, options), captions)
            .fill()
            .label(request.caption.clone())
            .disabled(request.read_only),
    )
}

/// Which option a field holds, by its place in the list.
fn option_index(request: &EditorRequest, options: &[ravenrs_app::EnumOption]) -> Binding<usize> {
    let codes: Vec<String> = options.iter().map(|o| o.code.clone()).collect();
    let (read, write) = (request.value.clone(), request.value.clone());
    Binding::new(
        {
            let codes = codes.clone();
            move || match read.get() {
                Value::Text(code) => codes.iter().position(|c| *c == code).unwrap_or(0),
                _ => 0,
            }
        },
        move |at: usize| write.set(codes.get(at).map_or(Value::Null, |code| Value::Text(code.clone()))),
    )
}

/// What names a record, for lists to pick from.
fn caption_of(entity: &EntityInfo, object: &Object) -> String {
    match entity.default_property() {
        Some(field) => display(field, &object.get(&field.name)),
        None => entity.key_field().map_or_else(String::new, |field| display(field, &object.get(&field.name))),
    }
}

fn reference_editor(request: &EditorRequest, target: &str) -> AnyView {
    let session = request.session.clone();
    let Some(entity) = session.entity(target) else {
        return text_editor(request);
    };
    let (value, write) = (request.value.clone(), request.value.clone());
    let (reader, known, key_entity) = (session.clone(), entity.clone(), entity.clone());
    let selected = Binding::new(
        move || {
            let key = value.get();
            if matches!(key, Value::Null) {
                return None;
            }
            let found = reader.frame().space().borrow().find(&known.name, &key).ok().flatten();
            let label = found.map_or_else(|| key_text(&known, &key), |object| caption_of(&known, &object));
            Some(LookupItem::new(key_text(&known, &key), label))
        },
        move |item: Option<LookupItem>| {
            write.set(item.map_or(Value::Null, |item| key_from_text(&key_entity, &item.key)));
        },
    );
    let (loader, source) = (session, entity);
    Box::new(
        LookupField::remote(selected, move |asked, answer| {
            let mut query = Query::all().page(0, asked.limit);
            if let Some(field) = source.default_property() {
                query = query.sort(field.name.clone(), true);
                let typed = asked.query.trim().replace('\'', "''");
                if !typed.is_empty()
                    && let Ok(filter) =
                        Criteria::parse(&format!("Contains(Upper([{}]), Upper('{typed}'))", field.name))
                {
                    query = query.filter(filter);
                }
            }
            match loader.frame().space().borrow().query(&source.name, &query) {
                Ok(found) => answer.respond(
                    found
                        .iter()
                        .map(|object| {
                            let key = source.key_field().map_or(Value::Null, |f| object.get(&f.name));
                            LookupItem::new(key_text(&source, &key), caption_of(&source, object))
                        })
                        .collect(),
                ),
                Err(error) => answer.fail(error.to_string()),
            }
        })
        .label(request.caption.clone())
        .width(320.0)
        .read_only(request.read_only),
    )
}

fn text_editor(request: &EditorRequest) -> AnyView {
    let field = TextField::new(text_binding(&request.value)).read_only(request.read_only);
    match &request.field.placeholder {
        Some(text) => Box::new(field.placeholder(text.clone())),
        None => Box::new(field),
    }
}

/// What shows a value with no control of its own: its text, which cannot be changed.
fn read_only_text(request: &EditorRequest) -> AnyView {
    let text = display(&request.field, &request.value.get());
    Box::new(TextField::new(Binding::constant(text)).read_only(true))
}

fn built_in(request: &EditorRequest) -> AnyView {
    let (value, read_only) = (&request.value, request.read_only);
    if request.field.calculated.is_some() {
        return read_only_text(request);
    }
    match &request.field.kind {
        FieldKind::Text | FieldKind::Criteria => text_editor(request),
        FieldKind::LongText => Box::new(TextArea::new(text_binding(value)).read_only(read_only)),
        FieldKind::Integer => Box::new(
            NumberField::new(number_binding(value)).label(request.caption.clone()).read_only(read_only),
        ),
        FieldKind::Decimal(places) => Box::new(
            NumberField::new(number_binding(value))
                .decimals(usize::from(*places))
                .label(request.caption.clone())
                .read_only(read_only),
        ),
        FieldKind::Money => Box::new(
            NumberField::money(cents_binding(value), request.session.words().money_prefix.clone())
                .placeholder(request.field.placeholder.clone().unwrap_or_default())
                .label(request.caption.clone())
                .read_only(read_only),
        ),
        FieldKind::Date => {
            let (read, write) = (value.clone(), value.clone());
            let date = Binding::new(move || date_of(&read.get()), move |d| write.set(value_of_date(d)));
            Box::new(
                DatePicker::new(date, today(&request.session))
                    .label(request.caption.clone())
                    .read_only(read_only),
            )
        }
        FieldKind::DateTime => {
            let (read, write) = (value.clone(), value.clone());
            let moment =
                Binding::new(move || date_time_of(&read.get()), move |m| write.set(value_of_date_time(m)));
            Box::new(
                DateTimePicker::new(moment, today(&request.session))
                    .label(request.caption.clone())
                    .read_only(read_only),
            )
        }
        FieldKind::Boolean => {
            let (read, write) = (value.clone(), value.clone());
            let on = Binding::new(
                move || matches!(read.get(), Value::Bool(true)),
                move |on: bool| write.set(Value::Bool(on)),
            );
            Box::new(Checkbox::new(request.caption.clone(), on).read_only(read_only))
        }
        FieldKind::Enum(options) => enum_editor(request, options),
        FieldKind::Reference(target) => reference_editor(request, target),
        FieldKind::Color => {
            let (read, write) = (value.clone(), value.clone());
            let color = Binding::new(
                move || if let Value::Text(hex) = read.get() { parse_color(&hex) } else { None },
                move |color: Option<Color>| {
                    write.set(color.map_or(Value::Null, |c| Value::Text(color_text(c))));
                },
            );
            Box::new(ColorPicker::new(color).label(request.caption.clone()).read_only(read_only))
        }
        // Their controls need somewhere to keep bytes, which comes with the relations.
        _ => read_only_text(request),
    }
}

fn parse_color(hex: &str) -> Option<Color> {
    let digits = hex.trim().trim_start_matches('#');
    (digits.len() == 6).then(|| u32::from_str_radix(digits, 16).ok().map(Color::hex)).flatten()
}

fn color_text(color: Color) -> String {
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", byte(color.r), byte(color.g), byte(color.b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_kept_as_text() {
        assert_eq!(parse_color("#2563EB").map(color_text).as_deref(), Some("#2563EB"));
        assert_eq!(parse_color("nope"), None);
    }
}
