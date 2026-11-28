use std::path::PathBuf;
use std::process::{Command, Output};

/// A folder of data of its own, removed at the end.
struct Data(PathBuf);

impl Data {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("raven-ci-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self(dir)
    }

    /// `raven-ci --dados <its folder> --plugins <the example plugins> <args>`.
    fn run(&self, args: &[&str]) -> Output {
        let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        Command::new(env!("CARGO_BIN_EXE_raven-ci"))
            .arg("--dados")
            .arg(&self.0)
            .arg("--plugins")
            .arg(examples)
            .args(args)
            .output()
            .expect("the binary runs")
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn listing_gives_each_plugin_and_its_state_as_json() {
    let data = Data::new("listar");
    let out = data.run(&["--format", "json", "plugins", "listar"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let list: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON");
    let hello =
        list.as_array().expect("a list").iter().find(|p| p["name"] == "hello").expect("the example plugin");
    assert_eq!(hello["version"], "0.1.0");
    assert_eq!(hello["state"]["state"], "on");
    assert_eq!(hello["points"], serde_json::json!(["step", "cli_command"]));
    assert_eq!(hello["memory"]["kind"], "not_running", "nothing called it yet");
}

#[test]
fn switching_hello_off_and_on_is_kept_in_the_folder_of_data() {
    let data = Data::new("ligar");
    let off = data.run(&["plugins", "desligar", "hello"]);
    assert!(text(&off).starts_with("hello: desligado\n"), "{}", text(&off));
    // It says it went to the folder, which a server already running reads only when it restarts (#946).
    assert!(text(&off).contains("use --servidor URL"), "{}", text(&off));
    let listed = text(&data.run(&["plugins", "listar"]));
    let line = listed.lines().find(|l| l.starts_with("hello")).expect("hello is listed");
    assert!(line.contains("desligado"), "{line}");
    let on = data.run(&["plugins", "ligar", "hello"]);
    assert!(text(&on).starts_with("hello: ligado\n"), "{}", text(&on));
}

#[test]
fn info_of_a_plugin_that_is_not_there_exits_with_2_saying_so() {
    let data = Data::new("info");
    let out = data.run(&["plugins", "info", "nao-existe"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(text(&out), "não há plugin com o nome nao-existe\n");
    let hello = text(&data.run(&["plugins", "info", "hello"]));
    assert!(hello.contains("origem:        externo, em "), "{hello}");
    assert!(hello.contains("permissões:    declaradas secrets; concedidas secrets"), "{hello}");
}

#[test]
fn a_command_of_a_plugin_runs_from_the_command_line() {
    let python = ravenrs_process::resolve(if cfg!(windows) { "python" } else { "python3" }).is_some();
    if !python {
        eprintln!("pulado: sem Python 3 nesta máquina");
        return;
    }
    let data = Data::new("hello");
    let out = data.run(&["hello", "Ana"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(text(&out), "Olá, Ana!\n");
    let missing = data.run(&["inventado"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(text(&missing).starts_with("não há o comando inventado"), "{}", text(&missing));
}

#[test]
fn a_plugin_refused_shows_why() {
    let data = Data::new("recusa");
    let broken = data.0.join("plugins/exige-outro");
    std::fs::create_dir_all(&broken).expect("folder");
    std::fs::write(
        broken.join("plugin.toml"),
        "name = \"exige-outro\"\nversion = \"1.0.0\"\ncommand = \"nada\"\n[requires]\nplugin-que-nao-existe = \"^2\"\n[[commands]]\nname = \"x\"\n",
    )
    .expect("written");
    let listed = text(&data.run(&["plugins", "listar"]));
    assert!(listed.contains("exige-outro"), "{listed}");
    assert!(listed.contains("  precisa de plugin-que-nao-existe, que não está instalado"), "{listed}");
}

#[test]
fn a_new_folder_of_data_is_said_and_a_home_that_is_one_is_used_with_no_dados() {
    let data = Data::new("pasta-nova");
    let first = data.run(&["plugins", "listar"]);
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    let said = String::from_utf8_lossy(&first.stderr).into_owned();
    assert!(said.contains(&format!("pasta de dados nova em {}", data.0.display())), "{said}");
    let again = data.run(&["plugins", "listar"]);
    assert!(!String::from_utf8_lossy(&again.stderr).contains("pasta de dados nova"), "only the first time");

    // The user of the RPM: its home is the folder of the service, and no `--dados` reads it (#952).
    let home = Command::new(env!("CARGO_BIN_EXE_raven-ci"))
        .env("HOME", &data.0)
        .env_remove("RAVEN_CI_HOME")
        .args(["plugins", "listar"])
        .output()
        .expect("the binary runs");
    assert!(home.status.success(), "{}", String::from_utf8_lossy(&home.stderr));
    assert!(!String::from_utf8_lossy(&home.stderr).contains("pasta de dados nova"), "the home is the folder");
    assert!(!data.0.join(".raven-ci").exists(), "no folder of data inside the folder of data");
}
