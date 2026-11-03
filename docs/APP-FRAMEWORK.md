# Application framework

Declare what a record is, and get the screens: navigation, a list, a form, commands and rules. Everything else changes through metadata, actions and controllers. The behavior follows what is documented for model-driven application frameworks; the code and the words are our own.

> **Status:** in progress, usable from code, with the API still moving. Nothing here is in the `ravenrs` facade yet.

## What exists

| Crate | What it has |
|---|---|
| `ravenrs-app` | No GUI. `#[derive(BusinessObject)]` and `#[derive(BusinessEnum)]`, from `ravenrs-app-derive`; `EntityInfo` and `Object`; `generate_model`, `ModelLayers`, `Diff`, `ModelStore`, `check_model`; `ObjectSpace` and `MemoryDatabase`; `Action`, `TargetScope`, `ViewController`, `WindowController`, `Frame`; `Module`, `Application` |
| `ravenrs-app-ui` | `App`, `AppShell`, `EntityPane`, `ListView`, `DetailView`, `EditorRegistry`, `ActionContainer`, `FrameTemplate`, `SystemModule`, `Words` |
| `ravenrs-app-sqlite` | `SqliteDatabase`, `TableMap`: on tables it creates or tables that exist, on a connection the app shares |
| `ravenrs-app-report` | `ReportModule`: an action that lays out a list as a report and shows it in the viewer |
| `ravenrs-app-library` | `LibraryModule`: people, organizations, addresses, tasks, events, notes and files, to use or extend, with `PT_BR` for their words |
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

`EntityForm` is a form in a window for a record the application keeps in its own way: it checks the rules and hands over the record, and the application's own code writes it. `ListView` takes a `filter`, a `sort`, a `column_shape` and a `grid_shape`, and `ActionButton` shows one action as a button, so a screen made by hand can keep its look with lists and commands from the framework: `Session::embedded` is what they share. `examples/family-finance/src/ui/ledger.rs`, `bills.rs` and `forms.rs` do all of that.

### Relations

```rust
EntityInfo::new("Customer")
    .field(FieldInfo::new("id", FieldKind::Integer).key())
    .field(FieldInfo::new("name", FieldKind::Text).default_property())
    // On the form: the orders that point here, with Link and Unlink, kept right away.
    .field(FieldInfo::new("orders", FieldKind::Collection("Order".into())));

EntityInfo::new("Order")
    .field(FieldInfo::new("id", FieldKind::Integer).key())
    .field(FieldInfo::new("customer", FieldKind::Reference("Customer".into())))
    // `Select` lists every choice; without it, a reference is looked up as people type.
    .field(FieldInfo::new("tag", FieldKind::Reference("Tag".into()))
        .editor("Select")
        .lookup_filter("[side] = ?side")
        .none_caption("None"))
    // With two or more, a menu picks among them, and the choice stays in the person's layer.
    .variant(ViewVariant::new("all", "All"))
    .variant(ViewVariant::new("open", "Open").filter("[status] = 'open'"));
```

`Action::popup` on a list view asks for a record to be picked, and on a detail view for a form to be filled in, which `on_prepare` starts and the action receives as `ActionInput::Record`. An entity no storage knows works for that. `Application::builder(...).dashboard(DashboardInfo::new("Home", "Home").view("Orders", "Order_ListView").control("Chart", "chart"))` puts views and controls together, and `Session::register_control` says what a control is.

### Beyond the grid, and beyond the code

- `register_list_editor` adds a way to show a list, picked by `EditorType` on a view in the model, or by `DefaultListEditor` on `Views` for all of them.
- `ListFilter` and `list_criteria` put filters in the model. `ViewVariant::columns` changes what a way to see a list shows. `register_function` adds a function to every expression; a filter that uses one runs in memory.
- `Rules` under an entity in the model add rules with no code. `translation` gives the model's words in another language, and captions follow to the views unless one has its own.
- `checks_on_leave` makes a form speak about a field as it is left. `transient` marks a record no storage keeps, and `ListView::source` gives a list its records.
- `cloneable` with `CloneModule` offers cloning. What a record holds comes along, a reference points to the same record, `non_cloneable` stays behind, and only a collection marked `aggregated` is cloned with it.
- `use_attachment_store` keeps files apart from their records, with `MemoryAttachmentStore` or `SqliteAttachmentStore`.
- `SqliteBuilder::migration` runs something once in a database's life, `managed` adds columns for new fields, and `entities_from_schema` starts from a database that came first. `SqliteModelStore` keeps a person's settings in it, through `use_model_store`.
- `ApplicationBuilder::service` hands actions what the application has, by type, through `ActionContext::service`. `Module::initial_records` puts in place what a module needs.

