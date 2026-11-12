//! Lists the Rust sources and the guides of the workspace so the server can embed them. Both are
//! found by looking, so a new crate or document needs no change here.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

fn main() -> io::Result<()> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("../..").canonicalize()?;

    let mut files = Vec::new();
    for parent in ["crates", "examples"] {
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

    let mut out = table("RUST_SOURCES", &root, &files)?;
    out.push_str(&table("GUIDES", &root, &guides)?);

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

/// The Markdown written for people: at the root, under `docs`, and beside each crate and example.
fn guides(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut guides = Vec::new();
    for dir in [root.to_path_buf(), root.join("docs"), root.join("docs/design-system")] {
        markdown_in(&dir, &mut guides)?;
    }
    for parent in ["crates", "examples"] {
        for package in fs::read_dir(root.join(parent))? {
            markdown_in(&package?.path(), &mut guides)?;
        }
    }
    // The changelog is long and about versions, not about how things work.
    guides.retain(|path| path.file_name().is_none_or(|name| name != "CHANGELOG.md"));
    guides.sort();
    Ok(guides)
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
