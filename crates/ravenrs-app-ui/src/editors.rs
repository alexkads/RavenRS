//! A control for each kind of field.

use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Object, Query, Value};
use ravenrs_core::Color;
use ravenrs_criteria::Criteria;
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};
use ravenrs_widgets::{
    AnyView, BuildCx, Button, ButtonSize, ButtonStyle, Checkbox, ColorPicker, Combobox, Date, DatePicker,
    DateTimePicker, FileUpload, FilterBuilder, FilterField, FilterFieldKind, LookupField, LookupItem,
    MaskedField, NodeId, NumberField, Paint, PickedFile, ProgressBar, SegmentedControl, Select, TagBox, Text,
    TextArea, TextField, VStack, View,
};

use crate::session::Session;
use crate::values::{
    date_of, date_time_of, display, file_name, file_of, key_from_text, key_text, value_of_date,
    value_of_date_time, value_of_file,
};

/// What a control is being asked for.
#[derive(Clone, Debug)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct EditorRegistry(BTreeMap<String, EditorFactory>);

impl std::fmt::Debug for EditorRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.keys()).finish()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
            (None, "Gtin", FieldKind::Text) => gtin_editor(request),
            (None, "Segmented", FieldKind::Enum(options)) => segmented_editor(request, options),
            (None, "Select", FieldKind::Reference(target)) => reference_select(request, target),
            (None, "Progress", FieldKind::Integer | FieldKind::Decimal(_)) => progress_editor(request),
            (None, "Text", FieldKind::Criteria) => text_editor(request),
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
        .read_only(request.read_only),
    )
}

/// A value as a literal of the expression language.
pub(crate) fn literal_of(value: &Value) -> String {
    match value {
        Value::Null => "Null".to_owned(),
        Value::Bool(on) => on.to_string(),
        Value::Number(number) => number.to_string(),
        Value::Text(text) => format!("'{}'", text.replace('\'', "''")),
        Value::Date(moment) => format!("#{moment}#"),
        other => format!("'{other:?}'"),
    }
}

/// What the records of a reference have to meet, given the record being edited.
fn lookup_filter(request: &EditorRequest) -> Option<Criteria> {
    let text = request.field.lookup_filter.as_ref()?;
    filter_for(text, request.session.form_record().as_ref())
}

/// An expression where `?field` takes the value of that field of a record.
pub(crate) fn filter_for(text: &str, record: Option<&Object>) -> Option<Criteria> {
    let parsed = Criteria::parse(text).ok()?;
    let filled = parsed.parameters().iter().fold(text.to_owned(), |filled, name| {
        let value = record.map_or(Value::Null, |record| record.get(name));
        filled.replace(&format!("?{name}"), &literal_of(&value))
    });
    Criteria::parse(&filled).ok()
}

/// A reference picked from a list of all its choices, for the few records of a register.
fn reference_select(request: &EditorRequest, target: &str) -> AnyView {
    let session = &request.session;
    let Some(entity) = session.entity(target) else {
        return text_editor(request);
    };
    let mut query = Query::all();
    if let Some(field) = entity.default_property() {
        query = query.sort(field.name.clone(), true);
    }
    if let Some(filter) = lookup_filter(request) {
        query = query.filter(filter);
    }
    let found = session.frame().space().borrow().query(&entity.name, &query).unwrap_or_default();
    let key = entity.key_field().map(|field| field.name.clone()).unwrap_or_default();
    let mut keys: Vec<Value> = Vec::new();
    let mut captions: Vec<String> = Vec::new();
    if let Some(none) = &request.field.none_caption {
        keys.push(Value::Null);
        captions.push(none.clone());
    }
    for object in &found {
        keys.push(object.get(&key));
        captions.push(caption_of(&entity, object));
    }
    // What it held is no longer among the choices, as when a filter changed with another field.
    if !keys.contains(&*request.value.peek()) {
        request.value.set(keys.first().cloned().unwrap_or(Value::Null));
    }
    let (read, write) = (request.value.clone(), request.value.clone());
    let known = keys.clone();
    let at = Binding::new(
        move || {
            let held = read.get();
            known.iter().position(|key| *key == held).unwrap_or(0)
        },
        move |at: usize| write.set(keys.get(at).cloned().unwrap_or(Value::Null)),
    );
    Box::new(Select::new(at, captions).fill().label(request.caption.clone()).disabled(request.read_only))
}

/// How far along something is, from 0 to 100, which only shows.
fn progress_editor(request: &EditorRequest) -> AnyView {
    let held = if let Value::Number(number) = request.value.get() { number } else { 0.0 };
    Box::new(
        VStack::new((Text::new(format!("{held:.0}%")), ProgressBar::new((held / 100.0) as f32)))
            .spacing(4.0)
            .align(Align::Stretch),
    )
}

