//! What a record is: its fields, what they hold and where they show.

use std::collections::BTreeMap;
use std::fmt;

use ravenrs_criteria::{Criteria, Environment, FieldType, Record, Schema, Scope, Value};

/// What a field holds, which decides how it is edited and stored.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum FieldKind {
    /// A line of text.
    Text,
    /// Text of any length, edited in several lines.
    LongText,
    /// A whole number.
    Integer,
    /// A number, with these decimal places.
    Decimal(u8),
    /// Money, in whole cents, so it is always exact.
    Money,
    /// A date.
    Date,
    /// A date and a time.
    DateTime,
    /// Yes or no.
    Boolean,
    /// One of these options.
    Enum(Vec<EnumOption>),
    /// Another record, by the name of its entity. It holds that record's key.
    Reference(String),
    /// The records of another entity that point to this one.
    Collection(String),
    /// A color, as `#RRGGBB`.
    Color,
    /// A picture.
    Image,
    /// A file.
    File,
    /// An expression of the criteria language.
    Criteria,
    /// A key made up when a record is created, the same everywhere: no two machines repeat one.
    Guid,
    /// Several of these options at once, kept as their codes separated by commas.
    Tags(Vec<EnumOption>),
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FieldKind {
    /// How expressions see it.
    fn field_type(&self) -> FieldType {
        match self {
            Self::Text
            | Self::LongText
            | Self::Color
            | Self::Criteria
            | Self::Enum(_)
            | Self::Guid
            | Self::Tags(_) => FieldType::Text,
            Self::Integer | Self::Decimal(_) | Self::Money => FieldType::Number,
            Self::Date | Self::DateTime => FieldType::Date,
            Self::Boolean => FieldType::Boolean,
            Self::Reference(_) | Self::Collection(_) | Self::Image | Self::File => FieldType::Any,
        }
    }

    /// Whether its editor takes the height it needs, instead of one line.
    pub(crate) fn is_tall(&self) -> bool {
        matches!(self, Self::LongText | Self::Image | Self::File | Self::Criteria)
    }

    /// Whether it lists other records instead of holding a value.
    pub(crate) fn is_collection(&self) -> bool {
        matches!(self, Self::Collection(_))
    }

    /// Its name in the model.
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::LongText => "LongText",
            Self::Integer => "Integer",
            Self::Decimal(_) => "Decimal",
            Self::Money => "Money",
            Self::Date => "Date",
            Self::DateTime => "DateTime",
            Self::Boolean => "Boolean",
            Self::Enum(_) => "Enum",
            Self::Reference(_) => "Reference",
            Self::Collection(_) => "Collection",
            Self::Color => "Color",
            Self::Image => "Image",
            Self::File => "File",
            Self::Criteria => "Criteria",
            Self::Guid => "Guid",
            Self::Tags(_) => "Tags",
        }
    }
}

/// One of the values of a [`FieldKind::Enum`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct EnumOption {
    /// What is stored.
    pub code: String,
    /// What people see.
    pub caption: String,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl EnumOption {
    /// An option stored as `code` and shown as `caption`.
    pub fn new(code: impl Into<String>, caption: impl Into<String>) -> Self {
        Self { code: code.into(), caption: caption.into() }
    }
}

