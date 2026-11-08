//! Turns code written in the RavenRS playground into a Cargo project that runs on the desktop
//! with `cargo run` and in the browser, as files or as one `.zip`.
//!
//! The code is kept as it was written. The project adds what a program needs around it: the
//! crates it uses, at the version this crate was published with, a `main` that opens a window,
//! the browser's entry point and its page.
//!
//! ```
//! use ravenrs_project::Project;
//!
//! let project = Project::new("Hello world")
//!     .source("use ravenrs::prelude::*;\n\nfn hello() -> impl View {\n    Text::new(\"Hello\")\n}\n");
//! assert_eq!(project.crate_name(), "hello-world");
//! let lib = project.files()?.into_iter().find(|file| file.path == "src/lib.rs").unwrap();
//! assert!(lib.contents.contains("pub fn app() -> impl ravenrs::widgets::View {\n    hello()\n}"));
//! # Ok::<(), String>(())
//! ```

use std::io::{Cursor, Write};
use std::path::PathBuf;

/// The version of the RavenRS crates a project depends on: the one this crate was published with.
pub const RAVENRS_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The `wasm-bindgen` the RavenRS crates were built with. The command line that makes the web
/// build has to be this same version, so it is pinned.
pub const WASM_BINDGEN_VERSION: &str = "0.2.130";

/// A RavenRS crate a project uses, with the features it turns on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    /// Its name on crates.io, such as `ravenrs-app`.
    pub name: String,
    /// The features it turns on.
    pub features: Vec<String>,
}

/// One file of a project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectFile {
    /// Where it goes, with `/` between folders.
    pub path: String,
    /// What it holds.
    pub contents: String,
}

/// A Cargo project made from code of the playground.
#[derive(Clone, Debug)]
pub struct Project {
    title: String,
    source: String,
    helpers: Vec<String>,
    dependencies: Vec<Dependency>,
    records: Option<String>,
    local: Option<PathBuf>,
}

