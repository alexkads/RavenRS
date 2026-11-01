//! Lists the Rust sources of the workspace so the server can embed them.

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

    let mut out = String::from("static RUST_SOURCES: &[(&str, &str)] = &[\n");
    for file in &files {
        let relative = file.strip_prefix(&root).map_err(io::Error::other)?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        writeln!(out, "    ({relative:?}, include_str!({:?})),", file.display().to_string())
            .map_err(io::Error::other)?;
    }
    out.push_str("];\n");

    let out_dir = env::var_os("OUT_DIR").ok_or_else(|| io::Error::other("OUT_DIR is not set"))?;
    fs::write(Path::new(&out_dir).join("sources.rs"), out)
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
