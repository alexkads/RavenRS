//! The derives of `ravenrs-app`, which re-exports them. See `BusinessObject` there.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{quote, quote_spanned};
use ravenrs_criteria::Criteria;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Error, Fields, Ident, LitInt, LitStr, Result, parse_macro_input};

/// Declares a struct as a business object: an entity for the model, and the way to and from
/// a record.
///
/// On the struct, `#[object(caption = "...", navigation = "...", icon = "...",
/// list_criteria = "...", form_columns = 2, cloneable, transient, checks_on_leave, list_editable,
/// data_access = "server", form_actions = "name")]`, where `data_access` is `client`, `server` or
/// `data_view`.
///
/// On a field, `#[field(...)]`:
///
/// * `key` numbered by the application, or `typed_key` typed by people. Exactly one field has one.
///   With `guid`, a `key` is made up when a record is created instead of being numbered.
/// * `caption = "..."` what people see.
/// * `default_property` what names a record. `read_only`. `no_list`, `no_detail`, `lookup`.
/// * What it holds, when the type does not say: `money`, `long`, `color`, `date`, `guid`,
///   `decimals = 2`, `reference = "Entity"`, `collection = "Entity"`, `tags = "a,b"`.
/// * `calculated = "[a] + [b]"`, in the criteria language, checked here.
/// * `default = ...` what a new record starts with.
/// * `required`, `unique`, `min = 1`, `max = 10`, `matches = "^[A-Z]+$"`: what it has to meet on
///   saving, and `message = "..."` what to say instead of the usual words.
/// * On a form: `editor = "..."`, `placeholder = "..."`, `hint = "..."`, `wide`, `autofocus`,
///   `group = "..."` for the titled group it goes in, `edit_mask = "(00) 0000-0000"`,
///   `predefined = "a,b"` for values to pick from, `criteria_of = "Entity"` for a filter builder.
/// * `display_format = "{:.2} %"`, how its value shows when it is not being edited.
/// * On a reference: `lookup_filter = "[flow] = ?flow"`, `none_caption = "..."`. On a collection:
///   `back_reference = "..."`, `aggregated`. `non_cloneable` stays behind on cloning, and
///   `most_bytes = 1024` limits a picture or a file.
///
/// On the struct, as many as needed:
///
/// * `#[rule(criteria = "[a] <= [b]", message = "...", context = "Pay", target = "[c] > 0")]`, or
///   `#[rule(unique = "a,b")]`. With `warning` people can go on anyway, and with `information` it
///   only leaves a note. A message can say `{field}` and `{reference.field}`.
/// * `#[appearance(names = "a,b*", criteria = "...", font = "danger", back = "warning", bold,
///   enabled = false, visible = false, priority = 1, context = "list", action)]`. With `layout`,
///   the names are groups of a form; with `action`, actions. A context is `list`, `detail` or a
///   view's id. Tints are `danger`, `warning`, `success`, `accent` and `muted`.
/// * `#[variant(id = "...", caption = "...", filter = "...", columns = "a,b")]` and
///   `#[list_filter(id = "...", caption = "...", filter = "...")]`, for people to pick on the list.
#[proc_macro_derive(BusinessObject, attributes(object, field, rule, appearance, variant, list_filter))]
pub fn derive_business_object(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    business_object(&input).unwrap_or_else(Error::into_compile_error).into()
}

/// Declares an enum with no data as the options of a field: `#[option(caption = "...", code = "...")]`
/// on a variant. The first variant is what a new record starts with.
#[proc_macro_derive(BusinessEnum, attributes(option))]
pub fn derive_business_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    business_enum(&input).unwrap_or_else(Error::into_compile_error).into()
}

#[derive(Default)]
struct ObjectAttrs {
    caption: Option<LitStr>,
    navigation: Option<LitStr>,
    icon: Option<LitStr>,
    list_criteria: Option<LitStr>,
    form_columns: Option<LitInt>,
    cloneable: bool,
    transient: bool,
    checks_on_leave: bool,
    list_editable: bool,
    data_access: Option<LitStr>,
    form_actions: Vec<LitStr>,
}

