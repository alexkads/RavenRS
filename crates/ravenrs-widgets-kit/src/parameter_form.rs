//! A form made from a schema of parameters, for what a node, a connector or a credential asks:
//! text, numbers, yes or no, a choice, options the app loads (that may depend on another field),
//! named groups and lists of rows that grow and shrink. The values are JSON; the form edits a
//! copy and gives it back only on Aplicar, and Descartar forgets the copy whole.
//!
//! Made of the editors there are (`FormField`, `TextField`, `TextArea`, `NumberField`, `Select`,
//! `Toggle`, `SegmentedControl`, `Alert`), so it looks and reads like any other form.
//!
//! A field that takes expressions can hold one instead of a value: it is kept as an object
//! with the key [`EXPRESSION_KEY`], `{"$expr": "[valor] > 100"}`, and the field shows the mode
//! it is in.

use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs_layout::Align;
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};
use serde_json::{Map, Value};

use crate::context::{BuildCx, NodeId};
use crate::editors::{NumberField, TextArea};
use crate::{
    Alert, AnyView, Button, ButtonSize, ButtonStyle, FieldDrag, FormField, HStack, IconButton, IconName,
    SegmentedControl, Select, Spacer, Spinner, Text, TextField, Toggle, Tone, VStack, View,
};

/// The key of the object a field holds when it holds an expression instead of a value.
pub const EXPRESSION_KEY: &str = "$expr";

/// What a parameter holds, which picks its editor.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum ParamKind {
    /// A line of text.
    Text,
    /// Text of several lines.
    LongText,
    /// A number.
    Number,
    /// Yes or no.
    Boolean,
    /// One of these options, each its value and the words shown.
    Choice(Vec<(String, String)>),
    /// One of the options the app loads with [`ParameterForm::on_load`] and gives with
    /// [`ParameterForm::options`].
    Loaded,
    /// A named group of fields, kept as an object.
    Group(Vec<ParamField>),
    /// Rows of these fields that the person adds and removes, kept as a list of objects.
    List(Vec<ParamField>),
}

/// A parameter of a schema.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ParamField {
    /// Its key in the values.
    pub name: String,
    /// What the form calls it.
    pub label: String,
    /// What it holds.
    pub kind: ParamKind,
    /// Its value when the values have none.
    pub default: Option<Value>,
    /// Aplicar asks for a value.
    pub required: bool,
    /// A line under it.
    pub hint: Option<String>,
    /// Shown only while this other field has this value.
    pub shown_when: Option<(String, Value)>,
    /// The fields whose values its options depend on, for a [`ParamKind::Loaded`] field.
    pub depends_on: Vec<String>,
    /// It may hold an expression instead of a value.
    pub expressions: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ParamField {
    /// A parameter `name`, called `label`, holding `kind`.
    pub fn new(name: impl Into<String>, label: impl Into<String>, kind: ParamKind) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            kind,
            default: None,
            required: false,
            hint: None,
            shown_when: None,
            depends_on: Vec::new(),
            expressions: false,
        }
    }

    /// Its value when the values have none.
    pub fn default(mut self, value: impl Into<Value>) -> Self {
        self.default = Some(value.into());
        self
    }

    /// Aplicar asks for a value.
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// A line under it.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Shown only while the field `field` holds `value`.
    pub fn shown_when(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.shown_when = Some((field.into(), value.into()));
        self
    }

    /// Its options depend on these fields: when one changes, they are asked for again.
    pub fn depends_on(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.depends_on = fields.into_iter().map(Into::into).collect();
        self
    }

    /// It may hold an expression instead of a value.
    pub fn expressions(mut self) -> Self {
        self.expressions = true;
        self
    }
}

/// The options of a [`ParamKind::Loaded`] field, as the app has them now.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum ParamOptions {
    /// Being loaded.
    Loading,
    /// Loaded: each option's value and the words shown.
    Ready(Vec<(String, String)>),
    /// The load failed, with why.
    Failed(String),
}

/// The values of a [`ParameterForm`]: a JSON object, each field by its name.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ParamValues(pub Map<String, Value>);

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ParamValues {
    /// No values: each field starts at its default.
    pub fn new() -> Self {
        Self::default()
    }

    /// The value of the field `name`.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.get(name)
    }

    /// The values as the JSON object they are.
    pub fn into_map(self) -> Map<String, Value> {
        self.0
    }
}

