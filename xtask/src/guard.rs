//! The shell check does not come back (#1068).
//!
//! The checks moved from `scripts/check.sh` and `scripts/build-web.sh` to `cargo xtask` (#1034),
//! and the Alex decided on 28/11/2026 that they never come back. This step fails when either file
//! is back, when `scripts/` gets a `*.sh` it did not have, or when a file of the repository cites
//! either script. Any citation counts: telling prose from a command to run is fragile, and the
//! history that may name them is short and listed below.

use std::fs;
use std::path::Path;

use crate::run::{Done, failure};

/// The scripts that were replaced, and the `cargo xtask` task that does their work now.
const REPLACED: [(&str, &str); 2] =
    [("check.sh", "cargo xtask dev | pr | release"), ("build-web.sh", "cargo xtask web <name>")];

/// The `*.sh` in `scripts/` when this step was written. A new one is not allowed: automation is Rust
/// (`cargo xtask`), so it runs the same on macOS, Linux and Windows.
const KNOWN_SCRIPTS: [&str; 5] = [
    "scripts/build-site.sh",
    "scripts/deploy-pages.sh",
    "scripts/deploy-vercel.sh",
    "scripts/perf-time.sh",
    "scripts/vm-linux.sh",
];

/// File names that are history: what each release changed, citations included.
fn is_history(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name == "CHANGELOG.md"
        || name == "historico-de-novidades.json"
        || (name.starts_with("novidades") && name.ends_with(".json"))
}

/// Lines allowed to cite the scripts: a file and a piece of the line. Keep it short, and only for
/// lines that tell what happened to the scripts, never for lines that tell someone to run them.
const ALLOWED: [(&str, &str); 6] = [
    // This step: its docs, messages and tests have to name what they forbid.
    ("xtask/src/guard.rs", ""),
    // The skill tells what the xtask replaced and that the script is gone.
    (".claude/skills/ravenrs-check/SKILL.md", "foi apagado"),
    (".claude/skills/ravenrs-check/SKILL.md", "o que o `scripts/build-web.sh` fazia"),
    (".claude/skills/ravenrs-check/SKILL.md", "mesma cobertura do antigo"),
    // The skill and the xtask's README describe this step.
    (".claude/skills/ravenrs-check/SKILL.md", "não volta"),
    ("xtask/README.md", "does not come back"),
];

/// A citation that is not allowed, as `path:line: text`.
fn citations(path: &str, text: &str) -> Vec<String> {
    if is_history(path) {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let Some((script, task)) = REPLACED.iter().find(|(script, _)| cites(line, script)) else { continue };
        let allowed = ALLOWED.iter().any(|(file, piece)| *file == path && line.contains(piece));
        if !allowed {
            found.push(format!("{path}:{}: cites {script}; use `{task}`: {}", number + 1, line.trim()));
        }
    }
    found
}

/// Whether `line` names `script` as a word of its own: `my-check.sh` is another script.
fn cites(line: &str, script: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    line.match_indices(script).any(|(at, _)| {
        let before = line[..at].chars().next_back().is_none_or(|c| !word(c));
        let after = line[at + script.len()..].chars().next().is_none_or(|c| !word(c));
        before && after
    })
}

/// The `*.sh` under `scripts/` that are not known, the replaced ones first.
fn new_scripts(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![root.join("scripts")];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else { continue };
            let relative = relative.to_string_lossy().replace('\\', "/");
            if relative.ends_with(".sh") && !KNOWN_SCRIPTS.contains(&relative.as_str()) {
                found.push(relative);
            }
        }
    }
    found.sort();
    found
}

/// What the step finds wrong in the repository at `root`, given its files (tracked and new).
pub fn problems(root: &Path, files: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    for script in new_scripts(root) {
        let name = script.rsplit('/').next().unwrap_or(&script);
        match REPLACED.iter().find(|(replaced, _)| *replaced == name) {
            Some((_, task)) => {
                problems.push(format!("{script} is back: it was replaced by `{task}` (#1034, #1068)"))
            }
            None => problems.push(format!(
                "{script} is a new shell script: automation is Rust, in `cargo xtask` (#1068)"
            )),
        }
    }
    for file in files {
        // Binary files and files gone from the disk have nothing to cite.
        let Ok(bytes) = fs::read(root.join(file)) else { continue };
        let Ok(text) = String::from_utf8(bytes) else { continue };
        problems.extend(citations(file, &text));
    }
    problems
}

