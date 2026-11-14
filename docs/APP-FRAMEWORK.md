# Application framework

Declare what a record is, and get the screens: navigation, a list, a form, commands and rules. Everything else changes through metadata, actions and controllers. The behavior follows what is documented for model-driven application frameworks; the code and the words are our own.

> **Status:** in progress, usable from code, with the API still moving. Nothing here is in the `ravenrs` facade yet.

## What exists

| Crate | What it has |
|---|---|
| `ravenrs-app` | No GUI. `#[derive(BusinessObject)]` and `#[derive(BusinessEnum)]`, from `ravenrs-app-derive`; `EntityInfo` and `Object`; `generate_model`, `ModelLayers`, `Diff`, `ModelStore`, `check_model`; `ObjectSpace` and `MemoryDatabase`; `Action`, `TargetScope`, `ViewController`, `WindowController`, `Frame`; `Module`, `Application` |
| `ravenrs-app-a11y` | `AccessibleText`, `Announcer` and `LiveRegion`, `AccessWords`, `Audit`: names from the model, announcements, and an audit that walks an application |
| `ravenrs-app-ui` | `App`, `AppShell`, `EntityPane`, `ListView`, `DetailView`, `EditorRegistry`, `ActionContainer`, `FrameTemplate`, `SystemModule`, `Words` |
| `ravenrs-app-i18n` | `Language`, `Catalog` and `Texts`: every word of the screens and of the components by key, in English and Brazilian Portuguese, with the translations of the model that come with the framework |
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
| `accessible_name = "..."`, `accessible_description = "..."` | What screen readers call it instead of the caption, and say after it instead of the hint |

On the struct, `#[object(focus_order = "name,amount")]` puts these fields first on the form, in this order, on the screen and for Tab.

`String`, `bool`, `i32`, `i64`, `u32`, `u64`, `usize`, `f64` and `Option` of them work as they are. Any other type implements `FieldValue`, which `#[derive(BusinessEnum)]` does for an enum.

### Reading the same structs while the program runs

With the `source` feature, `entities_from_source` reads Rust text with these structs and enums and gives their `EntityInfo`s without compiling, as the playground does. The derive and this function share `ravenrs-app-spec`, so they accept the same attributes and refuse the same mistakes with the same words, each with its line and column (`SourceError`). Only what a compiler would have to work out is refused on top: a type other than the ones above or an enum of the same file, and a `default`, `min` or `max` that is not a literal or an enum option.

```rust,ignore
let entities = ravenrs_app::entities_from_source(source)?; // Err holds every SourceError
```

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

### Actions declared as data

An action that only changes fields of the records selected needs no code:

```rust
#[derive(BusinessObject)]
#[action(caption = "Concluir", selection = "many", enabled = "[state] <> 'done'",
         set = "state = 'done'; finished = Today()", confirm = "Concluir as tarefas?")]
struct Task { /* ... */ }
```

`set` is `field = expression; ...`, in the criteria language, and every expression reads the record as it was before any of them. Each record changed is checked against its rules in the `Save` context, and then all are kept, or none: one record that breaks a rule keeps every record as it was, and says what is wrong. `enabled` is what every record selected has to meet for the action to be on, as `target_criteria` with `TrueForAll`. `selection` is `one`, `many`, the default, or `any`. The id is `Entity.Caption` unless `id = "..."` gives one. `confirm` asks before it runs, and `confirm_detail` says what the question says besides; without it the question says a neutral line of the application's words ("Confira antes de continuar.") and runs from the main button. Only deleting says it cannot be undone and asks with the red button of danger, as every action that `requires(Operation::Delete)` does. The derive checks that both expressions parse and name only fields of the struct; `EntityInfo::action(RecordAction::new(...))` does the same at run time, and what is wrong comes out when the application is put together.

`Action::assign(&entity, "field = expression; ...")` gives any action the same work, for one made in code with `on_execute` left out. State machines still guard what is kept, so an action that sets a state goes only by the ways declared.

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

`App` covers the usual case. Under it, `EntityInfo` declares a record at run time, `Application::builder` and `AppShell` take everything apart, `module` adds actions and controllers, `editors` changes a field's control, and `storage` keeps records elsewhere. `examples/component-gallery/src/samples/navigation.rs` has `AppShell` used directly.

## How it fits together

- **The model** is a tree. `generate_model` makes its lowest layer from the entities. Modules, an administrator and a user add layers on top, each a difference in JSON; the highest to set a value wins, and clearing a value brings back the layer below. A difference that finds no node is kept, and works again if the node comes back.
- **Object spaces** are units of work. Each view has its own, so canceling a form throws away only that form's changes.
- **Actions** are on or off by reasons, and every reason has to say yes. A view that does not fit takes an action away; a selection or criteria that does not fit fades it.
- **A frame** holds a view. Changing it tells whoever watches, turns the controllers off, changes the view, turns on those that fit, and tells again.
- **`Application::build`** says what is wrong with the declaration before any screen opens.

## Rules

- Screens use only `ravenrs-widgets`. What is missing goes there first, with tests and a gallery entry, and never into these crates.
- `ravenrs-widgets` knows nothing of `ravenrs-app`, and `ravenrs-criteria` nothing of either.
- `ravenrs-app-i18n` depends on nothing of RavenRS, so the components read their words from it too. A new word is a key in each language's JSON, never a text written in code.
- Money is a whole number of cents, kept in a number, and never a fraction.
- Tests drive whole applications through `Ui::access_tree`, with no window: see `crates/ravenrs-app-ui/tests/suite/e2e.rs`.

