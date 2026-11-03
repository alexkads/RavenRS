# Application framework

Declare what a record is, and get the screens: navigation, a list, a form, commands and rules. Everything else changes through metadata, actions and controllers. The behavior follows what is documented for model-driven application frameworks; the code and the words are our own.

> **Status:** in progress, usable from code, with the API still moving. Nothing here is in the `ravenrs` facade yet.

## What exists

| Crate | What it has |
|---|---|
| `ravenrs-app` | No GUI. `#[derive(BusinessObject)]` and `#[derive(BusinessEnum)]`, from `ravenrs-app-derive`; `EntityInfo` and `Object`; `generate_model`, `ModelLayers`, `Diff`, `ModelStore`, `check_model`; `ObjectSpace` and `MemoryDatabase`; `Action`, `TargetScope`, `ViewController`, `WindowController`, `Frame`; `Module`, `Application` |
| `ravenrs-app-ui` | `App`, `AppShell`, `EntityPane`, `ListView`, `DetailView`, `EditorRegistry`, `ActionContainer`, `FrameTemplate`, `SystemModule`, `Words` |
| `ravenrs-app-sqlite` | `SqliteDatabase`, `TableMap`: on tables it creates or tables that exist, on a connection the app shares |
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
| `required`, `unique`, `min = 1`, `max = 10`, `matches = "..."` | What it has to meet on saving, and `message = "..."` what to say then |

`String`, `bool`, `i32`, `i64`, `u32`, `u64`, `usize`, `f64` and `Option` of them work as they are. Any other type implements `FieldValue`, which `#[derive(BusinessEnum)]` does for an enum.

### Rules

On the struct, as many as needed:

```rust,ignore
#[rule(criteria = "[amount] <= [limit]", message = "Above the limit")]
#[rule(unique = "bank,number")]
#[rule(criteria = "[paid] = False", context = "Delete", message = "A paid bill stays")]
#[appearance(names = "amount", criteria = "[amount] > 1000", font = "danger", bold)]
#[appearance(names = "notes", criteria = "[paid] = True", visible = false)]
#[appearance(names = "Pay", action, criteria = "[paid] = True", enabled = false)]
#[appearance(names = "Long", layout, criteria = "[paid] = True", visible = false)]
struct Bill { /* ... */ }
```

A form that fails its rules keeps nothing, and says what is wrong on each field and in a list on top, which follows as people fix it. Rules are in the `Save` context unless they say otherwise. `Delete` applies on deleting, and an action names any other with `validation_contexts`. Rules do not run by themselves below the screens: `validate` is there for anything else that writes.

Appearance is about looks, and does not protect data. A column of a list hides only by a rule with no condition, since it is for all its rows. An action hidden by a rule on a list only fades, as hiding would move the buttons beside it.

### Inside an application that already exists

`EntityPane` is a list beside its form, as a piece of a screen an application made itself, and `SqliteDatabase` maps onto its tables without touching their schema, on the connection it already has. `examples/family-finance/src/ui/records.rs` does this for its accounts, categories and family members: the tables and their migrations stay where they were.

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
- On a form, a font color from a rule reaches the caption and not the value inside the editor, and a group's caption takes no color.
- Clippy's `duplicated_attributes` flags `#[appearance]` or `#[rule]` attributes that repeat a condition or a context. Allow it where they do, as a derive cannot do it for you.
- Fields are validated on saving, and not yet as they are left.
- `ListView` reads every row. Lists of many rows, through `RemoteRows`, come with the screens that need them.
- A pane finds a row's key by the order of the entity's fields, so a layer that reorders its columns breaks its buttons.
- With two people changing the same record in `MemoryDatabase`, the last to save wins.

## What comes next

| Step | What it brings |
|---|---|
| **F7** | Relations, nested lists, dashboards, view variants, popups with forms, reports. Migrates more of `examples/family-finance` |
| F8 | Permissions in an object space wrapper, users, a model editor, an audit trail |

Before more work on editors or templates, read what is documented on template customization and on each property type: it was not read when these screens were written. The same goes for the built-in node generators and list view column generation, before more of the model is generated.
