//! `cargo xtask web <name> [cargo flags]`: builds an example, or a tool such as the feature center,
//! for the browser into `examples/<name>/web/pkg` or `tools/<name>/web/pkg`.
//!
//! A tool whose own package does not compile for the browser (a server) names, in `web/crate`, the
//! package that does: `cargo xtask web raven-ci` builds `raven-ci-painel` into
//! `tools/raven-ci/web/pkg`. Extra flags go to cargo:
//! `cargo xtask web raven-feature-center --no-default-features --features widgets,playground`.
//!
//! It needs `rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli
//! --version 0.2.130` (the version in Cargo.toml). With `wasm-opt` (binaryen) installed, the output
//! is shrunk further.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::run::{Check, Done, failure};

/// What rustc enables by default for wasm32-unknown-unknown; `--all-features` would emit encodings
/// browsers do not accept yet.
const WASM_FEATURES: &[&str] = &[
    "--enable-bulk-memory",
    "--enable-mutable-globals",
    "--enable-multivalue",
    "--enable-nontrapping-float-to-int",
    "--enable-reference-types",
    "--enable-sign-ext",
];

/// Builds `name` for the browser, with `flags` passed to cargo.
pub fn build(check: &Check, name: &str, flags: &[&str]) -> Done {
    let folder = if check.root.join("tools").join(name).join("web").is_dir() { "tools" } else { "examples" };
    let web = check.root.join(folder).join(name).join("web");
    let out = web.join("pkg");
    let package = fs::read_to_string(web.join("crate"))
        .map(|text| text.trim().to_owned())
        .unwrap_or_else(|_| name.to_owned());
    let crate_name = package.replace('-', "_");

    let mut args = vec![
        "build",
        "-p",
        &package,
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
        "--profile",
        "wasm-release",
    ];
    args.extend_from_slice(flags);
    check.run("cargo", &args)?;

    let wasm = check.target.join("wasm32-unknown-unknown/wasm-release").join(format!("{crate_name}.wasm"));
    let mut bindgen = check.command("wasm-bindgen", &["--target", "web", "--no-typescript", "--out-dir"]);
    bindgen.arg(&out).arg(&wasm);
    check.run_command(bindgen, "wasm-bindgen", &["--target", "web", "--no-typescript"])?;

    let built = out.join(format!("{crate_name}_bg.wasm"));
    if Command::new("wasm-opt").arg("--version").output().is_ok_and(|o| o.status.success()) {
        let mut opt = check.command("wasm-opt", &["-Oz"]);
        opt.args(WASM_FEATURES).arg("-o").arg(&built).arg(&built);
        check.run_command(opt, "wasm-opt", &["-Oz"])?;
    }
    let size = fs::metadata(&built).map_err(|e| failure(format!("{}: {e}", built.display())))?.len();
    println!("Built {} ({size} bytes)", out.display());
    println!(
        "Serve it with: python3 -m http.server -d {} 8080",
        Path::new(folder).join(name).join("web").display()
    );
    Ok(())
}