## Trying everything

[`examples/feature-center`](../examples/feature-center) has every feature, one small demonstration each, with what to try on each list. Sign in as `ana`, `bia` or `caio`, with the password `raven-demo`, and compare. Its `tests/tour.rs` goes through it all, in memory and in SQLite, so a new feature gets a demonstration there and a place in that tour.

## Showing a list other ways

A view picks how it shows its records by name in the model, with nothing in code:

```json
{ "Views": { "Task_ListView": {
    "EditorType": "Chart",
    "ChartSettings": { "$new": true, "Argument": "owner", "Series": "cost:Sum, hours:Average", "Kind": "Line" } } } }
```

Or people pick it from a menu, with `ViewVariant::new("chart", "Chart").editor(CHART_EDITOR)`. In code, `EntityInfo::editor_setting` writes the same settings. Every setting is optional, as what is not said follows from the kinds of the fields, and a setting that names no field says so on the screen.

| Editor | Settings |
|---|---|
| `Chart` | `Argument`, `Series` as `field:Sum`, `Count`, `Average`, `Min` or `Max`, `Kind` as `Bar`, `Line`, `Area`, `Pie` or `Doughnut`, `Interval` as `Day`, `Month` or `Year` for a date |
| `Calendar` | `Start`, `Title`, `Done`, `Tone`, an option field that picks the color |
| `Scheduler` | `Start`, `End`, `Resource`, a reference with a row per record it can point to, `Title`, `Done`, `Tone` |
| `Tree` | `Parent`, a reference to the same entity, `Title` |

These read through the view's unit of work, so permissions, filters and the way picked all count. See `examples/quick-plan`, in a window and in the browser.

## Records that stay

`App::keep_in(storage)` keeps the records and the person's own settings between runs, with no database to set up. The records stay in memory while the app runs and are written after every commit; the `seed` runs only the first time, while nothing was kept yet.

```rust,ignore
// Desktop: a folder with one file for the records and one for the settings.
App::new("Raven Contas").entity::<Account>().keep_in(FileStorage::new(data_folder));

// Web: the browser's IndexedDB, read before the app opens (see examples/quick-app).
let storage = ravenrs::web::IndexedDbStorage::open("raven-contas").await?;
App::new("Raven Contas").entity::<Account>().keep_in(storage);
```

A `Storage` (in `ravenrs-core`) keeps named pieces of data: `FileStorage` on the desktop, `IndexedDbStorage` on the web, `MemoryStorage` for tests. Under `keep_in`, `MemoryDatabase::stored_in(storage, name)` reads what was kept and writes after each commit, `was_restored` says whether something was there, and `storage_problem` says what went wrong the last time it wrote, while the records stay in memory. `StoredModel` keeps the settings the same way.

`MemoryDatabase::export_json` gives every record as JSON (dates as `{"days", "seconds"}`), and `import_json` puts such JSON in place of all records, all or nothing: an entity it does not have, a record without its key or a value it cannot read leaves everything as it was. Records of an entity removed from the code are set aside when the database opens, not refused.

This is for one person's records on one machine. Several people sharing records need SQLite on a shared disk or a server.

## Work away from the screen

```rust
App::new("Raven Sweep")
    .entity::<Folder>()
    .action(|| {
        Action::background("Scan", "Varrer", |job| {
            for (done, path) in roots().iter().enumerate() {
                if job.cancelled() {
                    return Ok(());
                }
                job.progress(done as f32 / roots().len() as f32, format!("lendo {path}"));
                job.save(folder_of(path)?);
            }
            Ok(())
        })
        .target_entity("Folder")
    })
```

`App::action` adds an action with no module or controller to write. It is made again for each window, so each turns it on and off by itself.

`Action::background`, or `in_background` on any action, runs its work on a thread of its own once the action runs, after its question and `on_execute`. The work sees what was selected and what people gave it, and hands back through its `JobContext`:

- `progress(0.5, "lendo Caches")` says how far it got. `doing("…")` says what it is doing when it cannot tell how far.
- `save` and `delete` hand back records. They are kept in the frame's unit of work, on the screen's thread, and committed as they come; a change that fails undoes the ones that came with it and is told.
- `tell` shows a message, and an error it returns is shown too, as is a panic.
- `cancelled()` says someone pressed Cancel. It stops by returning, and what it handed back stays.

While it runs the action is off, by the reason `REASON_RUNNING`, so it never runs twice at once. The navigation shows each running job with its bar and Cancel. Lists read again as records come, and `on_executed` runs once the work ended. Nothing polls: the work wakes the screen. In the browser, where there are no threads, the work runs right away and the screen freezes while it does.

## States and reminders

```rust
EntityInfo::new("Task")
    // ...fields, with `state` among them...
    .state_machine(
        StateMachine::new("state")
            .state("draft", "Draft")
            .state("review", "In review")
            .state("done", "Approved")
            .start("draft")
            .transition("draft", "review", "Send to review")
            .transition("review", "done", "Approve")
            .when("[cost] < 1000"),
    )
    .state_appearance("state", "done", AppearanceRule::new("locked", ["cost"]).enabled(false))
    .reminder(Reminder::new("remind_at").criteria("[owner] = ?CurrentUserId"))
```

