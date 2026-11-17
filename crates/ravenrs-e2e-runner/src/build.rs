//! Compiles an app before it opens its test channel. This layer reads Cargo's JSON output
//! and uses the shared process executor for the deadline and process-group cleanup; it does
//! not know the Studio, installed apps or how a project is discovered.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ravenrs_process::{End, Event, Spec};
use serde_json::Value;

use crate::session::Failure;

/// The app could not be compiled, or Cargo did not produce its executable.
pub const APP_BUILD: &str = "E2E_APP_BUILD";

/// How many diagnostics and stderr lines the failure keeps, independently of build size.
const KEPT: usize = 16;

/// Compiles `binary` of `package` in `root` and returns the executable Cargo reports, honoring
/// its target directory and configuration. The build gets `timeout`; an error preserves the
/// rendered Rust diagnostic, including source location and suggestions, in the failure detail.
pub fn compile(root: &Path, package: &str, binary: &str, timeout: Duration) -> Result<PathBuf, Box<Failure>> {
    let process = Spec::new("cargo")
        .args([
            "build",
            "--message-format=json-render-diagnostics",
            "--color",
            "never",
            "-p",
            package,
            "--bin",
            binary,
        ])
        .dir(root)
        .inherit_env()
        .timeout(timeout)
        .spawn()
        .map_err(|error| failure(format!("Não deu para compilar {package}/{binary}: {error}")))?;
    let mut output = Output::default();
    let mut ended = None;
    for event in process.events() {
        match event {
            Event::Out(line) => output.read(&line, binary),
            Event::Err(line) => keep(&mut output.errors, line),
            Event::Ended(end) => ended = Some(end),
        }
    }
    output.finish(ended, binary)
}

#[derive(Default)]
struct Output {
    executable: Option<PathBuf>,
    diagnostics: VecDeque<String>,
    errors: VecDeque<String>,
    failed: bool,
}

impl Output {
    fn read(&mut self, line: &str, binary: &str) {
        let Ok(message) = serde_json::from_str::<Value>(line) else { return };
        match message["reason"].as_str() {
            Some("compiler-message") if message["message"]["level"] == "error" => {
                self.failed = true;
                let diagnostic = &message["message"];
                let rendered = diagnostic["rendered"].as_str().or_else(|| diagnostic["message"].as_str());
                if let Some(rendered) = rendered {
                    keep(&mut self.diagnostics, rendered.to_owned());
                }
            }
            Some("compiler-artifact")
                if message["target"]["name"] == binary
                    && message["target"]["kind"]
                        .as_array()
                        .is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin")) =>
            {
                self.executable = message["executable"].as_str().map(PathBuf::from);
            }
            Some("build-finished") if message["success"] == false => self.failed = true,
            _ => {}
        }
    }

    fn finish(self, ended: Option<End>, binary: &str) -> Result<PathBuf, Box<Failure>> {
        if ended.is_some_and(End::success)
            && !self.failed
            && let Some(executable) = self.executable
        {
            return Ok(executable);
        }
        let reason = match ended {
            Some(End::TimedOut) => "a compilação passou do tempo-limite e foi encerrada".to_owned(),
            Some(End::Stopped) => "a compilação foi interrompida".to_owned(),
            Some(End::Code(0)) => format!("o Cargo não informou o executável {binary:?}"),
            Some(end) => format!("a compilação terminou com {end:?}"),
            None => "a compilação terminou sem informar seu resultado".to_owned(),
        };
        let detail = self.diagnostics.into_iter().chain(self.errors).collect::<Vec<_>>().join("\n");
        Err(failure(if detail.is_empty() { reason } else { format!("{reason}\n{detail}") }))
    }
}

fn keep(lines: &mut VecDeque<String>, line: String) {
    if lines.len() == KEPT {
        lines.pop_front();
    }
    lines.push_back(line);
}

fn failure(detail: String) -> Box<Failure> {
    Box::new(ravenrs_e2e_wire::Failure::of(APP_BUILD, detail).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_the_requested_binary_artifact_counts_and_a_failed_build_never_opens_it() {
        let mut output = Output::default();
        for (name, kind, path) in [
            ("other", "bin", "/other"),
            ("shop", "custom-build", "/build-script"),
            ("shop", "bin", "/custom-target/debug/shop"),
        ] {
            output.read(&json!({"reason":"compiler-artifact", "target":{"name":name,"kind":[kind]},"executable":path}).to_string(), "shop");
        }
        assert_eq!(output.executable, Some(PathBuf::from("/custom-target/debug/shop")));
        output.read(&json!({"reason":"build-finished","success":false}).to_string(), "shop");
        assert_eq!(output.finish(Some(End::Code(0)), "shop").unwrap_err().code.as_deref(), Some(APP_BUILD));
    }

    #[test]
    fn missing_artifacts_and_timeouts_cannot_pass_and_diagnostics_are_bounded() {
        let mut output = Output::default();
        output.read("not JSON", "shop");
        for i in 0..100 {
            output.read(&json!({"reason":"compiler-message", "message":{"level":"error", "rendered":format!("src/main.rs:2: {i}")}}).to_string(), "shop");
        }
        assert_eq!(output.diagnostics.len(), KEPT);
        let error = output.finish(Some(End::TimedOut), "shop").unwrap_err();
        assert!(error.detail.as_deref().unwrap().contains("tempo-limite"));
        assert!(error.detail.as_deref().unwrap().contains("src/main.rs:2: 99"));
        assert!(
            Output::default()
                .finish(Some(End::Code(0)), "shop")
                .unwrap_err()
                .detail
                .as_deref()
                .unwrap()
                .contains("não informou")
        );
    }
}
