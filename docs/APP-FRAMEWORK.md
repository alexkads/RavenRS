# Application framework

Declare what a record is, and get the screens: navigation, a list, a form, commands and rules. Everything else changes through metadata, actions and controllers. The behavior follows what is documented for model-driven application frameworks; the code and the words are our own.

> **Status:** in progress, usable from code, with the API still moving. Nothing here is in the `ravenrs` facade yet.

## What exists

| Crate | What it has |
|---|---|
| `ravenrs-app` | No GUI. `#[derive(BusinessObject)]` and `#[derive(BusinessEnum)]`, from `ravenrs-app-derive`; `EntityInfo` and `Object`; `generate_model`, `ModelLayers`, `Diff`, `ModelStore`, `check_model`; `ObjectSpace` and `MemoryDatabase`; `Action`, `TargetScope`, `ViewController`, `WindowController`, `Frame`; `Module`, `Application` |
| `ravenrs-app-ui` | `App`, `AppShell`, `ListView`, `DetailView`, `EditorRegistry`, `ActionContainer`, `FrameTemplate`, `SystemModule`, `Words` |
| `ravenrs-grid-export` | What a `DataGrid` shows, as Excel, PDF or CSV |

## In five minutes

Add `ravenrs`, `ravenrs-app` and `ravenrs-app-ui` to a project, and declare your records:

```rust,ignore
use ravenrs_app::{BusinessEnum, BusinessObject};
use ravenrs_app_ui::{App, Language};

#[derive(BusinessEnum)]
enum Kind {
    #[option(caption = "Corrente")]
    Checking,
    #[option(caption = "Poupança")]
    Savings,
}

#[derive(BusinessObject)]
#[object(caption = "Contas", navigation = "Cadastros", icon = "wallet")]
struct Account {
    #[field(key)]
    id: i64,
    #[field(caption = "Nome", default_property)]
    name: String,
    #[field(caption = "Tipo")]
    kind: Kind,
    #[field(caption = "Saldo inicial", money)]
    opening: i64,
}

fn main() -> Result<(), ravenrs::native::PlatformError> {
    let window = App::new("Raven Contas")
        .language(Language::BrazilianPortuguese)
        .entity::<Account>()
        .into_view();
    ravenrs::native::run(window, ravenrs::native::WindowOptions::new("Raven Contas"))
}
```

That is the whole application: navigation, a list with search, filters and export to Excel, PDF and CSV, a form, and New, Save, Save and close, Save and new, Cancel, Delete, Refresh, Back, Previous and Next. [`examples/quick-app`](../examples/quick-app) is this with a second record and a reference between them, in under 100 lines, in a window with `cargo run -p quick-app` and in the browser with `scripts/build-web.sh quick-app`.

Records live in memory for now, so they are gone when the application closes. `seed` puts some in when it opens. A wrong attribute is a compile error that points at it.

### What a field can say

| On `#[field(...)]` | Means |
|---|---|
| `key` / `typed_key` | Identifies the record: numbered by the application, or typed by people. Exactly one field has one |
| `caption = "..."` | What people see |
| `default_property` | What names a record, in lists to pick from and references |
| `read_only`, `no_list`, `no_detail`, `lookup` | Where it shows, and whether it changes |
| `money` | A whole number of cents |
| `long`, `color`, `date`, `decimals = 2` | What it holds, when the Rust type does not say |
| `reference = "Entity"` | Holds the key of another record |
| `calculated = "[a] + [b]"` | Follows from the others, checked at compile time |
| `default = ...` | What a new record starts with |

`String`, `bool`, `i32`, `i64`, `u32`, `u64`, `usize`, `f64` and `Option` of them work as they are. Any other type implements `FieldValue`, which `#[derive(BusinessEnum)]` does for an enum.

### Going further

`App` covers the usual case. Under it, `EntityInfo` declares a record at run time, `Application::builder` and `AppShell` take everything apart, `module` adds actions and controllers, `editors` changes a field's control, and `storage` keeps records elsewhere. `examples/component-gallery/src/samples.rs` has `AppShell` used directly.

## How it fits together

- **The model** is a tree. `generate_model` makes its lowest layer from the entities. Modules, an administrator and a user add layers on top, each a difference in JSON; the highest to set a value wins, and clearing a value brings back the layer below. A difference that finds no node is kept, and works again if the node comes back.
- **Object spaces** are units of work. Each view has its own, so canceling a form throws away only that form's changes.
- **Actions** are on or off by reasons, and every reason has to say yes. A view that does not fit takes an action away; a selection or criteria that does not fit fades it.
- **A frame** holds a view. Changing it tells whoever watches, turns the controllers off, changes the view, turns on those that fit, and tells again.
- **`Application::build`** says what is wrong with the declaration before any screen opens.

## Rules

- Screens use only `ravenrs-widgets`. What is missing goes there first, with tests and a gallery entry, and never into these crates.
- `ravenrs-widgets` knows nothing of `ravenrs-app`, and `ravenrs-criteria` nothing of either.
- Money is a whole number of cents, kept in a number, and never a fraction.
- Tests drive whole applications through `Ui::access_tree`, with no window: see `crates/ravenrs-app-ui/tests/e2e.rs`.

## Known gaps

- Search, filters and export of a list are `DataGrid`'s own, not actions the model can move.
- Where an action shows comes from its category. There is no mapping of actions to places in the model yet.
- A popup shows only lists. Pictures, files and collections on a form have no real control yet.
- A reference is picked with `LookupField`, and not in a window of its own.
- Used directly, `AppShell` needs `Words` given to it and to `SystemModule`, and `Localized` around it. `App` does both.
- With two people changing the same record in `MemoryDatabase`, the last to save wins.

## What comes next

| Step | What it brings |
|---|---|
| **F5** | Validation rules and conditional appearance |
| F6 | `SqliteObjectSpace` and the records screens of `examples/family-finance` generated, with its tests unchanged |
| F7 | Relations, nested lists, dashboards, view variants, popups with forms, reports. Migrates more of `examples/family-finance` |
| F8 | Permissions in an object space wrapper, users, a model editor, an audit trail |

Before more work on editors or templates, read what is documented on template customization and on each property type: it was not read when these screens were written. The same goes for the built-in node generators and list view column generation, before more of the model is generated.