fn object_attrs(input: &DeriveInput) -> Result<ObjectAttrs> {
    let mut attrs = ObjectAttrs::default();
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
                if !["client", "server", "data_view"].contains(&text.value().as_str()) {
                    return Err(Error::new(text.span(), "use client, server or data_view"));
                }
                attrs.data_access = Some(text);
            } else if meta.path.is_ident("form_actions") {
                attrs.form_actions.push(meta.value()?.parse()?);
            } else {
                return Err(meta.error(
                    "unknown setting: use caption, navigation, icon, list_criteria, form_columns, cloneable, \
                     transient, checks_on_leave, list_editable, data_access or form_actions",
                ));
            }
            Ok(())
        })?;
    }
    Ok(attrs)
}

#[derive(Default)]
struct FieldAttrs {
    key: bool,
    typed_key: bool,
    caption: Option<LitStr>,
    default_property: bool,
    read_only: bool,
    no_list: bool,
    no_detail: bool,
    lookup: bool,
    kind: Option<Tokens>,
    calculated: Option<LitStr>,
    default: Option<syn::Expr>,
    required: bool,
    unique: bool,
    min: Option<syn::Expr>,
    max: Option<syn::Expr>,
    matches: Option<LitStr>,
    message: Option<LitStr>,
    editor: Option<LitStr>,
    placeholder: Option<LitStr>,
    hint: Option<LitStr>,
    lookup_filter: Option<LitStr>,
    none_caption: Option<LitStr>,
    back_reference: Option<LitStr>,
    most_bytes: Option<LitInt>,
    wide: bool,
    autofocus: bool,
    non_cloneable: bool,
    aggregated: bool,
    group: Option<LitStr>,
    display_format: Option<LitStr>,
    edit_mask: Option<LitStr>,
    predefined: Option<LitStr>,
    criteria_of: Option<LitStr>,
}

fn set_kind(attrs: &mut FieldAttrs, path: &syn::Path, kind: Tokens) -> Result<()> {
    if attrs.kind.is_some() {
        return Err(Error::new(path.span(), "a field holds one kind of thing: this says a second"));
    }
    attrs.kind = Some(kind);
    Ok(())
}