/// A field of an [`EntityInfo`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FieldInfo {
    /// Its name, in expressions and in the model.
    pub name: String,
    /// What people see. Follows from the name unless given.
    pub caption: String,
    /// What it holds.
    pub kind: FieldKind,
    /// It identifies the record.
    pub is_key: bool,
    /// It can be seen but not changed.
    pub read_only: bool,
    /// It has a column in lists.
    pub in_list: bool,
    /// It is on the form.
    pub in_detail: bool,
    /// It has a column when picking a record of this entity.
    pub in_lookup: bool,
    /// An expression that gives its value; nothing is stored.
    pub calculated: Option<String>,
    /// What a new record starts with.
    pub default_value: Value,
    /// The control that edits it, by name, instead of the one for its kind.
    pub editor: Option<String>,
    /// What an empty editor shows.
    pub placeholder: Option<String>,
    /// It takes the whole width of its group on a form.
    pub wide: bool,
    /// Takes the keyboard when its form opens.
    pub autofocus: bool,
    /// What the form says under it.
    pub hint: Option<String>,
    /// For a reference: what its records have to meet, where `?field` is the value of that field
    /// of the record being edited.
    pub lookup_filter: Option<String>,
    /// For a reference that can be left empty and is picked from a list: what empty is called.
    pub none_caption: Option<String>,
    /// For a collection: the field of the other entity that points back here, when it has
    /// more than one reference to this entity.
    pub back_reference: Option<String>,
    /// It stays behind when a record is cloned.
    pub non_cloneable: bool,
    /// For a collection: its records belong to this one, and are cloned with it.
    pub aggregated: bool,
    /// For a picture or a file: the largest taken, in bytes.
    pub most_bytes: Option<u64>,
    /// For a file: the extensions offered; none means any.
    pub accepted: Vec<String>,
    /// The titled group it goes in on a form, away from the other fields.
    pub group: Option<String>,
    /// How its value shows when it is not being edited: `{}` is the value, `{:.2}` a number with
    /// these decimals, and `{:dd/MM/yyyy HH:mm}` a date.
    pub display_format: Option<String>,
    /// For text: the mask it is typed through, where `0` is a digit, `A` a letter and `*` anything.
    pub edit_mask: Option<String>,
    /// For text: the values offered to pick from.
    pub predefined: Vec<String>,
    /// For an expression: the entity whose fields it can be about, which gives it a filter builder.
    pub criteria_of: Option<String>,
    /// It never leaves storage through a secured space, for anyone, and no screen shows it.
    pub secret: bool,
    /// What screen readers call it, when the caption is not enough. The caption by default.
    pub accessible_name: Option<String>,
    /// What screen readers say after its name, such as what goes in it. The hint by default.
    pub accessible_description: Option<String>,
}