impl From<Map<String, Value>> for ParamValues {
    fn from(map: Map<String, Value>) -> Self {
        Self(map)
    }
}

/// Takes the expression of a field of data carried to a field of the form.
type Take = Rc<dyn Fn(&str)>;

/// A field of the form as a place a field of data can be carried to: text takes it as an
/// expression, written where the value ends; anything else refuses it, saying why.
struct Takes {
    inner: AnyView,
    take: Option<Take>,
    refused: String,
}

impl View for Takes {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let node = cx.child(&*self.inner);
        let refused = self.refused.clone();
        match self.take.clone() {
            Some(take) => cx.drop_target(
                node,
                move |p| if FieldDrag::carried(p).is_some() { Ok(()) } else { Err(refused.clone()) },
                move |p, _| {
                    if let Some(field) = FieldDrag::carried(p) {
                        take(&field.expression);
                    }
                },
            ),
            None => cx.drop_target(node, move |_| Err(refused.clone()), |_, _| {}),
        }
        node
    }
}

/// One step into the values: a key of an object or a row of a list.
#[derive(Clone, Debug, PartialEq)]
enum Step {
    Key(String),
    Row(usize),
}

fn read(root: &Value, path: &[Step]) -> Value {
    let mut at = root;
    for step in path {
        let next = match step {
            Step::Key(k) => at.get(k.as_str()),
            Step::Row(i) => at.get(*i),
        };
        match next {
            Some(v) => at = v,
            None => return Value::Null,
        }
    }
    at.clone()
}

fn write(root: &mut Value, path: &[Step], value: Value) {
    let Some((last, before)) = path.split_last() else {
        *root = value;
        return;
    };
    let mut at = root;
    for step in before {
        at = match step {
            Step::Key(k) => {
                if !at.is_object() {
                    *at = Value::Object(Map::new());
                }
                at.as_object_mut()
                    .map(|m| m.entry(k.clone()).or_insert(Value::Null))
                    .unwrap_or_else(|| unreachable!())
            }
            Step::Row(i) => {
                if !at.is_array() {
                    *at = Value::Array(Vec::new());
                }
                let Some(list) = at.as_array_mut() else { return };
                while list.len() <= *i {
                    list.push(Value::Object(Map::new()));
                }
                &mut list[*i]
            }
        };
    }
    match last {
        Step::Key(k) => {
            if !at.is_object() {
                *at = Value::Object(Map::new());
            }
            if let Some(m) = at.as_object_mut() {
                m.insert(k.clone(), value);
            }
        }
        Step::Row(i) => {
            if let Some(list) = at.as_array_mut()
                && *i < list.len()
            {
                list[*i] = value;
            }
        }
    }
}

/// `values` with the default of each field it lacks, in groups too.
fn with_defaults(fields: &[ParamField], mut values: Map<String, Value>) -> Map<String, Value> {
    for field in fields {
        let missing = values.get(&field.name).is_none_or(Value::is_null);
        match &field.kind {
            ParamKind::Group(inner) => {
                let own = values.get(&field.name).and_then(Value::as_object).cloned().unwrap_or_default();
                values.insert(field.name.clone(), Value::Object(with_defaults(inner, own)));
            }
            _ if missing => {
                if let Some(default) = &field.default {
                    values.insert(field.name.clone(), default.clone());
                }
            }
            _ => {}
        }
    }
    values
}

/// Whether a value says nothing: null, empty text or an empty list.
fn empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

/// Whether `field` shows, in the object `here` it belongs to.
fn shown(field: &ParamField, here: &Value) -> bool {
    match &field.shown_when {
        Some((other, value)) => here.get(other.as_str()) == Some(value),
        None => true,
    }
}

/// What is missing among the required fields that show, in `here` and its groups and rows.
fn missing(fields: &[ParamField], here: &Value, words: &str, out: &mut Vec<String>) {
    for field in fields.iter().filter(|f| shown(f, here)) {
        let value = here.get(field.name.as_str()).cloned().unwrap_or(Value::Null);
        match &field.kind {
            ParamKind::Group(inner) => missing(inner, &value, words, out),
            ParamKind::List(inner) => {
                for row in value.as_array().into_iter().flatten() {
                    missing(inner, row, words, out);
                }
            }
            _ if field.required && empty(&value) => out.push(words.replace("{0}", &field.label)),
            _ => {}
        }
    }
}

