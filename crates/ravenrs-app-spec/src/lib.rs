//! What `#[derive(BusinessObject)]` and `#[derive(BusinessEnum)]` read from a struct or an
//! enum, as plain data: the derive turns it into code, and `ravenrs-app` (feature `source`)
//! turns it into entities while a program runs, so both accept and refuse the same things,
//! with the same words.
//!
//! ```
//! let file = ravenrs_app_spec::parse_file(
//!     r#"
//!     #[derive(BusinessObject)]
//!     #[object(caption = "Banks")]
//!     struct Bank {
//!         #[field(typed_key, caption = "Code")]
//!         code: String,
//!     }
//!     "#,
//! )
//! .map_err(|errors| errors[0].message.clone())?;
//! assert_eq!(file.objects[0].name.to_string(), "Bank");
//! assert!(file.objects[0].fields[0].settings.typed_key);
//! # Ok::<(), String>(())
//! ```

use proc_macro2::Span;
use ravenrs_criteria::Criteria;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Error, Fields, Ident, LitBool, LitInt, LitStr, Result};

macro_rules! opaque_debug {
    ($($ty:ty),*) => {$(
        impl std::fmt::Debug for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($ty)).finish_non_exhaustive()
            }
        }
    )*};
}

opaque_debug!(
    ActionSpec,
    ObjectSettings,
    KindSpec,
    FieldSettings,
    FieldSpec,
    RuleKindSpec,
    RuleSpec,
    ContextSpec,
    AppearancePiece,
    AppearanceSpec,
    ChoiceSpec,
    ObjectSpec,
    OptionSpec,
    EnumSpec,
    FileSpec
);

/// `#[object(...)]` on a struct.
#[derive(Clone, Default)]
pub struct ObjectSettings {
    /// `caption = "..."`.
    pub caption: Option<LitStr>,
    /// `navigation = "..."`.
    pub navigation: Option<LitStr>,
    /// `icon = "..."`.
    pub icon: Option<LitStr>,
    /// `list_criteria = "..."`, checked to parse.
    pub list_criteria: Option<LitStr>,
    /// `form_columns = 2`.
    pub form_columns: Option<LitInt>,
    /// `cloneable`.
    pub cloneable: bool,
    /// `transient`.
    pub transient: bool,
    /// `checks_on_leave`.
    pub checks_on_leave: bool,
    /// `list_editable`.
    pub list_editable: bool,
    /// `data_access = "client" | "server" | "data_view"`.
    pub data_access: Option<DataAccessSpec>,
    /// `form_actions = "..."`, as many as given.
    pub form_actions: Vec<LitStr>,
    /// `focus_order = "a,b"`, as field names, checked against the struct.
    pub focus_order: Vec<String>,
}

/// Where a list's records are read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataAccessSpec {
    /// All in memory.
    Client,
    /// A page at a time.
    Server,
    /// Only the shown values.
    DataView,
}

/// What a field holds when its attribute says so, instead of its type.
#[derive(Clone)]
pub enum KindSpec {
    /// `money`.
    Money,
    /// `long`.
    LongText,
    /// `color`.
    Color,
    /// `date`.
    Date,
    /// `guid`.
    Guid,
    /// `tags = "a,b"`.
    Tags(Vec<String>),
    /// `decimals = 2`.
    Decimal(u8),
    /// `reference = "Entity"`.
    Reference(LitStr),
    /// `collection = "Entity"`.
    Collection(LitStr),
}

