//! Entities read from Rust source while the program runs: the structs and enums that would
//! derive `BusinessObject` and `BusinessEnum`, read by the same code the derives use.

use std::collections::BTreeMap;

use ravenrs_app_spec::{
    AppearancePiece, ChoiceSpec, ContextSpec, DataAccessSpec, FieldSpec, FileSpec, KindSpec, ObjectSpec,
    RuleKindSpec, RuleSpec, SeveritySpec, TintSpec, TypeKind, parse_file, type_kind,
};
use ravenrs_criteria::Value;
use syn::spanned::Spanned;

use crate::{
    AppearanceContext, AppearanceRule, AppearanceTarget, DataAccess, EntityInfo, EnumOption, FieldInfo,
    FieldKind, ListFilter, Rule, RuleKind, Tint, ViewVariant,
};

/// Something wrong at a place of the source: its line and column (both from 1), how many
/// characters it covers, and what is wrong.
pub use ravenrs_app_spec::SpecError as SourceError;

/// The entities of a Rust source file: each struct with `#[derive(BusinessObject)]`, its enum
/// fields taken from the enums with `#[derive(BusinessEnum)]` in the same file. What the
/// derive refuses is refused here, with the same words; what only a compiler could work out,
/// such as a default that calls a function, is refused too, with its place.
///
/// ```
/// let entities = ravenrs_app::entities_from_source(
///     r#"
///     #[derive(BusinessEnum)]
///     enum Kind { Checking, Savings }
///
///     #[derive(BusinessObject)]
///     #[object(caption = "Accounts")]
///     struct Account {
///         #[field(key)]
///         id: i64,
///         #[field(caption = "Kind")]
///         kind: Kind,
///     }
///     "#,
/// )
/// .map_err(|errors| errors[0].to_string())?;
/// assert_eq!(entities[0].name, "Account");
/// # Ok::<(), String>(())
/// ```
pub fn entities_from_source(text: &str) -> Result<Vec<EntityInfo>, Vec<SourceError>> {
    entities_from_spec(&parse_file(text)?)
}

/// The entities of a file already read with [`ravenrs_app_spec::parse_file`].
pub fn entities_from_spec(file: &FileSpec) -> Result<Vec<EntityInfo>, Vec<SourceError>> {
    let enums: BTreeMap<String, Vec<EnumOption>> = file
        .enums
        .iter()
        .map(|e| {
            let options =
                e.options.iter().map(|o| EnumOption::new(o.code.clone(), o.caption.clone())).collect();
            (e.name.to_string(), options)
        })
        .collect();
    let mut entities = Vec::new();
    let mut errors = Vec::new();
    for object in &file.objects {
        match entity(object, &enums) {
            Ok(entity) => entities.push(entity),
            Err(found) => errors.extend(found),
        }
    }
    if errors.is_empty() { Ok(entities) } else { Err(errors) }
}

fn entity(
    object: &ObjectSpec,
    enums: &BTreeMap<String, Vec<EnumOption>>,
) -> Result<EntityInfo, Vec<SourceError>> {
    let settings = &object.settings;
    let mut info = EntityInfo::new(object.name.to_string());
    if let Some(caption) = &settings.caption {
        info = info.caption(caption.value());
    }
    if let Some(place) = &settings.navigation {
        info = info.navigation(place.value());
    }
    if let Some(icon) = &settings.icon {
        info = info.icon(icon.value());
    }
    if let Some(text) = &settings.list_criteria {
        info = info.list_criteria(text.value());
    }
    if settings.list_editable {
        info = info.list_editable();
    }
    if let Some(access) = settings.data_access {
        info = info.data_access(match access {
            DataAccessSpec::Client => DataAccess::Client,
            DataAccessSpec::Server => DataAccess::Server,
            DataAccessSpec::DataView => DataAccess::DataView,
        });
    }
    for name in &settings.form_actions {
        info = info.form_actions(name.value());
    }
    let mut errors = Vec::new();
    if let Some(columns) = &settings.form_columns {
        match columns.base10_parse::<u8>() {
            Ok(columns) => info = info.form_columns(columns),
            Err(e) => errors.push(SourceError::at(columns.span(), e.to_string())),
        }
    }
    if !settings.focus_order.is_empty() {
        info = info.focus_order(settings.focus_order.clone());
    }
    if settings.cloneable {
        info = info.cloneable();
    }
    if settings.transient {
        info = info.transient();
    }
    if settings.checks_on_leave {
        info = info.checks_on_leave();
    }
    for field in &object.fields {
        match field_info(field, enums) {
            Ok(field) => info = info.field(field),
            Err(e) => errors.push(e),
        }
    }
    for rule in &object.rules {
        match rule_of(rule) {
            Ok(rule) => info = info.rule(rule),
            Err(e) => errors.push(e),
        }
    }
    for spec in &object.appearance {
        let mut rule = AppearanceRule::new(spec.id.clone(), spec.targets.clone());
        for piece in &spec.pieces {
            rule = match piece {
                AppearancePiece::Criteria(expression) => rule.criteria(expression.value()),
                AppearancePiece::Font(color) => rule.font(tint(*color)),
                AppearancePiece::Back(color) => rule.back(tint(*color)),
                AppearancePiece::Bold => rule.bold(),
                AppearancePiece::Enabled(on) => rule.enabled(on.value),
                AppearancePiece::Visible(on) => rule.visible(on.value),
                AppearancePiece::Priority(priority) => match priority.base10_parse::<i32>() {
                    Ok(priority) => rule.priority(priority),
                    Err(e) => {
                        errors.push(SourceError::at(priority.span(), e.to_string()));
                        rule
                    }
                },
                AppearancePiece::Action => rule.target(AppearanceTarget::Action),
                AppearancePiece::Layout => rule.target(AppearanceTarget::Layout),
                AppearancePiece::Context(context) => rule.context(match context {
                    ContextSpec::List => AppearanceContext::List,
                    ContextSpec::Detail => AppearanceContext::Detail,
                    ContextSpec::View(view) => AppearanceContext::View(view.value()),
                }),
            };
        }
        info = info.appearance(rule);
    }
    for choice in &object.choices {
        info = match choice {
            ChoiceSpec::Variant { id, caption, filter, columns } => {
                let mut variant = ViewVariant::new(id.value(), caption.value());
                if let Some(text) = filter {
                    variant = variant.filter(text.value());
                }
                if !columns.is_empty() {
                    variant = variant.columns(columns.clone());
                }
                info.variant(variant)
            }
            ChoiceSpec::ListFilter { id, caption, filter } => {
                let mut list_filter = ListFilter::new(id.value(), caption.value());
                if let Some(text) = filter {
                    list_filter = list_filter.criteria(text.value());
                }
                info.filter(list_filter)
            }
        };
    }
    if errors.is_empty() { Ok(info) } else { Err(errors) }
}