fn field_attrs(field: &syn::Field) -> Result<FieldAttrs> {
    let mut attrs = FieldAttrs::default();
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
                set_kind(&mut attrs, path, quote!(::ravenrs_app::FieldKind::Money))?;
            } else if named("long") {
                set_kind(&mut attrs, path, quote!(::ravenrs_app::FieldKind::LongText))?;
            } else if named("color") {
                set_kind(&mut attrs, path, quote!(::ravenrs_app::FieldKind::Color))?;
            } else if named("date") {
                set_kind(&mut attrs, path, quote!(::ravenrs_app::FieldKind::Date))?;
            } else if named("guid") {
                set_kind(&mut attrs, path, quote!(::ravenrs_app::FieldKind::Guid))?;
            } else if named("tags") {
                let options: LitStr = meta.value()?.parse()?;
                let names = split_names(&options);
                set_kind(
                    &mut attrs,
                    path,
                    quote!(::ravenrs_app::FieldKind::Tags(::std::vec![
                        #(::ravenrs_app::EnumOption::new(#names, #names)),*
                    ])),
                )?;
            } else if named("decimals") {
                let places: LitInt = meta.value()?.parse()?;
                let places: u8 = places.base10_parse()?;
                set_kind(&mut attrs, path, quote!(::ravenrs_app::FieldKind::Decimal(#places)))?;
            } else if named("reference") {
                let target: LitStr = meta.value()?.parse()?;
                set_kind(&mut attrs, path, quote!(::ravenrs_app::FieldKind::Reference(#target.to_owned())))?;
            } else if named("collection") {
                let target: LitStr = meta.value()?.parse()?;
                set_kind(&mut attrs, path, quote!(::ravenrs_app::FieldKind::Collection(#target.to_owned())))?;
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
            } else if named("aggregated") {
                attrs.aggregated = true;
            } else if named("group") {
                attrs.group = Some(meta.value()?.parse()?);
            } else if named("display_format") {
                attrs.display_format = Some(meta.value()?.parse()?);
            } else if named("edit_mask") {
                attrs.edit_mask = Some(meta.value()?.parse()?);
            } else if named("predefined") {
                attrs.predefined = Some(meta.value()?.parse()?);
            } else if named("criteria_of") {
                attrs.criteria_of = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error(
                    "unknown setting: use key, typed_key, caption, default_property, read_only, no_list, \
                     no_detail, lookup, money, long, color, date, guid, tags, decimals, reference, collection, \
                     calculated, default, required, unique, min, max, matches, message, editor, placeholder, \
                     hint, lookup_filter, none_caption, back_reference, most_bytes, wide, autofocus, \
                     non_cloneable, aggregated, group, display_format, edit_mask, predefined or criteria_of",
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

fn check_expression(expression: &LitStr, names: &[String]) -> Result<()> {
    let parsed = Criteria::parse(&expression.value())
        .map_err(|e| Error::new(expression.span(), format!("this expression does not parse: {e}")))?;
    for property in parsed.properties() {
        if !names.contains(&property) {
            return Err(Error::new(expression.span(), format!("this struct has no field `{property}`")));
        }
    }
    Ok(())
}

fn field_rules(ident: &Ident, attrs: &FieldAttrs) -> Vec<Tokens> {
    let name = ident.to_string();
    let mut rules = Vec::new();
    if attrs.required {
        rules.push(quote!(::ravenrs_app::Rule::required(#name)));
    }
    if attrs.unique {
        rules.push(quote!(::ravenrs_app::Rule::unique(#name)));
    }
    if attrs.min.is_some() || attrs.max.is_some() {
        let limit = |value: &Option<syn::Expr>| match value {
            Some(value) => quote!(::std::option::Option::Some((#value) as f64)),
            None => quote!(::std::option::Option::None),
        };
        let (min, max) = (limit(&attrs.min), limit(&attrs.max));
        rules.push(quote!(::ravenrs_app::Rule::range(#name, #min, #max)));
    }
    if let Some(pattern) = &attrs.matches {
        rules.push(quote!(::ravenrs_app::Rule::matches(#name, #pattern)));
    }
    if let Some(message) = &attrs.message {
        for rule in &mut rules {
            *rule = quote!(#rule.message(#message));
        }
    }
    rules
}

fn split_names(text: &LitStr) -> Vec<String> {
    text.value().split(',').map(|name| name.trim().to_owned()).filter(|name| !name.is_empty()).collect()
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

fn struct_rules(input: &DeriveInput, names: &[String]) -> Result<Vec<Tokens>> {
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
        let mut rule = match (&found.criteria, &found.unique) {
            (Some(expression), None) => {
                check_expression(expression, names)?;
                quote!(::ravenrs_app::Rule::criteria(#id, #expression))
            }
            (None, Some(fields)) => {
                let fields = split_names(fields);
                if let Some(missing) = fields.iter().find(|f| !names.contains(f)) {
                    return Err(Error::new(attr.span(), format!("this struct has no field `{missing}`")));
                }
                quote!(::ravenrs_app::Rule::new(
                    #id,
                    ::ravenrs_app::RuleKind::UniqueTogether(::std::vec![#(#fields.to_owned()),*]),
                ))
            }
            _ => return Err(Error::new(attr.span(), "a rule says either criteria or unique")),
        };
        if let Some(text) = &found.message {
            rule = quote!(#rule.message(#text));
        }
        if let Some(text) = &found.context {
            rule = quote!(#rule.contexts([#text]));
        }
        if let Some(expression) = &found.target {
            check_expression(expression, names)?;
            rule = quote!(#rule.target_criteria(#expression));
        }
        if found.warning && found.information {
            return Err(Error::new(attr.span(), "a rule is a warning or information, not both"));
        }
        if found.warning {
            rule = quote!(#rule.warning());
        }
        if found.information {
            rule = quote!(#rule.information());
        }
        rules.push(rule);
    }
    Ok(rules)
}

fn tint(text: &LitStr) -> Result<Tokens> {
    Ok(match text.value().as_str() {
        "danger" => quote!(::ravenrs_app::Tint::Danger),
        "warning" => quote!(::ravenrs_app::Tint::Warning),
        "success" => quote!(::ravenrs_app::Tint::Success),
        "accent" => quote!(::ravenrs_app::Tint::Accent),
        "muted" => quote!(::ravenrs_app::Tint::Muted),
        _ => return Err(Error::new(text.span(), "use danger, warning, success, accent or muted")),
    })
}

fn struct_appearance(input: &DeriveInput, names: &[String]) -> Result<Vec<Tokens>> {
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
                pieces.push(quote!(.criteria(#expression)));
            } else if named("font") {
                let color = tint(&meta.value()?.parse()?)?;
                pieces.push(quote!(.font(#color)));
            } else if named("back") {
                let color = tint(&meta.value()?.parse()?)?;
                pieces.push(quote!(.back(#color)));
            } else if named("bold") {
                pieces.push(quote!(.bold()));
            } else if named("enabled") {
                let on: syn::LitBool = meta.value()?.parse()?;
                pieces.push(quote!(.enabled(#on)));
            } else if named("visible") {
                let on: syn::LitBool = meta.value()?.parse()?;
                pieces.push(quote!(.visible(#on)));
            } else if named("priority") {
                let priority: LitInt = meta.value()?.parse()?;
                pieces.push(quote!(.priority(#priority)));
            } else if named("action") {
                pieces.push(quote!(.target(::ravenrs_app::AppearanceTarget::Action)));
            } else if named("layout") {
                pieces.push(quote!(.target(::ravenrs_app::AppearanceTarget::Layout)));
            } else if named("context") {
                let context: LitStr = meta.value()?.parse()?;
                let context = match context.value().as_str() {
                    "list" => quote!(::ravenrs_app::AppearanceContext::List),
                    "detail" => quote!(::ravenrs_app::AppearanceContext::Detail),
                    _ => quote!(::ravenrs_app::AppearanceContext::View(#context.to_owned())),
                };
                pieces.push(quote!(.context(#context)));
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
        let targets = split_names(&targets);
        let id = format!("appearance{}", rules.len());
        rules.push(quote!(::ravenrs_app::AppearanceRule::new(#id, [#(#targets),*]) #(#pieces)*));
    }
    Ok(rules)
}

fn field_tokens(ident: &Ident, ty: &syn::Type, attrs: &FieldAttrs) -> Tokens {
    let name = ident.to_string();
    let kind = attrs
        .kind
        .clone()
        .unwrap_or_else(|| quote_spanned!(ty.span()=> <#ty as ::ravenrs_app::FieldValue>::kind()));
    let mut info = quote!(::ravenrs_app::FieldInfo::new(#name, #kind));
    let mut then = |piece: Tokens| info = quote!(#info #piece);
    if attrs.key {
        then(quote!(.key()));
    }
    if attrs.typed_key {
        then(quote!(.typed_key()));
    }
    if let Some(caption) = &attrs.caption {
        then(quote!(.caption(#caption)));
    }
    if attrs.default_property {
        then(quote!(.default_property()));
    }
    if attrs.read_only {
        then(quote!(.read_only()));
    }
    if attrs.no_list {
        then(quote!(.in_list(false)));
    }
    if attrs.no_detail {
        then(quote!(.in_detail(false)));
    }
    if attrs.lookup {
        then(quote!(.in_lookup(true)));
    }
    if let Some(expression) = &attrs.calculated {
        then(quote!(.calculated(#expression)));
    }
    if let Some(value) = &attrs.default {
        then(quote!(.default_value(#value)));
    }
    if let Some(editor) = &attrs.editor {
        then(quote!(.editor(#editor)));
    }
    if let Some(text) = &attrs.placeholder {
        then(quote!(.placeholder(#text)));
    }
    if let Some(text) = &attrs.hint {
        then(quote!(.hint(#text)));
    }
    if let Some(text) = &attrs.lookup_filter {
        then(quote!(.lookup_filter(#text)));
    }
    if let Some(text) = &attrs.none_caption {
        then(quote!(.none_caption(#text)));
    }
    if let Some(text) = &attrs.back_reference {
        then(quote!(.back_reference(#text)));
    }
    if let Some(bytes) = &attrs.most_bytes {
        then(quote!(.most_bytes(#bytes)));
    }
    if attrs.wide {
        then(quote!(.wide()));
    }
    if attrs.autofocus {
        then(quote!(.autofocus()));
    }
    if attrs.non_cloneable {
        then(quote!(.non_cloneable()));
    }
    if attrs.aggregated {
        then(quote!(.aggregated()));
    }
    if let Some(text) = &attrs.group {
        then(quote!(.group(#text)));
    }
    if let Some(text) = &attrs.display_format {
        then(quote!(.display_format(#text)));
    }
    if let Some(text) = &attrs.edit_mask {
        then(quote!(.edit_mask(#text)));
    }
    if let Some(text) = &attrs.predefined {
        let values = split_names(text);
        then(quote!(.predefined([#(#values),*])));
    }
    if let Some(text) = &attrs.criteria_of {
        then(quote!(.criteria_of(#text)));
    }
    info
}

/// `#[variant(...)]` and `#[list_filter(...)]` on a struct, as calls on its entity.
fn struct_choices(input: &DeriveInput, names: &[String]) -> Result<Vec<Tokens>> {
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
        let filter =
            filter.map(|text| if is_variant { quote!(.filter(#text)) } else { quote!(.criteria(#text)) });
        found.push(if is_variant {
            let shown = (!shown.is_empty()).then(|| quote!(.columns([#(#shown),*])));
            quote!(.variant(::ravenrs_app::ViewVariant::new(#id, #caption) #filter #shown))
        } else {
            quote!(.filter(::ravenrs_app::ListFilter::new(#id, #caption) #filter))
        });
    }
    Ok(found)
}

fn business_object(input: &DeriveInput) -> Result<Tokens> {
    let name = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(Error::new(input.generics.span(), "a business object cannot be generic"));
    }
    let Data::Struct(data) = &input.data else {
        return Err(Error::new(name.span(), "a business object is a struct with named fields"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(Error::new(name.span(), "a business object is a struct with named fields"));
    };
    let object = object_attrs(input)?;
    let names: Vec<String> =
        fields.named.iter().filter_map(|f| f.ident.as_ref().map(Ident::to_string)).collect();
    let mut infos = Vec::new();
    let mut rules = Vec::new();
    let mut to_values = Vec::new();
    let mut from_values = Vec::new();
    let mut keys = 0;
    for field in &fields.named {
        let Some(ident) = &field.ident else { continue };
        let attrs = field_attrs(field)?;
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
        infos.push(field_tokens(ident, &field.ty, &attrs));
        rules.extend(field_rules(ident, &attrs));
        let (text, ty) = (ident.to_string(), &field.ty);
        to_values.push(quote_spanned!(ty.span()=>
            (#text.to_owned(), <#ty as ::ravenrs_app::FieldValue>::to_value(&self.#ident))
        ));
        from_values.push(quote_spanned!(ty.span()=>
            #ident: <#ty as ::ravenrs_app::FieldValue>::from_value(object.get(#text))
        ));
    }
    if keys == 0 {
        return Err(Error::new(
            name.span(),
            "a business object needs a key: mark a field with #[field(key)] or #[field(typed_key)]",
        ));
    }
    rules.extend(struct_rules(input, &names)?);
    let appearance = struct_appearance(input, &names)?;
    let entity = name.to_string();
    let mut info = quote!(::ravenrs_app::EntityInfo::new(#entity));
    if let Some(caption) = &object.caption {
        info = quote!(#info.caption(#caption));
    }
    if let Some(place) = &object.navigation {
        info = quote!(#info.navigation(#place));
    }
    if let Some(icon) = &object.icon {
        info = quote!(#info.icon(#icon));
    }
    if let Some(text) = &object.list_criteria {
        info = quote!(#info.list_criteria(#text));
    }
    if object.list_editable {
        info = quote!(#info.list_editable());
    }
    if let Some(access) = &object.data_access {
        let access = match access.value().as_str() {
            "server" => quote!(::ravenrs_app::DataAccess::Server),
            "data_view" => quote!(::ravenrs_app::DataAccess::DataView),
            _ => quote!(::ravenrs_app::DataAccess::Client),
        };
        info = quote!(#info.data_access(#access));
    }
    for name in &object.form_actions {
        info = quote!(#info.form_actions(#name));
    }
    if let Some(columns) = &object.form_columns {
        info = quote!(#info.form_columns(#columns));
    }
    if object.cloneable {
        info = quote!(#info.cloneable());
    }
    if object.transient {
        info = quote!(#info.transient());
    }
    if object.checks_on_leave {
        info = quote!(#info.checks_on_leave());
    }
    let choices = struct_choices(input, &names)?;
    Ok(quote! {
        impl ::ravenrs_app::BusinessObject for #name {
            fn entity_info() -> ::ravenrs_app::EntityInfo {
                #info #(.field(#infos))* #(.rule(#rules))* #(.appearance(#appearance))* #(#choices)*
            }

            fn to_object(&self) -> ::ravenrs_app::Object {
                ::ravenrs_app::Object::from_values(
                    #entity,
                    ::std::collections::BTreeMap::from([#(#to_values),*]),
                )
            }

            fn from_object(object: &::ravenrs_app::Object) -> Self {
                Self { #(#from_values),* }
            }
        }
    })
}

fn business_enum(input: &DeriveInput) -> Result<Tokens> {
    let name = &input.ident;
    let Data::Enum(data) = &input.data else {
        return Err(Error::new(name.span(), "a business enum is an enum with no data"));
    };
    let Some(first) = data.variants.first() else {
        return Err(Error::new(name.span(), "a business enum needs at least one option"));
    };
    let first = &first.ident;
    let mut options = Vec::new();
    let mut codes = Vec::new();
    let mut parses = Vec::new();
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
        options.push(quote!(::ravenrs_app::EnumOption::new(#code, #caption)));
        codes.push(quote!(Self::#ident => #code));
        parses.push(quote!(#code => Self::#ident));
    }
    Ok(quote! {
        impl ::ravenrs_app::FieldValue for #name {
            fn kind() -> ::ravenrs_app::FieldKind {
                ::ravenrs_app::FieldKind::Enum(::std::vec![#(#options),*])
            }

            fn to_value(&self) -> ::ravenrs_app::Value {
                ::ravenrs_app::Value::Text(match self { #(#codes),* }.to_owned())
            }

            fn from_value(value: ::ravenrs_app::Value) -> Self {
                match value {
                    ::ravenrs_app::Value::Text(code) => match code.as_str() {
                        #(#parses,)*
                        _ => Self::#first,
                    },
                    _ => Self::#first,
                }
            }
        }
    })
}