/// `#[field(...)]` on a field.
#[derive(Clone, Default)]
pub struct FieldSettings {
    /// `key` in the attribute.
    pub key: bool,
    /// `typed_key` in the attribute.
    pub typed_key: bool,
    /// What people see.
    pub caption: Option<LitStr>,
    /// `default_property` in the attribute.
    pub default_property: bool,
    /// `read_only` in the attribute.
    pub read_only: bool,
    /// `no_list` in the attribute.
    pub no_list: bool,
    /// `no_detail` in the attribute.
    pub no_detail: bool,
    /// `lookup` in the attribute.
    pub lookup: bool,
    /// What it holds, when the attribute says.
    pub kind: Option<KindSpec>,
    /// `calculated = "..."`, checked against the struct's fields.
    pub calculated: Option<LitStr>,
    /// `default = ...`, any expression; a running program reads only literals and enum options.
    pub default: Option<syn::Expr>,
    /// `required` in the attribute.
    pub required: bool,
    /// `unique` in the attribute.
    pub unique: bool,
    /// `min` in the attribute.
    pub min: Option<syn::Expr>,
    /// `max` in the attribute.
    pub max: Option<syn::Expr>,
    /// `matches = "..."`, checked to be a valid pattern.
    pub matches: Option<LitStr>,
    /// What to say instead of the usual words.
    pub message: Option<LitStr>,
    /// `editor` in the attribute.
    pub editor: Option<LitStr>,
    /// `placeholder` in the attribute.
    pub placeholder: Option<LitStr>,
    /// `hint` in the attribute.
    pub hint: Option<LitStr>,
    /// `accessible_name` in the attribute.
    pub accessible_name: Option<LitStr>,
    /// `accessible_description` in the attribute.
    pub accessible_description: Option<LitStr>,
    /// `lookup_filter` in the attribute.
    pub lookup_filter: Option<LitStr>,
    /// `none_caption` in the attribute.
    pub none_caption: Option<LitStr>,
    /// `back_reference` in the attribute.
    pub back_reference: Option<LitStr>,
    /// `most_bytes` in the attribute.
    pub most_bytes: Option<LitInt>,
    /// `wide` in the attribute.
    pub wide: bool,
    /// `autofocus` in the attribute.
    pub autofocus: bool,
    /// `non_cloneable` in the attribute.
    pub non_cloneable: bool,
    /// `secret` in the attribute.
    pub secret: bool,
    /// `aggregated` in the attribute.
    pub aggregated: bool,
    /// `group` in the attribute.
    pub group: Option<LitStr>,
    /// `display_format` in the attribute.
    pub display_format: Option<LitStr>,
    /// `edit_mask` in the attribute.
    pub edit_mask: Option<LitStr>,
    /// `predefined = "a,b"`, as values.
    pub predefined: Option<Vec<String>>,
    /// `criteria_of` in the attribute.
    pub criteria_of: Option<LitStr>,
}

/// A field of a business object.
#[derive(Clone)]
pub struct FieldSpec {
    /// Its name in the code.
    pub ident: Ident,
    /// Its Rust type.
    pub ty: syn::Type,
    /// What its attribute says.
    pub settings: FieldSettings,
}

/// What a rule checks.
#[derive(Clone)]
pub enum RuleKindSpec {
    /// `#[field(required)]`.
    Required(String),
    /// `#[field(unique)]`.
    Unique(String),
    /// `#[field(min = .., max = ..)]`.
    Range {
        /// The field.
        field: String,
        /// The least value, if any.
        min: Option<Box<syn::Expr>>,
        /// The greatest value, if any.
        max: Option<Box<syn::Expr>>,
    },
    /// `#[field(matches = "...")]`.
    Matches(String, LitStr),
    /// `#[rule(criteria = "...")]`, with its id.
    Criteria(String, LitStr),
    /// `#[rule(unique = "a,b")]`, with its id.
    UniqueTogether(String, Vec<String>),
}

/// How much a broken rule stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeveritySpec {
    /// It stops saving.
    Error,
    /// `warning`: people can go on.
    Warning,
    /// `information`: only a note.
    Information,
}

/// A rule of a field or of a struct.
#[derive(Clone)]
pub struct RuleSpec {
    /// What it checks.
    pub kind: RuleKindSpec,
    /// What to say instead of the usual words.
    pub message: Option<LitStr>,
    /// Where it applies.
    pub context: Option<LitStr>,
    /// Which records it checks, as an expression.
    pub target: Option<LitStr>,
    /// How much it stops.
    pub severity: SeveritySpec,
}

/// A color of an appearance rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TintSpec {
    /// `Danger`.
    Danger,
    /// `Warning`.
    Warning,
    /// `Success`.
    Success,
    /// `Accent`.
    Accent,
    /// `Muted`.
    Muted,
}

/// Where an appearance rule applies.
#[derive(Clone)]
pub enum ContextSpec {
    /// `List`.
    List,
    /// `Detail`.
    Detail,
    /// `View`.
    View(LitStr),
}

/// One setting of `#[appearance(...)]`, in the order written.
#[derive(Clone)]
pub enum AppearancePiece {
    /// `Criteria`.
    Criteria(LitStr),
    /// `Font`.
    Font(TintSpec),
    /// `Back`.
    Back(TintSpec),
    /// `Bold`.
    Bold,
    /// `Enabled`.
    Enabled(LitBool),
    /// `Visible`.
    Visible(LitBool),
    /// `Priority`.
    Priority(LitInt),
    /// `Action`.
    Action,
    /// `Layout`.
    Layout,
    /// `Context`.
    Context(ContextSpec),
}

/// `#[appearance(...)]` on a struct.
#[derive(Clone)]
pub struct AppearanceSpec {
    /// Its id.
    pub id: String,
    /// The names it applies to.
    pub targets: Vec<String>,
    /// Its settings, in the order written.
    pub pieces: Vec<AppearancePiece>,
}