impl Project {
    /// A project with this title, which names the window, the page and, in lower case, the crate.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            source: String::new(),
            helpers: Vec::new(),
            dependencies: vec![Dependency { name: "ravenrs".into(), features: Vec::new() }],
            records: None,
            local: None,
        }
    }

    /// The code, as written. Its first function is what the window and the page show.
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// Code the source calls that the playground gave it, such as a helper of its examples. It
    /// goes after the source.
    pub fn helper(mut self, code: impl Into<String>) -> Self {
        self.helpers.push(code.into());
        self
    }

    /// Uses another RavenRS crate, or turns features on in one already used.
    pub fn dependency(mut self, name: impl Into<String>, features: &[&str]) -> Self {
        let name = name.into();
        let features = features.iter().map(|feature| (*feature).to_owned());
        match self.dependencies.iter_mut().find(|dependency| dependency.name == name) {
            Some(dependency) => {
                for feature in features {
                    if !dependency.features.contains(&feature) {
                        dependency.features.push(feature);
                    }
                }
            }
            None => self.dependencies.push(Dependency { name, features: features.collect() }),
        }
        self
    }

    /// The records the app had, as `MemoryDatabase::export_json` wrote them. They go in
    /// `src/seed.json`, which `SEED` holds.
    pub fn records(mut self, json: impl Into<String>) -> Self {
        self.records = Some(json.into());
        self
    }

    /// Takes the RavenRS crates from a checkout of the repository instead of crates.io, by
    /// `crates/<name>` under `root`: for checking projects against code not published yet.
    pub fn local_crates(mut self, root: impl Into<PathBuf>) -> Self {
        self.local = Some(root.into());
        self
    }

    /// The name of the crate: the title in lower case, with `-` for what is not a letter or a
    /// digit.
    pub fn crate_name(&self) -> String {
        let mut name = String::new();
        for c in self.title.chars().flat_map(char::to_lowercase) {
            if c.is_ascii_alphanumeric() {
                name.push(c);
            } else if !name.is_empty() && !name.ends_with('-') {
                name.push('-');
            }
        }
        let name = name.trim_end_matches('-').to_owned();
        match name.chars().next() {
            Some(first) if first.is_ascii_alphabetic() => name,
            Some(_) => format!("app-{name}"),
            None => "app".to_owned(),
        }
    }

    /// The function the app starts from: the first one of the source.
    pub fn entry(&self) -> Result<String, String> {
        let file = syn::parse_file(&self.source).map_err(|e| {
            let at = e.span().start();
            format!("line {}, column {}: {e}", at.line, at.column + 1)
        })?;
        file.items
            .iter()
            .find_map(|item| match item {
                syn::Item::Fn(function) => Some(function.sig.ident.to_string()),
                _ => None,
            })
            .ok_or_else(|| {
                "there is no function to start from: write `fn example() -> impl View { .. }`".into()
            })
    }

    /// Every file of the project.
    pub fn files(&self) -> Result<Vec<ProjectFile>, String> {
        let entry = self.entry()?;
        let file = |path: &str, contents: String| ProjectFile { path: path.to_owned(), contents };
        let mut files = vec![
            file("Cargo.toml", self.manifest()),
            file("src/lib.rs", self.lib(&entry)),
            file("src/main.rs", self.main()),
            file("web/index.html", self.page()),
            file("README.md", self.readme()),
            file(".gitignore", "/target\n/web/pkg\n".into()),
        ];
        if let Some(records) = &self.records {
            files.push(file("src/seed.json", records.clone()));
        }
        Ok(files)
    }

    /// Every file of the project in one `.zip`, in a folder named after the crate.
    pub fn zip(&self) -> Result<Vec<u8>, String> {
        let folder = self.crate_name();
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            // There is no clock to read in the browser, and a fixed time keeps the file the same.
            .last_modified_time(zip::DateTime::default());
        for file in self.files()? {
            zip.start_file(format!("{folder}/{}", file.path), options).map_err(|e| e.to_string())?;
            zip.write_all(file.contents.as_bytes()).map_err(|e| e.to_string())?;
        }
        zip.finish().map(Cursor::into_inner).map_err(|e| e.to_string())
    }

    fn requirement(&self, dependency: &Dependency) -> String {
        let place = match &self.local {
            Some(root) => {
                let path = root.join("crates").join(&dependency.name);
                format!("path = {:?}", path.to_string_lossy())
            }
            None => format!("version = \"{RAVENRS_VERSION}\""),
        };
        if dependency.features.is_empty() {
            format!("{} = {{ {place} }}", dependency.name)
        } else {
            let features: Vec<String> = dependency.features.iter().map(|f| format!("{f:?}")).collect();
            format!("{} = {{ {place}, features = [{}] }}", dependency.name, features.join(", "))
        }
    }

    fn manifest(&self) -> String {
        let dependencies: Vec<String> = self.dependencies.iter().map(|d| self.requirement(d)).collect();
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
{dependencies}

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "={WASM_BINDGEN_VERSION}"
wasm-bindgen-futures = "0.4"
console_error_panic_hook = "0.1"
web-sys = {{ version = "0.3", features = ["console"] }}

[profile.release]
opt-level = "s"
lto = true
"#,
            name = self.crate_name(),
            dependencies = dependencies.join("\n"),
        )
    }

    fn lib(&self, entry: &str) -> String {
        let mut lib = format!(
            "//! {title}, made in the RavenRS playground.\n#![allow(unused_imports, dead_code)]\n\n{source}\n",
            title = self.title,
            source = self.source.trim_end(),
        );
        for helper in &self.helpers {
            lib.push('\n');
            lib.push_str(helper.trim_end());
            lib.push('\n');
        }
        lib.push_str(&format!(
            "\n/// What the window and the page show.\npub fn app() -> impl ravenrs::widgets::View {{\n    {entry}()\n}}\n"
        ));
        if self.records.is_some() {
            lib.push_str(
                "\n/// The records the app had in the playground, as `MemoryDatabase::export_json` wrote them:\n\
                 /// `MemoryDatabase::import_json(SEED)` puts them back.\n\
                 pub const SEED: &str = include_str!(\"seed.json\");\n",
            );
        }
        lib.push_str(WEB_ENTRY);
        lib
    }

    fn main(&self) -> String {
        format!(
            r#"//! The desktop: `cargo run`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {{
    let options = ravenrs::native::WindowOptions::new({title:?}).size(1000.0, 700.0);
    ravenrs::native::run({krate}::app(), options)
}}

// The browser starts from `lib.rs`.
#[cfg(target_arch = "wasm32")]
fn main() {{}}
"#,
            title = self.title,
            krate = self.crate_name().replace('-', "_"),
        )
    }

    fn page(&self) -> String {
        format!(
            r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{title}</title>
    <style>
      html, body {{ height: 100%; margin: 0; overflow: hidden; background: #0b0d12; }}
      #raven-canvas {{ display: block; width: 100vw; height: 100vh; }}
    </style>
  </head>
  <body>
    <canvas id="raven-canvas" aria-label="{title}"></canvas>
    <script type="module">
      import init from "./pkg/{krate}.js";
      init();
    </script>
  </body>
</html>
"#,
            title = escape_html(&self.title),
            krate = self.crate_name().replace('-', "_"),
        )
    }

    fn readme(&self) -> String {
        let krate = self.crate_name().replace('-', "_");
        format!(
            r"# {title}

Made in the RavenRS playground. The code is in `src/lib.rs`.

## On the desktop

```sh
cargo run
```

## In the browser

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version {WASM_BINDGEN_VERSION}
cargo build --release --lib --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir web/pkg target/wasm32-unknown-unknown/release/{krate}.wasm
python3 -m http.server -d web 8080
```

Then open http://localhost:8080.
",
            title = self.title,
        )
    }
}

const WEB_ENTRY: &str = r#"
#[cfg(target_arch = "wasm32")]
mod web_entry {
    use std::cell::RefCell;

    use wasm_bindgen::prelude::*;

    thread_local! {
        static APP: RefCell<Option<ravenrs::web::WebApp>> = const { RefCell::new(None) };
    }

    /// Runs when the page loads the module.
    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            match ravenrs::web::start(super::app(), ravenrs::web::WebOptions::new("raven-canvas")).await {
                Ok(app) => APP.with(|kept| *kept.borrow_mut() = Some(app)),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }
}
"#;

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
