# Application framework parity

What exists of the behavior documented for model-driven application frameworks, and the step that brings the rest. ✅ works, 🟡 in part, ⬜ planned. See [APP-FRAMEWORK.md](APP-FRAMEWORK.md).

| Area | State | Step |
|---|---|---|
| Records declared by attributes, or at run time | ✅ `BusinessObject`, `EntityInfo` | |
| Application model in layers, with differences in JSON | ✅ `ModelLayers`, `Diff`, `ModelStore` | |
| Generated lists, lists to pick from, forms and navigation | ✅ `generate_model` | |
| Model editor at run time | ⬜ | F8 |
| Unit of work | ✅ `ObjectSpace`, `MemoryDatabase` | |
| SQLite, on a managed schema or tables that exist | ✅ `ravenrs-app-sqlite`: filters in the database with bound values, words for what it refuses. | |
| A list beside its form, inside a screen of the app | ✅ `EntityPane`, in use in the registers of `examples/family-finance` | |
| Simple, popup, single-choice, parametrized and URL actions | ✅ `Action`; a popup shows a list to pick from or a form to fill in, and a parametrized action reads what is typed as the kind it asks for | |
| Action scope, selection dependency and target criteria | ✅ `TargetScope`, `Selection`, `CriteriaMode` | |
| Mapping actions to containers in the model | ⬜ by category for now | |
| View and window controllers, frames, modules | ✅ | |
| Templates | 🟡 `FrameTemplate`; one built in | |
| List views | 🟡 `ListView` over `DataGrid`; search, filters and export are the grid's own | |
| Detail views and layout | ✅ `DetailView`; pictures, files and collections have real controls | |
| A form in a window, kept by the application's own rules | ✅ `EntityForm`, in use in the transactions, bills and plans of `examples/family-finance` | |
| References: picked from a list or looked up, filtered by the record being edited | ✅ `Select` editor, `lookup_filter`, `none_caption`, `LookupField` | |
| Property editors by kind, replaceable | ✅ `EditorRegistry` | |
| Save, cancel, new, delete, refresh, back, moving between records | ✅ `SystemModule` | |
| Dashboards | ✅ `DashboardInfo`, `DashboardView`: views and controls, each in its own frame | |
| Lists inside a form, link and unlink | ✅ `FieldKind::Collection`; what links is kept right away, in a unit of work of its own | |
| View variants | ✅ `ViewVariant`: a filter each, with the choice in the person's layer. Variants that change the columns are not there | |
| Lists of many rows | ✅ `ListView::remote`, over `RemoteRows`, continuing from the last key when nothing asks for another order | |
| Validation: required, range, pattern, text and value comparison, criteria, is true, unique, unique together, exists, referenced, custom; contexts; target criteria | ✅ `Rule`, `validate`; forms and delete check it, and actions name their contexts. Validating as a field is left is planned | |
| Conditional appearance: font, back, bold, enabled, visible; on items, layout and actions; by criteria or function; by priority | 🟡 `AppearanceRule`. On a form, a font color reaches the caption, not the value inside the editor; a group's caption takes no color | |
| Reports of a list | ✅ `ravenrs-app-report`: an action that lays out the list and shows it in the viewer | |
| Security: roles, permissions, users, audit trail | ⬜ | F8 |
| Replaceable list editors, filters in the model, migrations of a managed schema, model differences in a database, localized captions | ⬜ | F7.5 |
| Clone object, file attachments with their own storage and download | ⬜ | F7.5 |
| Chart and scheduler list editors | ⬜ | F9 |
| Tree and pivot list editors | ⬜ needs the tree list and the pivot grid | F9 |
| Report designer and definitions in a database, dashboards people create | ⬜ | F10 |
| State machines, notifications | ⬜ | F11 |
| Office modules | 🟡 PDF through `DocumentViewer` to be checked; rich text and spreadsheet need those controls | F11 |
| Web API, remote object space, other databases | ⬜ | F12 |
| Multi-tenancy | ⬜ | F13 |
| AI: natural language filters, writing help, smart paste | ⬜ local models only, so not on the web | F14 |
| Project generator, deployment and diagnostics guides | ⬜ | F15 |