### When things meet in real use

- `Rule::warning` asks and lets people go on, and `Rule::information` saves and leaves a note on the field. `Rule::message` can say `{field}`, `{reference.field}`, `{$field}`, `{$entity}`, `{$min}` and `{$max}`.
- Saving a form puts what it changed together with what someone else kept since it opened: `merge_changes` keeps both when the fields differ, and the form asks when they are the same. A record that is gone says so.
- `list_editable` lets people change plain values right on a list, refusing what breaks a rule on that field or on the whole record. A cell already wrong shows it, leaving out rules that read other records, so a long list stays quick.
- `data_access` picks how a list reads: `Client`, all of it; `Server`, as needed; `DataView`, only its columns and with no editing, finding the whole record when one is selected.
- A difference can add fields under `Members`, each with `"$new": true`, `Kind` and `Caption`, `Expression` for one worked out, and `Options` for a list. They come in order of name, and get a column and a place on the form. `App::administrator_settings` gives them before storage opens, and `App::storage_for` hands a database the entities with them.
- `FieldKind::Guid` makes up a key. `FieldKind::Tags` holds several options. `predefined`, `edit_mask`, `criteria_of`, `display_format` and `group` shape a field, and the `Progress` editor shows a number from 0 to 100.
- `ActionContext::message` tells people something, with `Message::action` and `Message::on_close`. `ActionCategory::InForm` with `EntityInfo::form_actions` puts an action inside a form's layout.
- `use_records` and `use_collection`, or the same on `App`, give records no storage keeps. A navigation item can open a form directly: on a new record, or with `ObjectKey` on one by its key.
- `DashboardInfo::variant` lets people change a dashboard to another; it lists itself too, since a menu takes two.
- `export_captions` writes what people see as CSV, with an empty column to fill in, and `import_captions` takes it back as a language.

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
- Without an attachment store, a picture or file stays as text in its field, and the whole record is read with it. With one, replacing a file does not yet remove the old content from the store.
- What links and unlinks in a list inside a form is kept right away, and not with the form's save. A new record has to be saved before its list opens.
- A remote list searches only fields with text, and has no grouping. A column of a reference shows the name, but is not searched by it.
- A reference is picked in its form, and not in a window of its own.
- Used directly, `AppShell` needs `Words` given to it and to `SystemModule`, and `Localized` around it. `App` does both.
- On a form, a font color from a rule reaches the caption and not the value inside the editor, and a group's caption takes no color.
- Clippy's `duplicated_attributes` flags `#[appearance]` or `#[rule]` attributes that repeat a condition or a context. Allow it where they do, as a derive cannot do it for you.
- Only text and number fields tell when they are left. Validation contexts do not reach into collections yet.
- `register_function` is per thread, as all of an application's state is. A function of the application is not translated to SQL, so a filter with one reads the whole table.
- A translation changes the model's words; `Words` follows `App::language` and does not change at run time.
- A pane finds a row's key by the order of the entity's fields, so a layer that reorders its columns breaks its buttons.
- Putting two people's changes together reads and then writes, in two steps. Two programs on one SQLite file can still meet between them, until there is a server.
- Editing on a list needs a list that holds its records, so not `Server`, `DataView` or a given source. A cell that fails is marked, and its message shows only when it is edited.
- A `display_format` on a date makes its cells text, so they sort as text. An `edit_mask` has no regular expressions: `Rule::matches` checks those on saving.
- Predefined values are picked and not typed. A Rust struct does not expand into a group of fields: `group` puts plain fields together.
- Fields added in the model hold plain values, not references or lists. Used without `App`, whoever opens storage has to give it `Application::entities`.

## What comes next

| Step | What it brings |
|---|---|
| **F8** | Permissions in an object space wrapper, users, a model editor, an audit trail |

Before more work on editors or templates, read what is documented on template customization and on each property type: it was not read when these screens were written. The same goes for the built-in node generators and list view column generation, before more of the model is generated.
