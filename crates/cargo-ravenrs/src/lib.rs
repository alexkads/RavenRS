//! Offline project generation and version-matched AI tooling for RavenRS.
mod ai;
mod mcp;
mod project;

pub use ai::configure_ai;
pub use mcp::doctor;
pub use project::{new_project, project_files};

/// Version of the generator executable.
pub const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Framework version shared by templates, skill and the embedded MCP server.
/// The CLI package may receive independent fixes without republishing the framework.
pub const VERSION: &str = ravenrs_project::RAVENRS_VERSION;
/// Skill embedded from the plugin in this release.
pub const SKILL: &str = include_str!("../../../plugins/ravenrs/skills/ravenrs/SKILL.md");
/// Toolchain shared by every template.
pub const TOOLCHAIN: &str = include_str!("../../../rust-toolchain.toml");

fn read(path: &std::path::Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn write(root: &std::path::Path, path: &str, contents: &str) -> Result<(), String> {
    let path = root.join(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, contents).map_err(|e| format!("{}: {e}", path.display()))
}

fn run(root: &std::path::Path, command: &str, args: &[&str]) -> Result<(), String> {
    let status = std::process::Command::new(command)
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|e| format!("{command}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{command} {}: {status}", args.join(" "))) }
}
