# ravenrs-live

Part of [RavenRS](https://github.com/alexkads/RavenRS): an interpreter of the Rust that small RavenRS examples are written in, so a playground can run edited code at once, in the desktop app and in the browser, without a compiler.

## Role

A RavenRS example is a short Rust function that returns a view:

```rust,ignore
fn counter() -> impl View {
    let clicks = State::new(0);
    VStack::new((
        Text::bound(&clicks, |n| format!("{n} clicks")),
        Button::new("Add").on_press(move || clicks.update(|n| *n += 1)),
    ))
}
```

`ravenrs-live` reads that text with `syn`, walks the syntax tree and evaluates it. Every `Button::new`, `.on_press(..)` or `State::new` it meets is a call into the real component compiled into the host program, so what comes out is the same `Button` a compiled example builds, not a look-alike.

Who uses it:

- **Most people never call it directly.** The `Playground` component (in `ravenrs-widgets-chat`, feature `code`) takes any `Runner`, and `LiveRunner` in `ravenrs-widgets` (feature `live`) is the runner built on this crate. `LiveRunner::new()` already knows every component. The `ravenrs` facade re-exports this crate as `ravenrs::live` behind its `live` feature, for an app that adds its own types to the registry (the component gallery's `sample_runner` in `examples/component-gallery/src/samples/mod.rs` does that).
- **A host that runs code of its own**, such as the test that builds `ravenrs-app` actions from interpreted source (`crates/ravenrs-app/tests/suite/live.rs`), calls `run` with a `Registry` it fills.
- **Crates that want their types reachable** put `#[ravenrs_live::expose]` on them. The attribute lives in `ravenrs-live-derive` and is re-exported here as `ravenrs_live::expose`; its README says what it generates and what it leaves out.

What it calls: only `syn` and `proc-macro2` to read the source, and `ravenrs-reactive` for `State` and `Binding`. It knows nothing about views itself; everything the program can reach in the host goes through the `Registry` the host hands it.

### Why an interpreter and not the compiler

The playground runs in the browser as WebAssembly, where there is no `rustc` and no linker, and a server that compiles each edit would mean a wait of seconds per run and a machine to keep running. Reading the source and calling the components already compiled into the page gives a result right after Ctrl/Cmd+Enter, offline, with the same components the app ships. The price is that only the part of Rust examples use is read, and types are checked when a value is used rather than before the program runs (see "Limits").

## How it works

1. **Parse.** `run` parses the whole source with `syn::parse_file`. A syntax error comes back as `LiveError`s at once, before anything runs.
2. **Declare.** Every top-level item is taken in: functions, `const` and `static`, structs (without generics) and their `impl` blocks. `use` lines are read and left alone, because names are resolved by their last segments: `std::rc::Rc::new` is `Rc::new`, `ravenrs::prelude::Button` is `Button`. Any other item, such as a `trait` or an `enum`, is an error at its place.
3. **Call the entry.** The function named by `entry`, or the first function in the file when `entry` is `None`, is called with no arguments. Its body is evaluated expression by expression; a call such as `Type::name(..)` looks first at the program's own `impl` blocks, then at the `Registry`. The function's value is returned to the host as a `Value`. Evaluation stops at the first error.
4. **Later.** Closures the program gave the host (an `on_press`, a `Text::bound` formatter, a `Canvas` painter) stay alive inside the host's components and run when the host calls them, long after `run` returned. A failure there cannot stop a program that already finished, so it is kept with `report` and read with `late_problems`, and the host gets a fallback value meanwhile (see "When something goes wrong").

The model, at the depth a host needs:

- **Values.** A `Value` is a plain value (`Unit`, `Bool`, `Int`, `Float`, `Char`, `Str`, `Tuple`, `List`, `Option`, `Result`, ranges), a closure of the program, a `State` or `Binding` of values, a `Record` (a value of a struct the program declares) or a `Host`: an object of the host program that the interpreter only carries around and passes back.
- **Host values move, as in Rust.** A `Host` made with `Host::new` gives its contents out once: a builder method that takes `self` takes it, and a second use is the error "this Button was already used: in Rust it moved, so clone it first or build it again". A `Host::cloneable` value (colors, insets, anything `Clone` the host marks so) is copied instead, by `.clone()` and when read through a reference.
- **Conversions.** When a host function asks for one type and gets another, `Host::take_as` looks for a conversion registered with `Registry::conversion`, such as from `Button` to `AnyView`, so a view goes wherever any view goes.
- **One thread.** The registry is shared through `Rc`, values hold `Rc` and `RefCell`, and the current registry, the printed lines and the late problems are thread-locals. Everything runs on the thread that calls `run`, which in a RavenRS app is the UI thread, and nothing here is `Send`.
- **Nothing is cached between runs.** Each `run` parses and declares again. The `Registry` is the only thing kept, and the host may reuse it for every run.

## Use from start to finish

A host registers a type, runs a program, and gets back the value, or the errors with their places:

```rust
use std::rc::Rc;

use ravenrs_live::{FromLive, Host, Registry, Value, run};

/// A type of the host program.
#[derive(Debug, PartialEq)]
struct Label {
    text: String,
    bold: bool,
}

let mut registry = Registry::new();
// `Label::new(text)`: the arguments arrive as values, checked here.
registry.function("Label", "new", |args| {
    let [text] = <[Value; 1]>::try_from(args).map_err(|_| "`Label::new` takes one text")?;
    let text = String::from_live(text)?;
    Ok(Value::Host(Host::new("Label", Label { text, bold: false })))
});
// `label.bold()`: a method gets its receiver first. It takes `self`, so the old label moves.
registry.method("Label", "bold", |args| {
    let Some(Value::Host(this)) = args.first().map(Value::read) else {
        return Err("`bold` is a method of a Label".into());
    };
    let mut label: Label = this.take()?;
    label.bold = true;
    Ok(Value::Host(Host::new("Label", label)))
});
let registry = Rc::new(registry);

let source = r#"
fn greeting() -> Label {
    let name = "world";
    println!("building for {name}");
    Label::new(format!("hello, {name}")).bold()
}
"#;
let value = run(source, None, registry.clone()).map_err(|errors| errors[0].to_string())?;
let Value::Host(host) = value else { return Err("expected a Label".into()) };
let label: Label = host.take()?;
assert_eq!(label, Label { text: "hello, world".into(), bold: true });
// What the program printed waits for whoever shows it.
assert_eq!(ravenrs_live::printed(), vec!["building for world".to_owned()]);

// What the host refuses comes back with its place: line 2, where the call is.
let errors = run("fn broken() -> Label {\n    Label::new(1, 2)\n}", None, registry.clone()).unwrap_err();
assert_eq!(errors[0].line, 2);
assert_eq!(errors[0].message, "`Label::new` takes one text");

// A type nobody registered is named in the error, never guessed.
let errors = run("fn f() -> Label { Gizmo::new() }", None, registry).unwrap_err();
assert!(errors[0].message.contains("Gizmo"), "{}", errors[0]);
# Ok::<(), String>(())
```

In a RavenRS app the same flow is a single line, because `LiveRunner` owns the registry, turns the result into a view and the errors into the editor's diagnostics:

```rust,ignore
use ravenrs_widgets::{LiveRunner, Playground};

let playground = Playground::new(LiveRunner::new())
    .example("Hello", "fn hello() -> impl View { Text::new(\"hello\") }");
```

### The parts, and when to reach for each

- **`run(source, entry, registry)`** runs one program. Pass `Some("name")` to pick the function when one source has several entry points (the `ravenrs-app` test runs `task` and `count` from one source); `None` runs the first function, which is what a playground wants.
- **`Registry`** is everything the program can reach. Fill it with the `__live_expose` functions `#[expose]` generates (the normal way; see `ravenrs-live-derive`), and add by hand what the attribute cannot describe:
  - `function(ty, name, f)` for `Type::name(..)`, and `method(ty, name, f)` for `value.name(..)` on a host value whose type name is `ty`; `method_for::<T>` when several Rust types share one name, as the forms of a generic component do. A method's receiver is the first value it gets.
  - `constant(ty, name, make)` for `Type::NAME` and enum variants, made fresh each time it is read.
  - `free_function(name, f)` for a helper called by its name alone, such as the gallery's `picture(0.4)`.
  - `field`, `set_field` and `record` for `value.field`, `value.field = ..` and `Type { a: .., ..base }` on host structs.
  - `conversion::<From, To>(name, f)` so a value of one type goes where another is asked for; `name_type::<T>` names a type for those conversions; `alias("ChartStack", "Stack")` for a type re-exported under another name.
  - `LiveRunner`'s registry, in `crates/ravenrs-widgets/src/live.rs`, is the worked example of the hand-written parts: `Text::bound` (generic over the state's type), the methods every view has under the name `View`, `view_fn`, `Canvas::new` and `cx.use_state`.
- **`Host`** wraps a host value: `Host::new` for one that moves, `Host::cloneable` for one that copies. `take` gets it out as its own type, and `take_as` also through a registered conversion.
- **`FromLive` and `IntoLive`** turn values into Rust types and back: the integer and float types, `String`, `bool`, `char`, `Option`, `Result`, `Vec`, `BTreeSet`, tuples of two to four, `State<Value>`, `Binding<T>`, and `Box<dyn Fn(..) -> R>` or `Rc<dyn Fn(..) -> R>` of up to three arguments, which wrap a closure of the program so the host can call it later.
- **`Closure`** is a closure of the program: `call` runs it with values, and `error` makes a `LiveError` at its place in the source.
- **`Fallback`** is what a closure gives the host when it fails: the default for numbers, text and `()`, `None`, an `Err`, an empty `Vec`. Implement it for a host type a closure must return.
- **`Record`** is a value of a struct the program declares itself; `field` reads it and `Record::call_method` calls a method the program wrote for it, which is how a host component calls back into a program's own type.
- **`printed`, `late_problems`, `on_output` and `report`** carry what happens around and after `run`: lines from `print!` and `println!` (and the `e` forms), failures in closures called later, and a watcher called whenever either changes, which the `Playground` uses to fill its Console and Problems tabs as they come. All of them are per thread, shared by every run on that thread.

### What the program can be written with

Items: functions (also nested inside a function, where they are seen only there), `const`, `static`, structs without generics, and `impl` blocks for them. `use` lines are skipped.

Statements and expressions: `let` with patterns and `let .. else`; closures, `move` or not, and calls of them; chains of builder methods; `if`, `if let` and `match` with guards and `|` patterns; `for` over lists, ranges, options and the characters of a text; `while`; `break`, `continue` and `return`; `?` on `Option` and `Result`; field access and assignment, `+=` and the other compound operators; indexing; `as` casts; arrays and `[x; n]`; tuples; ranges `a..b` and `a..=b`; `&` and `*` (a borrow is read as the value itself); struct literals with `..base`.

Macros: `vec!`, `format!`, `print!`, `println!`, `eprint!`, `eprintln!` (with `{name}`, `{}`, `{:?}`, width, alignment and precision), `panic!`, `assert!`, `debug_assert!`, `matches!`, and `todo!`, `unreachable!` and `unimplemented!`, which stop with an error saying they were reached.

From the standard library: `Option` and `Result` with their usual combinators; the iterator methods `map`, `for_each`, `filter`, `filter_map`, `flat_map`, `flatten`, `enumerate`, `rev`, `take`, `skip`, `step_by`, `chain`, `zip`, `collect`, `count`, `first`, `next`, `last`, `nth`, `contains`, `sum` and `product`; methods of numbers, text and characters; and `State` with `get`, `set`, `update` and `Binding::map`. The full list is in `src/eval.rs`; anything missing is an error naming the method, never a silent no-op.

## What comes for free

- **Errors with places.** Every refusal is a `LiveError` with a line, a column and a length in characters, so an editor underlines the exact call. `LiveRunner` maps them to `CodeDiagnostic`s, and the `Playground` marks them on their lines.
- **Messages a Rust reader recognizes.** A host value used twice says it moved; a wrong argument says what was expected and what was found ("expected a bool, found integer"); an unknown type says the playground does not know it yet.
- **Closures that fail do not take the app down.** A closure the host calls later that fails is reported, and the host gets the type's fallback, so clicking a broken button shows a problem instead of a panic.
- **Output as it comes.** With `on_output`, a watcher hears about every printed line and late problem, so the `Playground` shows them without polling.

## Per platform

The crate is plain Rust with no system calls, no features of its own and no platform code. It behaves the same on macOS, Windows, Linux and WebAssembly in the browser, which is the case it exists for. Only what the host registers differs between platforms.

## Limits

**What does not run.** Traits and trait `impl`s written in the program, enums declared by the program, generics on the program's structs, `loop` (use `while`), `async`, ranges without an end such as `0..` used as a value, and any host type, function or method that is not in the registry. Types written in the program are not checked: they are read as documentation, and a wrong type shows up when the value is used. All whole numbers are `i64` and all fractions `f64`; `+`, `-` and `*` on integers wrap around instead of panicking, while dividing by zero is an error.

**Performance.** It is a tree-walking interpreter: each expression is matched on its syntax node every time it runs, variables live in a hash map per scope, iterator chains build a whole `Vec` at each step instead of streaming, and `for` collects its range before looping. That is enough for what examples do, building a few hundred components and running closures on events, but it is far slower than compiled code on loops over large data. Keep heavy work in the host and expose it as a function.

**Isolation and safety.** A program reaches only what the registry offers: there is no file system, network, process or environment access unless the host registers a function that does it, so what a program can do is decided by the host. It is still not a sandbox for untrusted code:

- It runs on the calling thread with no time budget. Only `while` is capped, at 100 000 rounds. A `for` over a huge range keeps the thread busy, and a function that calls itself without end overflows the thread's stack, as compiled Rust would.
- It has no memory budget: `vec![0; n]` with a large `n` allocates all of it.
- A host function that panics panics the host; the interpreter does not catch panics. Host functions should return `Err` instead.

Treat the source like code the user types into their own app, which is what a playground is for.

## When something goes wrong

Every error is a `LiveError { line, column, length, message }`, with lines and columns counted from 1; its `Display` is `line:column: message`. The common messages:

- **"this is not Rust the playground reads: .."** comes from the parser: the source is not valid Rust. Fix the syntax at the place given.
- **"the playground does not know `Gizmo` yet"**: no registered function, constant or struct literal uses that type name. Register the type, or check that its crate's `__live_expose` functions are called when the registry is built (for the RavenRS crates, `scripts/gen-live.py` writes those lists, and `scripts/check-component-list.py` fails while they are out of date).
- **"`Type::name` is not one the playground knows yet"** or **"`Button` has no method `x` the playground knows"**: the type is known but not that function or method. `#[expose]` may have left it out because of its signature (see `ravenrs-live-derive`); register it by hand.
- **"this Button was already used: in Rust it moved .."**: a host value that moves was used twice. Clone it or build it again, as compiled Rust would require.
- **"expected a bool, found integer"** and the like: an argument does not fit what the host function takes.
- **"this expression is not read by the playground yet"**, **"this pattern is not read here"**, **"the playground does not read the macro `x!`"**: the syntax is outside what is read (see "Limits").
- **"there is no function to run"**: the source has no `fn`. **"there is no function `name`"**: the `entry` given does not exist.
- **"this loop ran 100 000 times: the playground stops it"**: a `while` that does not end.
- **Problems that appear later**, after `run` returned, are failures in closures the host called; read them with `late_problems`.

To see what is happening, print from the program with `println!` and read `printed`, or call `printed` and `late_problems` from a test. A bug report is most useful with the source that failed, the full `LiveError` (line, column and message), and whether the same code compiles as an ordinary example. The tests in `crates/ravenrs-live/tests/eval.rs` and `crates/ravenrs-widgets/tests/suite/live.rs` show the form a regression test takes.

## License

MIT OR Apache-2.0.