Each way gets an action, on when what is selected stands where it starts. The field is read-only on forms and starts in the first state. Any other change is refused at the commit, in memory and in SQLite, with no screen around it; several saves in one unit of work count as one change, from what is stored to what is kept. An `ObjectSpace` of your own calls `check_transitions`.

A record with a time in its reminder field comes up when that is due, for whoever can read it and meets the criteria, with what to do: open it, put it off 15 minutes, or dismiss it. The time is kept on the record, so it is one for everyone who sees it. Nothing runs while waiting: the screen asks to be built again at the next time due. This is not a business process engine: there is no routing, approval by several people or timing out.

## At a counter

An application used all day from the keyboard needs little of its own. These come with the framework, and `feature-center` shows them in "Caixa de exemplo", or alone with `cargo run -p feature-center -- caixa`.

```rust
App::new("Caixa")
    .counter()
    .theme(Theme::raven_light())
    .module(AppRecordsModule)
    .entity::<Product>()
```

- **The window.** `App::counter` picks `CounterTemplate`: the view takes it all, and the keys of what can be done stay below it, made from the actions that have one. `App::theme` starts on a theme of its own.
- **Keys.** `Action::hotkey` takes `f1` to `f24`, `alt+c` and `command+s`. On a Mac, Option composes an accent, and it still counts as the letter. `Session::show_shortcuts` lists the keys of what can be done now.
- **Notices.** `Module::notices` returns what everyone has to know while the module is in the application, such as that it is a beta. It stays on top of every window, a locked one too, no template can leave it out, and it cannot be dismissed.
- **Work in the background.** `Session::run_background(title, work, done)` runs `work` on another thread with a `JobProgress`, and `done` on the screens' thread. `cancel_background` asks it to stop, and what it brings still comes. On the web there are no threads, so it runs in line and the screen waits.
- **One record.** `EntityInfo::singleton`, or `#[object(singleton)]`, opens the form of the one record, or of a new one if there is none.
- **Numbers.** With `AppRecordsModule`, `next_number(space, "sale")` gives the next number in the unit of work. A rollback gives the same number again, so a commit leaves no gaps.
- **Drafts.** `save_draft`, `drafts` and `take_draft` keep work to pick up later, in text its owner reads back.
- **Locking.** `Session::lock(title, check)` hides everything until what is typed passes `check`. What people were doing stays as it was.
- **Fields.** The editor `Gtin` checks the digit while typing. `rust_decimal::Decimal` is a field type, exact up to 6 places.

## Reports and panels

```rust
App::new("Shop")
    .entity_info(order)
    .module(StoredReportsModule::new(ReportWords::pt_br()))
    .module(AnalyticsModule::new(ReportWords::pt_br()))
    .translation("pt-BR", ravenrs_app_report::PT_BR)
```

A report is a record: its name, the entity it is about and its layout. A new one starts from a table of what the entity's list shows, and **Design** opens the designer inside the application, over a sample of the records. **Show** asks for its parameters and shows its pages, from where it prints and exports. A list has **Show in report**, for the records selected or all it shows.

Permissions on `ReportDefinition` say who sees or changes a report, like any other record. Whoever runs one gets only what they can read: rows come from the view's unit of work, and whatever a layout says about addresses, queries or data of its own does not run. What is kept is the layout alone, with nobody's data.

A stored report also goes straight to a printer: `render_stored_report` lays it out over chosen records, each as many times as asked, and `send_to_printer` sends what `ravenrs-report-label` makes of it, out of the way of the screens. [REPORTING.md](REPORTING.md) has the details.

A panel is a record with tiles, edited on its own form: a title, an entity, a kind, what goes along the axis, values as `field:Sum`, and a filter. A tile that cannot be drawn says why, and the others still show.

## Languages and words

Every word the framework shows has a key in `ravenrs-app-i18n`: `ui.save` for the screens, `report.show` for reports, `rules.required` for rules, `widgets.close` for the components, `viewer.print` for the document viewer, and so on, with `format.decimal`, `format.thousands` and `format.money_prefix` for numbers. They come in English and Brazilian Portuguese, in `crates/ravenrs-app-i18n/texts/<language>/<part>.json`. A language without a key falls back to English, and a key nobody has shows as itself.

An application picks the language and, when it wants other words, gives a flat JSON of keys. Nothing of it is code of the screens:

```rust
App::new("Raven Contas")
    .language(Language::BrazilianPortuguese)
    .texts("pt-BR", r#"{ "ui.refresh": "Recarregar", "shop.hello": "Olá, {0}!" }"#)
```

What it replaces reaches every screen and every component inside them, as `Labels` comes from the same catalog. Keys of its own, for a module or an action, are read with `Session::texts()`, and `format` fills in `{0}`, `{1}`. `SecuredApp::texts` does the same for the screen of signing in and the application of each person. A module that brings words of its own ships them as a JSON constant for the application to give to `texts`.

Each set of words has `from_texts(&Texts)`: `Words`, `RuleWords`, `ReportWords` (and `ReportWords::of(language)`), `Labels`, `ViewerLabels`, `ParameterLabels`, the labels of `ravenrs-movie` and `ravenrs-ai`, and `SqlWords`. `default()` is English and `pt_br()` Brazilian Portuguese, both from the catalog, so a new language is a folder of JSON. `PaneWords::with_texts` takes the words a pane shares with the others.