/// Asks for the options of a field, with the values being edited.
type Load = Rc<dyn Fn(&str, &ParamValues)>;

/// Hears the values given back.
type Apply = Rc<dyn Fn(&ParamValues)>;

/// A form made from a schema of parameters, editing a copy of `values` that Aplicar gives back
/// and Descartar forgets.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ParameterForm {
    fields: Vec<ParamField>,
    values: Binding<ParamValues>,
    options: BTreeMap<String, ParamOptions>,
    on_load: Option<Load>,
    on_apply: Option<Apply>,
    on_discard: Option<Rc<dyn Fn()>>,
}

impl std::fmt::Debug for ParameterForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParameterForm").field("fields", &self.fields.len()).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ParameterForm {
    /// A form of `fields` over `values`. The fields' defaults fill what the values lack.
    pub fn new(fields: Vec<ParamField>, values: Binding<ParamValues>) -> Self {
        Self { fields, values, options: BTreeMap::new(), on_load: None, on_apply: None, on_discard: None }
    }

    /// The options the app has now for the [`ParamKind::Loaded`] field `field`; with none, it
    /// shows it is loading. A choice that is no longer among them is cleared.
    pub fn options(mut self, field: impl Into<String>, options: ParamOptions) -> Self {
        self.options.insert(field.into(), options);
        self
    }

    /// Asks the app for the options of a [`ParamKind::Loaded`] field, with the values being
    /// edited: the first time it shows, when a field it depends on changes, and on Tentar de
    /// novo after a failure.
    pub fn on_load(mut self, handler: impl Fn(&str, &ParamValues) + 'static) -> Self {
        self.on_load = Some(Rc::new(handler));
        self
    }

    /// Hears the values given back by Aplicar, which also sets them on the binding.
    pub fn on_apply(mut self, handler: impl Fn(&ParamValues) + 'static) -> Self {
        self.on_apply = Some(Rc::new(handler));
        self
    }

    /// Hears Descartar, after the copy went back to the values.
    pub fn on_discard(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_discard = Some(Rc::new(handler));
        self
    }
}

