# Application framework parity

What exists of the behavior documented for model-driven application frameworks, and the step that brings the rest. ✅ works, 🟡 in part, ⬜ planned. See [APP-FRAMEWORK.md](APP-FRAMEWORK.md).

| Area | State | Step |
|---|---|---|
| Records declared by attributes, or at run time | ✅ `BusinessObject`, `EntityInfo` | |
| Application model in layers, with differences in JSON | ✅ `ModelLayers`, `Diff`, `ModelStore` | |
| Generated lists, lists to pick from, forms and navigation | ✅ `generate_model` | |
| Model editor at run time | ⬜ | F8 |
| Unit of work | ✅ `ObjectSpace`, `MemoryDatabase` | |
| SQLite, on a managed schema or tables that exist | ⬜ | F6 |
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
| Validation | ⬜ `Frame::set_validator` is where it goes | F5 |
| Conditional appearance | ⬜ | F5 |
| Reports | ⬜ | F7 |
| Security: roles, permissions, users, audit trail | ⬜ | F8 |
| Multi-tenancy, state machines, web API, notifications, office modules | ⬜ not planned yet | |