/// An expression over the fields of an entity, put together by picking.
fn criteria_editor(request: &EditorRequest) -> AnyView {
    let entity = request.field.criteria_of.as_deref().and_then(|name| request.session.entity(name));
    let Some(entity) = entity.filter(|_| !request.read_only) else {
        return text_editor(request);
    };
    let fields = entity.fields.iter().filter_map(|field| {
        let kind = match &field.kind {
            FieldKind::Integer | FieldKind::Decimal(_) | FieldKind::Money => FilterFieldKind::Number,
            FieldKind::Boolean => FilterFieldKind::Boolean,
            FieldKind::Date | FieldKind::DateTime => FilterFieldKind::Date,
            FieldKind::Text | FieldKind::LongText | FieldKind::Enum(_) | FieldKind::Tags(_) => {
                FilterFieldKind::Text
            }
            _ => return None,
        };
        Some(FilterField::new(field.name.clone(), field.caption.clone(), kind))
    });
    Box::new(FilterBuilder::new(text_binding(&request.value), fields.collect::<Vec<_>>()))
}

/// Several options at once, kept as their codes separated by commas.
fn tags_editor(request: &EditorRequest, options: &[ravenrs_app::EnumOption]) -> AnyView {
    let (read, write) = (request.value.clone(), request.value.clone());
    let chosen = Binding::new(
        move || match read.get() {
            Value::Text(codes) if !codes.is_empty() => codes.split(',').map(str::to_owned).collect(),
            _ => Vec::new(),
        },
        move |codes: Vec<String>| {
            write.set(if codes.is_empty() { Value::Null } else { Value::Text(codes.join(",")) });
        },
    );
    let items = options.iter().map(|option| LookupItem::new(option.code.clone(), option.caption.clone()));
    Box::new(
        TagBox::new(chosen, items.collect())
            .fill()
            .label(request.caption.clone())
            .read_only(request.read_only),
    )
}

/// Text picked from values ready for it. What it held before stays among them.
fn predefined_editor(request: &EditorRequest) -> AnyView {
    let mut values = request.field.predefined.clone();
    if let Value::Text(held) = &*request.value.peek()
        && !values.contains(held)
    {
        values.push(held.clone());
    }
    let (read, write) = (request.value.clone(), request.value.clone());
    let known = values.clone();
    let at = Binding::new(
        move || if let Value::Text(held) = read.get() { known.iter().position(|v| *v == held) } else { None },
        {
            let values = values.clone();
            move |at: Option<usize>| {
                write.set(at.and_then(|at| values.get(at)).map_or(Value::Null, |v| Value::Text(v.clone())));
            }
        },
    );
    Box::new(Combobox::new(at, values).fill().label(request.caption.clone()).read_only(request.read_only))
}

/// Text typed through a mask.
fn masked_editor(request: &EditorRequest, mask: &str) -> AnyView {
    Box::new(
        MaskedField::new(text_binding(&request.value), mask.to_owned())
            .keep_literals()
            .label(request.caption.clone())
            .read_only(request.read_only),
    )
}

fn text_editor(request: &EditorRequest) -> AnyView {
    let field = TextField::new(text_binding(&request.value)).read_only(request.read_only);
    let field = if request.field.autofocus { field.autofocus() } else { field };
    let field = match on_leave(request) {
        Some(left) => field.on_leave(left),
        None => field,
    };
    match &request.field.placeholder {
        Some(text) => Box::new(field.placeholder(text.clone())),
        None => Box::new(field),
    }
}

/// A barcode number, saying as it is typed whether its check digit adds up.
struct GtinEditor {
    value: State<Value>,
    field: AnyView,
}

impl View for GtinEditor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let typed = match self.value.get() {
            Value::Text(text) => text,
            _ => String::new(),
        };
        let typed = typed.trim();
        let mut nodes = vec![cx.child(self.field.as_ref())];
        if !typed.is_empty() {
            let (words, tone) = if ravenrs_barcode::is_valid_gtin(typed) {
                let format = match typed.len() {
                    8 => "EAN-8",
                    12 => "UPC-A",
                    14 => "ITF-14",
                    _ => "EAN-13",
                };
                (format!("\u{2713} {format}"), Semantic::Success)
            } else {
                ("\u{2717} GTIN".to_owned(), Semantic::Danger)
            };
            nodes.push(cx.child(&Text::new(words).style(TextStyle::Callout).foreground(tone)));
        }
        cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &nodes,
            Paint::None,
        )
    }
}

fn gtin_editor(request: &EditorRequest) -> AnyView {
    Box::new(GtinEditor { value: request.value.clone(), field: text_editor(request) })
}

/// What shows a value with no control of its own: its text, which cannot be changed.
fn read_only_text(request: &EditorRequest) -> AnyView {
    let text = display(&request.field, &request.value.get());
    let decimal = request.session.words().decimal;
    // Only numbers with a format of their own are written here, with a point.
    let text = if request.field.display_format.is_some() && decimal != '.' {
        text.replace('.', &decimal.to_string())
    } else {
        text
    };
    Box::new(TextField::new(Binding::constant(text)).read_only(true))
}

