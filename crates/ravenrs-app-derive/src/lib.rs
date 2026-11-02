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
/// On the struct, `#[object(caption = "...", navigation = "...", icon = "...")]`.
///
/// On a field, `#[field(...)]`:
///
/// * `key` numbered by the application, or `typed_key` typed by people. Exactly one field has one.
/// * `caption = "..."` what people see.
/// * `default_property` what names a record. `read_only`. `no_list`, `no_detail`, `lookup`.
/// * What it holds, when the type does not say: `money`, `long`, `color`, `date`,
///   `decimals = 2`, `reference = "Entity"`, `collection = "Entity"`.
/// * `calculated = "[a] + [b]"`, in the criteria language, checked here.
/// * `default = ...` what a new record starts with.
#[proc_macro_derive(BusinessObject, attributes(object, field))]
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
            } else {
                return Err(meta.error("unknown setting: use caption, navigation or icon"));
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
            } else {
                return Err(meta.error(
                    "unknown setting: use key, typed_key, caption, default_property, read_only, no_list, \
                     no_detail, lookup, money, long, color, date, decimals, reference, collection, \
                     calculated or default",
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

fn check_calculated(expression: &LitStr, names: &[String]) -> Result<()> {
    let parsed = Criteria::parse(&expression.value())
        .map_err(|e| Error::new(expression.span(), format!("this expression does not parse: {e}")))?;
    for property in parsed.properties() {
        if !names.contains(&property) {
            return Err(Error::new(expression.span(), format!("this struct has no field `{property}`")));
        }
    }
    Ok(())
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
    info
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
            check_calculated(expression, &names)?;
        }
        infos.push(field_tokens(ident, &field.ty, &attrs));
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
    Ok(quote! {
        impl ::ravenrs_app::BusinessObject for #name {
            fn entity_info() -> ::ravenrs_app::EntityInfo {
                #info #(.field(#infos))*
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