/// `#[variant(...)]` or `#[list_filter(...)]` on a struct.
#[derive(Clone)]
pub enum ChoiceSpec {
    /// `Variant`.
    Variant {
        /// Its id.
        id: LitStr,
        /// What people see.
        caption: LitStr,
        /// The records it shows, as an expression.
        filter: Option<LitStr>,
        /// The columns it shows.
        columns: Vec<String>,
    },
    /// `ListFilter`.
    ListFilter {
        /// Its id.
        id: LitStr,
        /// What people see.
        caption: LitStr,
        /// The records it keeps, as an expression.
        filter: Option<LitStr>,
    },
}

/// A struct with `#[derive(BusinessObject)]`.
#[derive(Clone)]
pub struct ObjectSpec {
    /// Its name in the code.
    pub name: Ident,
    /// What its attribute says.
    pub settings: ObjectSettings,
    /// Its fields, in order.
    pub fields: Vec<FieldSpec>,
    /// The rules of the fields, in their order, then those of the struct.
    pub rules: Vec<RuleSpec>,
    /// Its appearance rules.
    pub appearance: Vec<AppearanceSpec>,
    /// Its view variants and list filters.
    pub choices: Vec<ChoiceSpec>,
    /// Its actions, from `#[action(...)]`.
    pub actions: Vec<ActionSpec>,
}

/// How many records an action takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionSpec {
    /// `selection = "one"`.
    One,
    /// `selection = "many"`, one or more.
    Many,
    /// `selection = "any"`, none or any number.
    Any,
}

/// `#[action(...)]` on a struct: an action that sets fields of the records selected.
#[derive(Clone)]
pub struct ActionSpec {
    /// `caption = "..."`.
    pub caption: LitStr,
    /// `id = "..."`.
    pub id: Option<LitStr>,
    /// `selection = "one" | "many" | "any"`.
    pub selection: Option<SelectionSpec>,
    /// `enabled = "..."`, checked to parse and to name only fields of the struct.
    pub enabled: Option<LitStr>,
    /// `set = "field = expression; ..."`, checked the same way.
    pub set: LitStr,
    /// `confirm = "..."`.
    pub confirm: Option<LitStr>,
}

/// An option of a business enum.
#[derive(Clone)]
pub struct OptionSpec {
    /// Its name in the code.
    pub ident: Ident,
    /// What a record holds for it.
    pub code: String,
    /// What people see.
    pub caption: String,
}

/// An enum with `#[derive(BusinessEnum)]`.
#[derive(Clone)]
pub struct EnumSpec {
    /// Its name in the code.
    pub name: Ident,
    /// Its options, the first one being what a new record starts with.
    pub options: Vec<OptionSpec>,
}

/// What a field's Rust type holds, by the `FieldValue` implementations of `ravenrs-app`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeKind {
    /// `String`.
    Text,
    /// `bool`.
    Boolean,
    /// `i32`, `i64`, `u32`, `u64`, `usize`.
    Integer,
    /// `f64`, with two decimals.
    Decimal,
    /// `DateTime`.
    DateTime,
    /// Any other name: a business enum, if one has it.
    Named(String),
}

/// What a field's type holds, looking through `Option`. `None` for types no field can have,
/// such as tuples or references.
pub fn type_kind(ty: &syn::Type) -> Option<TypeKind> {
    let syn::Type::Path(path) = ty else { return None };
    if path.qself.is_some() {
        return None;
    }
    let last = path.path.segments.last()?;
    let name = last.ident.to_string();
    if name == "Option" {
        let syn::PathArguments::AngleBracketed(arguments) = &last.arguments else { return None };
        let Some(syn::GenericArgument::Type(inner)) = arguments.args.first() else { return None };
        return type_kind(inner);
    }
    if !last.arguments.is_empty() {
        return None;
    }
    Some(match name.as_str() {
        "String" => TypeKind::Text,
        "bool" => TypeKind::Boolean,
        "i32" | "i64" | "u32" | "u64" | "usize" => TypeKind::Integer,
        "f64" => TypeKind::Decimal,
        "DateTime" => TypeKind::DateTime,
        _ => TypeKind::Named(name),
    })
}

/// Names separated by commas, trimmed, without empty ones.
pub fn split_names(text: &LitStr) -> Vec<String> {
    text.value().split(',').map(|name| name.trim().to_owned()).filter(|name| !name.is_empty()).collect()
}