/// The step: the files are what git tracks plus the new ones it does not ignore.
pub fn shell_check_stays_gone(root: &Path, listed: &str) -> Done {
    let files: Vec<String> = listed.split('\0').filter(|name| !name.is_empty()).map(str::to_owned).collect();
    let problems = problems(root, &files);
    if problems.is_empty() {
        return Ok(());
    }
    for problem in &problems {
        eprintln!("{problem}");
    }
    Err(failure(format!("{} citation(s) or script(s) of the old shell check (#1068)", problems.len())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A repository of its own under the system's temporary directory, removed when dropped.
    struct Repo(PathBuf);

    impl Repo {
        fn new(name: &str) -> Repo {
            let dir = std::env::temp_dir().join(format!("xtask-guard-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join("scripts")).ok();
            Repo(dir)
        }

        fn write(&self, path: &str, text: &str) -> String {
            let full = self.0.join(path);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).ok();
            }
            fs::write(&full, text).ok();
            path.to_owned()
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_shell_check_back_fails_and_names_the_xtask() {
        let repo = Repo::new("back");
        let file = repo.write("scripts/check.sh", "#!/bin/sh\n");
        let problems = problems(&repo.0, &[file]);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("scripts/check.sh is back"));
        assert!(problems[0].contains("cargo xtask dev | pr | release"));
    }

    #[test]
    fn a_new_shell_script_fails_and_the_known_ones_pass() {
        let repo = Repo::new("new");
        repo.write("scripts/deploy-pages.sh", "#!/bin/sh\n");
        repo.write("scripts/seo/run.sh", "#!/bin/sh\n");
        let problems = problems(&repo.0, &[]);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("scripts/seo/run.sh is a new shell script"));
    }

    #[test]
    fn a_new_citation_fails_with_its_file_and_line() {
        let repo = Repo::new("cite");
        let file = repo.write("docs/GUIA.md", "# Guia\n\nAntes do PR, rode ./scripts/check.sh\n");
        let web = repo.write("docs/WEB.md", "Use scripts/build-web.sh gallery\n");
        let problems = problems(&repo.0, &[file, web]);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(
            problems[0].starts_with("docs/GUIA.md:3: cites check.sh; use `cargo xtask dev | pr | release`")
        );
        assert!(problems[1].starts_with("docs/WEB.md:1: cites build-web.sh; use `cargo xtask web <name>`"));
    }

    #[test]
    fn history_passes() {
        let repo = Repo::new("history");
        let files = [
            repo.write("CHANGELOG.md", "- O scripts/check.sh roda o clippy\n"),
            repo.write("tools/x/novidades.json", "{\"t\": \"check.sh\"}\n"),
            repo.write("tools/x/novidades-0.1.json", "{\"t\": \"check.sh\"}\n"),
            repo.write("tools/x/historico-de-novidades.json", "{\"t\": \"build-web.sh\"}\n"),
        ];
        assert!(problems(&repo.0, &files).is_empty());
    }

    #[test]
    fn the_allowed_lines_pass_and_only_them() {
        let skill = ".claude/skills/ravenrs-check/SKILL.md";
        assert!(citations(skill, "O `scripts/check.sh` foi apagado.").is_empty());
        assert_eq!(citations(skill, "Rode o `scripts/check.sh`.").len(), 1);
    }

    #[test]
    fn another_script_with_a_like_name_is_not_a_citation() {
        assert!(!cites("rode o pre-check.sh", "check.sh"));
        assert!(!cites("check.shell", "check.sh"));
        assert!(cites("(scripts/check.sh)", "check.sh"));
        assert!(cites("check.sh", "check.sh"));
    }
}