The words of the model are a translation, not texts: `App::translation` takes a difference in JSON, and `export_captions` and `import_captions` make and read a sheet for whoever translates. The ones that come with the framework are in `ravenrs_app_i18n::model`, and `ravenrs_app_report::PT_BR` is one of them. `Catalog::missing` lists the keys a language does not have yet.

In the feature center, *Textos por chave* replaces a word of every list and has an action that reads a key of the center's own.

## Security

```rust
let mut entities = security_entities();
entities.push(audit_entity());
entities.push(account.clone());
let database = MemoryDatabase::new(entities)?;

let window = SecuredApp::new("Accounts", move || database.space(), move |_person| {
    App::new("Accounts").entity_info(account.clone())
})
.role(Role::new("Sales").policy(PermissionPolicy::ReadOnlyAll).object_permission(
    "Account",
    "[owner] = ?CurrentUserName",
    [Operation::Write],
    Access::Allow,
))
.audit(AuditSettings::new(AuditMode::Full))
.build()?;
```

A new installation asks for the first person, who becomes the administrator. Administrators manage users and roles on generated screens, under Administration.

How a question is answered, for each role: a field by criteria, then a field, then a record by criteria, then the entity, then the owner of an aggregated collection, then the role's policy. A denial beats an allowance at the same level. Creating takes `Create` and `Write`. A write is checked against the record before and after the change, so nobody can take a record over or give one away past what they can write.

`SecuredObjectSpace` is what protects records. It works on any `ObjectSpace` with no screen around it, which is what a service will use. A record that cannot be read is not there. A field that cannot be read reads as empty, also for filters, sorting and calculated fields that use it, and saving it back empty leaves what is stored. A list with a permission that depends on the record reads the whole table to answer, as the database does not know the permission. Records of users and roles are for administrators only, the audit trail can only be read, and a `secret` field is kept from everyone. What the screens hide on top of this, navigation among it, is courtesy.

Keep these in mind:

- **A database file on someone's machine protects nothing from whoever holds the file.** They can read it, change it and rewrite the trail with any SQLite tool. Real protection takes a server between people and the data.
- The audit trail only sees what goes through it. Changes made to the database some other way are not there, and neither is what `SecuredApp::seed` writes.
- A permission on an aggregated collection's owner that depends on the record cannot be told for the lines, so they stay closed until they have permissions of their own.
- Five wrong passwords make an account wait 15 minutes, so someone who knows a name can keep it waiting, the only administrator included. Answers take about the same time for any name, but a wrong password on a real account also writes to storage, which careful measuring could tell.
- Inserting a record with a key that exists says so, even for a record that cannot be read, so with keys that can be guessed, someone can tell which exist. GUID keys avoid this.
- Who someone is, is read again at each new view, so what someone else changed about them takes a moment to count. Two administrators demoting each other at the same moment from two programs can leave nobody, as the check happens in each one's unit of work.
- Whoever opens a new installation first becomes the administrator. That suits a program on someone's own machine; a server has to create its first user in its setup.
- Argon2id runs on the UI thread, about a fraction of a second, when signing in or changing a password.
- A web build keeps records in memory for now, so users last as long as the page.

## Accessibility

Every generated application reaches screen readers and the keyboard with nothing written for them. `ravenrs-app-a11y` holds what the framework does about it, and `ravenrs-app-ui` uses it everywhere.

- **Names.** A field is called by its caption, and an action by its caption, unless the declaration says otherwise: `FieldInfo::accessible_name` and `accessible_description`, `Action::accessible_name` and `accessible_description`, or the same on `#[field(...)]`. The model copies a field's words to its form item as `AccessibleName` and `AccessibleDescription`, so a translation or an administrator changes them as they change the caption. A field's description replaces its hint for screen readers; an error or a note on the field still comes first.
- **Focus order.** `EntityInfo::focus_order` puts fields first on the form, and the form shows them in the same order, so the eye and Tab go the same way. A focus order that names a field that is not there stops `Application::build`.
- **Shortcuts.** An action with a `hotkey` says its keys after its description.
- **Announcements.** Saving says `Saved: ` and the record's name, a save that fails its rules says why at once, deleting says how many, and opening a list says its title. They come from `Session::announcer`, which an application uses for its own news, or silences with `set_quiet`. The region takes no room, and keeps the last three, so one said right after another is not lost. Messages and toasts were already statuses and alerts.
- **Words.** The words of the announcements have keys under `a11y.`, in English and Brazilian Portuguese, and follow `App::language`. `AccessWords::from_lookup` takes them from wherever the application keeps its words, which is where the framework's catalog of words plugs in.

### The audit

```rust,ignore
let mut ui = Ui::new(quick_app::app(), Theme::raven_dark());
ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
Audit::new().theme(Theme::raven_light()).open("Novo").app(&mut ui).assert_clean();
```