fn tint(color: TintSpec) -> Tint {
    match color {
        TintSpec::Danger => Tint::Danger,
        TintSpec::Warning => Tint::Warning,
        TintSpec::Success => Tint::Success,
        TintSpec::Accent => Tint::Accent,
        TintSpec::Muted => Tint::Muted,
    }
}

fn kind_of(field: &FieldSpec, enums: &BTreeMap<String, Vec<EnumOption>>) -> Result<FieldKind, SourceError> {
    if let Some(kind) = &field.settings.kind {
        return Ok(match kind {
            KindSpec::Money => FieldKind::Money,
            KindSpec::LongText => FieldKind::LongText,
            KindSpec::Color => FieldKind::Color,
            KindSpec::Date => FieldKind::Date,
            KindSpec::Guid => FieldKind::Guid,
            KindSpec::Tags(names) => FieldKind::Tags(names.iter().map(|n| EnumOption::new(n, n)).collect()),
            KindSpec::Decimal(places) => FieldKind::Decimal(*places),
            KindSpec::Reference(target) => FieldKind::Reference(target.value()),
            KindSpec::Collection(target) => FieldKind::Collection(target.value()),
        });
    }
    let unknown = || {
        SourceError::at(
            field.ty.span(),
            "a field is a String, bool, i32, i64, u32, u64, usize, f64, DateTime, an Option of one, or an \
             enum with #[derive(BusinessEnum)] in this file",
        )
    };
    Ok(match type_kind(&field.ty).ok_or_else(unknown)? {
        TypeKind::Text => FieldKind::Text,
        TypeKind::Boolean => FieldKind::Boolean,
        TypeKind::Integer => FieldKind::Integer,
        TypeKind::Decimal => FieldKind::Decimal(2),
        TypeKind::DateTime => FieldKind::DateTime,
        TypeKind::Named(name) => FieldKind::Enum(enums.get(&name).cloned().ok_or_else(unknown)?),
    })
}

/// A literal, a negative number, a text with `.into()` or `.to_owned()`, or an enum option.
fn value_of(expression: &syn::Expr, kind: &FieldKind) -> Result<Value, SourceError> {
    let refused = || {
        SourceError::at(
            expression.span(),
            "while the program runs only literals and enum options are read here, such as 3, -1.5, true, \"text\" \
             or Kind::Option",
        )
    };
    match expression {
        syn::Expr::Lit(literal) => match &literal.lit {
            syn::Lit::Int(n) => n.base10_parse::<i64>().map(Value::from).map_err(|_| refused()),
            syn::Lit::Float(n) => n.base10_parse::<f64>().map(Value::from).map_err(|_| refused()),
            syn::Lit::Bool(b) => Ok(Value::from(b.value)),
            syn::Lit::Str(s) => Ok(Value::from(s.value())),
            _ => Err(refused()),
        },
        syn::Expr::Unary(syn::ExprUnary { op: syn::UnOp::Neg(_), expr, .. }) => match value_of(expr, kind)? {
            Value::Number(n) => Ok(Value::Number(-n)),
            _ => Err(refused()),
        },
        syn::Expr::MethodCall(call)
            if call.args.is_empty()
                && ["into", "to_owned", "to_string"].contains(&call.method.to_string().as_str()) =>
        {
            value_of(&call.receiver, kind)
        }
        syn::Expr::Paren(inner) => value_of(&inner.expr, kind),
        syn::Expr::Path(path) => {
            let option = path.path.segments.last().map(|s| s.ident.to_string()).ok_or_else(refused)?;
            match kind {
                FieldKind::Enum(options) if options.iter().any(|o| o.code == option) => {
                    Ok(Value::Text(option))
                }
                _ => Err(refused()),
            }
        }
        _ => Err(refused()),
    }
}

