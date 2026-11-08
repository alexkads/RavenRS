# ravenrs-project

Turns code written in the RavenRS playground into a Cargo project: `Cargo.toml` on the published
crates, `src/lib.rs` with the code and the browser's entry, `src/main.rs` for the desktop,
`web/index.html`, and the records the app had as `src/seed.json`. `Project::zip` hands it over as
one file, which the playground saves with the person's file dialog or as a download.

```rust
use ravenrs_project::Project;

let project = Project::new("Hello")
    .source("use ravenrs::prelude::*;\n\nfn hello() -> impl View {\n    Text::new(\"Hello\")\n}\n");
let files = project.files()?;
assert!(files.iter().any(|file| file.path == "src/main.rs"));
# Ok::<(), String>(())
```