`Audit::app` walks the application with no window: the screen it opens on, every item of the navigation, and on each, the buttons given to `open`. On every screen it reports a control with no name, an id that appears twice, focus on a node that is not there, Tab that does not move or goes round some controls and never reaches the others, a window over the screen that Escape does not close, and two commands on the same keys. On the theme on screen and on every theme given, it checks text and the focus ring against the WCAG AA contrast. `Audit::screen` looks at one screen. `Audit::items` lists the items of the navigation, and `Audit::item` opens one by its name and looks at it as `app` would, or reports `NoSuchItem` when the navigation no longer has it. `feature-center` audits each demonstration in a test of its own that way, so the 37 screens run in parallel, and one more test fails when a demonstration has no test. As before, `check_tree`, `check_focus`, `check_theme` and `check_shortcuts` are there one by one. `quick-app`, `quick-plan` and `feature-center` run it in their tests, and any example can.

## PostgreSQL

`ravenrs-app-postgres` keeps records in PostgreSQL, with the same builder as SQLite: `managed` creates and grows tables, `mapped` leaves a table that came first as it is, and `migration` runs something once. What the two share is in `ravenrs-app-sql`, and another database is a `SqlBackend`.

```rust,ignore
let backend = PostgresBackend::connect(&std::env::var("DATABASE_URL")?)?;
let database = PostgresDatabase::builder(backend).managed(order, TableMap::new("orders"))?.build();
```

`PostgresBackend::connect` takes the address as a database's panel gives it, `postgresql://user:password@host/db?sslmode=require` or `host=… user=…`, and does three things a hosted database needs:

- **TLS.** With any `sslmode` but `disable` it speaks TLS through rustls, and always checks the server's certificate against the Mozilla roots. Neon's certificates pass as they are. Supabase signs its direct connection with an authority of its own: download it from the project settings and add `sslrootcert=/path/prod-ca.crt` to the address, or give it with `PostgresOptions::root_certificate_pem`. `sslmode=prefer`, the default, still falls back to plain text when the server has no TLS, as on a local database; a certificate that fails the check is never taken.
- **Poolers.** A pooler in transaction mode hands each transaction to whichever server connection is free, so a statement prepared on one is missing on the next. Supavisor on port 6543, Neon's `-pooler` hosts and an address with `pgbouncer=true` are recognised, and statements then go unprepared: one round trip each, nothing left on the server, values as text that the server reads as it would in the statement. `PostgresOptions::statements` says it outright, and `PostgresBackend::with_statements` does it for a `Client` you opened. Behind a pooler, `SET`, `LISTEN` and session locks do not last past the transaction.
- **A connection that drops.** Neon puts a database to sleep after a while with nothing to do, and the connection goes with it. Outside a transaction the backend opens the connection again before the next statement, and a read that met the drop goes once more. Inside one it never does: the commit fails, nothing of it stays, and the next unit of work starts on a new connection. With no `connect_timeout` in the address it waits 15 seconds, time for a database to wake up.

| Service | Address to use | What to know |
|---|---|---|
| Supabase, direct | `db.<project>.supabase.co:5432`, `sslmode=require&sslrootcert=…` | IPv6 only unless the IPv4 add-on is on |
| Supabase, Supavisor | `aws-0-<region>.pooler.supabase.com`: 5432 is a session, 6543 transactions | 6543 goes unprepared by itself; if the certificate check fails, add the project's authority with `sslrootcert` as for the direct connection |
| Neon | the host with `-pooler` for many short connections, without it for one long one | the first statement after a sleep waits for it to wake |
| Nhost | the Postgres address of the project, when its plan lets one in | Hasura's GraphQL is another way in, not this one |

`PostgresBackend::from(client)` still takes a connection you opened yourself, with the TLS and credentials you chose; it is not opened again when it drops.

A value goes in as what its column asks for, so a table with `NUMERIC`, `TIMESTAMP`, `DATE`, `SMALLINT` or `UUID` works as it is; a managed table keeps dates as text, as SQLite does.

### How a `Decimal` is kept

A record holds every number as an `f64`, which keeps about 15 significant digits. A field `Decimal(n)` is rounded to its `n` places before any database writes it, half to even as `rust_decimal` does, starting from the shortest text that reads back as the number, which is what the person typed: `0.1 + 0.2` is written as `0.3` in a field with 2 places. A value with more than 15 significant digits after rounding (`EXACT_DIGITS`) is refused with `SpaceError::Refused`, naming the field, and never rounded where nobody sees it. `ravenrs_app::exact_decimal` and `exact_decimals` do this, for any other place that writes records.

| Database | Column of a managed table | Why |
|---|---|---|
| PostgreSQL | `NUMERIC(18, n)` | Exact: `16.47` is `16.47` in the table, for reports, sums and other programs that read it. The value goes and comes as decimal digits, never through a binary sum |
| SQLite | `REAL` | The same binary number `Value::Number` is, so nothing changes on the way. Scaled whole numbers would need every filter value scaled too, and text would sort `9.5` after `10` |
| PostgREST | what the server's table has | The JSON number comes back as the same `f64`; use `NUMERIC(18, n)` on the server |

A managed table made before this, with `DOUBLE PRECISION` or `REAL`, is brought up to date once, as a migration named `decimal <table>.<column> <n>`: PostgreSQL turns the column into `NUMERIC(18, n)` with what it holds rounded, and SQLite rounds what it holds. Each column changes in its own transaction, so one that fails, say with a value too large for `NUMERIC(18, n)`, stays as it was, and opening the database says why. Changing the places of a field later runs it again with the new ones.

