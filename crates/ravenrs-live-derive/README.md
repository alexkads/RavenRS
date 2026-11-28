# ravenrs-live-derive

Part of [RavenRS](https://github.com/alexkads/RavenRS): the attribute `#[ravenrs_live::expose]`, which writes for you the code that lets the playground's interpreter (`ravenrs-live`) call a type's functions, read its fields and name its variants.

## Role

The interpreter in `ravenrs-live` reaches the host program only through a `Registry`: a table from names such as `("Button", "new")` to closures that take interpreter `Value`s, turn them into Rust arguments, call the real function and turn the result back. Writing those closures by hand for hundreds of components would be thousands of lines that drift from the code they wrap.

`#[expose]` reads an item's signatures at compile time and generates those registrations as one hidden function per item. It is a procedural macro and nothing else: it has no runtime, and the code it generates calls into `ravenrs_live` and `ravenrs_live::bridge`.

Use it through its re-export, `ravenrs_live::expose`; this crate is never named directly. In the RavenRS repository nobody writes the attribute by hand either: `scripts/gen-live.py` puts it on every inherent `impl` block, public struct, public enum and inline public module of the crates the examples use, and writes the `src/live_list.rs` that calls what it generates. An app only writes it for its own types.

## How it works

On each item the attribute keeps the item unchanged and adds, behind `#[cfg(feature = "live")]`:

```rust,ignore
impl Badge {
    #[doc(hidden)]
    pub fn __live_expose(registry: &mut ::ravenrs_live::Registry) {
        registry.function("Badge", "new", |args| { /* check the count, make each argument, call, wrap */ });
        registry.method_for::<Badge>("Badge", "count", |args| { /* args[0] is the receiver */ });
        // ...
    }
}
```

On a module the function is added inside the module instead. Nothing runs at compile time beyond generating this; the registrations happen when the host calls the function while it builds its registry.

The type is registered under the last segment of its path, without generics (`ravenrs_core::Color` is `Color`), which is the name a program writes and the name `ravenrs_live::bridge::host_name` gives the values the generated code returns.

Each generated closure:

1. checks the number of arguments, and refuses with "`count` takes 2 arguments, and got 1";
2. makes each argument from its `Value`, choosing the conversion at compile time from how the parameter's type is written (see "What each item gives");
3. calls the real function: by value for `self`, which moves the host value out, through `bridge::with_ref` for `&self`, and through `bridge::with_mut` for `&mut self`, which changes the host value in place;
4. turns the result into a `Value`: `()` into `Unit`, numbers, text and bools into themselves, an `impl View` into an `AnyView` host value, an iterator into a list, a `Clone` type into a host value that copies, and anything else into a host value that moves (`Host::new`).

The conversions go through traits in `ravenrs_live::bridge` that are picked by autoref specialization: the generated code always compiles, whatever traits the type implements, and a type that cannot be converted fails with a message when the program uses it, instead of failing the build.

## Use from start to finish

The crate that uses the attribute needs `ravenrs-live` as a dependency, reachable as `::ravenrs_live`, and a feature named `live`, because the generated code is behind `cfg(feature = "live")`. Making the dependency optional and the attribute conditional keeps it out of builds that have no playground:

```toml
[features]
live = ["dep:ravenrs-live"]

[dependencies]
ravenrs-live = { version = "...", optional = true }
```

Then, on the types:

```rust,ignore
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Badge {
    pub text: String,
    pub count: u32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Badge {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), count: 0 }
    }

    pub fn count(mut self, count: u32) -> Self {
        self.count = count;
        self
    }

    pub fn label(&self) -> String {
        format!("{} ({})", self.text, self.count)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Tone {
    Neutral,
    Danger,
}

/// Everything of this crate the interpreter can reach.
#[cfg(feature = "live")]
pub fn live_registry() -> ravenrs_live::Registry {
    let mut registry = ravenrs_live::Registry::new();
    Badge::__live_expose(&mut registry);
    Badge::__live_expose_fields(&mut registry);
    Tone::__live_expose_variants(&mut registry);
    registry
}
```

And on the host side, a program can now use all of it:

```rust,ignore
use std::rc::Rc;

let source = r#"
fn badge() -> String {
    let mut badge = Badge::new("inbox").count(3);
    badge.count = badge.count + 1;          // the field setter
    let copy = Badge { text: "sent".to_owned(), ..Badge::default() };  // the record and `default`
    let _ = Tone::Danger;                   // a constant from a unit variant
    format!("{} / {}", badge.label(), copy.label())
}
"#;
let value = ravenrs_live::run(source, None, Rc::new(live_registry())).map_err(|e| e[0].to_string())?;
assert_eq!(value, ravenrs_live::Value::Str("inbox (4) / sent (0)".into()));
```

The examples here are `ignore`d: the generated code exists only with the `live` feature of the crate that writes the attribute, which a doctest of this proc-macro crate does not have. The runnable end-to-end example, with a registry filled by hand, is in the `ravenrs-live` README; `crates/ravenrs-widgets/src/live_list.rs` is the real use of hundreds of generated functions.

### What each item gives

**Inherent `impl` block.** Each `pub fn` becomes `Type::name(..)` when it has no receiver (`registry.function`) or `value.name(..)` when it takes `self`, `&self` or `&mut self` (`registry.method_for::<Type>`, so forms of one generic type that share a name stay apart). Each `pub const` becomes `Type::NAME` (`registry.constant`). A `#[cfg(..)]` on a function or constant is copied onto its registration.

**Trait `impl` block** (`impl Trait for Type`). Every method is registered, whatever its visibility, since they are as public as the trait; `scripts/gen-live.py` uses this for the traits it lists.

**Struct with named fields.** `Type::default()` is always registered, and refuses at run time with "`Badge` has no default" when the type has none. Each `pub` field gets a reader (`value.field`, which needs the field to be `Clone`, else "the field `x` does not copy") and, when its type can be made from a value, a setter (`value.field = ..`). When every field is public and settable, `Type { a: .., ..base }` builds the struct (`registry.record`); missing fields come from `..base`, and a field the struct does not have is an error. Above 32 fields the struct is built from `..base` or its `Default`, one field at a time, because one expression with hundreds of fields takes hours to compile optimized. A tuple or unit struct only gets `default`.

**Enum.** A unit variant becomes the constant `Type::Variant`; a tuple variant becomes the function `Type::Variant(..)`, its fields made like arguments. Variants with named fields are left out.

**Inline module** (`mod checks { .. }`). Each `pub fn` becomes `checks::name(..)`. A module in its own file (`mod checks;`) is refused, because the attribute cannot see its contents.

### Arguments

- **`part = "name"`** names the generated function `__live_expose_name` instead of `__live_expose`. Each attribute on a type adds an inherent function to it, so a type with its struct and its `impl` exposed needs two names. The repository's convention is `part = "fields"` on structs, `part = "variants"` on enums, nothing on the first `impl` block and `part = "part1"`, `part = "part2"`, ... on the others of the same type. The host calls each of them.
- **`skip = "a, b"`** leaves those functions out, for one that should not be reachable or that needs a hand-written registration instead. Functions whose names start with `__` are always left out.
- **`with = "P = (); P = Other"`** makes a generic block concrete: the block is registered once for each `;`-separated set of `Name = Type`, all from the one generated function, which is put on the block's type with the first set.

### Generics, and what a signature turns into

On the block, a type parameter bounded by `View` becomes `AnyView`, one bounded by `ViewSeq` becomes `Vec<AnyView>`, and any other needs `with`. On a function:

- a parameter bounded by `View` or `ViewSeq` becomes the same;
- one bounded only by `Clone`, `PartialEq`, `Debug` or lifetimes takes any interpreter `Value`;
- `impl Into<T>` (or `T: Into<..>`) takes a `T`, or a text, number or bool a `T` can be made from;
- `impl AsRef<str>`, `impl ToString` and `impl Display` take a text;
- `impl Fn(..)`, `FnMut`, `FnOnce`, `Box<dyn Fn(..)>` and `Rc<dyn Fn(..)>` take a closure of the program. The host calls it later; a failure there is reported with `ravenrs_live::report` and the closure gives the result type's `Fallback` or `Default`. A closure parameter `&str` arrives as text, `&T` as a copy or host value, and `&mut T` as a fresh `T` that is merged back afterwards when `T` implements `bridge::Scratch` (as a canvas's scene does);
- `impl IntoIterator<Item = T>` takes a list.