/// `opening_cents` as `Opening cents`.
fn caption_of(name: &str) -> String {
    let spaced = name.replace('_', " ");
    let mut chars = spaced.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FieldInfo {
    /// A field shown in lists and on the form.
    pub fn new(name: impl Into<String>, kind: FieldKind) -> Self {
        let name = name.into();
        // A list of options always shows one, so a new record has to hold it.
        let default_value = match &kind {
            FieldKind::Enum(options) => options.first().map_or(Value::Null, |o| Value::Text(o.code.clone())),
            _ => Value::Null,
        };
        Self {
            caption: caption_of(&name),
            name,
            in_list: !kind.is_tall() && !kind.is_collection(),
            in_detail: true,
            in_lookup: false,
            is_key: false,
            read_only: false,
            calculated: None,
            default_value,
            editor: None,
            placeholder: None,
            wide: false,
            autofocus: false,
            hint: None,
            lookup_filter: None,
            none_caption: None,
            back_reference: None,
            non_cloneable: false,
            aggregated: false,
            most_bytes: None,
            accepted: Vec::new(),
            group: None,
            display_format: None,
            edit_mask: None,
            predefined: Vec::new(),
            criteria_of: None,
            secret: false,
            accessible_name: None,
            accessible_description: None,
            kind,
        }
    }

    /// What people see.
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }

    /// Makes it the key: read-only, and out of lists and forms unless told otherwise.
    pub fn key(mut self) -> Self {
        self.is_key = true;
        self.read_only = true;
        self.in_list = false;
        self.in_detail = false;
        self
    }

    /// Makes it the key, typed by people, as a code is.
    pub fn typed_key(mut self) -> Self {
        self.is_key = true;
        self
    }

    /// Makes it what names a record, for [`EntityInfo::default_property`].
    pub fn default_property(mut self) -> Self {
        self.in_lookup = true;
        self
    }

    /// It can be seen but not changed.
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    /// Whether it has a column in lists.
    pub fn in_list(mut self, show: bool) -> Self {
        self.in_list = show;
        self
    }

    /// Whether it is on the form.
    pub fn in_detail(mut self, show: bool) -> Self {
        self.in_detail = show;
        self
    }

    /// Whether it has a column when picking a record.
    pub fn in_lookup(mut self, show: bool) -> Self {
        self.in_lookup = show;
        self
    }

    /// Gets its value from this expression, read-only and not stored.
    pub fn calculated(mut self, expression: impl Into<String>) -> Self {
        self.calculated = Some(expression.into());
        self.read_only = true;
        self
    }

    /// The control that edits it, by name: `Segmented` for options side by side, or one
    /// registered in an editor registry.
    pub fn editor(mut self, editor: impl Into<String>) -> Self {
        self.editor = Some(editor.into());
        self
    }

    /// What an empty editor shows.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Takes the whole width of its group on a form.
    pub fn wide(mut self) -> Self {
        self.wide = true;
        self
    }

    /// Takes the keyboard when its form opens.
    pub fn autofocus(mut self) -> Self {
        self.autofocus = true;
        self
    }

    /// What the form says under it.
    pub fn hint(mut self, text: impl Into<String>) -> Self {
        self.hint = Some(text.into());
        self
    }

    /// What screen readers call it, instead of its caption: for a caption that only makes sense
    /// next to what is around it, such as `Até` for the end of a period.
    pub fn accessible_name(mut self, name: impl Into<String>) -> Self {
        self.accessible_name = Some(name.into());
        self
    }

    /// What screen readers say after its name, instead of its hint.
    pub fn accessible_description(mut self, text: impl Into<String>) -> Self {
        self.accessible_description = Some(text.into());
        self
    }

    /// For a reference: what its records have to meet, where `?field` is the value of that field
    /// of the record being edited, as in `[flow] = ?flow`.
    pub fn lookup_filter(mut self, expression: impl Into<String>) -> Self {
        self.lookup_filter = Some(expression.into());
        self
    }

    /// For a reference that can be left empty and is picked from a list: what empty is called.
    pub fn none_caption(mut self, caption: impl Into<String>) -> Self {
        self.none_caption = Some(caption.into());
        self
    }

    /// For a collection: the field of the other entity that points back here.
    pub fn back_reference(mut self, field: impl Into<String>) -> Self {
        self.back_reference = Some(field.into());
        self
    }

    /// Stays behind when a record is cloned.
    pub fn non_cloneable(mut self) -> Self {
        self.non_cloneable = true;
        self
    }

    /// For a collection: its records belong to this one, and are cloned with it.
    pub fn aggregated(mut self) -> Self {
        self.aggregated = true;
        self
    }

    /// For a picture or a file: the largest taken, in bytes.
    pub fn most_bytes(mut self, bytes: u64) -> Self {
        self.most_bytes = Some(bytes);
        self
    }

    /// For a file: the extensions offered, such as `pdf`.
    pub fn accepted(mut self, extensions: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.accepted = extensions.into_iter().map(Into::into).collect();
        self
    }

    /// What a new record starts with.
    pub fn default_value(mut self, value: impl Into<Value>) -> Self {
        self.default_value = value.into();
        self
    }

    /// Goes in the group with this title on a form, as the parts of one thing do.
    pub fn group(mut self, title: impl Into<String>) -> Self {
        self.group = Some(title.into());
        self
    }

    /// How its value shows when it is not being edited: `{}` is the value, `{:.2}` a number with
    /// these decimals, and `{:dd/MM/yyyy HH:mm}` a date.
    pub fn display_format(mut self, format: impl Into<String>) -> Self {
        self.display_format = Some(format.into());
        self
    }

    /// For text: the mask it is typed through, where `0` is a digit, `A` a letter and `*` anything.
    pub fn edit_mask(mut self, mask: impl Into<String>) -> Self {
        self.edit_mask = Some(mask.into());
        self
    }

    /// For text: the values offered to pick from.
    pub fn predefined(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.predefined = values.into_iter().map(Into::into).collect();
        self
    }

    /// Keeps it from everyone: a secured space never gives it out or takes it in, and no screen
    /// shows it, as with what is kept for a password.
    pub fn secret(mut self) -> Self {
        self.secret = true;
        self.in_list = false;
        self.in_detail = false;
        self.in_lookup = false;
        self
    }

    /// For an expression: the entity whose fields it can be about, which gives it a filter builder.
    pub fn criteria_of(mut self, entity: impl Into<String>) -> Self {
        self.criteria_of = Some(entity.into());
        self
    }

    /// A field added in the model, under an entity's `Members`: `Kind` is that of a field with
    /// plain values, with `Caption`, `Places` for decimals, `Options` for a list of options and
    /// `Expression` for what is worked out and not stored.
    fn from_model(entity: &str, node: &crate::ModelNode) -> Result<Self, ModelError> {
        let wrong = |what: String| ModelError(format!("{entity}: the field {} {what}", node.id));
        let options = || {
            node.get("Options").map_or_else(Vec::new, |all| {
                all.in_order()
                    .iter()
                    .map(|option| EnumOption::new(&option.id, option.text("Caption").unwrap_or(&option.id)))
                    .collect()
            })
        };
        let kind = match node.text("Kind").unwrap_or("Text") {
            "Text" => FieldKind::Text,
            "LongText" => FieldKind::LongText,
            "Integer" => FieldKind::Integer,
            "Decimal" => FieldKind::Decimal(node.integer("Places").unwrap_or(2).clamp(0, 12) as u8),
            "Money" => FieldKind::Money,
            "Date" => FieldKind::Date,
            "DateTime" => FieldKind::DateTime,
            "Boolean" => FieldKind::Boolean,
            "Color" => FieldKind::Color,
            "Enum" => FieldKind::Enum(options()),
            "Tags" => FieldKind::Tags(options()),
            other => return Err(wrong(format!("cannot be added in the model as {other}"))),
        };
        let mut field = Self::new(&node.id, kind);
        if let Some(caption) = node.text("Caption") {
            field.caption = caption.to_owned();
        }
        if let Some(expression) = node.text("Expression") {
            field = field.calculated(expression);
        }
        Ok(field)
    }

    /// Whether the model added it: it comes with what it holds, which a declared one does not need.
    pub(crate) fn is_custom(node: &crate::ModelNode, declared: &EntityInfo) -> bool {
        declared.field_named(&node.id).is_none()
    }
}

