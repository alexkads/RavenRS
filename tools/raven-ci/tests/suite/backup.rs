//! `raven-ci backup criar|ver|restaurar` (#917), on the binary as a person runs it: a backup made
//! with the server running, the folder of data lost and put back with its jobs, builds, logs,
//! attachments, people and credentials; the key of the vault only when asked; and the refusals: a
//! server running, a folder that is not a backup, a database damaged, two servers on one folder.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

use raven_ci_core::{Ci, JobSpec};
use serde_json::{Value, json};

use crate::server::{Running, raven, start_with, token};

const SECRET: &str = "valor-do-cofre-917";

/// `usa` reads the credential `segredo` and leaves `relatorio.txt` when it holds the value.
fn jobs(dir: &Path) {
    let ci = Ci::open(dir).expect("a core");
    let settings = json!({ "triggers": [{ "id": "api" }] });
    let usa = format!(
        "artifacts = [\"relatorio.txt\"]\n\n[[stages]]\nname = \"Usa\"\nsteps = [{{ sh = 'test \"$X\" = {SECRET} && echo confere > relatorio.txt && echo linha-do-log', credentials = {{ X = \"segredo\" }} }}]\n"
    );
    ci.save_job(JobSpec::new("usa", "toml", usa).settings(settings.clone())).expect("usa");
    let lento = "[[stages]]\nname = \"Lento\"\nsteps = [\"for i in 1 2 3 4 5 6 7 8; do echo passo-$i; sleep 0.25; done\"]\n";
    ci.save_job(JobSpec::new("lento", "toml", lento).settings(settings)).expect("lento");
}

fn run(dir: &PathBuf, args: &[&str]) -> Output {
    raven(dir).args(args).env_remove("RAVEN_CI_SERVIDOR").output().expect("runs")
}