fn object_settings(input: &DeriveInput) -> Result<(ObjectSettings, Option<LitStr>)> {
    let mut attrs = ObjectSettings::default();
    let mut focus_order = None;
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("object")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("caption") {
                attrs.caption = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("navigation") {
                attrs.navigation = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("icon") {
                attrs.icon = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("list_criteria") {
                let text: LitStr = meta.value()?.parse()?;
                Criteria::parse(&text.value()).map_err(|e| Error::new(text.span(), e.to_string()))?;
                attrs.list_criteria = Some(text);
            } else if meta.path.is_ident("form_columns") {
                attrs.form_columns = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("cloneable") {
                attrs.cloneable = true;
            } else if meta.path.is_ident("transient") {
                attrs.transient = true;
            } else if meta.path.is_ident("checks_on_leave") {
                attrs.checks_on_leave = true;
            } else if meta.path.is_ident("list_editable") {
                attrs.list_editable = true;
            } else if meta.path.is_ident("data_access") {
                let text: LitStr = meta.value()?.parse()?;
                attrs.data_access = Some(match text.value().as_str() {
                    "client" => DataAccessSpec::Client,
                    "server" => DataAccessSpec::Server,
                    "data_view" => DataAccessSpec::DataView,
                    _ => return Err(Error::new(text.span(), "use client, server or data_view")),
                });
            } else if meta.path.is_ident("form_actions") {
                attrs.form_actions.push(meta.value()?.parse()?);
            } else if meta.path.is_ident("focus_order") {
                focus_order = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error(
                    "unknown setting: use caption, navigation, icon, list_criteria, form_columns, cloneable, \
                     transient, checks_on_leave, list_editable, data_access, form_actions or focus_order",
                ));
            }
            Ok(())
        })?;
    }
    Ok((attrs, focus_order))
}

fn set_kind(attrs: &mut FieldSettings, path: &syn::Path, kind: KindSpec) -> Result<()> {
    if attrs.kind.is_some() {
        return Err(Error::new(path.span(), "a field holds one kind of thing: this says a second"));
    }
    attrs.kind = Some(kind);
    Ok(())
}

/// `#[field(...)]` on one field.
pub fn field_settings(field: &syn::Field) -> Result<FieldSettings> {
    let mut attrs = FieldSettings::default();
    for attr in field.attrs.iter().filter(|a| a.path().is_ident("field")) {
        attr.parse_nested_meta(|meta| {
            let path = &meta.path;
            let named = |name: &str| path.is_ident(name);
            if named("key") {
                attrs.key = true;
            } else if named("typed_key") {
                attrs.typed_key = true;
            } else if named("caption") {
                attrs.caption = Some(meta.value()?.parse()?);
            } else if named("default_property") {
                attrs.default_property = true;
            } else if named("read_only") {
                attrs.read_only = true;
            } else if named("no_list") {
                attrs.no_list = true;
            } else if named("no_detail") {
                attrs.no_detail = true;
            } else if named("lookup") {
                attrs.lookup = true;
            } else if named("money") {
                set_kind(&mut attrs, path, KindSpec::Money)?;
            } else if named("long") {
                set_kind(&mut attrs, path, KindSpec::LongText)?;
            } else if named("color") {
                set_kind(&mut attrs, path, KindSpec::Color)?;
            } else if named("date") {
                set_kind(&mut attrs, path, KindSpec::Date)?;
            } else if named("guid") {
                set_kind(&mut attrs, path, KindSpec::Guid)?;
            } else if named("tags") {
                let options: LitStr = meta.value()?.parse()?;
                set_kind(&mut attrs, path, KindSpec::Tags(split_names(&options)))?;
            } else if named("decimals") {
                let places: LitInt = meta.value()?.parse()?;
                let places: u8 = places.base10_parse()?;
                set_kind(&mut attrs, path, KindSpec::Decimal(places))?;
            } else if named("reference") {
                let target: LitStr = meta.value()?.parse()?;
                set_kind(&mut attrs, path, KindSpec::Reference(target))?;
            } else if named("collection") {
                let target: LitStr = meta.value()?.parse()?;
                set_kind(&mut attrs, path, KindSpec::Collection(target))?;
            } else if named("calculated") {
                attrs.calculated = Some(meta.value()?.parse()?);
            } else if named("default") {
                attrs.default = Some(meta.value()?.parse()?);
            } else if named("required") {
                attrs.required = true;
            } else if named("unique") {
                attrs.unique = true;
            } else if named("min") {
                attrs.min = Some(meta.value()?.parse()?);
            } else if named("max") {
                attrs.max = Some(meta.value()?.parse()?);
            } else if named("matches") {
                let pattern: LitStr = meta.value()?.parse()?;
                regex_lite::Regex::new(&pattern.value())
                    .map_err(|e| Error::new(pattern.span(), format!("this pattern is not valid: {e}")))?;
                attrs.matches = Some(pattern);
            } else if named("message") {
                attrs.message = Some(meta.value()?.parse()?);
            } else if named("editor") {
                attrs.editor = Some(meta.value()?.parse()?);
            } else if named("placeholder") {
                attrs.placeholder = Some(meta.value()?.parse()?);
            } else if named("hint") {
                attrs.hint = Some(meta.value()?.parse()?);
            } else if named("accessible_name") {
                attrs.accessible_name = Some(meta.value()?.parse()?);
            } else if named("accessible_description") {
                attrs.accessible_description = Some(meta.value()?.parse()?);
            } else if named("lookup_filter") {
                let text: LitStr = meta.value()?.parse()?;
                Criteria::parse(&text.value()).map_err(|e| Error::new(text.span(), e.to_string()))?;
                attrs.lookup_filter = Some(text);
            } else if named("none_caption") {
                attrs.none_caption = Some(meta.value()?.parse()?);
            } else if named("back_reference") {
                attrs.back_reference = Some(meta.value()?.parse()?);
            } else if named("most_bytes") {
                attrs.most_bytes = Some(meta.value()?.parse()?);
            } else if named("wide") {
                attrs.wide = true;
            } else if named("autofocus") {
                attrs.autofocus = true;
            } else if named("non_cloneable") {
                attrs.non_cloneable = true;
            } else if named("secret") {
                attrs.secret = true;
            } else if named("aggregated") {
                attrs.aggregated = true;
            } else if named("group") {
                attrs.group = Some(meta.value()?.parse()?);
            } else if named("display_format") {
                attrs.display_format = Some(meta.value()?.parse()?);
            } else if named("edit_mask") {
                attrs.edit_mask = Some(meta.value()?.parse()?);
            } else if named("predefined") {
                let text: LitStr = meta.value()?.parse()?;
                attrs.predefined = Some(split_names(&text));
            } else if named("criteria_of") {
                attrs.criteria_of = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error(
                    "unknown setting: use key, typed_key, caption, default_property, read_only, no_list, \
                     no_detail, lookup, money, long, color, date, guid, tags, decimals, reference, collection, \
                     calculated, default, required, unique, min, max, matches, message, editor, placeholder, \
                     hint, accessible_name, accessible_description, lookup_filter, none_caption, back_reference, most_bytes, wide, autofocus, \
                     non_cloneable, secret, aggregated, group, display_format, edit_mask, predefined or criteria_of",
                ));
            }
            Ok(())
        })?;
    }
    if attrs.key && attrs.typed_key {
        return Err(Error::new(field.span(), "a field is key or typed_key, not both"));
    }
    Ok(attrs)
}

