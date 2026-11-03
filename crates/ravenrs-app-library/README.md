# ravenrs-app-library

Records most business applications need, for the RavenRS application framework: people, organizations, addresses, countries, tasks, events, notes and files. Use them as they are, or add fields and rules to them before handing them over. It depends only on `ravenrs-app`, so its entities also work with no screens.

```rust
use ravenrs_app::{Application, FieldInfo, FieldKind};
use ravenrs_app_library::{LibraryModule, person};

let application = Application::builder("Contacts")
    .module(LibraryModule::new().task().entity(person().field(FieldInfo::new("badge", FieldKind::Text))))
    .build()?;
assert!(application.entities().iter().any(|entity| entity.name == "Address"));
# Ok::<(), ravenrs_app::ModelError>(())
```

Entities are named `Person`, `Task` and so on. An application with its own record of the same name gets a clear error: leave that one out of the module, or rename it. `PT_BR` is their words in Brazilian Portuguese, for `translation`.