fn field_info(
    field: &FieldSpec,
    enums: &BTreeMap<String, Vec<EnumOption>>,
) -> Result<FieldInfo, SourceError> {
    let attrs = &field.settings;
    let kind = kind_of(field, enums)?;
    let default = attrs.default.as_ref().map(|value| value_of(value, &kind)).transpose()?;
    let mut info = FieldInfo::new(field.ident.to_string(), kind);
    if attrs.key {
        info = info.key();
    }
    if attrs.typed_key {
        info = info.typed_key();
    }
    if let Some(caption) = &attrs.caption {
        info = info.caption(caption.value());
    }
    if attrs.default_property {
        info = info.default_property();
    }
    if attrs.read_only {
        info = info.read_only();
    }
    if attrs.no_list {
        info = info.in_list(false);
    }
    if attrs.no_detail {
        info = info.in_detail(false);
    }
    if attrs.lookup {
        info = info.in_lookup(true);
    }
    if let Some(expression) = &attrs.calculated {
        info = info.calculated(expression.value());
    }
    if let Some(value) = default {
        info = info.default_value(value);
    }
    if let Some(editor) = &attrs.editor {
        info = info.editor(editor.value());
    }
    if let Some(text) = &attrs.placeholder {
        info = info.placeholder(text.value());
    }
    if let Some(text) = &attrs.hint {
        info = info.hint(text.value());
    }
    if let Some(text) = &attrs.accessible_name {
        info = info.accessible_name(text.value());
    }
    if let Some(text) = &attrs.accessible_description {
        info = info.accessible_description(text.value());
    }
    if let Some(text) = &attrs.lookup_filter {
        info = info.lookup_filter(text.value());
    }
    if let Some(text) = &attrs.none_caption {
        info = info.none_caption(text.value());
    }
    if let Some(text) = &attrs.back_reference {
        info = info.back_reference(text.value());
    }
    if let Some(bytes) = &attrs.most_bytes {
        let bytes = bytes.base10_parse::<u64>().map_err(|e| SourceError::at(bytes.span(), e.to_string()))?;
        info = info.most_bytes(bytes);
    }
    if attrs.wide {
        info = info.wide();
    }
    if attrs.autofocus {
        info = info.autofocus();
    }
    if attrs.secret {
        info = info.secret();
    }
    if attrs.non_cloneable {
        info = info.non_cloneable();
    }
    if attrs.aggregated {
        info = info.aggregated();
    }
    if let Some(text) = &attrs.group {
        info = info.group(text.value());
    }
    if let Some(text) = &attrs.display_format {
        info = info.display_format(text.value());
    }
    if let Some(text) = &attrs.edit_mask {
        info = info.edit_mask(text.value());
    }
    if let Some(values) = &attrs.predefined {
        info = info.predefined(values.clone());
    }
    if let Some(text) = &attrs.criteria_of {
        info = info.criteria_of(text.value());
    }
    Ok(info)
}

fn limit(value: &Option<Box<syn::Expr>>) -> Result<Option<f64>, SourceError> {
    match value {
        None => Ok(None),
        Some(expression) => match value_of(expression, &FieldKind::Integer)? {
            Value::Number(n) => Ok(Some(n)),
            _ => Err(SourceError::at(expression.span(), "a limit is a number")),
        },
    }
}

fn rule_of(spec: &RuleSpec) -> Result<Rule, SourceError> {
    let mut rule = match &spec.kind {
        RuleKindSpec::Required(name) => Rule::required(name),
        RuleKindSpec::Unique(name) => Rule::unique(name),
        RuleKindSpec::Range { field, min, max } => Rule::range(field, limit(min)?, limit(max)?),
        RuleKindSpec::Matches(name, pattern) => Rule::matches(name, pattern.value()),
        RuleKindSpec::Criteria(id, expression) => Rule::criteria(id, expression.value()),
        RuleKindSpec::UniqueTogether(id, fields) => Rule::new(id, RuleKind::UniqueTogether(fields.clone())),
    };
    if let Some(text) = &spec.message {
        rule = rule.message(text.value());
    }
    if let Some(text) = &spec.context {
        rule = rule.contexts([text.value()]);
    }
    if let Some(expression) = &spec.target {
        rule = rule.target_criteria(expression.value());
    }
    Ok(match spec.severity {
        SeveritySpec::Error => rule,
        SeveritySpec::Warning => rule.warning(),
        SeveritySpec::Information => rule.information(),
    })
}