/// Checks that an expression parses and names only fields of the struct.
pub fn check_expression(expression: &LitStr, names: &[String]) -> Result<()> {
    let parsed = Criteria::parse(&expression.value())
        .map_err(|e| Error::new(expression.span(), format!("this expression does not parse: {e}")))?;
    for property in parsed.properties() {
        if !names.contains(&property) {
            return Err(Error::new(expression.span(), format!("this struct has no field `{property}`")));
        }
    }
    Ok(())
}

fn field_rules(ident: &Ident, attrs: &FieldSettings) -> Vec<RuleSpec> {
    let name = ident.to_string();
    let mut kinds = Vec::new();
    if attrs.required {
        kinds.push(RuleKindSpec::Required(name.clone()));
    }
    if attrs.unique {
        kinds.push(RuleKindSpec::Unique(name.clone()));
    }
    if attrs.min.is_some() || attrs.max.is_some() {
        kinds.push(RuleKindSpec::Range {
            field: name.clone(),
            min: attrs.min.clone().map(Box::new),
            max: attrs.max.clone().map(Box::new),
        });
    }
    if let Some(pattern) = &attrs.matches {
        kinds.push(RuleKindSpec::Matches(name, pattern.clone()));
    }
    kinds
        .into_iter()
        .map(|kind| RuleSpec {
            kind,
            message: attrs.message.clone(),
            context: None,
            target: None,
            severity: SeveritySpec::Error,
        })
        .collect()
}

#[derive(Default)]
struct RuleAttrs {
    criteria: Option<LitStr>,
    unique: Option<LitStr>,
    message: Option<LitStr>,
    context: Option<LitStr>,
    target: Option<LitStr>,
    warning: bool,
    information: bool,
}

fn struct_rules(input: &DeriveInput, names: &[String]) -> Result<Vec<RuleSpec>> {
    let mut rules = Vec::new();
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("rule")) {
        let mut found = RuleAttrs::default();
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("warning") {
                found.warning = true;
                return Ok(());
            }
            if meta.path.is_ident("information") {
                found.information = true;
                return Ok(());
            }
            let slot = if meta.path.is_ident("criteria") {
                &mut found.criteria
            } else if meta.path.is_ident("unique") {
                &mut found.unique
            } else if meta.path.is_ident("message") {
                &mut found.message
            } else if meta.path.is_ident("context") {
                &mut found.context
            } else if meta.path.is_ident("target") {
                &mut found.target
            } else {
                return Err(meta.error(
                    "unknown setting: use criteria, unique, message, context, target, warning or information",
                ));
            };
            *slot = Some(meta.value()?.parse()?);
            Ok(())
        })?;
        let id = format!("rule{}", rules.len());
        let kind = match (&found.criteria, &found.unique) {
            (Some(expression), None) => {
                check_expression(expression, names)?;
                RuleKindSpec::Criteria(id, expression.clone())
            }
            (None, Some(fields)) => {
                let fields = split_names(fields);
                if let Some(missing) = fields.iter().find(|f| !names.contains(f)) {
                    return Err(Error::new(attr.span(), format!("this struct has no field `{missing}`")));
                }
                RuleKindSpec::UniqueTogether(id, fields)
            }
            _ => return Err(Error::new(attr.span(), "a rule says either criteria or unique")),
        };
        if let Some(expression) = &found.target {
            check_expression(expression, names)?;
        }
        if found.warning && found.information {
            return Err(Error::new(attr.span(), "a rule is a warning or information, not both"));
        }
        let severity = if found.warning {
            SeveritySpec::Warning
        } else if found.information {
            SeveritySpec::Information
        } else {
            SeveritySpec::Error
        };
        rules.push(RuleSpec {
            kind,
            message: found.message,
            context: found.context,
            target: found.target,
            severity,
        });
    }
    Ok(rules)
}