Limits to know:
- Numbered keys come from the highest in the table, so one process at a time should create records on it. A second one gets a conflict, and not a wrong record.
- `Like` is case-sensitive here and not in SQLite, as each database does.
- `TIMESTAMPTZ` comes in UTC, with no time zone of its own, since `DateTime` has none.
- Reading entities from a schema, and keeping the model and attachments in the database, are only in SQLite for now.

Its tests run when `RAVENRS_POSTGRES_URL` says where a database is, and otherwise have nothing to run against: `scripts/check.sh` does not start one. `RAVENRS_POSTGRES_STATEMENTS=unprepared` runs them all with statements as they go to a pooler. `tests/managed.rs` takes `RAVENRS_POSTGRES_TLS_URL` with `RAVENRS_POSTGRES_CA` (a server whose certificate an authority of its own signed) and `RAVENRS_POSTGRES_POOLER_URL` (PgBouncer in transaction mode with `max_prepared_statements = 0`).

## PostgREST, Supabase and Neon's Data API

`ravenrs-app-postgrest` keeps records in PostgreSQL through PostgREST, the HTTP API that Supabase puts in front of every project and that Neon's Data API speaks too. The station holds the token of the person signed in, never the database's password: what that person may read and change is decided by the database, with its grants and row level security.

```rust,ignore
let database = PostgrestDatabase::builder("https://<project>.supabase.co/rest/v1")
    .api_key(public_key)                       // Supabase's anon key; Neon needs none
    .token_source(Rc::new(move || session.access_token()))
    .resource(order, ResourceMap::new("orders").column("active", "is_active"))?
    .build();
let mut space = database.space();
```

How it works:
- **Reading.** A query becomes `GET /orders?select=…&and=(…)&order=…&limit=…&offset=…`. The filter is said in PostgREST's own words: comparisons, `Like`, `In`, `Between`, `Is Null`, `Not`, `And` and `Or` at any depth, `StartsWith`, `EndsWith` and `Contains`. Text always goes in quotes, so a comma or a parenthesis in it stays text. What those words cannot say (arithmetic, date functions, a calculated field, a path through a relation) is read and filtered in memory, as the SQL spaces do. Records come page after page until an empty one, so a server whose `max-rows` is smaller than the page loses none.
- **Committing.** Every change of the unit of work goes in one call, `POST /rpc/ravenrs_commit`, to a function that applies them in one transaction. It runs as whoever called it, so the policies that hold over REST hold here too, and a failure anywhere leaves nothing. A record to update that is gone, or not the caller's, is a `Conflict`. No answer to a commit is `SpaceError::Unknown`: it may have happened, and it is not sent again.
- **The token** is asked for at each request, so one that was renewed is used at once. Neither it nor the key appears in `Debug` or in errors.

On the server, once, as the owner of the schema PostgREST serves:

```sql
SET search_path TO api;
-- the text of ravenrs_app_postgrest::SERVER_SQL
REVOKE EXECUTE ON FUNCTION ravenrs_commit(jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION ravenrs_commit(jsonb) TO authenticated;
NOTIFY pgrst, 'reload schema';
```

A tenant comes from the token, never from what the station sends: give the table a column that defaults to the claim, and a policy on it.

```sql
ALTER TABLE orders ADD COLUMN tenant text NOT NULL
  DEFAULT (current_setting('request.jwt.claims', true)::json ->> 'tenant');
ALTER TABLE orders ENABLE ROW LEVEL SECURITY;
CREATE POLICY own_tenant ON orders
  USING (tenant = current_setting('request.jwt.claims', true)::json ->> 'tenant')
  WITH CHECK (tenant = current_setting('request.jwt.claims', true)::json ->> 'tenant');
```

| Service | Address | Token |
|---|---|---|
| Your own PostgREST | where it listens; `.schema()` when it serves more than one | signed with its `jwt-secret` |
| Supabase | `https://<project>.supabase.co/rest/v1`, with `.api_key(anon key)` | the session's access token from Supabase Auth; the anon key alone is the `anon` role |
| Neon Data API | the endpoint the console shows for the branch | from Neon Auth, or your own provider through its JWKS |

Limits to know:
- Desktop only: a space answers at once, and a browser cannot wait for HTTP.
- Numbered keys come from the highest in the resource, as in the SQL spaces: one writer at a time, or keys the database makes.
- State machines are checked against what was read before the commit, not inside its transaction.
- A table without row level security is open to every role that has grants on it: the function adds no rule of its own.

Its tests run against a real PostgREST when `RAVENRS_POSTGREST_URL`, `RAVENRS_POSTGREST_ADMIN_URL` and `RAVENRS_POSTGREST_TOKEN_A`/`_B` say where; `tests/postgrest.rs` says what each must be.

## A web API

`ravenrs-app-server` serves the same model to clients that do not use RavenRS. Reads and writes go through `SecuredObjectSpace`, so what a role cannot do on the screens it cannot do here, and rules, state machines and the audit trail hold.

```rust,ignore
ravenrs_app_server::serve(listener, || {
    // On the thread that serves requests: storage and permissions belong to one thread.
    ApiService::new(space, roles).map(|service| service.audit(settings)).map_err(|e| e.to_string())
})
```