fn said(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn remote(running: &Running, key: &str, args: &[&str]) -> Output {
    raven(&running.dir)
        .args(args)
        .args(["--servidor", &format!("http://{}", running.address)])
        .env("RAVEN_CI_TOKEN", key)
        .env_remove("RAVEN_CI_SERVIDOR")
        .output()
        .expect("runs")
}

fn credential(running: &Running, key: &str) {
    use std::io::Write as _;
    let mut child = raven(&running.dir)
        .args(["credencial", "criar", "segredo", "--tipo", "token"])
        .args(["--servidor", &format!("http://{}", running.address)])
        .env("RAVEN_CI_TOKEN", key)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(json!({ "token": SECRET }).to_string().as_bytes())
        .expect("sent");
    let out = child.wait_with_output().expect("ends");
    assert_eq!(out.status.code(), Some(0), "{}", said(&out));
}

/// The server again on a folder that already has its people.
fn serve_again(dir: &Path) -> Running {
    let mut child = raven(&dir.to_path_buf())
        .args(["servir", "--endereco", "127.0.0.1:0"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("the server starts");
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout")).read_line(&mut line).expect("it says where");
    let address = line
        .split_whitespace()
        .find_map(|word| word.strip_prefix("http://"))
        .map(|at| at.trim_end_matches(',').to_owned())
        .unwrap_or_else(|| panic!("no address in {line:?}"));
    Running { child, address, dir: dir.to_owned() }
}

fn stop(running: &mut Running) {
    let _ = running.child.kill();
    let _ = running.child.wait();
}

#[test]
fn a_folder_of_data_lost_comes_back_from_a_backup_made_with_the_server_running() {
    let mut running = start_with("backup", jobs);
    let dir = running.dir.clone();
    let backups = std::env::temp_dir().join(format!("raven-ci-backups-917-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&backups);
    let key = token(&running, "ana");
    credential(&running, &key);
    for _ in 0..2 {
        let ran = remote(&running, &key, &["rodar", "usa", "--esperar"]);
        assert_eq!(ran.status.code(), Some(0), "{}", said(&ran));
    }

    // With the server running and a build writing its log: a backup with the key and one
    // without, each checked (`quick_check` in `ver`).
    let asked = remote(&running, &key, &["rodar", "lento"]);
    assert_eq!(asked.status.code(), Some(0), "{}", said(&asked));
    let full = backups.join("com-chave");
    let made =
        run(&dir, &["backup", "criar", &full.display().to_string(), "--com-chave", "--format", "json"]);
    assert_eq!(made.status.code(), Some(0), "{}", said(&made));
    let made: Value = serde_json::from_str(&said(&made)).expect("json");
    assert_eq!(made["manifesto"]["com_chave"], true, "{made}");
    assert!(full.join("cofre.chave").exists());
    let bare = backups.join("sem-chave");
    let made = run(&dir, &["backup", "criar", &bare.display().to_string()]);
    assert_eq!(made.status.code(), Some(0), "{}", said(&made));
    assert!(said(&made).contains("sem a chave do cofre"), "{}", said(&made));
    assert!(!bare.join("cofre.chave").exists(), "no key unless asked");
    let seen = run(&dir, &["backup", "ver", &bare.display().to_string(), "--format", "json"]);
    let seen: Value = serde_json::from_str(&said(&seen)).expect("json");
    assert_eq!(seen["manifesto"]["com_chave"], false, "{seen}");
    let files: Vec<&str> = seen["manifesto"]["arquivos"]
        .as_array()
        .expect("files")
        .iter()
        .filter_map(|f| f["caminho"].as_str())
        .collect();
    assert!(files.contains(&"raven-ci.sqlite") && files.contains(&"usuarios.sqlite"), "{files:?}");
    assert!(!files.iter().any(|f| f.starts_with("espacos") || f.ends_with("-wal")), "{files:?}");

    // Refused while the server runs, and nothing changes; a second server is refused too.
    let refused = run(&dir, &["backup", "restaurar", &full.display().to_string()]);
    assert_eq!(refused.status.code(), Some(1), "{}", said(&refused));
    assert!(said(&refused).contains("pare-o"), "{}", said(&refused));
    let second = run(&dir, &["servir", "--endereco", "127.0.0.1:0"]);
    assert_ne!(second.status.code(), Some(0));
    let both = format!("{}{}", said(&second), String::from_utf8_lossy(&second.stderr));
    assert!(both.contains("outro servidor do Raven CI já usa a pasta de dados"), "{both}");

    // The folder lost, and put back.
    stop(&mut running);
    std::fs::remove_dir_all(&dir).expect("lost");
    std::fs::create_dir_all(&dir).expect("an empty folder");
    let back = run(&dir, &["backup", "restaurar", &full.display().to_string()]);
    assert_eq!(back.status.code(), Some(0), "{}", said(&back));
    let mut running = serve_again(&dir);
    let key = token(&running, "ana");
    let status = remote(&running, &key, &["status", "usa", "2", "--format", "json"]);
    let status: Value = serde_json::from_str(&said(&status)).expect("json");
    assert_eq!(status["build"]["status"], "succeeded", "{status}");
    let artifact =
        status["attachments"].as_array().expect("attachments").iter().find(|a| a["name"] == "relatorio.txt");
    assert!(artifact.is_some(), "{status}");
    let log = said(&remote(&running, &key, &["log", "usa", "1"]));
    assert!(log.contains("linha-do-log"), "{log}");
    // The credential opens with the key that came back: a third build reads it.
    let ran = remote(&running, &key, &["rodar", "usa", "--esperar"]);
    assert_eq!(ran.status.code(), Some(0), "{}", said(&ran));

    // A restore over a folder that has things keeps them aside; without the key in the backup,
    // the key that is there stays.
    stop(&mut running);
    let again = run(&dir, &["backup", "restaurar", &bare.display().to_string(), "--format", "json"]);
    assert_eq!(again.status.code(), Some(0), "{}", said(&again));
    let again: Value = serde_json::from_str(&said(&again)).expect("json");
    let aside = PathBuf::from(again["restaurado"]["antes"].as_str().expect("aside"));
    assert!(aside.join("raven-ci.sqlite").exists() && aside.starts_with(&dir), "{again}");
    assert_eq!(again["restaurado"]["chave_mantida"], true, "{again}");
    assert!(dir.join("cofre.chave").exists());

    // Not a backup, and a database damaged: refused, nothing changes.
    let not_one = run(&dir, &["backup", "restaurar", &backups.display().to_string()]);
    assert_eq!(not_one.status.code(), Some(1));
    assert!(said(&not_one).contains("não é um backup do Raven CI"), "{}", said(&not_one));
    std::fs::write(bare.join("usuarios.sqlite"), b"isto nao e um banco").expect("damaged");
    let damaged = run(&dir, &["backup", "ver", &bare.display().to_string()]);
    assert_eq!(damaged.status.code(), Some(1), "{}", said(&damaged));
    let damaged = run(&dir, &["backup", "restaurar", &bare.display().to_string()]);
    assert_eq!(damaged.status.code(), Some(1));
    assert!(dir.join("raven-ci.sqlite").exists(), "a refused restore leaves the folder as it was");
    let _ = std::fs::remove_dir_all(&backups);
    drop(running);
}