fn tint(text: &LitStr) -> Result<TintSpec> {
    Ok(match text.value().as_str() {
        "danger" => TintSpec::Danger,
        "warning" => TintSpec::Warning,
        "success" => TintSpec::Success,
        "accent" => TintSpec::Accent,
        "muted" => TintSpec::Muted,
        _ => return Err(Error::new(text.span(), "use danger, warning, success, accent or muted")),
    })
}

fn struct_appearance(input: &DeriveInput, names: &[String]) -> Result<Vec<AppearanceSpec>> {
    let mut rules = Vec::new();
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("appearance")) {
        let mut targets: Option<LitStr> = None;
        let mut pieces = Vec::new();
        attr.parse_nested_meta(|meta| {
            let named = |name: &str| meta.path.is_ident(name);
            if named("names") {
                targets = Some(meta.value()?.parse()?);
            } else if named("criteria") {
                let expression: LitStr = meta.value()?.parse()?;
                check_expression(&expression, names)?;
                pieces.push(AppearancePiece::Criteria(expression));
            } else if named("font") {
                pieces.push(AppearancePiece::Font(tint(&meta.value()?.parse()?)?));
            } else if named("back") {
                pieces.push(AppearancePiece::Back(tint(&meta.value()?.parse()?)?));
            } else if named("bold") {
                pieces.push(AppearancePiece::Bold);
            } else if named("enabled") {
                pieces.push(AppearancePiece::Enabled(meta.value()?.parse()?));
            } else if named("visible") {
                pieces.push(AppearancePiece::Visible(meta.value()?.parse()?));
            } else if named("priority") {
                pieces.push(AppearancePiece::Priority(meta.value()?.parse()?));
            } else if named("action") {
                pieces.push(AppearancePiece::Action);
            } else if named("layout") {
                pieces.push(AppearancePiece::Layout);
            } else if named("context") {
                let context: LitStr = meta.value()?.parse()?;
                pieces.push(AppearancePiece::Context(match context.value().as_str() {
                    "list" => ContextSpec::List,
                    "detail" => ContextSpec::Detail,
                    _ => ContextSpec::View(context),
                }));
            } else {
                return Err(meta.error(
                    "unknown setting: use names, criteria, font, back, bold, enabled, visible, priority, \
                     context, action or layout",
                ));
            }
            Ok(())
        })?;
        let Some(targets) = targets else {
            return Err(Error::new(attr.span(), "an appearance rule needs names = \"...\""));
        };
        let id = format!("appearance{}", rules.len());
        rules.push(AppearanceSpec { id, targets: split_names(&targets), pieces });
    }
    Ok(rules)
}

fn struct_actions(input: &DeriveInput, names: &[String]) -> Result<Vec<ActionSpec>> {
    let mut found = Vec::new();
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("action")) {
        let (mut caption, mut id, mut selection, mut enabled, mut set, mut confirm) =
            (None::<LitStr>, None, None, None, None::<LitStr>, None);
        attr.parse_nested_meta(|meta| {
            let named = |name: &str| meta.path.is_ident(name);
            if named("caption") {
                caption = Some(meta.value()?.parse()?);
            } else if named("id") {
                id = Some(meta.value()?.parse()?);
            } else if named("selection") {
                let text: LitStr = meta.value()?.parse()?;
                selection = Some(match text.value().as_str() {
                    "one" => SelectionSpec::One,
                    "many" => SelectionSpec::Many,
                    "any" => SelectionSpec::Any,
                    _ => return Err(Error::new(text.span(), "use one, many or any")),
                });
            } else if named("enabled") {
                let expression: LitStr = meta.value()?.parse()?;
                check_expression(&expression, names)?;
                enabled = Some(expression);
            } else if named("set") {
                let text: LitStr = meta.value()?.parse()?;
                check_assignments(&text, names)?;
                set = Some(text);
            } else if named("confirm") {
                confirm = Some(meta.value()?.parse()?);
            } else {
                return Err(
                    meta.error("unknown setting: use caption, id, selection, enabled, set or confirm")
                );
            }
            Ok(())
        })?;
        let (Some(caption), Some(set)) = (caption, set) else {
            return Err(Error::new(attr.span(), "an action needs caption = \"...\" and set = \"...\""));
        };
        found.push(ActionSpec { caption, id, selection, enabled, set, confirm });
    }
    Ok(found)
}