| Route | What it does |
|---|---|
| `POST /api/auth/token` | Signs in with `name` and `password`, for a bearer token |
| `POST /api/auth/revoke`, `POST /api/auth/password` | Ends a token; changes the password, ending this person's other tokens |
| `GET /api/data/{entity}` | Records, with `filter` (the criteria language), `order`, `skip`, `top`, `count` and `expand` |
| `GET`, `PATCH`, `DELETE /api/data/{entity}/{key}` | One record. `PATCH` changes only the fields sent |
| `POST /api/data/{entity}` | A new record. Aggregated collections come along, as records to create or change, or `{"$delete": key}` |
| `POST /api/batch` | Several writes that stay or go together |
| `POST /api/validate/{entity}` | What the rules would say, with nothing kept |
| `POST /api/run/{name}` | Something of the application's own, from `ApiService::endpoint`, with the permissions of who calls |
| `GET /api/openapi.json`, `GET /api/me` | The description of what this person can reach; who they are |
| `GET /health` | The only route that needs no token |

Answers: `400` what was asked does not fit, including any field that cannot be written; `401` no valid token; `403` not allowed; `404` no such record, or one this person cannot see; `409` someone got there first; `422` rules or states refuse it, with `violations`; `429` too many tries to sign in; `503` busy.

Other systems sign in with a **key**, in the same `Authorization: Bearer` header. An administrator makes one with `POST /api/auth/keys`, giving a `name` and optionally a `user`, `roles` and `expires_in_days`, and it shows in that answer and never again, since only a hash is kept. `GET /api/auth/keys` lists them, `POST /api/auth/keys/{id}/rotate` gives a new secret and `DELETE /api/auth/keys/{id}` ends one, each at once. A key acts as its user and never as more: its roles are those it names that the user still has, so losing a role or being turned off counts at once. With `RoleMerge::AllRoles` a key cannot name roles, since leaving one out would take away what it denies. A key makes no keys, changes no passwords and reaches no setting that belongs to a person. Changing a user's password does not end their keys, which is what lets a nightly job go on. There is no screen for keys yet.

**Settings** are JSON under a name, for everyone or for one person, and add no column, unlike a custom field. `ApiService::setting(name, SettingAccess::PerPerson)` or `Shared` opens one at `/api/settings/{name}`, with `GET`, `PUT` and `DELETE`; one nobody declared cannot be reached. Storage has to know `settings_entity()`. `App::settings` gives the screens a `SettingsStore`, and a `SecuredApp` over storage that knows that entity uses it by itself: the theme someone chose comes back when they open again, each person their own. So does "Follow the system": it is kept as `theme.follow_system` next to the theme, and when it is on, the system's light or dark wins over the theme kept as soon as the platform says which it is. An application built `without_theme_menu` reads and writes neither. A setting is at most 64 KB. What comes back as a theme goes through `Theme::from_tokens_json` and `Theme::is_readable`, so someone who saved a blank theme still opens to a window they can read, and the theme at the door stays the same for whoever comes next.

An application with a look of its own gives its themes with `App::themes([light, dark])`. The Theme menu then offers only those, under their names, and starts on the first; the one someone chooses is kept as above. With a light and a dark one among them, "Follow the system" switches between the first of each instead of Raven Light and Raven Dark. Each theme is kept as what differs from the built-in theme it is `based_on`, so a theme made with `Theme { name: "Mine", ..Theme::raven_light() }`, or read from tokens with a `"name"`, comes back under its own name and colors. A kept theme that fails `is_readable`, as white text on a light accent, is not brought back.

The top of the navigation can carry the application's brand: `App::brand(IconTile::new(IconName::Church))` with `brand_subtitle("Central Church")`. Reduced to icons, only the mark stays, and screen readers hear the title and the line. `App::navigation_count("Lesson", |session| ...)` puts a count beside that item of the navigation, read again whenever records change, with `NavigationCount::spoken` for what screen readers hear ("1 with attendance to take"). Cmd/Ctrl+K opens a command palette with the places the person can open. The list report of `ReportModule` comes out in the colors on screen: the accent and its text for the headers, and the theme's text where it is dark enough for white paper.

It is not OData: the query options are its own, which is the difference to keep in mind for clients made for OData. Money travels in cents, as it is stored, and dates as text. Users, roles and the audit trail are not exposed, whatever the permissions say.

Limits to know:
- One thread answers everything, one request at a time, as storage belongs to one thread. No throughput has been measured, so none is promised.
- Tokens live in memory, so a restart asks everyone to sign in again.
- Plain HTTP only: outside the machine it belongs behind something that does TLS.
- Tries to sign in are limited by address, and behind a proxy everyone is one address.
- Five wrong passwords lock a user for fifteen minutes, from any address, so someone who knows a name can keep that person out. It is the price of stopping guesses; the lock is per user because an address is cheap.
- A unique key or rule says that a value is taken, even to someone who cannot read that record: any unique value does, since refusing it is the only honest answer. Do not put a secret in a key someone can pick.
- What a rule says goes back as written: a message that quotes a field the person cannot read would show it, so keep those out of rule messages.
- A key's expiry goes by the server's clock, in its local time, so a change of time zone moves it.
- Two people changing the same field: the last one stays, since `PATCH` carries no version. Different fields do not get in each other's way.

Try it with `cargo run -p feature-center --bin api`.

## An application over a server

`ravenrs-app-remote` puts the screens over the web API, on the desktop and in the browser, which gives the browser real storage.

