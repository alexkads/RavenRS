# Application framework

Declare what a record is, and get the screens: navigation, a list, a form, commands and rules. Everything else changes through metadata, actions and controllers. The behavior follows what is documented for model-driven application frameworks; the code and the words are our own.

> **Status:** in progress, usable from code, with the API still moving. Nothing here is in the `ravenrs` facade yet.

## What exists

| Crate | What it has |
|---|---|
| `ravenrs-app` | No GUI. `#[derive(BusinessObject)]` and `#[derive(BusinessEnum)]`, from `ravenrs-app-derive`; `EntityInfo` and `Object`; `generate_model`, `ModelLayers`, `Diff`, `ModelStore`, `check_model`; `ObjectSpace` and `MemoryDatabase`; `Action`, `TargetScope`, `ViewController`, `WindowController`, `Frame`; `Module`, `Application` |
| `ravenrs-app-a11y` | `AccessibleText`, `Announcer` and `LiveRegion`, `AccessWords`, `Audit`: names from the model, announcements, and an audit that walks an application |
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
| `accessible_name = "..."`, `accessible_description = "..."` | What screen readers call it instead of the caption, and say after it instead of the hint |

On the struct, `#[object(focus_order = "name,amount")]` puts these fields first on the form, in this order, on the screen and for Tab.

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

A panel is a record with tiles, edited on its own form: a title, an entity, a kind, what goes along the axis, values as `field:Sum`, and a filter. A tile that cannot be drawn says why, and the others still show.

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

`Audit::app` walks the application with no window: the screen it opens on, every item of the navigation, and on each, the buttons given to `open`. On every screen it reports a control with no name, an id that appears twice, focus on a node that is not there, Tab that does not move or goes round some controls and never reaches the others, a window over the screen that Escape does not close, and two commands on the same keys. On the theme on screen and on every theme given, it checks text and the focus ring against the WCAG AA contrast. `Audit::screen` looks at one screen, and `check_tree`, `check_focus`, `check_theme` and `check_shortcuts` are there one by one. `quick-app`, `quick-plan` and `feature-center` run it in their tests, and any example can.

## Known gaps

- The audit checks the contrast of a theme's pairs of tokens, not of each pixel drawn: a color a rule or a screen chose for itself is not checked. It does not look at the size of targets, at text resized, or at the reading order against the screen.
- A focus order reorders fields within their group of the form. Moving a field to another group is still done in the model.
- A field has no keys of its own to reach it; shortcuts are for actions.
- The words of the announcements live in `ravenrs-app-a11y` until the framework keeps all its words in one catalog.
- The region of announcements is in the window, so while a window is open over it, what changes behind is heard once that window closes.

- State machines are declared in code. People cannot define states or ways while the application runs.
- A reminder someone else set or changed shows the next time records are read here, as nothing polls. Reminders do not go anywhere outside the application.
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
| **F12** | An API over the same permissions, a remote client and other databases |

Before more work on editors or templates, read what is documented on template customization and on each property type: it was not read when these screens were written. The same goes for the built-in node generators and list view column generation, before more of the model is generated.