/// Checks `field = expression; ...`: it parses, sets something, and names only fields of the
/// struct.
pub fn check_assignments(text: &LitStr, names: &[String]) -> Result<()> {
    let found = Criteria::assignments(&text.value())
        .map_err(|e| Error::new(text.span(), format!("this does not parse: {e}")))?;
    if found.is_empty() {
        return Err(Error::new(text.span(), "this sets nothing: use `field = expression; ...`"));
    }
    for (field, expression) in &found {
        let unknown = std::iter::once(field.clone()).chain(expression.properties());
        if let Some(unknown) = unknown.into_iter().find(|name| !names.contains(name)) {
            return Err(Error::new(text.span(), format!("this struct has no field `{unknown}`")));
        }
    }
    Ok(())
}

fn struct_choices(input: &DeriveInput, names: &[String]) -> Result<Vec<ChoiceSpec>> {
    let mut found = Vec::new();
    for attr in &input.attrs {
        let is_variant = attr.path().is_ident("variant");
        if !is_variant && !attr.path().is_ident("list_filter") {
            continue;
        }
        let (mut id, mut caption, mut filter, mut columns) =
            (None::<LitStr>, None::<LitStr>, None::<LitStr>, None::<LitStr>);
        attr.parse_nested_meta(|meta| {
            let slot = if meta.path.is_ident("id") {
                &mut id
            } else if meta.path.is_ident("caption") {
                &mut caption
            } else if meta.path.is_ident("filter") {
                &mut filter
            } else if is_variant && meta.path.is_ident("columns") {
                &mut columns
            } else {
                return Err(meta.error("unknown setting: use id, caption, filter or, on a variant, columns"));
            };
            *slot = Some(meta.value()?.parse()?);
            Ok(())
        })?;
        let (Some(id), Some(caption)) = (id, caption) else {
            return Err(Error::new(attr.span(), "this needs an id and a caption"));
        };
        if let Some(text) = &filter {
            let parsed =
                Criteria::parse(&text.value()).map_err(|e| Error::new(text.span(), e.to_string()))?;
            if let Some(unknown) = parsed.properties().iter().find(|name| !names.contains(name)) {
                return Err(Error::new(text.span(), format!("there is no field {unknown}")));
            }
        }
        let shown: Vec<String> = columns
            .iter()
            .flat_map(|text| text.value().split(',').map(|c| c.trim().to_owned()).collect::<Vec<_>>())
            .collect();
        if let (Some(text), Some(unknown)) = (&columns, shown.iter().find(|name| !names.contains(name))) {
            return Err(Error::new(text.span(), format!("there is no field {unknown}")));
        }
        found.push(if is_variant {
            ChoiceSpec::Variant { id, caption, filter, columns: shown }
        } else {
            ChoiceSpec::ListFilter { id, caption, filter }
        });
    }
    Ok(found)
}

/// Reads a struct with `#[derive(BusinessObject)]`, checking what the derive checks, in the
/// same order.
pub fn parse_object(input: &DeriveInput) -> Result<ObjectSpec> {
    let name = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(Error::new(input.generics.span(), "a business object cannot be generic"));
    }
    let Data::Struct(data) = &input.data else {
        return Err(Error::new(name.span(), "a business object is a struct with named fields"));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(Error::new(name.span(), "a business object is a struct with named fields"));
    };
    let names: Vec<String> =
        named.named.iter().filter_map(|f| f.ident.as_ref().map(Ident::to_string)).collect();
    let (mut settings, focus_order) = object_settings(input)?;
    let mut fields = Vec::new();
    let mut rules = Vec::new();
    let mut keys = 0;
    for field in &named.named {
        let Some(ident) = &field.ident else { continue };
        let attrs = field_settings(field)?;
        if attrs.key || attrs.typed_key {
            keys += 1;
            if keys > 1 {
                return Err(Error::new(
                    ident.span(),
                    "only one field can be the key, and another one already is",
                ));
            }
        }
        if let Some(expression) = &attrs.calculated {
            check_expression(expression, &names)?;
        }
        rules.extend(field_rules(ident, &attrs));
        fields.push(FieldSpec { ident: ident.clone(), ty: field.ty.clone(), settings: attrs });
    }
    if keys == 0 {
        return Err(Error::new(
            name.span(),
            "a business object needs a key: mark a field with #[field(key)] or #[field(typed_key)]",
        ));
    }
    rules.extend(struct_rules(input, &names)?);
    let appearance = struct_appearance(input, &names)?;
    if let Some(order) = &focus_order {
        let order_fields = split_names(order);
        if let Some(missing) = order_fields.iter().find(|field| !names.contains(field)) {
            return Err(Error::new(order.span(), format!("this struct has no field `{missing}`")));
        }
        settings.focus_order = order_fields;
    }
    let choices = struct_choices(input, &names)?;
    let actions = struct_actions(input, &names)?;
    Ok(ObjectSpec { name: name.clone(), settings, fields, rules, appearance, choices, actions })
}