/// How a list gets its records.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum DataAccess {
    /// All of them at once, kept in memory, so grouping and every search work on the spot.
    #[default]
    Client,
    /// As they are needed, a chunk at a time, for lists too long to read at once.
    Server,
    /// All of them at once, but only what the columns show, and with no editing on the list.
    DataView,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DataAccess {
    /// Its name in the model.
    pub fn name(self) -> &'static str {
        match self {
            Self::Client => "Client",
            Self::Server => "Server",
            Self::DataView => "DataView",
        }
    }

    /// From its name in the model; anything else is the usual.
    pub fn from_name(name: &str) -> Self {
        match name {
            "Server" => Self::Server,
            "DataView" => Self::DataView,
            _ => Self::Client,
        }
    }
}

/// Something wrong in how the application was declared, in words for its developer.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ModelError(pub String);

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ModelError {}

/// Another way to see the list of an entity, which people pick from a menu once there are two.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ViewVariant {
    /// What it is known by.
    pub id: String,
    /// What people see.
    pub caption: String,
    /// What its records have to meet; nothing means all of them.
    pub filter: Option<String>,
    /// The fields its columns show, in order; nothing means those of the list.
    pub columns: Vec<String>,
    /// The way it shows its records, by the name of a list editor; nothing means what the list uses.
    pub editor: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ViewVariant {
    /// A variant that shows every record.
    pub fn new(id: impl Into<String>, caption: impl Into<String>) -> Self {
        Self { id: id.into(), caption: caption.into(), filter: None, columns: Vec::new(), editor: None }
    }

    /// Shows only the records that meet this expression.
    pub fn filter(mut self, expression: impl Into<String>) -> Self {
        self.filter = Some(expression.into());
        self
    }

    /// Shows its records with the list editor of this name.
    pub fn editor(mut self, editor: impl Into<String>) -> Self {
        self.editor = Some(editor.into());
        self
    }

    /// Shows these fields as its columns, in this order.
    pub fn columns(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.columns = fields.into_iter().map(Into::into).collect();
        self
    }
}

/// A filter ready for people to pick from a menu on a list.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ListFilter {
    /// What it is known by.
    pub id: String,
    /// What people see.
    pub caption: String,
    /// What its records have to meet; nothing means all of them.
    pub criteria: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ListFilter {
    /// A filter that lets everything through, as the choice that takes the others off.
    pub fn new(id: impl Into<String>, caption: impl Into<String>) -> Self {
        Self { id: id.into(), caption: caption.into(), criteria: None }
    }

    /// Shows only the records that meet this expression.
    pub fn criteria(mut self, expression: impl Into<String>) -> Self {
        self.criteria = Some(expression.into());
        self
    }
}

