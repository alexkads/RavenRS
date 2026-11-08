//! `#[ravenrs_live::expose]`: registers what an `impl` block or an enum offers with a
//! `ravenrs_live::Registry`, so a playground can call it without a compiler.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::{
    FnArg, GenericArgument, GenericParam, ImplItem, Item, PathArguments, ReturnType, Type, TypeParamBound,
    parse_macro_input, parse_quote,
};

/// Registers the public functions and constants of an inherent `impl` block, or the variants of
/// an enum, behind the `live` feature of the crate it is used in. It generates
/// `__live_expose` (or `__live_expose_<part>` with `part = "..."`) on the type, which the crate
/// calls when it builds its registry. `skip = "a, b"` leaves functions out.
///
/// Generic parameters bounded by `View` become `AnyView`, and by `ViewSeq` a `Vec<AnyView>`;
/// blocks with other generic parameters, and functions it cannot call with interpreter values,
/// are left out without failing the build.
#[proc_macro_attribute]
pub fn expose(attr: TokenStream, item: TokenStream) -> TokenStream {
    let options = parse_macro_input!(attr as Options);
    let original: Tokens = item.clone().into();
    let item = parse_macro_input!(item as Item);
    let generated = match &item {
        Item::Impl(block) => expose_impl(block, &options),
        Item::Enum(choice) => expose_enum(choice, &options),
        other => Err(syn::Error::new(other.span(), "#[expose] goes on an inherent impl block or an enum")),
    };
    match generated {
        Ok(generated) => quote!(#original #generated).into(),
        Err(error) => {
            let error = error.into_compile_error();
            quote!(#original #error).into()
        }
    }
}

#[derive(Default)]
struct Options {
    part: Option<String>,
    skip: Vec<String>,
}

impl syn::parse::Parse for Options {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let mut options = Options::default();
        let pairs =
            syn::punctuated::Punctuated::<syn::MetaNameValue, syn::Token![,]>::parse_terminated(input)?;
        for pair in pairs {
            let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(text), .. }) = &pair.value else {
                return Err(syn::Error::new(pair.value.span(), "a text goes here"));
            };
            if pair.path.is_ident("part") {
                options.part = Some(text.value());
            } else if pair.path.is_ident("skip") {
                options.skip =
                    text.value().split(',').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect();
            } else {
                return Err(syn::Error::new(pair.path.span(), "use part = \"...\" or skip = \"a, b\""));
            }
        }
        Ok(options)
    }
}

fn entry_name(options: &Options) -> syn::Ident {
    match &options.part {
        Some(part) => format_ident!("__live_expose_{}", part),
        None => format_ident!("__live_expose"),
    }
}

fn last_ident(path: &syn::Path) -> String {
    path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default()
}

/// What a generic parameter bounded by `bounds` becomes, if it can become anything.
fn concrete_for(bounds: &[&TypeParamBound]) -> Option<Type> {
    for bound in bounds {
        if let TypeParamBound::Trait(trait_bound) = bound {
            match last_ident(&trait_bound.path).as_str() {
                "View" => return Some(parse_quote!(crate::AnyView)),
                "ViewSeq" => return Some(parse_quote!(::std::vec::Vec<crate::AnyView>)),
                _ => {}
            }
        }
    }
    None
}

/// Puts concrete types in place of generic parameters.
struct Substitute<'a>(&'a HashMap<String, Type>);

impl VisitMut for Substitute<'_> {
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(path) = ty
            && path.qself.is_none()
            && path.path.segments.len() == 1
            && let Some(concrete) = self.0.get(&path.path.segments[0].ident.to_string())
        {
            *ty = concrete.clone();
            return;
        }
        syn::visit_mut::visit_type_mut(self, ty);
    }
}

fn substituted(ty: &Type, map: &HashMap<String, Type>) -> Type {
    let mut ty = ty.clone();
    Substitute(map).visit_type_mut(&mut ty);
    ty
}

/// The generic parameters of `generics` as concrete types, or `None` when one cannot be.
fn substitutions(generics: &syn::Generics) -> Option<HashMap<String, Type>> {
    let mut map = HashMap::new();
    for param in &generics.params {
        match param {
            GenericParam::Type(param) => {
                let mut bounds: Vec<&TypeParamBound> = param.bounds.iter().collect();
                if let Some(clause) = &generics.where_clause {
                    for predicate in &clause.predicates {
                        if let syn::WherePredicate::Type(typed) = predicate
                            && let Type::Path(path) = &typed.bounded_ty
                            && path.path.is_ident(&param.ident)
                        {
                            bounds.extend(typed.bounds.iter());
                        }
                    }
                }
                map.insert(param.ident.to_string(), concrete_for(&bounds)?);
            }
            GenericParam::Lifetime(_) => return None,
            GenericParam::Const(_) => return None,
        }
    }
    Some(map)
}

