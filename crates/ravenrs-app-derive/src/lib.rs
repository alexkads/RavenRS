//! The derives of `ravenrs-app`, which re-exports them. See `BusinessObject` there.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{quote, quote_spanned};
use ravenrs_app_spec::{
    ActionSpec, AppearancePiece, AppearanceSpec, ChoiceSpec, ContextSpec, DataAccessSpec, FieldSpec,
    KindSpec, RuleKindSpec, RuleSpec, SelectionSpec, SeveritySpec, TintSpec, parse_enum, parse_object,
};
use syn::spanned::Spanned;
use syn::{DeriveInput, Error, Result, parse_macro_input};

/// Declares a struct as a business object: an entity for the model, and the way to and from
/// a record.
///
/// On the struct, `#[object(caption = "...", navigation = "...", icon = "...", empty_text = "...",
/// list_criteria = "...", form_columns = 2, cloneable, singleton, transient, checks_on_leave, list_editable,
/// data_access = "server", form_actions = "name", focus_order = "a,b")]`, where `data_access` is
/// `client`, `server` or `data_view`, and `focus_order` the fields the form goes through first.
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
/// * For screen readers: `accessible_name = "..."` instead of the caption, and
///   `accessible_description = "..."` instead of the hint.
/// * `display_format = "{:.2} %"`, how its value shows when it is not being edited.
/// * On a reference: `lookup_filter = "[flow] = ?flow"`, `none_caption = "..."`. On a collection:
///   `back_reference = "..."`, `aggregated`. `secret` never leaves a secured space. `non_cloneable` stays behind on cloning, and
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
/// * `#[action(caption = "...", set = "state = 'done'; finished = Today()", enabled = "...",
///   selection = "many", confirm = "...", id = "...")]`: an action that sets fields of the records
///   selected, each read from the record as it was, and keeps them once they meet the rules.
///   `selection` is `one`, `many` (the default) or `any`; `enabled` is what every record selected
///   has to meet for it to be on. Both expressions are checked here.
/// * `#[variant(id = "...", caption = "...", filter = "...", columns = "a,b")]` and
///   `#[list_filter(id = "...", caption = "...", filter = "...")]`, for people to pick on the list.
#[proc_macro_derive(
    BusinessObject,
    attributes(object, field, rule, appearance, variant, list_filter, action)
)]
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