/// A kind of record.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct EntityInfo {
    /// Its name, in the model and in code.
    pub name: String,
    /// What people see. Follows from the name unless given.
    pub caption: String,
    /// The group it goes under in the navigation; without one, it stays out.
    pub navigation: Option<String>,
    /// The name of its icon.
    pub icon: Option<String>,
    /// Its fields, in the order they were declared.
    pub fields: Vec<FieldInfo>,
    /// What its records have to meet.
    pub rules: Vec<crate::Rule>,
    /// How things look and behave by what its records hold.
    pub appearance: Vec<crate::AppearanceRule>,
    /// The ways to see its list.
    pub variants: Vec<ViewVariant>,
    /// What its list says it is there for, above the records.
    pub description: Option<String>,
    /// The states its records go through.
    pub state_machines: Vec<crate::StateMachine>,
    /// When to remind people of a record.
    pub reminder: Option<crate::Reminder>,
    /// Actions that set fields of the records selected.
    pub actions: Vec<crate::RecordAction>,
    /// What the list editors are told about this list: an editor's settings node, and in it names and values.
    pub editor_settings: Vec<(String, String, String)>,
    /// How many columns its form's fields go in; by how many there are, unless given.
    pub form_columns: Option<u8>,
    /// The filters people pick from on its list.
    pub filters: Vec<ListFilter>,
    /// What every record its list shows has to meet.
    pub list_criteria: Option<String>,
    /// Its records can be cloned.
    pub cloneable: bool,
    /// Its records are not kept by any storage: they live on a form or in a list given to them.
    pub transient: bool,
    /// A form says what is wrong with a field as it is left, and not only on saving.
    pub checks_on_leave: bool,
    /// People can change values right on its list.
    pub list_editable: bool,
    /// How its list gets its records.
    pub data_access: DataAccess,
    /// Places on its form for actions, by the name of their `ActionCategory::InForm`.
    pub form_actions: Vec<String>,
    /// The fields its form goes through first with the keyboard, in this order. The form shows
    /// them in the same order, so the eye and the keyboard go the same way.
    pub focus_order: Vec<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl EntityInfo {
    /// An entity with no fields yet.
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            caption: caption_of(&name),
            name,
            navigation: None,
            icon: None,
            fields: Vec::new(),
            rules: Vec::new(),
            appearance: Vec::new(),
            variants: Vec::new(),
            description: None,
            state_machines: Vec::new(),
            actions: Vec::new(),
            reminder: None,
            editor_settings: Vec::new(),
            form_columns: None,
            filters: Vec::new(),
            list_criteria: None,
            cloneable: false,
            transient: false,
            checks_on_leave: false,
            list_editable: false,
            data_access: DataAccess::Client,
            form_actions: Vec::new(),
            focus_order: Vec::new(),
        }
    }

    /// What people see.
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }

    /// The group it goes under in the navigation.
    pub fn navigation(mut self, group: impl Into<String>) -> Self {
        self.navigation = Some(group.into());
        self
    }

    /// The name of its icon.
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Appends a field.
    pub fn field(mut self, field: FieldInfo) -> Self {
        self.fields.push(field);
        self
    }

    /// Appends something its records have to meet.
    pub fn rule(mut self, rule: crate::Rule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Appends a rule about how things look.
    pub fn appearance(mut self, rule: crate::AppearanceRule) -> Self {
        self.appearance.push(rule);
        self
    }

    /// Appends a filter people pick from on its list.
    pub fn filter(mut self, filter: ListFilter) -> Self {
        self.filters.push(filter);
        self
    }

    /// What every record its list shows has to meet.
    pub fn list_criteria(mut self, expression: impl Into<String>) -> Self {
        self.list_criteria = Some(expression.into());
        self
    }

    /// Its records can be cloned, with `CloneModule`.
    pub fn cloneable(mut self) -> Self {
        self.cloneable = true;
        self
    }

    /// Its records are not kept by any storage: they live on a form or in a list given to them.
    pub fn transient(mut self) -> Self {
        self.transient = true;
        self
    }

    /// A form says what is wrong with a field as it is left, and not only on saving.
    pub fn checks_on_leave(mut self) -> Self {
        self.checks_on_leave = true;
        self
    }

    /// People can change values right on its list, with the rules checked as they do.
    pub fn list_editable(mut self) -> Self {
        self.list_editable = true;
        self
    }

    /// How its list gets its records.
    pub fn data_access(mut self, access: DataAccess) -> Self {
        self.data_access = access;
        self
    }

    /// A place on its form for the actions of `ActionCategory::InForm` with this name.
    pub fn form_actions(mut self, name: impl Into<String>) -> Self {
        self.form_actions.push(name.into());
        self
    }

    /// The fields its form goes through first with the keyboard, in this order; the others follow
    /// in the order they were declared. The form shows them in the same order.
    pub fn focus_order(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.focus_order = fields.into_iter().map(Into::into).collect();
        self
    }

    /// How many columns its form's fields go in.
    pub fn form_columns(mut self, columns: u8) -> Self {
        self.form_columns = Some(columns);
        self
    }

    /// What its list says it is there for, above the records.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Gives its records states, and only these ways between them. Its field becomes read-only on
    /// forms and starts in the machine's first state, its actions come by themselves, and any other
    /// change is refused when it is kept. Declare the field first.
    pub fn state_machine(mut self, machine: crate::StateMachine) -> Self {
        if let Some(field) = self.fields.iter_mut().find(|field| field.name == machine.field) {
            field.read_only = true;
            if let Some(start) = &machine.start {
                field.default_value = Value::Text(start.clone());
            }
        }
        self.state_machines.push(machine);
        self
    }

    /// An appearance that holds while its field is in this state.
    pub fn state_appearance(self, field: &str, state: &str, rule: crate::AppearanceRule) -> Self {
        let state = state.replace('\'', "''");
        self.appearance(rule.criteria(format!("[{field}] = '{state}'")))
    }

    /// An action that sets fields of the records selected. It comes by itself on its list and
    /// its form.
    pub fn action(mut self, action: crate::RecordAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Reminds people of its records.
    pub fn reminder(mut self, reminder: crate::Reminder) -> Self {
        self.reminder = Some(reminder);
        self
    }

    /// Tells a list editor something about its list, as `ChartSettings`, `Kind` and `Line`. It goes
    /// in the model under the list's view, so a person can change it like any other setting.
    pub fn editor_setting(
        mut self,
        node: impl Into<String>,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.editor_settings.push((node.into(), name.into(), value.into()));
        self
    }

    /// Appends a way to see its list.
    pub fn variant(mut self, variant: ViewVariant) -> Self {
        self.variants.push(variant);
        self
    }

    /// A field by name.
    pub fn field_named(&self, name: &str) -> Option<&FieldInfo> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// The field that identifies a record.
    pub fn key_field(&self) -> Option<&FieldInfo> {
        self.fields.iter().find(|f| f.is_key)
    }

    /// The field that names a record: the first in lookups, or else the first with text.
    pub fn default_property(&self) -> Option<&FieldInfo> {
        self.fields
            .iter()
            .find(|f| f.in_lookup)
            .or_else(|| self.fields.iter().find(|f| matches!(f.kind, FieldKind::Text) && !f.is_key))
    }

    /// What expressions over this entity may refer to.
    pub fn schema(&self) -> Schema {
        self.fields
            .iter()
            .filter(|f| !f.kind.is_collection())
            .fold(Schema::new(), |schema, f| schema.field(&f.name, f.kind.field_type()))
    }

    /// Says what is wrong with how it was declared.
    pub fn check(&self) -> Result<(), ModelError> {
        let wrong = |what: String| Err(ModelError(format!("{}: {what}", self.name)));
        if self.name.trim().is_empty() {
            return Err(ModelError("an entity has no name".into()));
        }
        match self.fields.iter().filter(|f| f.is_key).count() {
            1 => {}
            0 => return wrong("it has no key field".into()),
            n => return wrong(format!("it has {n} key fields, and only one can be")),
        }
        for (i, field) in self.fields.iter().enumerate() {
            if self.fields[..i].iter().any(|f| f.name == field.name) {
                return wrong(format!("the field {} is declared twice", field.name));
            }
            if let Some(expression) = &field.calculated {
                let parsed = Criteria::parse(expression)
                    .map_err(|e| ModelError(format!("{}.{}: {e}", self.name, field.name)))?;
                if let Some(problem) = ravenrs_criteria::validate(&parsed, &self.schema()).first() {
                    return wrong(format!("the field {}: {problem}", field.name));
                }
            }
        }
        for field in &self.focus_order {
            if self.field_named(field).is_none() {
                return wrong(format!("the focus order names {field}, which is not a field"));
            }
        }
        for action in &self.actions {
            action.check(self).map_err(|e| ModelError(format!("{}: {}", self.name, e.0)))?;
        }
        for machine in &self.state_machines {
            if self.field_named(&machine.field).is_none() {
                return wrong(format!("a state machine is over {}, which is not a field", machine.field));
            }
            if let Some(problem) = machine.problem() {
                return wrong(problem);
            }
        }
        if let Some(reminder) = &self.reminder
            && !self
                .field_named(&reminder.field)
                .is_some_and(|field| matches!(field.kind, FieldKind::Date | FieldKind::DateTime))
        {
            return wrong(format!("a reminder needs a date in {}", reminder.field));
        }
        let check_expression = |what: String, expression: &str| -> Result<(), ModelError> {
            let parsed =
                Criteria::parse(expression).map_err(|e| ModelError(format!("{}, {what}: {e}", self.name)))?;
            // A function the application added is known only where it runs.
            let environment = crate::environment();
            let problems = ravenrs_criteria::validate(&parsed, &self.schema());
            let found = problems.iter().find(|problem| {
                !matches!(problem, ravenrs_criteria::Problem::UnknownFunction(name) if environment.has_function(name))
            });
            match found {
                Some(problem) => Err(ModelError(format!("{}: {what}: {problem}", self.name))),
                None => Ok(()),
            }
        };
        for filter in &self.filters {
            if let Some(expression) = &filter.criteria {
                check_expression(format!("the filter {}", filter.id), expression)?;
            }
        }
        if let Some(expression) = &self.list_criteria {
            check_expression("the list's criteria".to_owned(), expression)?;
        }
        for variant in &self.variants {
            if let Some(gone) = variant.columns.iter().find(|name| self.field_named(name).is_none()) {
                return wrong(format!("the variant {} shows {gone}, which is not a field", variant.id));
            }
            if let Some(expression) = &variant.filter {
                let parsed = Criteria::parse(expression)
                    .map_err(|e| ModelError(format!("{}, variant {}: {e}", self.name, variant.id)))?;
                if let Some(problem) = ravenrs_criteria::validate(&parsed, &self.schema()).first() {
                    return wrong(format!("the variant {}: {problem}", variant.id));
                }
            }
        }
        for field in &self.fields {
            if let Some(expression) = &field.lookup_filter {
                Criteria::parse(expression)
                    .map_err(|e| ModelError(format!("{}.{}: {e}", self.name, field.name)))?;
            }
        }
        for rule in &self.rules {
            rule.check(self)?;
        }
        for rule in &self.appearance {
            rule.check(self)?;
        }
        Ok(())
    }

    /// A record of this entity with what a new one starts with.
    pub fn new_object(&self) -> Object {
        let values = self
            .fields
            .iter()
            .filter(|f| f.calculated.is_none() && !f.kind.is_collection())
            .map(|f| (f.name.clone(), f.default_value.clone()))
            .collect();
        let mut object = Object { entity: self.name.clone(), values };
        for field in self.fields.iter().filter(|field| matches!(field.kind, FieldKind::Guid)) {
            if matches!(object.get(&field.name), Value::Null) {
                object.set(&field.name, uuid::Uuid::new_v4().to_string());
            }
        }
        object
    }

    /// This entity as the model has it now: what people see and the rules it added.
    pub(crate) fn as_in_model(&self, root: &crate::ModelNode) -> Result<Self, ModelError> {
        let mut entity = self.clone();
        let Some(node) = root.at(&["Entities", &self.name]) else { return Ok(entity) };
        if let Some(members) = node.get("Members") {
            for member in members.children.iter().filter(|member| FieldInfo::is_custom(member, self)) {
                entity.fields.push(FieldInfo::from_model(&self.name, member)?);
            }
        }
        if let Some(caption) = node.text("Caption") {
            entity.caption = caption.to_owned();
        }
        for field in &mut entity.fields {
            let Some(member) = node.at(&["Members", &field.name]) else { continue };
            if let Some(caption) = member.text("Caption") {
                field.caption = caption.to_owned();
            }
            if let FieldKind::Enum(options) = &mut field.kind {
                for option in options {
                    let found = member.at(&["Options", &option.code]).and_then(|n| n.text("Caption"));
                    if let Some(caption) = found {
                        option.caption = caption.to_owned();
                    }
                }
            }
        }
        if let Some(rules) = node.get("Rules") {
            for rule in rules.in_order() {
                entity.rules.push(crate::Rule::from_model(rule)?);
            }
        }
        entity.check()?;
        Ok(entity)
    }

    /// Fills in the calculated fields, in the order declared, so one can read another.
    pub fn compute(&self, object: &mut Object, environment: &Environment) {
        for field in &self.fields {
            let Some(expression) = &field.calculated else { continue };
            // `check` already refused what does not parse.
            let value = Criteria::parse(expression)
                .ok()
                .and_then(|parsed| Scope::new(environment).row(&*object).evaluate(&parsed).ok())
                .unwrap_or(Value::Null);
            object.values.insert(field.name.clone(), value);
        }
    }

    /// The object as it is stored, with no calculated values.
    pub fn strip_calculated(&self, object: &mut Object) {
        for field in self.fields.iter().filter(|f| f.calculated.is_some()) {
            object.values.remove(&field.name);
        }
    }
}

/// A record: its entity's name and a value per field. Dynamic, so filters, rules and
/// permissions read every entity the same way.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Object {
    entity: String,
    values: BTreeMap<String, Value>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Object {
    /// A record of this entity with these values, as storage hands them over.
    pub fn from_values(entity: impl Into<String>, values: BTreeMap<String, Value>) -> Self {
        Self { entity: entity.into(), values }
    }

    /// The name of its entity.
    pub fn entity(&self) -> &str {
        &self.entity
    }

    /// The value of a field; nothing is `Null`.
    pub fn get(&self, field: &str) -> Value {
        self.values.get(field).cloned().unwrap_or(Value::Null)
    }

    /// Changes the value of a field.
    pub fn set(&mut self, field: &str, value: impl Into<Value>) {
        self.values.insert(field.to_owned(), value.into());
    }

    /// Every value, by field.
    pub fn values(&self) -> &BTreeMap<String, Value> {
        &self.values
    }

    /// Leaves only these fields.
    pub fn keep_only(&mut self, fields: &[&str]) {
        self.values.retain(|name, _| fields.contains(&name.as_str()));
    }
}

impl Record for Object {
    fn field(&self, path: &[String]) -> Option<Value> {
        match path {
            [name] => self.values.get(name).cloned(),
            _ => None,
        }
    }

    fn collection(&self, _path: &[String]) -> Option<Vec<&Self>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captions_follow_from_names() {
        assert_eq!(caption_of("opening_cents"), "Opening cents");
        assert_eq!(FieldInfo::new("name", FieldKind::Text).caption, "Name");
    }

    #[test]
    fn a_declaration_says_what_is_wrong_with_it() {
        let key = || FieldInfo::new("id", FieldKind::Integer).key();
        let name = |e: &EntityInfo| e.check().map_err(|e| e.0);
        assert!(name(&EntityInfo::new("A")).is_err_and(|e| e.contains("no key")));
        let two = EntityInfo::new("A").field(key()).field(FieldInfo::new("b", FieldKind::Text).typed_key());
        assert!(name(&two).is_err_and(|e| e.contains("2 key fields")));
        let twice = EntityInfo::new("A").field(key()).field(FieldInfo::new("id", FieldKind::Text));
        assert!(name(&twice).is_err_and(|e| e.contains("twice")));
        let bad = EntityInfo::new("A")
            .field(key())
            .field(FieldInfo::new("c", FieldKind::Integer).calculated("[nope] + 1"));
        assert!(name(&bad).is_err_and(|e| e.contains("A") && e.contains("c")));
        assert!(EntityInfo::new("A").field(key()).check().is_ok());
    }

    #[test]
    fn the_default_property_is_the_one_named_or_the_first_text() {
        let plain = EntityInfo::new("A")
            .field(FieldInfo::new("code", FieldKind::Text).typed_key())
            .field(FieldInfo::new("title", FieldKind::Text));
        assert_eq!(plain.default_property().map(|f| f.name.as_str()), Some("title"));
        let named = plain.field(FieldInfo::new("other", FieldKind::Text).default_property());
        assert_eq!(named.default_property().map(|f| f.name.as_str()), Some("other"));
    }

    #[test]
    fn the_schema_answers_for_the_fields() {
        let entity = EntityInfo::new("A")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text));
        let good = Criteria::parse("[name] = 'x' And [id] > 1").expect("parses");
        assert!(ravenrs_criteria::validate(&good, &entity.schema()).is_empty());
        let bad = Criteria::parse("[nope] = 1").expect("parses");
        assert!(!ravenrs_criteria::validate(&bad, &entity.schema()).is_empty());
    }
}
