# Application framework

Declare what a record is, and get the screens: navigation, a list, a form, commands and rules. Everything else changes through metadata, actions and controllers. The behavior follows what is documented for model-driven application frameworks; the code and the words are our own.

> **Status:** in progress, usable from code, with the API still moving. Nothing here is in the `ravenrs` facade yet.

## What exists

| Crate | What it has |
|---|---|
| `ravenrs-app` | No GUI. `EntityInfo` and `Object`; `generate_model`, `ModelLayers`, `Diff`, `ModelStore`, `check_model`; `ObjectSpace` and `MemoryDatabase`; `Action`, `TargetScope`, `ViewController`, `WindowController`, `Frame`; `Module`, `Application` |
| `ravenrs-app-ui` | `AppShell`, `ListView`, `DetailView`, `EditorRegistry`, `ActionContainer`, `FrameTemplate`, `SystemModule`, `Words` |
| `ravenrs-grid-export` | What a `DataGrid` shows, as Excel, PDF or CSV |

## In five minutes

```rust,ignore
use ravenrs_app::{Application, EntityInfo, FieldInfo, FieldKind, MemoryDatabase};
use ravenrs_app_ui::{AppShell, SystemModule};

let account = EntityInfo::new("Account")
    .caption("Accounts")
    .navigation("Records")
    .field(FieldInfo::new("id", FieldKind::Integer).key())
    .field(FieldInfo::new("name", FieldKind::Text).default_property())
    .field(FieldInfo::new("opening", FieldKind::Money).default_value(0.0));

let database = MemoryDatabase::new(vec![account.clone()])?;
let application = Application::builder("Accounts").module(SystemModule::new()).entity(account).build()?;
let window = AppShell::new(application, move || database.space());
// `window` is a View: give it to ravenrs::native::run or ravenrs::web::start.
```

That is a list with search, filters and export, a form, and New, Save, Save and close, Save and new, Cancel, Delete, Refresh, Back, Previous and Next. `examples/component-gallery/src/samples.rs` has a fuller one, under `AppShell`.

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
- `Words` has to be given to both `SystemModule` and `AppShell`, and the app has to wrap the shell in `Localized` for the grid's own words.
- With two people changing the same record in `MemoryDatabase`, the last to save wins.

## What comes next

| Step | What it brings |
|---|---|
| **F4** | `#[derive(BusinessObject)]` in `ravenrs-app-derive`, `examples/quick-app` in under 100 lines, this guide as a tutorial, the site |
| F5 | Validation rules and conditional appearance |
| F6 | `SqliteObjectSpace` and the records screens of `examples/family-finance` generated, with its tests unchanged |
| F7 | Relations, nested lists, dashboards, view variants, popups with forms, reports. Migrates more of `examples/family-finance` |
| F8 | Permissions in an object space wrapper, users, a model editor, an audit trail |

Before F3-level work on editors or templates, read what is documented on template customization and on each property type: it was not read when these screens were written. The same goes for the built-in node generators and list view column generation, before more of the model is generated.