/// What the parts of the form share while it builds.
struct Parts {
    draft: State<Value>,
    options: BTreeMap<String, ParamOptions>,
    on_load: Option<Load>,
    labels: Rc<crate::Labels>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Parts {
    fn binding(&self, path: Vec<Step>) -> Binding<Value> {
        let (a, b, p) = (self.draft.clone(), self.draft.clone(), path.clone());
        Binding::new(move || read(&a.get(), &path), move |v| b.update(|root| write(root, &p, v)))
    }

    fn draft_map(&self) -> Map<String, Value> {
        self.draft.get().as_object().cloned().unwrap_or_default()
    }

    /// The editor of `field` at `path`, inside the object `here`.
    fn field(&self, field: &ParamField, path: Vec<Step>) -> AnyView {
        let value = self.binding(path.clone());
        let now = value.get();
        let is_expression = now.get(EXPRESSION_KEY).is_some();
        let editor: AnyView = if is_expression {
            let text = value.clone().map(
                |v| v.get(EXPRESSION_KEY).and_then(Value::as_str).unwrap_or_default().to_owned(),
                |v, text: String| *v = serde_json::json!({ EXPRESSION_KEY: text }),
            );
            Box::new(TextField::new(text).placeholder(self.labels.param_expression.clone()))
        } else {
            self.editor(field, value.clone(), &path)
        };
        // A field of data carried here: an expression takes its expression at the end, text
        // takes it as `{{ … }}`; other kinds refuse it.
        let take: Option<Take> = match (&field.kind, is_expression) {
            (ParamKind::Group(_) | ParamKind::List(_), _) => None,
            (_, true) => {
                let value = value.clone();
                Some(Rc::new(move |expression: &str| {
                    value.update(|v| {
                        let before =
                            v.get(EXPRESSION_KEY).and_then(Value::as_str).unwrap_or_default().to_owned();
                        let joined = if before.trim().is_empty() {
                            expression.to_owned()
                        } else {
                            format!("{before} {expression}")
                        };
                        *v = serde_json::json!({ EXPRESSION_KEY: joined });
                    });
                }))
            }
            (ParamKind::Text | ParamKind::LongText, false) => {
                let value = value.clone();
                Some(Rc::new(move |expression: &str| {
                    value.update(|v| {
                        let before = v.as_str().unwrap_or_default().to_owned();
                        *v = Value::String(format!("{before}{{{{ {expression} }}}}"));
                    });
                }))
            }
            _ => None,
        };
        let editor: AnyView = match &field.kind {
            ParamKind::Group(_) | ParamKind::List(_) => editor,
            _ => Box::new(Takes { inner: editor, take, refused: self.labels.param_no_field.clone() }),
        };
        let wrapped: AnyView = match &field.kind {
            ParamKind::Group(_) | ParamKind::List(_) => return editor,
            // A switch carries its own label beside it, so a label above would say it twice.
            ParamKind::Boolean if !is_expression => match &field.hint {
                Some(hint) => Box::new(
                    VStack::new((
                        editor,
                        Text::new(hint.clone()).style(TextStyle::Caption).foreground(Semantic::InkMuted),
                    ))
                    .spacing(4.0)
                    .align(Align::Start),
                ),
                None => editor,
            },
            _ => {
                let mut wrapped = FormField::new(field.label.clone(), editor);
                if field.required {
                    wrapped = wrapped.required();
                }
                if let Some(hint) = &field.hint {
                    wrapped = wrapped.hint(hint.clone());
                }
                Box::new(wrapped)
            }
        };
        if !field.expressions {
            return wrapped;
        }
        // The switch between value and expression sits above the field, outside it, so the
        // field's label still names the field.
        let mode = value.map(
            |v| usize::from(v.get(EXPRESSION_KEY).is_some()),
            |v, mode: usize| match (mode, v.get(EXPRESSION_KEY).is_some()) {
                // The value written becomes the expression's first words.
                (1, false) => {
                    let text = match &*v {
                        Value::Null => String::new(),
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    *v = serde_json::json!({ EXPRESSION_KEY: text });
                }
                (0, true) => *v = Value::Null,
                _ => {}
            },
        );
        Box::new(
            VStack::new((
                HStack::new((
                    Spacer::new(),
                    SegmentedControl::new(
                        mode,
                        [self.labels.param_fixed.clone(), self.labels.param_expression.clone()],
                    )
                    .label(format!("{}: {}", field.label, self.labels.param_mode)),
                )),
                wrapped,
            ))
            .spacing(4.0)
            .align(Align::Stretch),
        )
    }

    /// The editor of a value of `field`'s kind.
    fn editor(&self, field: &ParamField, value: Binding<Value>, path: &[Step]) -> AnyView {
        match &field.kind {
            ParamKind::Text => Box::new(TextField::new(text_of(value))),
            ParamKind::LongText => Box::new(TextArea::new(text_of(value))),
            ParamKind::Number => Box::new(NumberField::new(value.map(
                |v| v.as_f64(),
                |v, n: Option<f64>| {
                    *v = n.and_then(serde_json::Number::from_f64).map_or(Value::Null, Value::Number)
                },
            ))),
            ParamKind::Boolean => Box::new(
                Toggle::new(value.map(|v| v.as_bool().unwrap_or(false), |v, b: bool| *v = Value::Bool(b)))
                    .label(field.label.clone()),
            ),
            ParamKind::Choice(options) => self.choice(field, value, options),
            ParamKind::Loaded => match self.options.get(&field.name) {
                Some(ParamOptions::Ready(options)) => self.choice(field, value, options),
                Some(ParamOptions::Failed(why)) => {
                    let (load, name, draft) = (self.on_load.clone(), field.name.clone(), self.draft.clone());
                    Box::new(
                        Alert::new(self.labels.param_load_failed.clone(), why.clone())
                            .tone(Tone::Danger)
                            .action(self.labels.param_retry.clone(), move || {
                                if let Some(load) = &load {
                                    load(
                                        &name,
                                        &ParamValues(draft.get().as_object().cloned().unwrap_or_default()),
                                    );
                                }
                            }),
                    )
                }
                _ => Box::new(
                    HStack::new((
                        Spinner::new(),
                        Text::new(self.labels.param_loading.clone())
                            .style(TextStyle::Callout)
                            .foreground(Semantic::InkMuted),
                    ))
                    .spacing(8.0)
                    .align(Align::Center),
                ),
            },
            ParamKind::Group(inner) => {
                let rows: Vec<AnyView> = inner
                    .iter()
                    .filter(|f| shown(f, &read(&self.draft.get(), path)))
                    .map(|f| self.field(f, [path, &[Step::Key(f.name.clone())]].concat()))
                    .collect();
                Box::new(
                    VStack::new((
                        Text::new(field.label.clone()).style(TextStyle::Headline),
                        VStack::new(rows).spacing(10.0).align(Align::Stretch),
                    ))
                    .spacing(8.0)
                    .align(Align::Stretch),
                )
            }
            ParamKind::List(inner) => self.list(field, inner, value, path),
        }
    }

    fn choice(&self, field: &ParamField, value: Binding<Value>, options: &[(String, String)]) -> AnyView {
        let values: Vec<String> = options.iter().map(|(v, _)| v.clone()).collect();
        let words: Vec<String> = std::iter::once(self.labels.param_none.clone())
            .chain(options.iter().map(|(_, w)| w.clone()))
            .collect();
        let (find, keep) = (values.clone(), values);
        let selected = value.map(
            move |v| v.as_str().and_then(|s| find.iter().position(|x| x == s)).map_or(0, |i| i + 1),
            move |v, i: usize| {
                *v = if i == 0 {
                    Value::Null
                } else {
                    keep.get(i - 1).cloned().map_or(Value::Null, Value::String)
                }
            },
        );
        Box::new(Select::new(selected, words).label(field.label.clone()))
    }

    fn list(
        &self,
        field: &ParamField,
        inner: &[ParamField],
        value: Binding<Value>,
        path: &[Step],
    ) -> AnyView {
        let rows = value.get().as_array().map_or(0, Vec::len);
        let mut views: Vec<AnyView> = Vec::new();
        for i in 0..rows {
            let row: Vec<Step> = [path, &[Step::Row(i)]].concat();
            let cells: Vec<AnyView> = inner
                .iter()
                .map(|f| -> AnyView {
                    Box::new(
                        VStack::new((self.field(f, [&row[..], &[Step::Key(f.name.clone())]].concat()),))
                            .grow(1.0),
                    )
                })
                .collect();
            let gone = value.clone();
            views.push(Box::new(
                HStack::new((
                    HStack::new(cells).spacing(8.0).align(Align::End).grow(1.0),
                    IconButton::new(IconName::Trash2)
                        .label(self.labels.param_remove.replace("{0}", &(i + 1).to_string()))
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Small)
                        .on_click(move || {
                            gone.update(|v| {
                                if let Some(list) = v.as_array_mut()
                                    && i < list.len()
                                {
                                    list.remove(i);
                                }
                            });
                        }),
                ))
                .spacing(8.0)
                .align(Align::End),
            ));
        }
        let (more, fresh) = (value, Value::Object(with_defaults(inner, Map::new())));
        Box::new(
            VStack::new((
                Text::new(field.label.clone()).style(TextStyle::Headline),
                VStack::new(views).spacing(8.0).align(Align::Stretch),
                Button::new(self.labels.param_add.clone())
                    .icon(IconName::Plus)
                    .style(ButtonStyle::Secondary)
                    .size(ButtonSize::Small)
                    .on_click(move || {
                        let row = fresh.clone();
                        more.update(|v| match v.as_array_mut() {
                            Some(list) => list.push(row),
                            None => *v = Value::Array(vec![row]),
                        });
                    }),
            ))
            .spacing(8.0)
            .align(Align::Start),
        )
    }
}

/// A value as text: text as it is, a number written, nothing as empty.
fn text_of(value: Binding<Value>) -> Binding<String> {
    value.map(
        |v| match v {
            Value::String(s) => s.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        },
        |v, s: String| *v = Value::String(s),
    )
}

/// The fields, in groups too, that load their options, with the path of the object they sit in.
fn loaded<'a>(fields: &'a [ParamField], out: &mut Vec<&'a ParamField>) {
    for field in fields {
        match &field.kind {
            ParamKind::Loaded => out.push(field),
            ParamKind::Group(inner) => loaded(inner, out),
            _ => {}
        }
    }
}

