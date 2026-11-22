//! "Delegar ao Codex" in the window (#637): the agent works in a thread of its own, the window
//! keeps answering, the Studio's own report marks the blocks, "Parar" ends the agent, and with
//! no agent installed the button is off and says why. The agent is a fake CLI, a shell script
//! that answers as Codex does, so nothing is spent.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use raven_test_studio::{app_delegating, sample};
use ravenrs::prelude::*;
use ravenrs_agent_cli::{Agent, Cli};
use ravenrs_e2e::E2e;

/// A fake `codex` that answers `verdict` after `ticks` tenths of a second, saying something
/// each tenth so "Parar" is heard.
fn fake_codex(name: &str, verdict: &str, ticks: u32) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rts-fake-codex-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("codex");
    let answer = format!(
        r#"{{\"verdict\": \"{verdict}\", \"summary\": \"1 de 3 passos passaram\", \"failed_step\": 2, \"code\": \"E2E_NOT_FOUND\", \"cause\": \"o alvo #nao-existe não está na tela\", \"fix\": \"tools/raven-test-studio/src/sample.rs:89\"}}"#
    );
    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in *--version*) echo 'codex-cli 9.9.9'; exit 0;; esac\n\
         case \"$*\" in *--help*) printf 'Usage: codex exec [OPTIONS] [PROMPT]\\n  --json\\n  --cd <DIR>\\n  -s, --sandbox <SANDBOX_MODE>\\n  -m, --model <MODEL>\\n'; exit 0;; esac\n\
         cat >/dev/null\n\
         echo '{{\"type\":\"thread.started\",\"thread_id\":\"t-1\"}}'\n\
         i=0; while [ $i -lt {ticks} ]; do echo '{{\"type\":\"turn.started\"}}'; sleep 0.1; i=$((i+1)); done\n\
         echo '{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"{answer}\"}}}}'\n\
         echo '{{\"type\":\"turn.completed\",\"usage\":{{\"input_tokens\":10}}}}'\n"
    );
    std::fs::write(&file, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    file
}

fn window(agent: Cli) -> E2e {
    E2e::new(app_delegating(sample::broken_scenario(), agent), Theme::raven_dark())
        .viewport(Size::new(1440.0, 900.0), 1.0)
}

const LONG: Duration = Duration::from_secs(30);

#[cfg(unix)]
#[test]
fn the_agent_answers_without_holding_up_the_window_and_the_studio_confirms() {
    let codex = fake_codex("confirma", "failed", 8);
    let mut studio = window(Cli::new(Agent::Codex).program(&codex));
    studio.by_test_id("delegate").click().unwrap();
    studio.by_test_id("delegation").expect().to_mention("Delegado ao Codex").unwrap();
    // While the agent works, the window still takes clicks and answers at once.
    let started = Instant::now();
    studio.by_test_id("node:after").click().unwrap();
    assert!(started.elapsed() < Duration::from_millis(500), "{:?}", started.elapsed());
    studio.by_test_id("delegate").expect().to_be_disabled().unwrap();
    studio.by_test_id("delegation").wait_to_mention("confirmou", LONG).unwrap();
    studio.by_test_id("delegation").expect().to_mention("o alvo #nao-existe").unwrap();
    // The blocks are marked by the Studio's own run of the same scenario.
    studio.by_test_id("node:lost").expect().to_mention("E2E_NOT_FOUND").unwrap();
    studio.by_test_id("delegate").expect().to_be_enabled().unwrap();
    let _ = std::fs::remove_dir_all(codex.parent().unwrap());
}

#[cfg(unix)]
#[test]
fn an_agent_that_says_otherwise_is_not_taken_on_trust() {
    let codex = fake_codex("discorda", "passed", 1);
    let mut studio = window(Cli::new(Agent::Codex).program(&codex));
    studio.by_test_id("delegate").click().unwrap();
    studio.by_test_id("delegation").wait_to_mention("outro veredito", LONG).unwrap();
    studio.by_test_id("node:lost").expect().to_mention("E2E_NOT_FOUND").unwrap();
    let _ = std::fs::remove_dir_all(codex.parent().unwrap());
}

#[cfg(unix)]
#[test]
fn stop_ends_the_agent() {
    let codex = fake_codex("parar", "failed", 300);
    let mut studio = window(Cli::new(Agent::Codex).program(&codex));
    studio.by_test_id("delegate").click().unwrap();
    studio.by_test_id("stop").click().unwrap();
    let started = Instant::now();
    studio.by_test_id("delegation").wait_to_mention("parado", LONG).unwrap();
    assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
    let _ = std::fs::remove_dir_all(codex.parent().unwrap());
}

#[test]
fn with_no_agent_installed_the_button_is_off_and_says_why() {
    let mut studio = window(Cli::new(Agent::Codex).program("/nowhere/codex"));
    studio.by_test_id("delegate").expect().to_be_disabled().unwrap();
    studio.by_test_id("delegate-hint").expect().to_mention("não está instalado").unwrap();
}
