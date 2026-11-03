# Application framework parity

What exists of the behavior documented for model-driven application frameworks, and the step that brings the rest. ✅ works, 🟡 in part, ⬜ planned. See [APP-FRAMEWORK.md](APP-FRAMEWORK.md).

| Area | State | Step |
|---|---|---|
| Records declared by attributes, or at run time | ✅ `BusinessObject`, `EntityInfo` | |
| Application model in layers, with differences in JSON | ✅ `ModelLayers`, `Diff`, `ModelStore` | |
| Generated lists, lists to pick from, forms and navigation | ✅ `generate_model` | |
| Model editor at run time | ⬜ | F8 |
| Unit of work | ✅ `ObjectSpace`, `MemoryDatabase` | |
| SQLite, on a managed schema or tables that exist | ✅ `ravenrs-app-sqlite`: filters in the database with bound values, words for what it refuses. Lists of many rows still read them all | |
| A list beside its form, inside a screen of the app | ✅ `EntityPane`, in use in the registers of `examples/family-finance` | |
| Simple, popup, single-choice, parametrized and URL actions | 🟡 `Action`; popups show only lists | F7 |
| Action scope, selection dependency and target criteria | ✅ `TargetScope`, `Selection`, `CriteriaMode` | |
| Mapping actions to containers in the model | ⬜ by category for now | |
| View and window controllers, frames, modules | ✅ | |
| Templates | 🟡 `FrameTemplate`; one built in | |
| List views | 🟡 `ListView` over `DataGrid`; search, filters and export are the grid's own | |
| Detail views and layout | 🟡 `DetailView`; pictures, files and collections have no real control | F7 |
| Property editors by kind, replaceable | ✅ `EditorRegistry` | |
| Save, cancel, new, delete, refresh, back, moving between records | ✅ `SystemModule` | |
| Dashboards, nested lists, link and unlink, view variants | ⬜ | F7 |
| Validation: required, range, pattern, text and value comparison, criteria, is true, unique, unique together, exists, referenced, custom; contexts; target criteria | ✅ `Rule`, `validate`; forms and delete check it, and actions name their contexts. Validating as a field is left is planned | |
| Conditional appearance: font, back, bold, enabled, visible; on items, layout and actions; by criteria or function; by priority | 🟡 `AppearanceRule`. On a form, a font color reaches the caption, not the value inside the editor; a group's caption takes no color | |
| Reports | ⬜ | F7 |
| Security: roles, permissions, users, audit trail | ⬜ | F8 |
| Multi-tenancy, state machines, web API, notifications, office modules | ⬜ not planned yet | |
