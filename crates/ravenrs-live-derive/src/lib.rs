//! `#[ravenrs_live::expose]`: registers what an `impl` block, a struct, an enum or a module
//! offers with a `ravenrs_live::Registry`, so a playground can call it without a compiler.

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

/// Registers the public functions and constants of an inherent `impl` block, the public fields
/// of a struct, the variants of an enum or the public functions of an inline module, behind the
/// `live` feature of the crate it is used in. It generates `__live_expose` (or
/// `__live_expose_<part>` with `part = "..."`) on the type, or inside the module, which the crate
/// calls when it builds its registry. `skip = "a, b"` leaves functions out.
///
/// Generic parameters bounded by `View` become `AnyView`, and by `ViewSeq` a `Vec<AnyView>`;
/// `with = "A = (); A = Other"` registers a block once for each of these types. A function's
/// own parameters bounded by `Into`, `AsRef`, `Fn` or `IntoIterator` take what those take, and
/// ones bounded only by `Clone`, `PartialEq` or `Debug` take any value. What it cannot call with
/// interpreter values is left out without failing the build.
#[proc_macro_attribute]
pub fn expose(attr: TokenStream, item: TokenStream) -> TokenStream {
    let options = parse_macro_input!(attr as Options);
    let original: Tokens = item.clone().into();
    let item = parse_macro_input!(item as Item);
    let generated = match &item {
        Item::Impl(block) => expose_impl(block, &options),
        Item::Enum(choice) => expose_enum(choice, &options),
        Item::Struct(declared) => expose_struct(declared, &options),
        Item::Mod(module) => {
            return expose_mod(module, &options)
                .unwrap_or_else(|e| {
                    let error = e.into_compile_error();
                    quote!(#original #error)
                })
                .into();
        }
        other => Err(syn::Error::new(
            other.span(),
            "#[expose] goes on an inherent impl block, a struct, an enum or a module",
        )),
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
    with: Vec<Vec<(String, Type)>>,
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
            } else if pair.path.is_ident("with") {
                for alternative in text.value().split(';').filter(|a| !a.trim().is_empty()) {
                    let mut set = Vec::new();
                    for pair in alternative.split(',') {
                        let (name, ty) = pair
                            .split_once('=')
                            .ok_or_else(|| syn::Error::new(text.span(), "write `A = Type`"))?;
                        set.push((name.trim().to_owned(), syn::parse_str::<Type>(ty.trim())?));
                    }
                    options.with.push(set);
                }
            } else {
                return Err(syn::Error::new(
                    pair.path.span(),
                    "use part = \"...\", skip = \"a, b\" or with = \"A = T\"",
                ));
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

/// Whether a bound only says what any interpreter value already is.
fn is_marker(bound: &TypeParamBound) -> bool {
    match bound {
        TypeParamBound::Lifetime(_) => true,
        TypeParamBound::Trait(t) => matches!(last_ident(&t.path).as_str(), "Clone" | "PartialEq" | "Debug"),
        _ => false,
    }
}

fn bounds_of<'a>(param: &'a syn::TypeParam, generics: &'a syn::Generics) -> Vec<&'a TypeParamBound> {
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
    bounds
}

/// What a generic parameter bounded by `View` or `ViewSeq` becomes.
fn view_type(bounds: &[&TypeParamBound]) -> Option<Type> {
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
            && path.path.segments[0].arguments.is_empty()
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

/// The generic parameters of an `impl` block as concrete types, with `given` for the ones not
/// bounded by `View`, or `None` when one cannot be made concrete.
fn block_substitutions(generics: &syn::Generics, given: &[(String, Type)]) -> Option<HashMap<String, Type>> {
    let mut map = HashMap::new();
    for param in &generics.params {
        match param {
            GenericParam::Type(param) => {
                let name = param.ident.to_string();
                let ty = match given.iter().find(|(n, _)| *n == name) {
                    Some((_, ty)) => ty.clone(),
                    None => view_type(&bounds_of(param, generics))?,
                };
                map.insert(name, ty);
            }
            GenericParam::Lifetime(_) | GenericParam::Const(_) => return None,
        }
    }
    Some(map)
}

/// The generic parameters of a function: as their bounds take, `impl Bounds` for `Into`, `Fn`
/// and the like, or any value for marker bounds.
fn function_substitutions(generics: &syn::Generics) -> Option<HashMap<String, Type>> {
    let mut map = HashMap::new();
    for param in &generics.params {
        match param {
            GenericParam::Type(param) => {
                let bounds = bounds_of(param, generics);
                let ty = if let Some(view) = view_type(&bounds) {
                    view
                } else if bounds.iter().all(|b| is_marker(b)) {
                    parse_quote!(::ravenrs_live::Value)
                } else {
                    let kept: Vec<&TypeParamBound> =
                        bounds.iter().copied().filter(|b| !is_marker(b)).collect();
                    parse_quote!(impl #(#kept)+*)
                };
                map.insert(param.ident.to_string(), ty);
            }
            GenericParam::Lifetime(_) => {}
            GenericParam::Const(_) => return None,
        }
    }
    Some(map)
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
        TypeParamBound::Trait(t)
            if !matches!(last_ident(&t.path).as_str(), "Clone" | "PartialEq" | "Debug") =>
        {
            Some(t)
        }
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

/// How an argument of type `ty` is made and passed.
struct Param {
    shape: Shape,
    by_ref: bool,
}

/// How a value becomes a Rust value of a type, decided from how the type is written.
enum Shape {
    /// Through the autoref levels of `bridge::Arg`.
    Leaf(Type),
    List(Box<Shape>),
    Optional(Box<Shape>),
    Outcome(Box<Shape>, Box<Shape>),
    Tuple(Vec<Shape>),
    Closure {
        inputs: Vec<Input>,
        output: Box<Shape>,
        shared: bool,
    },
    /// Made as `inner`, then turned into `ty` with `.into()`, as a `Vec` into an `Rc<[T]>`.
    Into(Box<Shape>, Type),
    /// A `&'static str`, kept for as long as the program runs.
    Static,
    /// `impl Into<T>` of a host type: a `T`, or a text, number or bool `T` is made from.
    IntoOf(Type),
}

/// What a closure the host calls is given.
enum Input {
    Str,
    Scratch(Type),
    Borrowed(Type),
    Owned(Type),
}

impl Shape {
    fn ty(&self) -> Type {
        match self {
            Self::Leaf(ty) | Self::Into(_, ty) | Self::IntoOf(ty) => ty.clone(),
            Self::Static => parse_quote!(&'static str),
            Self::List(item) => {
                let item = item.ty();
                parse_quote!(::std::vec::Vec<#item>)
            }
            Self::Optional(item) => {
                let item = item.ty();
                parse_quote!(::std::option::Option<#item>)
            }
            Self::Outcome(ok, err) => {
                let (ok, err) = (ok.ty(), err.ty());
                parse_quote!(::std::result::Result<#ok, #err>)
            }
            Self::Tuple(items) => {
                let items: Vec<Type> = items.iter().map(Shape::ty).collect();
                parse_quote!((#(#items,)*))
            }
            Self::Closure { inputs, output, shared } => {
                let inputs: Vec<Type> = inputs.iter().map(Input::ty).collect();
                let output = output.ty();
                if *shared {
                    parse_quote!(::std::rc::Rc<dyn Fn(#(#inputs),*) -> #output>)
                } else {
                    parse_quote!(::std::boxed::Box<dyn Fn(#(#inputs),*) -> #output>)
                }
            }
        }
    }

    /// An expression making this type from `value`, a `Value`, inside code that returns
    /// `Result<_, String>`.
    fn make(&self, value: &Tokens) -> Tokens {
        match self {
            Self::Into(inner, ty) => {
                let inner = inner.make(value);
                quote!({ let made: #ty = (#inner).into(); made })
            }
            Self::IntoOf(ty) => {
                let leaf = Self::Leaf(ty.clone()).make(&quote!(value.clone()));
                let from = |source: Tokens, made: Tokens| quote!((&&::ravenrs_live::bridge::IntoFrom::<#ty, #source>::new(#made)).get());
                let (text, whole, small, float, single, truth) = (
                    from(quote!(::std::string::String), quote!(text.clone())),
                    from(quote!(i64), quote!(*n)),
                    from(quote!(i32), quote!(*n as i32)),
                    from(quote!(f64), quote!(*n as f64)),
                    from(quote!(f32), quote!(*n as f32)),
                    from(quote!(bool), quote!(*b)),
                );
                let (wide, narrow) = (from(quote!(f64), quote!(*n)), from(quote!(f32), quote!(*n as f32)));
                quote! {{
                    #[allow(unused_imports)]
                    use ::ravenrs_live::bridge::{IntoFromNone as _, IntoFromSome as _};
                    let value: ::ravenrs_live::Value = #value;
                    match (|| -> ::std::result::Result<#ty, ::std::string::String> { ::std::result::Result::Ok(#leaf) })() {
                        ::std::result::Result::Ok(made) => made,
                        ::std::result::Result::Err(problem) => match &value.read() {
                            ::ravenrs_live::Value::Str(text) => #text,
                            ::ravenrs_live::Value::Int(n) => #whole.or_else(|| #small).or_else(|| #float).or_else(|| #single),
                            ::ravenrs_live::Value::Float(n) => #wide.or_else(|| #narrow),
                            ::ravenrs_live::Value::Bool(b) => #truth,
                            _ => ::std::option::Option::None,
                        }
                        .ok_or(problem)?,
                    }
                }}
            }
            // Small texts such as labels, which a component keeps as `&'static str`.
            Self::Static => {
                let text = Self::Leaf(parse_quote!(::std::string::String)).make(value);
                quote!(::std::boxed::Box::leak((#text).into_boxed_str()))
            }
            Self::Leaf(ty) => quote! {{
                #[allow(unused_imports)]
                use ::ravenrs_live::bridge::{
                    ArgFromHost as _, ArgFromLive as _, ArgHostBinding as _, ArgHostBindingOf as _,
                    ArgHostClosure as _, ArgHostOption as _, ArgHostVec as _,
                };
                (&&&&&&::ravenrs_live::bridge::Arg::<#ty>::of()).make(#value)?
            }},
            Self::List(item) => {
                let ty = item.ty();
                let inner = item.make(&quote!(item));
                quote! {
                    ::ravenrs_live::bridge::list(&#value)?
                        .into_iter()
                        .map(|item| -> ::std::result::Result<#ty, ::std::string::String> { ::std::result::Result::Ok(#inner) })
                        .collect::<::std::result::Result<::std::vec::Vec<#ty>, ::std::string::String>>()?
                }
            }
            Self::Optional(item) => {
                let inner = item.make(&quote!(inner));
                quote! {
                    match ::ravenrs_live::bridge::option(&#value) {
                        ::std::option::Option::Some(inner) => ::std::option::Option::Some(#inner),
                        ::std::option::Option::None => ::std::option::Option::None,
                    }
                }
            }
            Self::Outcome(ok, err) => {
                let (ok, err) = (ok.make(&quote!(inner)), err.make(&quote!(inner)));
                quote! {
                    match ::ravenrs_live::bridge::result(&#value) {
                        ::std::result::Result::Ok(inner) => ::std::result::Result::Ok(#ok),
                        ::std::result::Result::Err(inner) => ::std::result::Result::Err(#err),
                    }
                }
            }
            Self::Tuple(items) => {
                let count = items.len();
                let names: Vec<syn::Ident> = (0..count).map(|i| format_ident!("t{}", i)).collect();
                let makes: Vec<Tokens> = items.iter().zip(&names).map(|(s, n)| s.make(&quote!(#n))).collect();
                quote! {{
                    let mut items = ::ravenrs_live::bridge::tuple(&#value, #count)?.into_iter();
                    #(let #names = items.next().unwrap_or(::ravenrs_live::Value::Unit);)*
                    (#(#makes,)*)
                }}
            }
            Self::Closure { inputs, output, shared } => {
                let out_ty = output.ty();
                let params: Vec<syn::Ident> = (0..inputs.len()).map(|i| format_ident!("p{}", i)).collect();
                let typed: Vec<Tokens> = inputs
                    .iter()
                    .zip(&params)
                    .map(|(input, p)| {
                        let ty = input.ty();
                        quote!(#p: #ty)
                    })
                    .collect();
                let checks: Vec<Tokens> = inputs
                    .iter()
                    .filter_map(|input| match input {
                        Input::Scratch(ty) => Some(quote! {{
                            #[allow(unused_imports)]
                            use ::ravenrs_live::bridge::{ScratchNone as _, ScratchSome as _};
                            (&&::ravenrs_live::bridge::ScratchOf::<#ty>::of()).check()?;
                        }}),
                        _ => None,
                    })
                    .collect();
                let values: Vec<syn::Ident> = (0..inputs.len()).map(|i| format_ident!("v{}", i)).collect();
                let lets: Vec<Tokens> = inputs
                    .iter()
                    .zip(&params)
                    .zip(&values)
                    .map(|((input, p), v)| {
                        let made = input.to_value(&quote!(#p));
                        quote!(let #v: ::ravenrs_live::Value = #made;)
                    })
                    .collect();
                let merges: Vec<Tokens> = inputs
                    .iter()
                    .zip(&params)
                    .zip(&values)
                    .filter_map(|((input, p), v)| match input {
                        Input::Scratch(ty) => Some(quote! {{
                            #[allow(unused_imports)]
                            use ::ravenrs_live::bridge::{ScratchNone as _, ScratchSome as _};
                            (&&::ravenrs_live::bridge::ScratchOf::<#ty>::of()).merge(&#v, #p);
                        }}),
                        _ => None,
                    })
                    .collect();
                let made = output.make(&quote!(out));
                let wrap = if *shared { quote!(::std::rc::Rc::new) } else { quote!(::std::boxed::Box::new) };
                let fn_ty = self.ty();
                quote! {{
                    let closure = ::ravenrs_live::bridge::closure_of(&#value)?;
                    let fallback: fn() -> #out_ty = {
                        #[allow(unused_imports)]
                        use ::ravenrs_live::bridge::{FallbackDefault as _, FallbackNone as _, FallbackOwn as _};
                        (&&&::ravenrs_live::bridge::FallbackOf::<#out_ty>::of()).get()?
                    };
                    #(#checks)*
                    let made: #fn_ty = #wrap(move |#(#typed),*| -> #out_ty {
                        #(#lets)*
                        let out = ::ravenrs_live::bridge::call_back(
                            &closure,
                            ::std::vec![#(#values.clone()),*],
                            |out: ::ravenrs_live::Value| -> ::std::result::Result<#out_ty, ::std::string::String> {
                                ::std::result::Result::Ok(#made)
                            },
                            fallback,
                        );
                        #(#merges)*
                        out
                    });
                    made
                }}
            }
        }
    }
}

impl Input {
    fn ty(&self) -> Type {
        match self {
            Self::Str => parse_quote!(&str),
            Self::Scratch(ty) => parse_quote!(&mut #ty),
            Self::Borrowed(ty) => parse_quote!(&#ty),
            Self::Owned(ty) => ty.clone(),
        }
    }

    fn to_value(&self, value: &Tokens) -> Tokens {
        match self {
            Self::Str => quote!(::ravenrs_live::Value::Str(#value.to_owned())),
            Self::Scratch(ty) => quote! {{
                #[allow(unused_imports)]
                use ::ravenrs_live::bridge::{ScratchNone as _, ScratchSome as _};
                let _ = &#value;
                (&&::ravenrs_live::bridge::ScratchOf::<#ty>::of()).fresh()
            }},
            Self::Borrowed(_) => quote! {{
                #[allow(unused_imports)]
                use ::ravenrs_live::bridge::{BorrowedCopy as _, BorrowedLive as _, BorrowedNone as _};
                (&&&::ravenrs_live::bridge::Borrowed(#value)).live()
            }},
            Self::Owned(ty) => to_value(ty, value),
        }
    }
}

/// An expression turning `value`, of type `ty`, into a `Value`.
fn to_value(ty: &Type, value: &Tokens) -> Tokens {
    let leaf = || {
        quote! {{
            #[allow(unused_imports)]
            use ::ravenrs_live::bridge::{
                RetCloneable as _, RetHost as _, RetIntoLive as _, RetOptionCopied as _, RetOptionMoved as _,
                RetVecCopied as _, RetVecMoved as _,
            };
            let out = ::ravenrs_live::bridge::Ret::new(#value);
            (&&&&&&&out).live()
        }}
    };
    match ty {
        Type::Paren(inner) => to_value(&inner.elem, value),
        Type::Group(inner) => to_value(&inner.elem, value),
        Type::Tuple(tuple) if !tuple.elems.is_empty() => {
            let names: Vec<syn::Ident> = (0..tuple.elems.len()).map(|i| format_ident!("t{}", i)).collect();
            let values: Vec<Tokens> =
                tuple.elems.iter().zip(&names).map(|(t, n)| to_value(t, &quote!(#n))).collect();
            quote! {{
                let (#(#names,)*) = #value;
                ::ravenrs_live::Value::Tuple(::std::vec![#(#values),*])
            }}
        }
        Type::Path(path) if path.qself.is_none() => {
            let Some(segment) = path.path.segments.last() else { return leaf() };
            let args = angle_types(segment);
            match (segment.ident.to_string().as_str(), args.as_slice()) {
                ("Vec", [item]) => {
                    let inner = to_value(item, &quote!(item));
                    quote!(::ravenrs_live::Value::List((#value).into_iter().map(|item| #inner).collect()))
                }
                ("Option", [item]) => {
                    let inner = to_value(item, &quote!(item));
                    quote!(::ravenrs_live::Value::Option((#value).map(|item| ::std::boxed::Box::new(#inner))))
                }
                ("Result", [ok, err]) => {
                    let (ok, err) = (to_value(ok, &quote!(item)), to_value(err, &quote!(item)));
                    quote! {
                        ::ravenrs_live::Value::Result(match #value {
                            ::std::result::Result::Ok(item) => ::std::result::Result::Ok(::std::boxed::Box::new(#ok)),
                            ::std::result::Result::Err(item) => ::std::result::Result::Err(::std::boxed::Box::new(#err)),
                        })
                    }
                }
                _ => leaf(),
            }
        }
        _ => leaf(),
    }
}

/// How a result of type `ty` becomes a value, written out, or `None` when it cannot.
fn result_value(ty: &Type, value: &Tokens, map: &HashMap<String, Type>) -> Option<Tokens> {
    if let Some(bound) = impl_bound(ty) {
        let segment = bound.path.segments.last()?;
        return match segment.ident.to_string().as_str() {
            "View" => Some(quote!({
                let view: crate::AnyView = ::std::boxed::Box::new(#value);
                ::ravenrs_live::Value::Host(::ravenrs_live::Host::new("AnyView", view))
            })),
            "Iterator" | "IntoIterator" | "DoubleEndedIterator" | "ExactSizeIterator" => {
                let item = assoc_item(segment)?;
                let item = substituted(item, map);
                if has_impl(&item) || borrows(&item) {
                    return None;
                }
                let inner = to_value(&item, &quote!(item));
                Some(quote!(::ravenrs_live::Value::List((#value).into_iter().map(|item| #inner).collect())))
            }
            _ => None,
        };
    }
    match ty {
        Type::Reference(reference) => match &*reference.elem {
            Type::Path(path) if path.path.is_ident("str") => {
                Some(quote!(::ravenrs_live::Value::Str((#value).to_owned())))
            }
            Type::Slice(slice) if !borrows(&slice.elem) && !has_impl(&slice.elem) => {
                let inner = to_value(&slice.elem, &quote!(item));
                Some(quote! {{
                    #[allow(unused_imports)]
                    use ::ravenrs_live::bridge::{CloneOfNone as _, CloneOfSome as _};
                    ::ravenrs_live::Value::List((#value).iter().filter_map(|item| (&&::ravenrs_live::bridge::CloneOf(item)).cloned()).map(|item| #inner).collect())
                }})
            }
            elem if !borrows(elem) && !has_impl(elem) => Some(quote! {{
                #[allow(unused_imports)]
                use ::ravenrs_live::bridge::{BorrowedCopy as _, BorrowedLive as _, BorrowedNone as _};
                (&&&::ravenrs_live::bridge::Borrowed(#value)).live()
            }}),
            _ => None,
        },
        _ if borrows(ty) || has_impl(ty) => None,
        _ => Some(to_value(&substituted(ty, map), value)),
    }
}

fn assoc_item(segment: &syn::PathSegment) -> Option<&Type> {
    let PathArguments::AngleBracketed(args) = &segment.arguments else { return None };
    args.args.iter().find_map(|a| match a {
        GenericArgument::AssocType(assoc) if assoc.ident == "Item" => Some(&assoc.ty),
        _ => None,
    })
}

/// The shape a value is made into for a parameter or part written `ty`.
fn shape(ty: &Type, map: &HashMap<String, Type>) -> Option<Shape> {
    if needs_threads(ty) {
        return None;
    }
    if let Some(bound) = impl_bound(ty) {
        let segment = bound.path.segments.last()?;
        return match segment.ident.to_string().as_str() {
            "Into" => match shape(angle_types(segment).first()?, map)? {
                Shape::Leaf(ty) if !plain(&ty) => Some(Shape::IntoOf(ty)),
                other => Some(other),
            },
            "AsRef" | "ToString" | "Display" => Some(Shape::Leaf(parse_quote!(::std::string::String))),
            "View" => Some(Shape::Leaf(parse_quote!(crate::AnyView))),
            "ViewSeq" => Some(Shape::Leaf(parse_quote!(::std::vec::Vec<crate::AnyView>))),
            "Fn" | "FnMut" | "FnOnce" => closure_shape(segment, map, false),
            "IntoIterator" => Some(Shape::List(Box::new(shape(assoc_item(segment)?, map)?))),
            _ => None,
        };
    }
    match ty {
        Type::Paren(inner) => shape(&inner.elem, map),
        Type::Group(inner) => shape(&inner.elem, map),
        Type::Tuple(tuple) if tuple.elems.is_empty() => Some(Shape::Leaf(parse_quote!(()))),
        Type::Tuple(tuple) => {
            Some(Shape::Tuple(tuple.elems.iter().map(|t| shape(t, map)).collect::<Option<_>>()?))
        }
        Type::Path(path) if path.qself.is_none() => {
            if path.path.segments.len() == 1
                && let Some(concrete) = map.get(&path.path.segments[0].ident.to_string())
                && path.path.segments[0].arguments.is_empty()
            {
                return shape(concrete, map);
            }
            let segment = path.path.segments.last()?;
            let args = angle_types(segment);
            match (segment.ident.to_string().as_str(), args.as_slice()) {
                ("Vec", [item]) => Some(Shape::List(Box::new(shape(item, map)?))),
                ("Option", [item]) => Some(Shape::Optional(Box::new(shape(item, map)?))),
                ("Result", [ok, err]) => {
                    Some(Shape::Outcome(Box::new(shape(ok, map)?), Box::new(shape(err, map)?)))
                }
                ("Rc" | "Arc", [Type::Slice(slice)]) => Some(Shape::Into(
                    Box::new(Shape::List(Box::new(shape(&slice.elem, map)?))),
                    substituted(ty, map),
                )),
                ("Box" | "Rc", [Type::TraitObject(object)]) => {
                    let segment = object.bounds.iter().find_map(|b| match b {
                        TypeParamBound::Trait(t) => t.path.segments.last(),
                        _ => None,
                    })?;
                    if !matches!(segment.ident.to_string().as_str(), "Fn" | "FnMut" | "FnOnce") {
                        return None;
                    }
                    if object.bounds.iter().any(|b| matches!(b, TypeParamBound::Trait(t) if matches!(last_ident(&t.path).as_str(), "Send" | "Sync"))) {
                        return None;
                    }
                    closure_shape(segment, map, segment.ident == "Fn" && last_ident(&path.path) == "Rc")
                }
                _ if has_impl(ty) => None,
                _ => {
                    let ty = substituted(ty, map);
                    if has_impl(&ty) { None } else { Some(Shape::Leaf(ty)) }
                }
            }
        }
        Type::Array(_) if !has_impl(ty) => Some(Shape::Leaf(substituted(ty, map))),
        Type::Reference(reference)
            if reference.mutability.is_none()
                && reference.lifetime.as_ref().is_some_and(|l| l.ident == "static")
                && matches!(&*reference.elem, Type::Path(p) if p.path.is_ident("str")) =>
        {
            Some(Shape::Static)
        }
        _ => None,
    }
}

/// Whether the interpreter has the type as a value of its own: text, a number or a bool.
fn plain(ty: &Type) -> bool {
    let Type::Path(path) = ty else { return matches!(ty, Type::Tuple(t) if t.elems.is_empty()) };
    if path.path.segments.iter().any(|s| s.ident == "ravenrs_live") {
        return true;
    }
    if last_ident(&path.path) == "Value" {
        return false;
    }
    matches!(
        last_ident(&path.path).as_str(),
        "String"
            | "bool"
            | "char"
            | "f32"
            | "f64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "usize"
            | "isize"
            | "AnyView"
            | "Value"
    )
}

fn closure_shape(segment: &syn::PathSegment, map: &HashMap<String, Type>, shared: bool) -> Option<Shape> {
    let PathArguments::Parenthesized(args) = &segment.arguments else { return None };
    let mut inputs = Vec::new();
    for input in &args.inputs {
        let input = substituted(input, map);
        inputs.push(match &input {
            Type::Reference(reference) => {
                let elem = (*reference.elem).clone();
                if matches!(elem, Type::TraitObject(_)) {
                    return None;
                }
                if reference.mutability.is_some() {
                    Input::Scratch(elem)
                } else if matches!(&elem, Type::Path(p) if p.path.is_ident("str")) {
                    Input::Str
                } else if has_impl(&elem) || borrows(&elem) || matches!(elem, Type::Slice(_)) {
                    return None;
                } else {
                    Input::Borrowed(elem)
                }
            }
            other if has_impl(other) || borrows(other) => return None,
            other => Input::Owned(other.clone()),
        });
    }
    let output = match &args.output {
        ReturnType::Default => Shape::Leaf(parse_quote!(())),
        ReturnType::Type(_, t) => shape(t, map)?,
    };
    Some(Shape::Closure { inputs, output: Box::new(output), shared })
}

fn param(ty: &Type, map: &HashMap<String, Type>) -> Option<Param> {
    if let Type::Reference(reference) = ty {
        if reference.mutability.is_some() || matches!(&*reference.elem, Type::TraitObject(_)) {
            return None;
        }
        if reference.lifetime.as_ref().is_some_and(|l| l.ident == "static") {
            return shape(ty, map).map(|shape| Param { shape, by_ref: false });
        }
        let shape = match &*reference.elem {
            Type::Path(path) if path.path.is_ident("str") => Shape::Leaf(parse_quote!(::std::string::String)),
            Type::Slice(slice) => Shape::List(Box::new(shape(&slice.elem, map)?)),
            other => shape(other, map)?,
        };
        return Some(Param { shape, by_ref: true });
    }
    Some(Param { shape: shape(ty, map)?, by_ref: false })
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

fn expose_impl(block: &syn::ItemImpl, options: &Options) -> syn::Result<Tokens> {
    // The methods of a trait it implements are all there is to see, as public as the trait.
    let of_trait = block.trait_.is_some();
    let Type::Path(self_path) = &*block.self_ty else {
        return Err(syn::Error::new(block.self_ty.span(), "#[expose] needs a named type"));
    };
    let name = last_ident(&self_path.path);
    let entry = entry_name(options);
    let alternatives: Vec<&[(String, Type)]> =
        if options.with.is_empty() { vec![&[]] } else { options.with.iter().map(Vec::as_slice).collect() };
    let mut bodies = Vec::new();
    let mut first_self = None;
    for given in alternatives {
        // A block whose generics cannot be made concrete offers nothing, and fails nothing.
        let Some(map) = block_substitutions(&block.generics, given) else { continue };
        let self_ty = substituted(&block.self_ty, &map);
        let mut registrations = Vec::new();
        for item in &block.items {
            match item {
                ImplItem::Fn(function) => {
                    if !of_trait && !matches!(function.vis, syn::Visibility::Public(_)) {
                        continue;
                    }
                    let call = Callee::Type(&self_ty);
                    if let Some(registration) =
                        function_registration(&function.sig, &name, call, &map, options)
                    {
                        registrations.push(registration);
                    }
                }
                ImplItem::Const(constant) if matches!(constant.vis, syn::Visibility::Public(_)) => {
                    if borrows(&constant.ty) {
                        continue;
                    }
                    let ident = &constant.ident;
                    let text = ident.to_string();
                    let value = to_value(&substituted(&constant.ty, &map), &quote!(<#self_ty>::#ident));
                    registrations.push(quote! {
                        registry.constant(#name, #text, || #value);
                    });
                }
                _ => {}
            }
        }
        bodies.push(quote!(#(#registrations)*));
        first_self.get_or_insert(self_ty);
    }
    let Some(self_ty) = first_self else { return Ok(Tokens::new()) };
    Ok(quote! {
        #[cfg(feature = "live")]
        impl #self_ty {
            #[doc(hidden)]
            #[allow(clippy::all, clippy::pedantic, unused_variables, unused_mut)]
            pub fn #entry(registry: &mut ::ravenrs_live::Registry) {
                #(#bodies)*
            }
        }
    })
}

/// Who a function belongs to.
#[derive(Clone, Copy)]
enum Callee<'a> {
    /// An associated function or method of this type.
    Type(&'a Type),
    /// A free function of the module the registration is written in.
    Module,
}

fn function_registration(
    sig: &syn::Signature,
    name: &str,
    callee: Callee<'_>,
    outer: &HashMap<String, Type>,
    options: &Options,
) -> Option<Tokens> {
    if sig.asyncness.is_some() || sig.unsafety.is_some() {
        return None;
    }
    let fn_name = sig.ident.to_string();
    if options.skip.contains(&fn_name) || fn_name.starts_with("__") {
        return None;
    }
    // A parameter no argument mentions cannot be inferred from what the call is given.
    let inputs = sig.inputs.iter().map(|i| quote!(#i).to_string()).collect::<Vec<_>>().join(" ");
    let words: Vec<&str> = inputs.split(|c: char| !c.is_alphanumeric() && c != '_').collect();
    for param in sig.generics.type_params() {
        if !words.contains(&param.ident.to_string().as_str()) {
            return None;
        }
    }
    let mut map = outer.clone();
    map.extend(function_substitutions(&sig.generics)?);
    if let Callee::Type(self_ty) = callee {
        map.insert("Self".to_owned(), self_ty.clone());
    }
    let mut receiver = None;
    let mut params = Vec::new();
    for input in &sig.inputs {
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
    let self_ty = match callee {
        Callee::Type(ty) => Some(ty),
        Callee::Module => None,
    };
    if receiver.is_some() && self_ty.is_none() {
        return None;
    }
    let offset = usize::from(receiver.is_some());
    let count = params.len() + offset;
    let ident = &sig.ident;
    let names: Vec<syn::Ident> = (0..params.len()).map(|i| format_ident!("a{}", i)).collect();
    let makes: Vec<Tokens> = params
        .iter()
        .zip(&names)
        .enumerate()
        .map(|(i, (p, n))| {
            let index = i + offset;
            let made = p.shape.make(&quote!(args[#index].clone()));
            let ty = p.shape.ty();
            quote!(let #n: #ty = #made;)
        })
        .collect();
    let passed: Vec<Tokens> =
        params.iter().zip(&names).map(|(p, n)| if p.by_ref { quote!(&#n) } else { quote!(#n) }).collect();
    let out = match &sig.output {
        ReturnType::Default => quote!({
            let _: () = out;
            ::ravenrs_live::Value::Unit
        }),
        ReturnType::Type(_, ty) => result_value(ty, &quote!(out), &map)?,
    };
    let call = match (receiver, self_ty) {
        (None, Some(self_ty)) => quote!({ let out = <#self_ty>::#ident(#(#passed),*); #out }),
        (None, None) => quote!({ let out = #ident(#(#passed),*); #out }),
        (Some((false, _)), Some(self_ty)) => {
            let this = Shape::Leaf(self_ty.clone()).make(&quote!(args[0].clone()));
            quote!({ let this: #self_ty = #this; let out = this.#ident(#(#passed),*); #out })
        }
        (Some((true, false)), Some(self_ty)) => quote! {
            ::ravenrs_live::bridge::with_ref::<#self_ty, _>(&args[0], |this| { let out = this.#ident(#(#passed),*); #out })?
        },
        (Some((true, true)), Some(self_ty)) => quote! {
            ::ravenrs_live::bridge::with_mut::<#self_ty, _>(&args[0], |this| { let out = this.#ident(#(#passed),*); #out })?
        },
        (Some(_), None) => return None,
    };
    let register = match (receiver, self_ty) {
        (Some(_), Some(self_ty)) => quote!(method_for::<#self_ty>),
        _ => quote!(function),
    };
    Some(quote! {
        registry.#register(#name, #fn_name, |args: ::std::vec::Vec<::ravenrs_live::Value>| {
            if args.len() != #count {
                return ::std::result::Result::Err(::std::format!("`{}` takes {} arguments, and got {}", #fn_name, #count, args.len()));
            }
            #(#makes)*
            ::std::result::Result::Ok(#call)
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
    let map = HashMap::new();
    let mut registrations = Vec::new();
    for variant in &choice.variants {
        let ident = &variant.ident;
        let text = ident.to_string();
        match &variant.fields {
            syn::Fields::Unit => {
                let value = to_value(&parse_quote!(#ty), &quote!(#ty::#ident));
                registrations.push(quote! {
                    registry.constant(#name, #text, || #value);
                });
            }
            syn::Fields::Unnamed(fields) => {
                let Some(shapes) =
                    fields.unnamed.iter().map(|f| shape(&f.ty, &map)).collect::<Option<Vec<Shape>>>()
                else {
                    continue;
                };
                let count = shapes.len();
                let names: Vec<syn::Ident> = (0..count).map(|i| format_ident!("a{}", i)).collect();
                let makes = shapes.iter().zip(&names).enumerate().map(|(i, (s, n))| {
                    let made = s.make(&quote!(args[#i].clone()));
                    let t = s.ty();
                    quote!(let #n: #t = #made;)
                });
                let value = to_value(&parse_quote!(#ty), &quote!(#ty::#ident(#(#names),*)));
                registrations.push(quote! {
                    registry.function(#name, #text, |args: ::std::vec::Vec<::ravenrs_live::Value>| {
                        if args.len() != #count {
                            return ::std::result::Result::Err(::std::format!("`{}` takes {} values", #text, #count));
                        }
                        #(#makes)*
                        ::std::result::Result::Ok(#value)
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
            #[allow(clippy::all, clippy::pedantic, unused_variables, unused_mut)]
            pub fn #entry(registry: &mut ::ravenrs_live::Registry) {
                #(#registrations)*
            }
        }
    })
}

/// From how many fields a struct literal is built field by field instead of in one expression.
const LARGE_STRUCT: usize = 32;

/// `Name { field: .., ..base }` for a struct with many fields. One expression with every field,
/// as smaller structs get, becomes a function that an optimized build takes hours to compile
/// (`Labels` has hundreds). Here the struct starts from `..base`, or from its `Default` when
/// every field is given, and each field is changed by a function of its own.
fn large_record(ty: &syn::Ident, name: &str, literal: &[(syn::Ident, String, Type, Tokens)]) -> Tokens {
    let texts: Vec<&String> = literal.iter().map(|(_, t, ..)| t).collect();
    let setters = literal.iter().map(|(ident, text, field_ty, make)| {
        quote! {
            {
                #[inline(never)]
                fn set(
                    made: &mut #ty,
                    fields: &mut ::std::collections::HashMap<::std::string::String, ::ravenrs_live::Value>,
                ) -> ::std::result::Result<(), ::std::string::String> {
                    if let ::std::option::Option::Some(value) = fields.remove(#text) {
                        let value: #field_ty = #make;
                        made.#ident = value;
                    }
                    ::std::result::Result::Ok(())
                }
                set(&mut made, &mut fields)?;
            }
        }
    });
    let base = Shape::Leaf(parse_quote!(#ty)).make(&quote!(base));
    let made = to_value(&parse_quote!(#ty), &quote!(made));
    quote! {
        registry.record(#name, |fields: ::std::vec::Vec<(::std::string::String, ::ravenrs_live::Value)>, base: ::std::option::Option<::ravenrs_live::Value>| {
            let mut fields: ::std::collections::HashMap<::std::string::String, ::ravenrs_live::Value> = fields.into_iter().collect();
            let mut made: #ty = match base {
                ::std::option::Option::Some(base) => #base,
                ::std::option::Option::None => {
                    for text in [#(#texts),*] {
                        if !fields.contains_key(text) {
                            return ::std::result::Result::Err(::std::format!("the field `{}` of `{}` is missing", text, #name));
                        }
                    }
                    #[allow(unused_imports)]
                    use ::ravenrs_live::bridge::{DefaultNone as _, DefaultSome as _};
                    match (&&::ravenrs_live::bridge::DefaultOf::<#ty>::of()).make() {
                        ::std::option::Option::Some(made) => made,
                        ::std::option::Option::None => {
                            return ::std::result::Result::Err(::std::format!("`{}` has too many fields to be written whole: start it from `..` another", #name));
                        }
                    }
                }
            };
            #(#setters)*
            if let ::std::option::Option::Some(extra) = fields.keys().next() {
                return ::std::result::Result::Err(::std::format!("`{}` has no field `{}`", #name, extra));
            }
            ::std::result::Result::Ok(#made)
        });
    }
}

/// A struct's public fields: read with `value.field`, changed with `value.field = ..`, and the
/// whole struct written as `Name { field: .., ..base }` when every field is public.
fn expose_struct(declared: &syn::ItemStruct, options: &Options) -> syn::Result<Tokens> {
    if !declared.generics.params.is_empty() {
        return Ok(Tokens::new());
    }
    let ty = &declared.ident;
    let name = ty.to_string();
    let entry = entry_name(options);
    let map = HashMap::new();
    let made_default = to_value(&parse_quote!(#ty), &quote!(made));
    let mut registrations = vec![quote! {
        registry.function(#name, "default", |args: ::std::vec::Vec<::ravenrs_live::Value>| {
            #[allow(unused_imports)]
            use ::ravenrs_live::bridge::{DefaultNone as _, DefaultSome as _};
            match (&&::ravenrs_live::bridge::DefaultOf::<#ty>::of()).make() {
                ::std::option::Option::Some(made) => ::std::result::Result::Ok(#made_default),
                ::std::option::Option::None => ::std::result::Result::Err(::std::format!("`{}` has no default", #name)),
            }
        });
    }];
    let empty = syn::punctuated::Punctuated::new();
    let named = match &declared.fields {
        syn::Fields::Named(fields) => &fields.named,
        _ => &empty,
    };
    let mut all_public = true;
    let mut literal = Vec::new();
    for field in named {
        let Some(ident) = &field.ident else { continue };
        let text = ident.to_string();
        if !matches!(field.vis, syn::Visibility::Public(_)) {
            all_public = false;
            continue;
        }
        if borrows(&field.ty) || has_impl(&field.ty) {
            all_public = false;
            continue;
        }
        let read = to_value(&field.ty, &quote!(item));
        registrations.push(quote! {
            registry.field(#name, #text, |value: &::ravenrs_live::Value| {
                #[allow(unused_imports)]
                use ::ravenrs_live::bridge::{CloneOfNone as _, CloneOfSome as _};
                let copy = ::ravenrs_live::bridge::with_ref::<#ty, _>(value, |this| (&&::ravenrs_live::bridge::CloneOf(&this.#ident)).cloned())?;
                match copy {
                    ::std::option::Option::Some(item) => ::std::result::Result::Ok(#read),
                    ::std::option::Option::None => ::std::result::Result::Err(::std::format!("the field `{}` does not copy", #text)),
                }
            });
        });
        match shape(&field.ty, &map) {
            Some(made) => {
                let field_ty = made.ty();
                let make = made.make(&quote!(value));
                registrations.push(quote! {
                    registry.set_field(#name, #text, |this: &::ravenrs_live::Value, value: ::ravenrs_live::Value| {
                        let made: #field_ty = #make;
                        ::ravenrs_live::bridge::with_mut::<#ty, _>(this, |this| this.#ident = made)
                    });
                });
                literal.push((ident.clone(), text, field_ty, make));
            }
            None => all_public = false,
        }
    }
    if all_public && literal.len() > LARGE_STRUCT {
        registrations.push(large_record(ty, &name, &literal));
    } else if all_public && !literal.is_empty() {
        let idents: Vec<&syn::Ident> = literal.iter().map(|(i, ..)| i).collect();
        let kept: Vec<syn::Ident> = idents.iter().map(|i| format_ident!("base_{}", i)).collect();
        let texts: Vec<&String> = literal.iter().map(|(_, t, ..)| t).collect();
        let types: Vec<&Type> = literal.iter().map(|(_, _, t, _)| t).collect();
        let makes: Vec<&Tokens> = literal.iter().map(|(.., m)| m).collect();
        let base = Shape::Leaf(parse_quote!(#ty)).make(&quote!(base));
        let made = to_value(&parse_quote!(#ty), &quote!(made));
        registrations.push(quote! {
            registry.record(#name, |fields: ::std::vec::Vec<(::std::string::String, ::ravenrs_live::Value)>, base: ::std::option::Option<::ravenrs_live::Value>| {
                let mut fields: ::std::collections::HashMap<::std::string::String, ::ravenrs_live::Value> = fields.into_iter().collect();
                let base: ::std::option::Option<#ty> = match base {
                    ::std::option::Option::Some(base) => ::std::option::Option::Some(#base),
                    ::std::option::Option::None => ::std::option::Option::None,
                };
                #(let mut #kept: ::std::option::Option<#types> = ::std::option::Option::None;)*
                if let ::std::option::Option::Some(#ty { #(#idents),* }) = base {
                    #(#kept = ::std::option::Option::Some(#idents);)*
                }
                let made = #ty {
                    #(#idents: match fields.remove(#texts) {
                        ::std::option::Option::Some(value) => { let made: #types = #makes; made }
                        ::std::option::Option::None => #kept.take().ok_or_else(|| ::std::format!("the field `{}` of `{}` is missing", #texts, #name))?,
                    },)*
                };
                if let ::std::option::Option::Some(extra) = fields.keys().next() {
                    return ::std::result::Result::Err(::std::format!("`{}` has no field `{}`", #name, extra));
                }
                ::std::result::Result::Ok(#made)
            });
        });
    }
    Ok(quote! {
        #[cfg(feature = "live")]
        impl #ty {
            #[doc(hidden)]
            #[allow(clippy::all, clippy::pedantic, unused_variables, unused_mut)]
            pub fn #entry(registry: &mut ::ravenrs_live::Registry) {
                #(#registrations)*
            }
        }
    })
}

/// The public functions of an inline module, called as `module::function(..)`.
fn expose_mod(module: &syn::ItemMod, options: &Options) -> syn::Result<Tokens> {
    let Some((brace, items)) = &module.content else {
        return Err(syn::Error::new(module.span(), "#[expose] goes on a module written inline"));
    };
    let name = module.ident.to_string();
    let entry = entry_name(options);
    let map = HashMap::new();
    let registrations: Vec<Tokens> = items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) if matches!(function.vis, syn::Visibility::Public(_)) => {
                function_registration(&function.sig, &name, Callee::Module, &map, options)
            }
            _ => None,
        })
        .collect();
    let mut module = module.clone();
    let added: Item = parse_quote! {
        #[cfg(feature = "live")]
        #[doc(hidden)]
        #[allow(clippy::all, clippy::pedantic, unused_variables, unused_mut)]
        pub fn #entry(registry: &mut ::ravenrs_live::Registry) {
            #(#registrations)*
        }
    };
    let _ = brace;
    if let Some((_, items)) = &mut module.content {
        items.push(added);
    }
    Ok(quote!(#module))
}