/// Reads an enum with `#[derive(BusinessEnum)]`.
pub fn parse_enum(input: &DeriveInput) -> Result<EnumSpec> {
    let name = &input.ident;
    let Data::Enum(data) = &input.data else {
        return Err(Error::new(name.span(), "a business enum is an enum with no data"));
    };
    if data.variants.is_empty() {
        return Err(Error::new(name.span(), "a business enum needs at least one option"));
    }
    let mut options = Vec::new();
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(Error::new(variant.ident.span(), "an option of a business enum cannot hold data"));
        }
        let ident = &variant.ident;
        let (mut caption, mut code) = (ident.to_string(), ident.to_string());
        for attr in variant.attrs.iter().filter(|a| a.path().is_ident("option")) {
            attr.parse_nested_meta(|meta| {
                let value: LitStr = if meta.path.is_ident("caption") || meta.path.is_ident("code") {
                    meta.value()?.parse()?
                } else {
                    return Err(meta.error("unknown setting: use caption or code"));
                };
                if meta.path.is_ident("caption") {
                    caption = value.value();
                } else {
                    code = value.value();
                }
                Ok(())
            })?;
        }
        options.push(OptionSpec { ident: ident.clone(), code, caption });
    }
    Ok(EnumSpec { name: name.clone(), options })
}

/// Something wrong at a place of a source text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecError {
    /// The line, from 1.
    pub line: usize,
    /// The column of the first character, from 1.
    pub column: usize,
    /// How many characters it covers, at least 1.
    pub length: usize,
    /// What is wrong.
    pub message: String,
}

impl SpecError {
    /// Where a span is, with a message.
    pub fn at(span: Span, message: impl Into<String>) -> Self {
        let (start, end) = (span.start(), span.end());
        let length = if end.line == start.line { end.column.saturating_sub(start.column).max(1) } else { 1 };
        Self { line: start.line, column: start.column + 1, length, message: message.into() }
    }

    /// Every error a `syn::Error` holds.
    pub fn all(error: &Error) -> Vec<Self> {
        error.clone().into_iter().map(|e| Self::at(e.span(), e.to_string())).collect()
    }
}

impl std::fmt::Display for SpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

/// The business objects and enums of a source file, in the order written, and the items it
/// leaves for others to read, such as functions.
#[derive(Clone, Default)]
pub struct FileSpec {
    /// The structs with `#[derive(BusinessObject)]`.
    pub objects: Vec<ObjectSpec>,
    /// The enums with `#[derive(BusinessEnum)]`.
    pub enums: Vec<EnumSpec>,
    /// Everything else in the file.
    pub items: Vec<syn::Item>,
}

/// Whether an item derives `name`, as `#[derive(name)]` or `#[derive(path::name)]`.
pub fn derives(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().filter(|a| a.path().is_ident("derive")).any(|attr| {
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.segments.last().is_some_and(|s| s.ident == name) {
                found = true;
            }
            Ok(())
        });
        found
    })
}

/// Reads a whole Rust file: the structs that derive `BusinessObject` and the enums that derive
/// `BusinessEnum`. Every problem found is returned, each with its place.
pub fn parse_file(text: &str) -> std::result::Result<FileSpec, Vec<SpecError>> {
    let file = syn::parse_file(text).map_err(|e| SpecError::all(&e))?;
    let mut spec = FileSpec::default();
    let mut errors = Vec::new();
    for item in file.items {
        match item {
            syn::Item::Struct(item) if derives(&item.attrs, "BusinessObject") => {
                match parse_object(&DeriveInput::from(item)) {
                    Ok(object) => spec.objects.push(object),
                    Err(e) => errors.extend(SpecError::all(&e)),
                }
            }
            syn::Item::Enum(item) if derives(&item.attrs, "BusinessEnum") => {
                match parse_enum(&DeriveInput::from(item)) {
                    Ok(options) => spec.enums.push(options),
                    Err(e) => errors.extend(SpecError::all(&e)),
                }
            }
            other => spec.items.push(other),
        }
    }
    if errors.is_empty() { Ok(spec) } else { Err(errors) }
}