fn kind_tokens(kind: &KindSpec) -> Tokens {
    match kind {
        KindSpec::Money => quote!(::ravenrs_app::FieldKind::Money),
        KindSpec::LongText => quote!(::ravenrs_app::FieldKind::LongText),
        KindSpec::Color => quote!(::ravenrs_app::FieldKind::Color),
        KindSpec::Date => quote!(::ravenrs_app::FieldKind::Date),
        KindSpec::Guid => quote!(::ravenrs_app::FieldKind::Guid),
        KindSpec::Tags(names) => quote!(::ravenrs_app::FieldKind::Tags(::std::vec![
            #(::ravenrs_app::EnumOption::new(#names, #names)),*
        ])),
        KindSpec::Decimal(places) => quote!(::ravenrs_app::FieldKind::Decimal(#places)),
        KindSpec::Reference(target) => quote!(::ravenrs_app::FieldKind::Reference(#target.to_owned())),
        KindSpec::Collection(target) => quote!(::ravenrs_app::FieldKind::Collection(#target.to_owned())),
    }
}

fn rule_tokens(rule: &RuleSpec) -> Tokens {
    let mut tokens = match &rule.kind {
        RuleKindSpec::Required(name) => quote!(::ravenrs_app::Rule::required(#name)),
        RuleKindSpec::Unique(name) => quote!(::ravenrs_app::Rule::unique(#name)),
        RuleKindSpec::Range { field, min, max } => {
            let limit = |value: &Option<Box<syn::Expr>>| match value {
                Some(value) => quote!(::std::option::Option::Some((#value) as f64)),
                None => quote!(::std::option::Option::None),
            };
            let (min, max) = (limit(min), limit(max));
            quote!(::ravenrs_app::Rule::range(#field, #min, #max))
        }
        RuleKindSpec::Matches(name, pattern) => quote!(::ravenrs_app::Rule::matches(#name, #pattern)),
        RuleKindSpec::Criteria(id, expression) => quote!(::ravenrs_app::Rule::criteria(#id, #expression)),
        RuleKindSpec::UniqueTogether(id, fields) => quote!(::ravenrs_app::Rule::new(
            #id,
            ::ravenrs_app::RuleKind::UniqueTogether(::std::vec![#(#fields.to_owned()),*]),
        )),
    };
    if let Some(text) = &rule.message {
        tokens = quote!(#tokens.message(#text));
    }
    if let Some(text) = &rule.context {
        tokens = quote!(#tokens.contexts([#text]));
    }
    if let Some(expression) = &rule.target {
        tokens = quote!(#tokens.target_criteria(#expression));
    }
    match rule.severity {
        SeveritySpec::Error => tokens,
        SeveritySpec::Warning => quote!(#tokens.warning()),
        SeveritySpec::Information => quote!(#tokens.information()),
    }
}

fn tint(color: TintSpec) -> Tokens {
    match color {
        TintSpec::Danger => quote!(::ravenrs_app::Tint::Danger),
        TintSpec::Warning => quote!(::ravenrs_app::Tint::Warning),
        TintSpec::Success => quote!(::ravenrs_app::Tint::Success),
        TintSpec::Accent => quote!(::ravenrs_app::Tint::Accent),
        TintSpec::Muted => quote!(::ravenrs_app::Tint::Muted),
    }
}

fn appearance_tokens(rule: &AppearanceSpec) -> Tokens {
    let (id, targets) = (&rule.id, &rule.targets);
    let pieces = rule.pieces.iter().map(|piece| match piece {
        AppearancePiece::Criteria(expression) => quote!(.criteria(#expression)),
        AppearancePiece::Font(color) => {
            let color = tint(*color);
            quote!(.font(#color))
        }
        AppearancePiece::Back(color) => {
            let color = tint(*color);
            quote!(.back(#color))
        }
        AppearancePiece::Bold => quote!(.bold()),
        AppearancePiece::Enabled(on) => quote!(.enabled(#on)),
        AppearancePiece::Visible(on) => quote!(.visible(#on)),
        AppearancePiece::Priority(priority) => quote!(.priority(#priority)),
        AppearancePiece::Action => quote!(.target(::ravenrs_app::AppearanceTarget::Action)),
        AppearancePiece::Layout => quote!(.target(::ravenrs_app::AppearanceTarget::Layout)),
        AppearancePiece::Context(context) => {
            let context = match context {
                ContextSpec::List => quote!(::ravenrs_app::AppearanceContext::List),
                ContextSpec::Detail => quote!(::ravenrs_app::AppearanceContext::Detail),
                ContextSpec::View(view) => quote!(::ravenrs_app::AppearanceContext::View(#view.to_owned())),
            };
            quote!(.context(#context))
        }
    });
    quote!(::ravenrs_app::AppearanceRule::new(#id, [#(#targets),*]) #(#pieces)*)
}

fn field_tokens(field: &FieldSpec) -> Tokens {
    let (ty, attrs) = (&field.ty, &field.settings);
    let name = field.ident.to_string();
    let kind = attrs
        .kind
        .as_ref()
        .map(kind_tokens)
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
    if let Some(text) = &attrs.accessible_name {
        then(quote!(.accessible_name(#text)));
    }
    if let Some(text) = &attrs.accessible_description {
        then(quote!(.accessible_description(#text)));
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
    if attrs.secret {
        then(quote!(.secret()));
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
    if let Some(values) = &attrs.predefined {
        then(quote!(.predefined([#(#values),*])));
    }
    if let Some(text) = &attrs.criteria_of {
        then(quote!(.criteria_of(#text)));
    }
    info
}

fn choice_tokens(choice: &ChoiceSpec) -> Tokens {
    match choice {
        ChoiceSpec::Variant { id, caption, filter, columns } => {
            let filter = filter.as_ref().map(|text| quote!(.filter(#text)));
            let shown = (!columns.is_empty()).then(|| quote!(.columns([#(#columns),*])));
            quote!(.variant(::ravenrs_app::ViewVariant::new(#id, #caption) #filter #shown))
        }
        ChoiceSpec::ListFilter { id, caption, filter } => {
            let filter = filter.as_ref().map(|text| quote!(.criteria(#text)));
            quote!(.filter(::ravenrs_app::ListFilter::new(#id, #caption) #filter))
        }
    }
}

fn action_tokens(action: &ActionSpec) -> Tokens {
    let (caption, set) = (&action.caption, &action.set);
    let mut tokens = quote!(::ravenrs_app::RecordAction::new(#caption).set(#set));
    if let Some(id) = &action.id {
        tokens = quote!(#tokens.id(#id));
    }
    if let Some(selection) = action.selection {
        let selection = match selection {
            SelectionSpec::One => quote!(RequireSingle),
            SelectionSpec::Many => quote!(RequireMultiple),
            SelectionSpec::Any => quote!(Independent),
        };
        tokens = quote!(#tokens.selection(::ravenrs_app::Selection::#selection));
    }
    if let Some(enabled) = &action.enabled {
        tokens = quote!(#tokens.enabled(#enabled));
    }
    if let Some(question) = &action.confirm {
        tokens = quote!(#tokens.confirm(#question));
    }
    tokens
}

fn business_object(input: &DeriveInput) -> Result<Tokens> {
    let spec = parse_object(input)?;
    let name = &spec.name;
    let object = &spec.settings;
    let infos: Vec<Tokens> = spec.fields.iter().map(field_tokens).collect();
    let rules: Vec<Tokens> = spec.rules.iter().map(rule_tokens).collect();
    let appearance: Vec<Tokens> = spec.appearance.iter().map(appearance_tokens).collect();
    let choices: Vec<Tokens> = spec.choices.iter().map(choice_tokens).collect();
    let actions: Vec<Tokens> = spec.actions.iter().map(action_tokens).collect();
    let to_values = spec.fields.iter().map(|field| {
        let (ident, ty) = (&field.ident, &field.ty);
        let text = ident.to_string();
        quote_spanned!(ty.span()=>
            (#text.to_owned(), <#ty as ::ravenrs_app::FieldValue>::to_value(&self.#ident))
        )
    });
    let from_values = spec.fields.iter().map(|field| {
        let (ident, ty) = (&field.ident, &field.ty);
        let text = ident.to_string();
        quote_spanned!(ty.span()=>
            #ident: <#ty as ::ravenrs_app::FieldValue>::from_value(object.get(#text))
        )
    });
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
    if let Some(text) = &object.empty_text {
        info = quote!(#info.empty_text(#text));
    }
    if let Some(text) = &object.list_criteria {
        info = quote!(#info.list_criteria(#text));
    }
    if object.list_editable {
        info = quote!(#info.list_editable());
    }
    if let Some(access) = object.data_access {
        let access = match access {
            DataAccessSpec::Server => quote!(::ravenrs_app::DataAccess::Server),
            DataAccessSpec::DataView => quote!(::ravenrs_app::DataAccess::DataView),
            DataAccessSpec::Client => quote!(::ravenrs_app::DataAccess::Client),
        };
        info = quote!(#info.data_access(#access));
    }
    for name in &object.form_actions {
        info = quote!(#info.form_actions(#name));
    }
    if let Some(columns) = &object.form_columns {
        info = quote!(#info.form_columns(#columns));
    }
    if !object.focus_order.is_empty() {
        let fields = &object.focus_order;
        info = quote!(#info.focus_order([#(#fields),*]));
    }
    if object.cloneable {
        info = quote!(#info.cloneable());
    }
    if object.singleton {
        info = quote!(#info.singleton());
    }
    if object.transient {
        info = quote!(#info.transient());
    }
    if object.checks_on_leave {
        info = quote!(#info.checks_on_leave());
    }
    Ok(quote! {
        impl ::ravenrs_app::BusinessObject for #name {
            fn entity_info() -> ::ravenrs_app::EntityInfo {
                #info #(.field(#infos))* #(.rule(#rules))* #(.appearance(#appearance))* #(#choices)* #(.action(#actions))*
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
    let spec = parse_enum(input)?;
    let name = &spec.name;
    let Some(first) = spec.options.first() else {
        return Err(Error::new(name.span(), "a business enum needs at least one option"));
    };
    let first = &first.ident;
    let options = spec.options.iter().map(|o| {
        let (code, caption) = (&o.code, &o.caption);
        quote!(::ravenrs_app::EnumOption::new(#code, #caption))
    });
    let codes = spec.options.iter().map(|o| {
        let (ident, code) = (&o.ident, &o.code);
        quote!(Self::#ident => #code)
    });
    let parses = spec.options.iter().map(|o| {
        let (ident, code) = (&o.ident, &o.code);
        quote!(#code => Self::#ident)
    });
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