/// How an argument of type `ty` is made and passed.
struct Param {
    made: Type,
    by_ref: bool,
}

/// Whether a type mentions `impl Trait` anywhere inside, which no binding can name.
fn has_impl(ty: &Type) -> bool {
    struct Finder(bool);
    impl<'ast> syn::visit::Visit<'ast> for Finder {
        fn visit_type_impl_trait(&mut self, _: &'ast syn::TypeImplTrait) {
            self.0 = true;
        }
    }
    let mut finder = Finder(false);
    syn::visit::Visit::visit_type(&mut finder, ty);
    finder.0
}

/// Whether `impl Trait` asks for `Send` or `Sync`, which the interpreter's closures are not.
fn needs_threads(ty: &Type) -> bool {
    let Type::ImplTrait(implied) = ty else { return false };
    implied.bounds.iter().any(
        |b| matches!(b, TypeParamBound::Trait(t) if matches!(last_ident(&t.path).as_str(), "Send" | "Sync")),
    )
}

fn impl_bound(ty: &Type) -> Option<&syn::TraitBound> {
    let Type::ImplTrait(implied) = ty else { return None };
    implied.bounds.iter().find_map(|b| match b {
        TypeParamBound::Trait(t) => Some(t),
        _ => None,
    })
}

fn angle_types(segment: &syn::PathSegment) -> Vec<&Type> {
    match &segment.arguments {
        PathArguments::AngleBracketed(args) => args
            .args
            .iter()
            .filter_map(|a| match a {
                GenericArgument::Type(t) => Some(t),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The type an argument is made as, for a parameter written `ty`.
fn made_type(ty: &Type, map: &HashMap<String, Type>) -> Option<Type> {
    if needs_threads(ty) {
        return None;
    }
    if let Some(bound) = impl_bound(ty) {
        let segment = bound.path.segments.last()?;
        return match segment.ident.to_string().as_str() {
            "Into" => made_type(angle_types(segment).first()?, map),
            "AsRef" => Some(parse_quote!(::std::string::String)),
            "View" => Some(parse_quote!(crate::AnyView)),
            "ViewSeq" => Some(parse_quote!(::std::vec::Vec<crate::AnyView>)),
            "Fn" | "FnMut" | "FnOnce" => {
                let PathArguments::Parenthesized(args) = &segment.arguments else { return None };
                let inputs: Vec<Type> =
                    args.inputs.iter().map(|t| made_type(t, map)).collect::<Option<_>>()?;
                let output: Type = match &args.output {
                    ReturnType::Default => parse_quote!(()),
                    ReturnType::Type(_, t) => made_type(t, map)?,
                };
                // Arguments given by reference arrive as references in the closure.
                if inputs.iter().any(|t| matches!(t, Type::Reference(_))) {
                    return None;
                }
                Some(parse_quote!(::std::boxed::Box<dyn Fn(#(#inputs),*) -> #output>))
            }
            "IntoIterator" => {
                let PathArguments::AngleBracketed(args) = &segment.arguments else { return None };
                let item = args.args.iter().find_map(|a| match a {
                    GenericArgument::AssocType(assoc) if assoc.ident == "Item" => Some(&assoc.ty),
                    _ => None,
                })?;
                let item = made_type(item, map)?;
                Some(parse_quote!(::std::vec::Vec<#item>))
            }
            _ => None,
        };
    }
    match ty {
        Type::Reference(_) => None,
        Type::ImplTrait(_) | Type::TraitObject(_) | Type::BareFn(_) | Type::Infer(_) | Type::Never(_) => None,
        Type::Path(path) if path.qself.is_none() && !has_impl(ty) => Some(substituted(ty, map)),
        Type::Tuple(_) | Type::Array(_) | Type::Paren(_) if !has_impl(ty) => Some(substituted(ty, map)),
        _ => None,
    }
}

fn param(ty: &Type, map: &HashMap<String, Type>) -> Option<Param> {
    if let Type::Reference(reference) = ty {
        if reference.mutability.is_some() {
            return None;
        }
        let made: Type = match &*reference.elem {
            Type::Path(path) if path.path.is_ident("str") => parse_quote!(::std::string::String),
            Type::Slice(slice) => {
                let item = made_type(&slice.elem, map)?;
                parse_quote!(::std::vec::Vec<#item>)
            }
            other => made_type(other, map)?,
        };
        return Some(Param { made, by_ref: true });
    }
    Some(Param { made: made_type(ty, map)?, by_ref: false })
}

/// Whether a type mentions a reference or a lifetime, which a result cannot carry out.
fn borrows(ty: &Type) -> bool {
    struct Finder(bool);
    impl<'ast> syn::visit::Visit<'ast> for Finder {
        fn visit_type_reference(&mut self, _: &'ast syn::TypeReference) {
            self.0 = true;
        }
        fn visit_lifetime(&mut self, _: &'ast syn::Lifetime) {
            self.0 = true;
        }
    }
    let mut finder = Finder(false);
    syn::visit::Visit::visit_type(&mut finder, ty);
    finder.0
}

fn make_arg(index: usize, count: usize, made: &Type) -> Tokens {
    quote! {{
        #[allow(unused_imports)]
        use ::ravenrs_live::bridge::{
            ArgFromHost as _, ArgFromLive as _, ArgHostBinding as _, ArgHostBindingOf as _, ArgHostClosure as _, ArgHostOption as _,
            ArgHostVec as _,
        };
        (&&&&&&::ravenrs_live::bridge::Arg::<#made>::of()).make(::ravenrs_live::bridge::arg(&args, #index, #count)?)?
    }}
}

fn result(value: Tokens) -> Tokens {
    quote! {{
        #[allow(unused_imports)]
        use ::ravenrs_live::bridge::{
            RetCloneable as _, RetHost as _, RetIntoLive as _, RetOptionCopied as _, RetOptionMoved as _, RetVecCopied as _,
            RetVecMoved as _,
        };
        let out = ::ravenrs_live::bridge::Ret::new(#value);
        ::std::result::Result::Ok((&&&&&&&out).live())
    }}
}

fn expose_impl(block: &syn::ItemImpl, options: &Options) -> syn::Result<Tokens> {
    if block.trait_.is_some() {
        return Err(syn::Error::new(block.span(), "#[expose] goes on an inherent impl block"));
    }
    let Type::Path(self_path) = &*block.self_ty else {
        return Err(syn::Error::new(block.self_ty.span(), "#[expose] needs a named type"));
    };
    let name = last_ident(&self_path.path);
    let entry = entry_name(options);
    // A block whose generics cannot be made concrete offers nothing, and fails nothing.
    let Some(map) = substitutions(&block.generics) else { return Ok(Tokens::new()) };
    let self_ty = substituted(&block.self_ty, &map);
    let mut registrations = Vec::new();
    for item in &block.items {
        match item {
            ImplItem::Fn(function) => {
                if let Some(registration) = function_registration(function, &name, &self_ty, &map, options) {
                    registrations.push(registration);
                }
            }
            ImplItem::Const(constant) if matches!(constant.vis, syn::Visibility::Public(_)) => {
                if borrows(&constant.ty) {
                    continue;
                }
                let ident = &constant.ident;
                let text = ident.to_string();
                let value = result(quote!(<#self_ty>::#ident));
                registrations.push(quote! {
                    registry.constant(#name, #text, || {
                        let made: ::std::result::Result<::ravenrs_live::Value, ::std::string::String> = #value;
                        made.unwrap_or(::ravenrs_live::Value::Unit)
                    });
                });
            }
            _ => {}
        }
    }
    Ok(quote! {
        #[cfg(feature = "live")]
        impl #self_ty {
            #[doc(hidden)]
            #[allow(clippy::too_many_lines, unused_variables, clippy::redundant_closure_call)]
            pub fn #entry(registry: &mut ::ravenrs_live::Registry) {
                #(#registrations)*
            }
        }
    })
}

fn function_registration(
    function: &syn::ImplItemFn,
    name: &str,
    self_ty: &Type,
    outer: &HashMap<String, Type>,
    options: &Options,
) -> Option<Tokens> {
    if !matches!(function.vis, syn::Visibility::Public(_))
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
    {
        return None;
    }
    let fn_name = function.sig.ident.to_string();
    if options.skip.contains(&fn_name) || fn_name.starts_with("__") {
        return None;
    }
    let mut map = outer.clone();
    map.extend(substitutions(&function.sig.generics)?);
    let mut receiver = None;
    let mut params = Vec::new();
    for input in &function.sig.inputs {
        match input {
            FnArg::Receiver(r) => {
                if r.colon_token.is_some() {
                    return None;
                }
                receiver = Some((r.reference.is_some(), r.mutability.is_some()));
            }
            FnArg::Typed(typed) => params.push(param(&typed.ty, &map)?),
        }
    }
    let returns_view = match &function.sig.output {
        ReturnType::Default => false,
        ReturnType::Type(_, ty) => match impl_bound(ty) {
            Some(bound) if last_ident(&bound.path) == "View" => true,
            Some(_) => return None,
            None if borrows(ty) => return None,
            None => false,
        },
    };
    let offset = usize::from(receiver.is_some());
    let count = params.len() + offset;
    let ident = &function.sig.ident;
    let names: Vec<syn::Ident> = (0..params.len()).map(|i| format_ident!("a{}", i)).collect();
    let makes: Vec<Tokens> = params
        .iter()
        .zip(&names)
        .enumerate()
        .map(|(i, (p, n))| {
            let made = make_arg(i + offset, count, &p.made);
            let ty = &p.made;
            quote!(let #n: #ty = #made;)
        })
        .collect();
    let passed: Vec<Tokens> =
        params.iter().zip(&names).map(|(p, n)| if p.by_ref { quote!(&#n) } else { quote!(#n) }).collect();
    let call = match receiver {
        None => quote!(<#self_ty>::#ident(#(#passed),*)),
        Some((false, _)) => {
            let this = make_arg(0, count, self_ty);
            quote!({ let this: #self_ty = #this; this.#ident(#(#passed),*) })
        }
        Some((true, false)) => {
            quote!(::ravenrs_live::bridge::with_ref::<#self_ty, _>(&args[0], |this| this.#ident(#(#passed),*))?)
        }
        Some((true, true)) => {
            quote!(::ravenrs_live::bridge::with_mut::<#self_ty, _>(&args[0], |this| this.#ident(#(#passed),*))?)
        }
    };
    let call = if returns_view {
        quote!({ let view: crate::AnyView = ::std::boxed::Box::new(#call); view })
    } else {
        call
    };
    let value = result(call);
    let register = if receiver.is_some() { quote!(method) } else { quote!(function) };
    let check = if receiver.is_some() {
        quote!(::ravenrs_live::bridge::arg(&args, 0, #count)?;)
    } else {
        Tokens::new()
    };
    Some(quote! {
        registry.#register(#name, #fn_name, |args: ::std::vec::Vec<::ravenrs_live::Value>| {
            if args.len() != #count {
                return ::std::result::Result::Err(::std::format!("`{}` takes {} arguments, and got {}", #fn_name, #count, args.len()));
            }
            #check
            #(#makes)*
            #value
        });
    })
}

fn expose_enum(choice: &syn::ItemEnum, options: &Options) -> syn::Result<Tokens> {
    if !choice.generics.params.is_empty() {
        return Ok(Tokens::new());
    }
    let ty = &choice.ident;
    let name = ty.to_string();
    let entry = entry_name(options);
    let mut registrations = Vec::new();
    for variant in &choice.variants {
        let ident = &variant.ident;
        let text = ident.to_string();
        match &variant.fields {
            syn::Fields::Unit => {
                let value = result(quote!(#ty::#ident));
                registrations.push(quote! {
                    registry.constant(#name, #text, || {
                        let made: ::std::result::Result<::ravenrs_live::Value, ::std::string::String> = #value;
                        made.unwrap_or(::ravenrs_live::Value::Unit)
                    });
                });
            }
            syn::Fields::Unnamed(fields) => {
                let map = HashMap::new();
                let Some(made) =
                    fields.unnamed.iter().map(|f| made_type(&f.ty, &map)).collect::<Option<Vec<Type>>>()
                else {
                    continue;
                };
                let count = made.len();
                let names: Vec<syn::Ident> = (0..count).map(|i| format_ident!("a{}", i)).collect();
                let makes = made.iter().zip(&names).enumerate().map(|(i, (t, n))| {
                    let m = make_arg(i, count, t);
                    quote!(let #n: #t = #m;)
                });
                let value = result(quote!(#ty::#ident(#(#names),*)));
                registrations.push(quote! {
                    registry.function(#name, #text, |args: ::std::vec::Vec<::ravenrs_live::Value>| {
                        if args.len() != #count {
                            return ::std::result::Result::Err(::std::format!("`{}` takes {} values", #text, #count));
                        }
                        #(#makes)*
                        #value
                    });
                });
            }
            syn::Fields::Named(_) => {}
        }
    }
    Ok(quote! {
        #[cfg(feature = "live")]
        impl #ty {
            #[doc(hidden)]
            #[allow(clippy::too_many_lines, unused_variables)]
            pub fn #entry(registry: &mut ::ravenrs_live::Registry) {
                #(#registrations)*
            }
        }
    })
}