Parameters may be `&str` and `&[T]` (made from text and lists, then borrowed), `&T`, `Vec`, `Option`, `Result`, tuples, arrays, `Rc<[T]>` and `Arc<[T]>`, and `&'static str`. Beware the last one: the text is leaked for the rest of the process, once per call, which is fine for labels and wrong for anything called in a loop.

Results may be anything above that does not borrow, plus `&str` (copied into a text), `&[T]` (copied into a list) and `&T` (copied when `T` is `Clone`), `impl View`, and `impl Iterator<Item = T>` and its relatives (collected into a list).

### What is left out, and what is refused

Left out without failing the build, because the interpreter could not call it with its values:

- `async` and `unsafe` functions, and receivers with a type, such as `self: Rc<Self>`;
- parameters `&mut T` and `&dyn Trait`, `impl Trait` bounds other than those above, and anything that asks for `Send` or `Sync`, which the interpreter's closures are not;
- a function type parameter that no argument mentions (it could not be inferred from the call), and const generics on a function;
- results that hold a reference or a lifetime (other than the copied forms above) or an `impl Trait` other than `View` and the iterators;
- `impl` blocks with lifetime or const parameters, or with a type parameter that is neither `View` nor `ViewSeq` nor given in `with`; structs and enums with any generics;
- private functions, constants and fields of an inherent block or struct.