impl View for ParameterForm {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let committed = with_defaults(&self.fields, self.values.get().0);
        let draft = cx.use_state(|| Value::Object(committed.clone()));
        let seen = cx.use_state(|| committed.clone());
        let errors = cx.use_state(Vec::<String>::new);
        let asked = cx.use_state(BTreeMap::<String, Value>::new);
        // Values changed from outside, as another node chosen: the copy starts again from them.
        if seen.get() != committed {
            seen.set(committed.clone());
            draft.set(Value::Object(committed.clone()));
            errors.set(Vec::new());
        }
        let now = draft.get();
        let mut with_options = Vec::new();
        loaded(&self.fields, &mut with_options);
        for field in with_options {
            // What its options depend on now: asked again when it changes.
            let key = Value::Array(
                field
                    .depends_on
                    .iter()
                    .map(|d| now.get(d.as_str()).cloned().unwrap_or(Value::Null))
                    .collect(),
            );
            if asked.get().get(&field.name) != Some(&key) {
                asked.update(|a| {
                    a.insert(field.name.clone(), key.clone());
                });
                if let Some(load) = &self.on_load {
                    load(&field.name, &ParamValues(now.as_object().cloned().unwrap_or_default()));
                }
            }
            // A choice the options no longer have is cleared.
            if let Some(ParamOptions::Ready(options)) = self.options.get(&field.name)
                && let Some(chosen) = now.get(field.name.as_str()).and_then(Value::as_str)
                && !options.iter().any(|(v, _)| v == chosen)
            {
                draft.update(|d| write(d, &[Step::Key(field.name.clone())], Value::Null));
            }
        }

