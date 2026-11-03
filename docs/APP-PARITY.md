# Application framework parity

What exists of the behavior documented for model-driven application frameworks, and the step that brings the rest. ✅ works, 🟡 in part, ⬜ planned. See [APP-FRAMEWORK.md](APP-FRAMEWORK.md).

| Area | State | Step |
|---|---|---|
| Records declared by attributes, or at run time | ✅ `BusinessObject`, `EntityInfo` | |
| Application model in layers, with differences in JSON | ✅ `ModelLayers`, `Diff`, `ModelStore` | |
| Generated lists, lists to pick from, forms and navigation | ✅ `generate_model` | |
| Model editor at run time | 🟡 `ModelEditor`: a tree with its settings, one layer, the person's own. Adding nodes and other layers are not there; measured against plain differences before going further | |
| Unit of work | ✅ `ObjectSpace`, `MemoryDatabase` | |
| SQLite, on a managed schema or tables that exist | ✅ `ravenrs-app-sqlite`: filters in the database with bound values, words for what it refuses. | |
| A list beside its form, inside a screen of the app | ✅ `EntityPane`, in use in the registers of `examples/family-finance` | |
| Simple, popup, single-choice, parametrized and URL actions | ✅ `Action`; a popup shows a list to pick from or a form to fill in, and a parametrized action reads what is typed as the kind it asks for | |
| Action scope, selection dependency and target criteria | ✅ `TargetScope`, `Selection`, `CriteriaMode` | |
| Mapping actions to containers in the model | ⬜ by category for now | |
| Actions inside a form's layout | ✅ `ActionCategory::InForm`, `EntityInfo::form_actions` | |
| Messages from actions | ✅ `ActionContext::message`: kind, duration, a button and what runs once it is gone | |
| View and window controllers, frames, modules | ✅ | |
| Templates | 🟡 `FrameTemplate`; one built in | |
| List views | 🟡 `ListView` over `DataGrid`; search, filters, sorting, grouping, column menu, summaries and export are the grid's own | |
| Editing on a list, with the rules checked | ✅ `list_editable`: refused values stay open and say why, and a cell that fails shows it. Plain values only, on lists that hold their records | |
| Data access modes | 🟡 `DataAccess`: `Client`, `Server` and `DataView`. Nothing like instant feedback, since `Server` already answers through a callback | |
| Detail views and layout | ✅ `DetailView`; pictures, files and collections have real controls | |
| A form in a window, kept by the application's own rules | ✅ `EntityForm`, in use in the transactions, bills and plans of `examples/family-finance` | |
| References: picked from a list or looked up, filtered by the record being edited | ✅ `Select` editor, `lookup_filter`, `none_caption`, `LookupField` | |
| Property editors by kind, replaceable | ✅ `EditorRegistry` | |
| Save, cancel, new, delete, refresh, back, moving between records | ✅ `SystemModule` | |
| Dashboards | ✅ `DashboardInfo`, `DashboardView`: views and controls, each in its own frame | |
| Lists inside a form, link and unlink | ✅ `FieldKind::Collection`; what links is kept right away, in a unit of work of its own | |
| View variants | ✅ `ViewVariant`: a filter and columns each, with the choice in the person's layer | |
| Lists of many rows | ✅ `ListView::remote`, over `RemoteRows`, continuing from the last key when nothing asks for another order | |
| Validation: required, range, pattern, text and value comparison, criteria, is true, unique, unique together, exists, referenced, custom; contexts; target criteria | ✅ `Rule`, `validate`; forms and delete check it, actions name their contexts, and `checks_on_leave` speaks as a field is left. Contexts in collections are planned | |
| Soft validation and message templates | ✅ `Severity`: a warning asks and lets people go on, information leaves a note on the field; `{field}` and `{reference.field}` in messages | |
| Conditional appearance: font, back, bold, enabled, visible; on items, layout and actions; by criteria or function; by priority | 🟡 `AppearanceRule`. On a form, a font color reaches the caption, not the value inside the editor; a group's caption takes no color | |
| Reports of a list | ✅ `ravenrs-app-report`: an action that lays out the list and shows it in the viewer | |
| Reports kept as records, designed inside the application, over entities, with parameters and permissions | ✅ `StoredReportsModule`, `Designer::embedded`, `ShowInReport` | |
| Scheduled delivery of stored reports | ⬜ the report server cannot read entities until there is an API, and has to run as someone | F12 |
| Mail merge templates | ⬜ takes the rich text editor of Wave C | |
| Analytical panels people create, kept as records | 🟡 `AnalyticsModule`: tiles of numbers and charts, edited on generated forms. No canvas designer, and tile sizes are not kept | |
| Roles, policies and permissions on entities, records, fields, navigation and actions; several roles in any or in all | ✅ `Role`, `Permissions`, enforced in `SecuredObjectSpace`, also with no screen | |
| Policy changed while running, by administrators on generated screens | ✅ `security_entities`; losing a role takes effect at once | |
| Permissions that follow associations | 🟡 aggregated collections go with their owner; everything else asks for permission on both sides, as the documented manual mode does | |
| Signing in with passwords, lockout, password changes, other ways in | ✅ `SecuredApp`, `UserDirectory`, `Authenticator`. No Active Directory or OAuth 2 provider built in: `Authenticator` is where one goes | |
| Audit trail: author, old and new values, modes, settings, collections, custom data | ✅ `AuditedObjectSpace`, `AuditSettings`, `AuditTrail::custom`. Only what goes through it is seen | |
| A list as a chart | 🟡 `Chart` editor: bars, lines, areas and pies, summed by argument, money in whole units. Not exported or printed with the list | |
| A list on a calendar, and on a timeline of resources | 🟡 `Calendar` shows a month, and `Scheduler` a row per resource where records move, resize and get created. No recurring events or day and week views of hours | |
| A list as a tree | 🟡 `Tree` editor over a reference to itself. Columns take the tree list of Wave C | |
| A pivot grid over a list | ⬜ takes the pivot grid of Wave C | |
| State machines: states, transitions, actions and appearance by state | 🟡 `StateMachine`, refused in the object space. Declared in code; defining them at run time is not there | |
| Reminders of records: come up when due, put off, dismiss | 🟡 `Reminder`, for the application only: no channels outside it, and a change made by someone else shows on the next read | |
| PDF viewer for files, rich text and spreadsheet editors | ⬜ `DocumentViewer` shows the engine's documents, and does not read PDF files; the rest takes Wave C | |
| Replaceable list editors | ✅ `register_list_editor`, picked by `EditorType` or `DefaultListEditor` in the model | |
| Filters in the model, a list's own criteria, filtered link window, functions of the application | ✅ `ListFilter`, `list_criteria`, `lookup_filter` on a collection, `register_function` | |
| Migrations of a managed schema, entities from an existing database, model differences in a database | ✅ `migration`, added columns, `entities_from_schema`, `SqliteModelStore` | |
| Translated captions and options, rules in the model | ✅ `translation`, `set_language`, `Rules` under an entity | |
| Records no storage keeps, services, initial records | ✅ `transient`, `ListView::source`, `service`, `Module::initial_records` | |
| Concurrent changes | ✅ `merge_changes`: different fields both stand, the same field asks. Read and write are not one transaction yet | |
| Custom fields in the model, stored or calculated | ✅ `$new` under `Members`, `App::administrator_settings`, `App::storage_for`; plain values only | |
| Keys: number, text, GUID | ✅ `FieldKind::Guid` | |
| More editors: criteria, tags, predefined values, masks, progress, display formats, titled groups | ✅ `criteria_of`, `FieldKind::Tags`, `predefined`, `edit_mask`, `Progress`, `display_format`, `group`. A Rust struct does not expand into a group by itself | |
| Records no storage keeps with references, by key, with collections, from the navigation | ✅ `use_records`, `use_collection`, `ObjectKey` | |
| Dashboards switched by variants | ✅ `DashboardInfo::variant` | |
| Translating captions outside the code | ✅ `export_captions`, `import_captions`, as CSV | |
| Ready-made records | ✅ `ravenrs-app-library`: person, organization, address, country, task, event, note and file | |
| Clone object | ✅ `CloneModule`: only what says it can be, with `non_cloneable` and `aggregated` | |
| File attachments | ✅ `AttachmentStore`, out of the record, with a button to save the file | |
| Chart and scheduler list editors | ⬜ | F9 |
| Tree and pivot list editors | ⬜ needs the tree list and the pivot grid | F9 |
| Report designer and definitions in a database, dashboards people create | ⬜ | F10 |
| Web API: protected CRUD, filters, pages, deep updates, batches, own endpoints, OpenAPI | ✅ `ravenrs-app-server`, on a contract of its own rather than OData | F12 |
| PostgreSQL | ✅ `ravenrs-app-postgres`, over what SQL databases share in `ravenrs-app-sql`; schema reading, model and attachment storage only in SQLite for now | F12 |
| Remote object space, so the web has real storage | ⬜ | F12 |
| Multi-tenancy | ⬜ | F13 |
| AI: natural language filters, writing help, smart paste | ⬜ local models only, so not on the web | F14 |
| A command-line tool that creates projects step by step from templates; deployment and diagnostics guides | ⬜ | F15 |
| An installer with Zed as the editor for projects, its extension and skills, and a graphical viewer of a project | ⬜ | F16 |