What is left out is simply missing from the registry, and a program that calls it gets "`Type::name` is not one the playground knows yet". Register such a function by hand with `Registry::function` or `Registry::method`, as `crates/ravenrs-widgets/src/live.rs` does for `Text::bound`.

Refused with a compile error at the attribute:

- any other item: a free function, a trait, a type alias, a `use` ("#[expose] goes on an inherent impl block, a struct, an enum or a module");
- an `impl` for a type that is not a path, such as a tuple or a reference ("#[expose] needs a named type");
- a module that is not inline;
- an argument other than `part`, `skip` and `with`, or one whose value is not a string literal.

## What comes for free

- **The argument count is checked** for every generated function and method, with the name in the message.
- **Moves behave as in Rust.** A builder method taking `self` consumes the host value; the program gets "already used" if it reuses it, and `Clone` results come back as values that copy, so colors and insets can be used twice.
- **Feature-gated by construction.** Without the `live` feature of the crate that writes the attribute, nothing is generated and the crate does not depend on the interpreter at all.

## Per platform

Nothing changes per platform: the attribute runs in the compiler, and the code it generates is plain Rust that works the same on desktop and WebAssembly.

## When something goes wrong

- **"no function or associated item named `__live_expose_fields` found"** in the host: the item has no attribute with that `part`, or the `live` feature is off in the crate that defines it. Enable the feature, which for RavenRS crates is `live` on `ravenrs-widgets` or on the `ravenrs` facade.
- **"duplicate definitions with name `__live_expose`"**: two attributes on one type without distinct `part`s.
- **"cannot find type `AnyView` in the crate root"**: a signature uses a `View` or `ViewSeq` bound, or returns `impl View`, in a crate with no `AnyView` at its root. The generated code names `crate::AnyView`; re-export it there, or `skip` those functions.
- **A function is missing at run time** ("is not one the playground knows yet"): its signature is one of those left out above. Change the signature, or register it by hand.
- **"a closure that gives a X cannot be written in the playground yet"**: a parameter takes a closure whose result type has neither `Fallback` nor `Default`, so there is nothing to give back when it fails. Implement `ravenrs_live::Fallback` for it.
- **"a closure that changes a X cannot be written in the playground yet"**: a closure parameter takes `&mut X` and `X` does not implement `ravenrs_live::bridge::Scratch`.

To see what was generated, expand the item with `cargo expand --features live` in the crate that uses it. A bug report is most useful with the item's signature, the program that called it and the message it got.

## License

MIT OR Apache-2.0.