        let parts = Parts {
            draft: draft.clone(),
            options: self.options.clone(),
            on_load: self.on_load.clone(),
            labels: labels.clone(),
        };
        let now = draft.get();
        let rows: Vec<AnyView> = self
            .fields
            .iter()
            .filter(|f| shown(f, &now))
            .map(|f| parts.field(f, vec![Step::Key(f.name.clone())]))
            .collect();
        let wrong: Vec<AnyView> = errors
            .get()
            .into_iter()
            .map(|e| -> AnyView {
                Box::new(Text::new(e).style(TextStyle::Callout).foreground(Semantic::Danger))
            })
            .collect();
        let changed = parts.draft_map() != committed;

        let (fields, values, apply_draft, apply_errors, on_apply) =
            (self.fields.clone(), self.values.clone(), draft.clone(), errors.clone(), self.on_apply.clone());
        let required = labels.param_required.clone();
        let apply = move || {
            let mut wrong = Vec::new();
            let now = apply_draft.get();
            missing(&fields, &now, &required, &mut wrong);
            if !wrong.is_empty() {
                apply_errors.set(wrong);
                return;
            }
            apply_errors.set(Vec::new());
            let given = ParamValues(now.as_object().cloned().unwrap_or_default());
            values.set(given.clone());
            if let Some(on_apply) = &on_apply {
                on_apply(&given);
            }
        };
        let (back, back_errors, back_values, on_discard, back_fields) = (
            draft.clone(),
            errors.clone(),
            self.values.clone(),
            self.on_discard.clone(),
            self.fields.clone(),
        );
        let discard = move || {
            back.set(Value::Object(with_defaults(&back_fields, back_values.get().0)));
            back_errors.set(Vec::new());
            if let Some(on_discard) = &on_discard {
                on_discard();
            }
        };
        cx.child(
            &VStack::new((
                VStack::new(rows).spacing(12.0).align(Align::Stretch),
                VStack::new(wrong).spacing(4.0),
                HStack::new((
                    Spacer::new(),
                    Button::new(labels.param_discard.clone())
                        .style(ButtonStyle::Ghost)
                        .disabled(!changed)
                        .on_click(discard),
                    Button::new(labels.param_apply.clone())
                        .icon(IconName::Check)
                        .disabled(!changed)
                        .on_click(apply),
                ))
                .spacing(8.0)
                .align(Align::Center),
            ))
            .spacing(14.0)
            .align(Align::Stretch),
        )
    }
}
