//! Lists the Rust sources and the guides of the workspace so the server can embed them. Both are
//! found by looking, so a new crate or document needs no change here.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

/// The folders whose packages are read: libraries, examples and the installable tools.
const PARENTS: [&str; 3] = ["crates", "examples", "tools"];

fn main() -> io::Result<()> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("../..").canonicalize()?;

    let mut files = Vec::new();
    for parent in PARENTS {
        for package in fs::read_dir(root.join(parent))? {
            let package = package?.path();
            if package.file_name().is_some_and(|name| name != "ravenrs-mcp") {
                println!("cargo:rerun-if-changed={}", package.join("src").display());
                collect(&package.join("src"), &mut files)?;
            }
        }
    }
    files.sort();

    // A new crate is not in the folders watched above, but it does change the workspace manifest.
    println!("cargo:rerun-if-changed={}", root.join("Cargo.toml").display());
    let guides = guides(&root)?;
    for guide in &guides {
        println!("cargo:rerun-if-changed={}", guide.display());
    }
    println!("cargo:rerun-if-changed={}", root.join("docs").display());

    let manifests = tool_manifests(&root)?;
    let skills = skills(&root)?;
    for file in manifests.iter().chain(&skills) {
        println!("cargo:rerun-if-changed={}", file.display());
    }
    println!("cargo:rerun-if-changed={}", root.join(".claude/skills").display());
    println!("cargo:rerun-if-changed={}", root.join("tools").display());

    let mut out = table("RUST_SOURCES", &root, &files)?;
    out.push_str(&table("GUIDES", &root, &guides)?);
    out.push_str(&table("TOOL_MANIFESTS", &root, &manifests)?);
    out.push_str(&table("SKILLS", &root, &skills)?);

    let out_dir = env::var_os("OUT_DIR").ok_or_else(|| io::Error::other("OUT_DIR is not set"))?;
    fs::write(Path::new(&out_dir).join("sources.rs"), out)
}

/// A static list of `(path from the root, contents)`.
fn table(name: &str, root: &Path, files: &[PathBuf]) -> io::Result<String> {
    let mut out = format!("pub static {name}: &[(&str, &str)] = &[\n");
    for file in files {
        let relative = file.strip_prefix(root).map_err(io::Error::other)?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        writeln!(out, "    ({relative:?}, include_str!({:?})),", file.display().to_string())
            .map_err(io::Error::other)?;
    }
    out.push_str("];\n");
    Ok(out)
}

/// The Markdown written for people: at the root, under `docs`, and beside each crate, example and tool.
fn guides(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut guides = Vec::new();
    for dir in [root.to_path_buf(), root.join("docs"), root.join("docs/design-system")] {
        markdown_in(&dir, &mut guides)?;
    }
    markdown_in(&root.join("tools"), &mut guides)?;
    for parent in PARENTS {
        for package in fs::read_dir(root.join(parent))? {
            markdown_in(&package?.path(), &mut guides)?;
        }
    }
    // The changelog is long and about versions, not about how things work.
    guides.retain(|path| path.file_name().is_none_or(|name| name != "CHANGELOG.md"));
    guides.sort();
    Ok(guides)
}

/// The `tool.json` of each tool in `tools/`: what the tool offers, kept true by its own tests.
fn tool_manifests(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for tool in fs::read_dir(root.join("tools"))? {
        let manifest = tool?.path().join("tool.json");
        if manifest.is_file() {
            found.push(manifest);
        }
    }
    found.sort();
    Ok(found)
}

/// Every skill in the repository: the project's (`.claude/skills`), the plugins' and the tools'
/// (`plugins/*/skills`, `tools/*/claude-pane/skills`), each a `SKILL.md` in a folder of its name.
fn skills(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut folders = vec![root.join(".claude/skills")];
    for (parent, inner) in [("plugins", "skills"), ("tools", "claude-pane/skills")] {
        let parent = root.join(parent);
        if parent.is_dir() {
            for entry in fs::read_dir(parent)? {
                folders.push(entry?.path().join(inner));
            }
        }
    }
    let mut found = Vec::new();
    for folder in folders.iter().filter(|folder| folder.is_dir()) {
        for skill in fs::read_dir(folder)? {
            let file = skill?.path().join("SKILL.md");
            if file.is_file() {
                found.push(file);
            }
        }
    }
    found.sort();
    Ok(found)
}

fn markdown_in(dir: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file() && path.extension().is_some_and(|extension| extension == "md") {
            found.push(path);
        }
    }
    Ok(())
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    Ok(())
}