```rust,ignore
RemoteApp::view("Orders", Language::English, RemoteClient::http(address), entities, |records| {
    let records = records.clone();
    App::new("Orders").entity::<Order>().storage(move || records.space())
})
```

It asks who is there, reads what they can see into a copy in memory, and the screens work over that copy. A commit goes to the server as one batch, and what it touched is read again, with the keys the server gave.

- **On the desktop** a commit waits for the server, and what it refuses is refused there and then.
- **In the browser** nothing can wait. The screen goes on at once, and the answer sets it right: what the server refused comes back, with a message. A form has already closed by then, so what was typed in it is gone. That is the price of a browser.

The server decides everything: the copy only holds what the person can see, so taking the client apart gives nobody anything more. A browser page on another origin needs `ApiOptions::allow_origin`, and there is no way to allow every origin.

Limits to know:
- Everything the person can see is read when they sign in, so this is for what fits in memory. `DataGrid::remote` over the API, for larger lists, is not done.
- What someone else changes shows only when this person's own write reads that entity again, or on signing in again.
- A record that refers to another one created in the same commit does not work, since the server gives the key. Keep them in two commits.
- An address comes from the build (`RAVEN_API_URL` in the center), and never from the page's own address, so a link cannot make a page send a password somewhere else.

Try it: `RAVEN_API_ORIGIN=http://localhost:8099 cargo run -p feature-center --bin api`, and `RAVEN_API_URL=http://127.0.0.1:8090 scripts/build-web.sh feature-center`, served on that port.

## Known gaps

- The audit checks the contrast of a theme's pairs of tokens, not of each pixel drawn: a color a rule or a screen chose for itself is not checked. It does not look at the size of targets, at text resized, or at the reading order against the screen.
- A focus order reorders fields within their group of the form. Moving a field to another group is still done in the model.
- A field has no keys of its own to reach it; shortcuts are for actions.
- The words of the announcements live in `ravenrs-app-a11y` until the framework keeps all its words in one catalog.
- The region of announcements is in the window, so while a window is open over it, what changes behind is heard once that window closes.

- A declared action sets fields and nothing else: it does not open views, ask for input or reach other records. One that needs to does it in `on_execute`.
- State machines are declared in code. People cannot define states or ways while the application runs.
- A reminder someone else set or changed shows the next time records are read here, as nothing polls. Reminders do not go anywhere outside the application.
- `Decimal` is exact in Rust, but a record holds it as a number, so criteria, filters and SQL work with the number. A database keeps it rounded to its places, and a value with more than 15 significant digits is refused (see [How a `Decimal` is kept](#how-a-decimal-is-kept)).
- `next_number` is exact within one program. Two programs on one SQLite file have not been tried.
- Closing a window and locking in the same frame leaves the field of the locked window without the focus. A frame between them is enough.
- `DocumentViewer` shows what the report engine lays out. It does not open PDF files, so an attached PDF is saved or opened with the system.
- Stored reports are not sent on a schedule yet: `ravenrs-report-server` reads its own sources, not entities, and a job has to run as someone. That takes the service of F12.
- The designer's own words are in Portuguese whatever the application's language, and its field list shows field names rather than captions.
- A report reads every record of its entity into memory, as its engine works over rows.
- A chart, month, timeline or tree has no search, column filters or export, which are `DataGrid`'s own. Filters in the model still count. Settings have to name fields the list reads, so `DataView` needs them among its columns.
- Moving a bar on a timeline snaps to what the axis shows, so at the scale of days a record loses its hour.
- The model editor changes settings, one at a time, in the person's own layer. It does not add nodes or edit the administrator's layer.
- A field a person cannot read shows as Protected on a form, but as an empty cell on a list, and its column stays.
- `SecuredApp` makes an application anew at each sign-in, with storage made before that: fields a layer adds have to be declared to storage by hand, so `App::storage_for` is not for it.

- Search, filters and export of a list are `DataGrid`'s own, not actions the model can move.
- Background work shows its progress in the navigation of `AppShell`. A pane embedded in a screen of your own keeps what the work hands back, but shows no bar.
- Background work hands back records and messages; it does not open views or ask questions.
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
- A translation changes the model's words; `Words` and the texts follow `App::language` and do not change at run time.
- `Culture` in `ravenrs-criteria`, which formats numbers and dates inside expressions and reports, is apart from the catalog. So are the words of the report designer.
- A pane finds a row's key by the order of the entity's fields, so a layer that reorders its columns breaks its buttons.
- Putting two people's changes together reads and then writes, in two steps. Two programs on one SQLite file can still meet between them, until there is a server.
- Editing on a list needs a list that holds its records, so not `Server`, `DataView` or a given source. A cell that fails is marked, and its message shows only when it is edited.
- A `display_format` on a date makes its cells text, so they sort as text. An `edit_mask` has no regular expressions: `Rule::matches` checks those on saving.
- Predefined values are picked and not typed. A Rust struct does not expand into a group of fields: `group` puts plain fields together.
- Fields added in the model hold plain values, not references or lists. Used without `App`, whoever opens storage has to give it `Application::entities`.

## What comes next

| Step | What it brings |
|---|---|
| **F12** | An API over the same permissions, a remote client and other databases |

Before more work on editors or templates, read what is documented on template customization and on each property type: it was not read when these screens were written. The same goes for the built-in node generators and list view column generation, before more of the model is generated.