/// A picture or a file on a form: taking one, and taking it out again.
struct FileEditor(EditorRequest);

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FileEditor {
    fn control(&self) -> AnyView {
        let request = &self.0;
        let session = request.session.clone();
        let store = session.attachment_store();
        let keep = {
            let (write, store) = (request.value.clone(), store.clone());
            let failed = session.clone();
            move |file: Option<&PickedFile>| match value_of_file(file, store.as_deref()) {
                Ok(value) => write.set(value),
                Err(error) => failed.set_error(Some(error)),
            }
        };
        let holding = {
            let (read, store) = (request.value.clone(), store);
            move || file_of(&read.get(), store.as_deref())
        };
        #[cfg(feature = "images")]
        if matches!(request.field.kind, FieldKind::Image) {
            let (read, write) = (holding, keep);
            let file = Binding::new(read, move |file: Option<PickedFile>| write(file.as_ref()));
            let mut field = ravenrs_widgets::ImageField::new(file)
                .label(request.caption.clone())
                .read_only(request.read_only);
            if let Some(bytes) = request.field.most_bytes {
                field = field.most_bytes(bytes);
            }
            return Box::new(field);
        }
        let (read, write) = (holding, keep);
        let files = Binding::new(
            move || read().into_iter().collect(),
            move |files: Vec<PickedFile>| write(files.first()),
        );
        let mut field =
            FileUpload::new(files).most_files(1).label(request.caption.clone()).read_only(request.read_only);
        if let Some(bytes) = request.field.most_bytes {
            field = field.most_bytes(bytes);
        }
        if !request.field.accepted.is_empty() {
            field = field.accept(request.field.accepted.clone());
        }
        Box::new(field)
    }
}

impl View for FileEditor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let request = &self.0;
        let session = request.session.clone();
        let held = request.value.get();
        let name = file_name(&held);
        let picker = cx.file_picker();
        let saving = name.map(|name| {
            let store = session.attachment_store();
            let words = session.words().save_file.clone();
            let session = session.clone();
            Button::new(format!("{words} {name}")).style(ButtonStyle::Ghost).size(ButtonSize::Small).on_click(
                move || {
                    if let Some(file) = file_of(&held, store.as_deref()) {
                        session.hand_file(file.name.clone(), file.bytes.to_vec(), &picker);
                    }
                },
            )
        });
        cx.child(&VStack::new((self.control(), saving)).spacing(8.0).align(Align::Start))
    }
}

/// What tells a session the keyboard left a field, when its form checks on leaving.
fn on_leave(request: &EditorRequest) -> Option<impl Fn() + 'static> {
    let checks = request.session.form_record().is_some_and(|record| {
        request.session.entity(record.entity()).is_some_and(|entity| entity.checks_on_leave)
    });
    checks.then(|| {
        let (session, field) = (request.session.clone(), request.field.name.clone());
        move || session.left_field(&field)
    })
}

/// A number field that says when it is left.
fn leaving_number(field: NumberField, request: &EditorRequest) -> NumberField {
    match on_leave(request) {
        Some(left) => field.on_leave(left),
        None => field,
    }
}

fn built_in(request: &EditorRequest) -> AnyView {
    let (value, read_only) = (&request.value, request.read_only);
    if request.field.calculated.is_some() {
        return read_only_text(request);
    }
    if request.read_only && request.field.display_format.is_some() {
        return read_only_text(request);
    }
    match &request.field.kind {
        FieldKind::Text if !request.field.predefined.is_empty() => predefined_editor(request),
        FieldKind::Text if request.field.edit_mask.is_some() => {
            masked_editor(request, request.field.edit_mask.as_deref().unwrap_or_default())
        }
        FieldKind::Text | FieldKind::Guid => text_editor(request),
        FieldKind::Criteria => criteria_editor(request),
        FieldKind::Tags(options) => tags_editor(request, options),
        FieldKind::LongText => Box::new(TextArea::new(text_binding(value)).read_only(read_only)),
        FieldKind::Integer => Box::new(leaving_number(
            NumberField::new(number_binding(value)).label(request.caption.clone()).read_only(read_only),
            request,
        )),
        FieldKind::Decimal(places) => Box::new(leaving_number(
            NumberField::new(number_binding(value))
                .decimals(usize::from(*places))
                .label(request.caption.clone())
                .read_only(read_only),
            request,
        )),
        FieldKind::Money => Box::new(leaving_number(
            NumberField::money(cents_binding(value), request.session.words().money_prefix.clone())
                .placeholder(request.field.placeholder.clone().unwrap_or_default())
                .label(request.caption.clone())
                .read_only(read_only),
            request,
        )),
        FieldKind::Date => {
            let (read, write) = (value.clone(), value.clone());
            let date = Binding::new(move || date_of(&read.get()), move |d| write.set(value_of_date(d)));
            Box::new(
                DatePicker::new(date, today(&request.session))
                    .fill()
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
        FieldKind::Image | FieldKind::File => Box::new(FileEditor(request.clone())),
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
