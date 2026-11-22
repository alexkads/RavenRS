//! An app installed as a service of the system: started with the machine, started again when it
//! falls, stopped with a signal (#883).
//!
//! # Its role
//!
//! Most apps of `tools/` open a window; a server (Raven CI) runs with no one logged in watching it.
//! What runs it is the system's own manager of services, and each system has its own: systemd on
//! Linux, launchd on macOS, the Task Scheduler on Windows. A [`ServiceSpec`] says what to run once,
//! and this module writes the file each manager reads and asks it to start, stop or say how it is.
//!
//! # How it works
//!
//! - **systemd** ([`ServiceSpec::systemd_unit`]): a unit `<name>.service` in
//!   `~/.config/systemd/user` (or `/etc/systemd/system` for the whole system), with
//!   `Restart=on-failure`, `MemoryMax=` when a limit is given (the kernel stops the service past
//!   it), `KillSignal=SIGTERM` and a time to stop. It is enabled and started with
//!   `systemctl [--user] enable --now`, and restarted, so a new binary is taken. A service of a
//!   user runs only while the user is logged in unless `loginctl enable-linger` was run: the
//!   [`Installed`] says so.
//! - **launchd** ([`ServiceSpec::launchd_plist`]): a `<label>.plist` in `~/Library/LaunchAgents`
//!   (or `/Library/LaunchDaemons`), with `RunAtLoad`, `KeepAlive` when it falls, its output in the
//!   folder of logs and `ProcessType` `Background`. It is loaded again with `launchctl bootout` and
//!   `bootstrap`. **macOS enforces no limit of memory** on a service; the limit is said in the
//!   file as a comment and nothing more.
//! - **Windows** ([`ServiceSpec::windows_task_xml`]): a task of the Task Scheduler that starts at
//!   logon (or at boot, as SYSTEM, for the whole system), starts again a minute after it falls and
//!   has no time limit, made with `schtasks /Create /XML` and run at once. A service of the Service
//!   Control Manager needs the binary to speak to it, which is left for when it can be tested on
//!   Windows; **no limit of memory** is enforced there either.
//! - **[`ServiceManager`]** does the work: [`install`](ServiceManager::install),
//!   [`uninstall`](ServiceManager::uninstall) and [`status`](ServiceManager::status). Its folder
//!   and whether it runs the commands can be changed ([`ServiceManager::dry_run`]), so the three
//!   systems are tested on any machine.
//!
//! # How to use it
//!
//! ```
//! use std::path::PathBuf;
//! use ravenrs_app_update::{ServiceManager, ServiceSpec, ServiceSystem};
//!
//! let spec = ServiceSpec::new("raven-ci", "dev.ravenrs.raven-ci", "Raven CI", PathBuf::from("/opt/raven-ci"))
//!     .args(["servir", "--endereco", "0.0.0.0:8080"])
//!     .memory_max("1G")
//!     .logs(PathBuf::from("/var/log/raven-ci"));
//! assert!(spec.systemd_unit().contains("MemoryMax=1G"));
//! let plan = ServiceManager::new(ServiceSystem::Systemd)
//!     .dry_run(std::env::temp_dir().join("servicos-exemplo"))
//!     .install(&spec)
//!     .expect("planned");
//! assert!(plan.commands.iter().any(|c| c.join(" ").contains("enable --now raven-ci")));
//! # let _ = std::fs::remove_dir_all(std::env::temp_dir().join("servicos-exemplo"));
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

/// What a service runs, and how.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceSpec {
    /// The name of the service for systemd and the Task Scheduler: `raven-ci`.
    pub name: String,
    /// The label of launchd, in reverse DNS: `dev.ravenrs.raven-ci`.
    pub label: String,
    /// What people read: `Raven CI`.
    pub description: String,
    /// The binary.
    pub program: PathBuf,
    /// Its arguments.
    pub args: Vec<String>,
    /// Variables of its environment.
    pub env: Vec<(String, String)>,
    /// The most memory it may use, as systemd writes it: `512M`, `1G`. Enforced by systemd only.
    pub memory_max: Option<String>,
    /// Where launchd writes its output; systemd sends it to the journal.
    pub logs: Option<PathBuf>,
    /// For the whole system (root, an administrator) rather than for the user who installs it.
    pub system: bool,
}

impl ServiceSpec {
    /// The service `name` (`label` for launchd), described as `description`, running `program`.
    pub fn new(
        name: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
        program: PathBuf,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            description: description.into(),
            program,
            args: Vec::new(),
            env: Vec::new(),
            memory_max: None,
            logs: None,
            system: false,
        }
    }

    /// The service of `app`, running `program`: its package as the name, its bundle id as the
    /// label, and its output in `~/<home_dir>/logs`.
    pub fn for_app(app: &crate::InstalledApp, program: PathBuf) -> Self {
        Self::new(app.package, app.bundle_id, app.name, program).logs(app.home().join("logs"))
    }

    /// With `args`.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// With the variable `name` at `value`.
    pub fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((name.into(), value.into()));
        self
    }

    /// Using at most `limit` of memory (`1G`).
    pub fn memory_max(mut self, limit: impl Into<String>) -> Self {
        self.memory_max = Some(limit.into());
        self
    }

    /// Writing its output under `folder`.
    pub fn logs(mut self, folder: PathBuf) -> Self {
        self.logs = Some(folder);
        self
    }

    /// For the whole system.
    pub fn system(mut self, system: bool) -> Self {
        self.system = system;
        self
    }

    /// Whether `memory_max` is a number with `K`, `M`, `G` or `T` (or none), as systemd takes it.
    pub fn check(&self) -> Result<(), String> {
        let plain = |text: &str, what: &str| {
            if text.is_empty() || text.chars().any(|c| c.is_control()) {
                Err(format!("{what} vazio ou com caractere de controle"))
            } else {
                Ok(())
            }
        };
        plain(&self.name, "o nome")?;
        plain(&self.label, "o rótulo")?;
        if !self.name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) {
            return Err(format!("o nome {:?} tem que ser letras, dígitos, - _ ou .", self.name));
        }
        if let Some(limit) = &self.memory_max {
            let digits = limit.trim_end_matches(['K', 'M', 'G', 'T', 'k', 'm', 'g', 't']);
            if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!("o limite de memória {limit:?} é um número com K, M, G ou T: 512M, 1G"));
            }
        }
        for arg in std::iter::once(self.program.to_string_lossy().as_ref())
            .chain(self.args.iter().map(String::as_str))
        {
            if arg.chars().any(|c| matches!(c, '\n' | '\r' | '\0')) {
                return Err(format!("o argumento {arg:?} tem quebra de linha"));
            }
        }
        Ok(())
    }

    /// The unit of systemd.
    pub fn systemd_unit(&self) -> String {
        let command: Vec<String> = std::iter::once(self.program.to_string_lossy().into_owned())
            .chain(self.args.iter().cloned())
            .map(|arg| systemd_quote(&arg))
            .collect();
        let mut unit = format!(
            "# Feito por ravenrs-app-update: {}.\n[Unit]\nDescription={}\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nExecStart={}\nRestart=on-failure\nRestartSec=5\nKillSignal=SIGTERM\nTimeoutStopSec=30\n",
            self.description,
            self.description,
            command.join(" ")
        );
        for (name, value) in &self.env {
            unit.push_str(&format!("Environment={}\n", systemd_quote(&format!("{name}={value}"))));
        }
        if let Some(limit) = &self.memory_max {
            unit.push_str(&format!("MemoryMax={limit}\n"));
        }
        unit.push_str(&format!(
            "\n[Install]\nWantedBy={}\n",
            if self.system { "multi-user.target" } else { "default.target" }
        ));
        unit
    }

    /// The property list of launchd.
    pub fn launchd_plist(&self) -> String {
        let mut arguments = format!("\t\t<string>{}</string>\n", xml(&self.program.to_string_lossy()));
        for arg in &self.args {
            arguments.push_str(&format!("\t\t<string>{}</string>\n", xml(arg)));
        }
        let mut plist = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
        );
        plist.push_str(&format!("<!-- Feito por ravenrs-app-update: {}. -->\n", xml(&self.description)));
        if let Some(limit) = &self.memory_max {
            plist.push_str(&format!(
                "<!-- Limite de memória pedido: {}. O macOS não o aplica a um serviço. -->\n",
                xml(limit)
            ));
        }
        plist.push_str("<plist version=\"1.0\">\n<dict>\n");
        plist.push_str(&format!("\t<key>Label</key><string>{}</string>\n", xml(&self.label)));
        plist.push_str(&format!("\t<key>ProgramArguments</key>\n\t<array>\n{arguments}\t</array>\n"));
        plist.push_str("\t<key>RunAtLoad</key><true/>\n");
        plist
            .push_str("\t<key>KeepAlive</key>\n\t<dict>\n\t\t<key>SuccessfulExit</key><false/>\n\t</dict>\n");
        plist.push_str("\t<key>ProcessType</key><string>Background</string>\n");
        plist.push_str("\t<key>ExitTimeOut</key><integer>30</integer>\n");
        if !self.env.is_empty() {
            plist.push_str("\t<key>EnvironmentVariables</key>\n\t<dict>\n");
            for (name, value) in &self.env {
                plist.push_str(&format!("\t\t<key>{}</key><string>{}</string>\n", xml(name), xml(value)));
            }
            plist.push_str("\t</dict>\n");
        }
        if let Some(logs) = &self.logs {
            let out = logs.join(format!("{}.log", self.name));
            plist.push_str(&format!(
                "\t<key>StandardOutPath</key><string>{}</string>\n",
                xml(&out.to_string_lossy())
            ));
            plist.push_str(&format!(
                "\t<key>StandardErrorPath</key><string>{}</string>\n",
                xml(&out.to_string_lossy())
            ));
        }
        plist.push_str("</dict>\n</plist>\n");
        plist
    }

    /// The task of the Task Scheduler of Windows, as `schtasks /Create /XML` reads it.
    pub fn windows_task_xml(&self) -> String {
        let arguments: Vec<String> = self.args.iter().map(|arg| windows_quote(arg)).collect();
        let (trigger, principal) = if self.system {
            (
                "<BootTrigger><Enabled>true</Enabled></BootTrigger>",
                "<UserId>S-1-5-18</UserId><RunLevel>HighestAvailable</RunLevel>",
            )
        } else {
            (
                "<LogonTrigger><Enabled>true</Enabled></LogonTrigger>",
                "<LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel>",
            )
        };
        let mut memory = String::new();
        if let Some(limit) = &self.memory_max {
            memory =
                format!("  <!-- Limite de memória pedido: {}. O Agendador não o aplica. -->\n", xml(limit));
        }
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n<Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n{memory}  <RegistrationInfo><Description>{}</Description></RegistrationInfo>\n  <Triggers>{trigger}</Triggers>\n  <Principals><Principal id=\"Author\">{principal}</Principal></Principals>\n  <Settings>\n    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>\n    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>\n    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>\n    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>\n    <RestartOnFailure><Interval>PT1M</Interval><Count>999</Count></RestartOnFailure>\n    <StartWhenAvailable>true</StartWhenAvailable>\n    <Enabled>true</Enabled>\n  </Settings>\n  <Actions Context=\"Author\"><Exec><Command>{}</Command><Arguments>{}</Arguments></Exec></Actions>\n</Task>\n",
            xml(&self.description),
            xml(&self.program.to_string_lossy()),
            xml(&arguments.join(" "))
        )
    }
}

/// An argument of `ExecStart`, in double quotes, with what systemd reads in it escaped.
fn systemd_quote(arg: &str) -> String {
    let mut out = String::from("\"");
    for c in arg.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '%' => out.push_str("%%"),
            '$' => out.push_str("$$"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// An argument of a command line of Windows, quoted when it has spaces or quotes.
fn windows_quote(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_owned();
    }
    format!("\"{}\"", arg.replace('"', "\\\""))
}

/// Text inside XML.
fn xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The manager of services of a system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceSystem {
    /// systemd, on Linux.
    Systemd,
    /// launchd, on macOS.
    Launchd,
    /// The Task Scheduler of Windows.
    TaskScheduler,
}

impl ServiceSystem {
    /// The one of this machine, or `None` on a system with none of them.
    pub fn current() -> Option<Self> {
        if cfg!(target_os = "macos") {
            Some(Self::Launchd)
        } else if cfg!(windows) {
            Some(Self::TaskScheduler)
        } else if cfg!(target_os = "linux") {
            Some(Self::Systemd)
        } else {
            None
        }
    }

    /// Its name, for people.
    pub fn words(self) -> &'static str {
        match self {
            Self::Systemd => "systemd",
            Self::Launchd => "launchd",
            Self::TaskScheduler => "Agendador de Tarefas do Windows",
        }
    }
}

/// How a service is, as its manager says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceStatus {
    /// Its file is in place.
    pub installed: bool,
    /// It is running now.
    pub running: bool,
    /// Where its file is.
    pub file: PathBuf,
    /// What the manager said, as it said it.
    pub said: String,
}

/// What [`ServiceManager::install`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
    /// The file written.
    pub file: PathBuf,
    /// The commands run (or planned, in a dry run), each as its words.
    pub commands: Vec<Vec<String>>,
    /// What is left for people to know or do: a limit not enforced, a linger to switch on.
    pub notes: Vec<String>,
}

/// Writes the files of services and asks the system's manager to run them.
#[derive(Clone, Debug)]
pub struct ServiceManager {
    system: ServiceSystem,
    folder: Option<PathBuf>,
    dry_run: bool,
}

fn home() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

impl ServiceManager {
    /// The manager of `system`.
    pub fn new(system: ServiceSystem) -> Self {
        Self { system, folder: None, dry_run: false }
    }

    /// The manager of this machine, or `None` where there is none.
    pub fn current() -> Option<Self> {
        ServiceSystem::current().map(Self::new)
    }

    /// Writes the files in `folder` and only says the commands, running none: for tests, and to
    /// see what would be done.
    pub fn dry_run(mut self, folder: PathBuf) -> Self {
        self.folder = Some(folder);
        self.dry_run = true;
        self
    }

    /// Its system.
    pub fn system(&self) -> ServiceSystem {
        self.system
    }

    /// Where the file of `spec` goes.
    pub fn file_of(&self, spec: &ServiceSpec) -> PathBuf {
        let (folder, name) = match self.system {
            ServiceSystem::Systemd => (
                if spec.system {
                    PathBuf::from("/etc/systemd/system")
                } else {
                    home().join(".config/systemd/user")
                },
                format!("{}.service", spec.name),
            ),
            ServiceSystem::Launchd => (
                if spec.system {
                    PathBuf::from("/Library/LaunchDaemons")
                } else {
                    home().join("Library/LaunchAgents")
                },
                format!("{}.plist", spec.label),
            ),
            ServiceSystem::TaskScheduler => (
                spec.logs
                    .clone()
                    .and_then(|l| l.parent().map(Path::to_path_buf))
                    .unwrap_or_else(|| home().join(format!(".{}", spec.name))),
                format!("{}-tarefa.xml", spec.name),
            ),
        };
        self.folder.clone().unwrap_or(folder).join(name)
    }

    /// Writes the file of `spec`, and (re)starts it so the binary there now is the one that runs.
    pub fn install(&self, spec: &ServiceSpec) -> Result<Installed, String> {
        spec.check()?;
        let file = self.file_of(spec);
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        // A dry run writes only in its own folder.
        if let Some(logs) = spec.logs.as_ref().filter(|_| !self.dry_run) {
            std::fs::create_dir_all(logs).map_err(|e| format!("{}: {e}", logs.display()))?;
        }
        let mut notes = Vec::new();
        let path = file.to_string_lossy().into_owned();
        let commands: Vec<(Vec<String>, bool)> = match self.system {
            ServiceSystem::Systemd => {
                std::fs::write(&file, spec.systemd_unit()).map_err(|e| format!("{path}: {e}"))?;
                if !spec.system {
                    notes.push("um serviço de usuário só roda com você logado; para subir com a máquina: loginctl enable-linger".to_owned());
                }
                let base = systemctl(spec);
                vec![
                    (with(&base, &["daemon-reload"]), true),
                    (with(&base, &["enable", "--now", &spec.name]), true),
                    (with(&base, &["restart", &spec.name]), true),
                ]
            }
            ServiceSystem::Launchd => {
                std::fs::write(&file, spec.launchd_plist()).map_err(|e| format!("{path}: {e}"))?;
                if spec.memory_max.is_some() {
                    notes.push("o macOS não aplica limite de memória a um serviço".to_owned());
                }
                let domain = self.launchd_domain(spec)?;
                vec![
                    (words(&["launchctl", "bootout", &format!("{domain}/{}", spec.label)]), false),
                    (words(&["launchctl", "bootstrap", &domain, &path]), true),
                ]
            }
            ServiceSystem::TaskScheduler => {
                std::fs::write(&file, utf16(&spec.windows_task_xml())).map_err(|e| format!("{path}: {e}"))?;
                if spec.memory_max.is_some() {
                    notes.push("o Agendador do Windows não aplica limite de memória".to_owned());
                }
                vec![
                    (words(&["schtasks", "/End", "/TN", &spec.name]), false),
                    (words(&["schtasks", "/Create", "/TN", &spec.name, "/XML", &path, "/F"]), true),
                    (words(&["schtasks", "/Run", "/TN", &spec.name]), true),
                ]
            }
        };
        let ran = self.run_all(commands)?;
        Ok(Installed { file, commands: ran, notes })
    }

    /// Stops `spec` and removes its file.
    pub fn uninstall(&self, spec: &ServiceSpec) -> Result<Vec<Vec<String>>, String> {
        let file = self.file_of(spec);
        let commands: Vec<(Vec<String>, bool)> = match self.system {
            ServiceSystem::Systemd => {
                let base = systemctl(spec);
                vec![(with(&base, &["disable", "--now", &spec.name]), false)]
            }
            ServiceSystem::Launchd => {
                let domain = self.launchd_domain(spec)?;
                vec![(words(&["launchctl", "bootout", &format!("{domain}/{}", spec.label)]), false)]
            }
            ServiceSystem::TaskScheduler => vec![
                (words(&["schtasks", "/End", "/TN", &spec.name]), false),
                (words(&["schtasks", "/Delete", "/TN", &spec.name, "/F"]), false),
            ],
        };
        let mut ran = self.run_all(commands)?;
        if file.exists() {
            std::fs::remove_file(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        }
        if self.system == ServiceSystem::Systemd && !self.dry_run {
            let reload = with(&systemctl(spec), &["daemon-reload"]);
            let _ = run(&reload);
            ran.push(reload);
        }
        Ok(ran)
    }

    /// How `spec` is.
    pub fn status(&self, spec: &ServiceSpec) -> Result<ServiceStatus, String> {
        let file = self.file_of(spec);
        let installed = file.exists();
        let command = match self.system {
            ServiceSystem::Systemd => with(&systemctl(spec), &["is-active", &spec.name]),
            ServiceSystem::Launchd => {
                words(&["launchctl", "print", &format!("{}/{}", self.launchd_domain(spec)?, spec.label)])
            }
            ServiceSystem::TaskScheduler => {
                words(&["schtasks", "/Query", "/TN", &spec.name, "/FO", "LIST", "/V"])
            }
        };
        if self.dry_run {
            return Ok(ServiceStatus { installed, running: false, file, said: command.join(" ") });
        }
        let said = run(&command).map(|(_, out)| out).unwrap_or_else(|e| e);
        let running = match self.system {
            ServiceSystem::Systemd => said.trim() == "active",
            ServiceSystem::Launchd => said.lines().any(|line| line.trim() == "state = running"),
            ServiceSystem::TaskScheduler => said.lines().any(|line| {
                let line = line.trim();
                line.starts_with("Status:") && line.ends_with("Running")
            }),
        };
        Ok(ServiceStatus { installed, running, file, said })
    }

    /// `gui/<uid>` for a user, `system` for the whole system.
    fn launchd_domain(&self, spec: &ServiceSpec) -> Result<String, String> {
        if spec.system {
            return Ok("system".to_owned());
        }
        if self.dry_run {
            return Ok("gui/UID".to_owned());
        }
        let (_, uid) = run(&words(&["id", "-u"]))?;
        Ok(format!("gui/{}", uid.trim()))
    }

    /// Runs each command; one marked as needed that fails stops it, saying what it said.
    fn run_all(&self, commands: Vec<(Vec<String>, bool)>) -> Result<Vec<Vec<String>>, String> {
        let mut ran = Vec::new();
        for (command, needed) in commands {
            if !self.dry_run {
                match run(&command) {
                    Ok(_) => {}
                    Err(why) if needed => return Err(why),
                    Err(_) => {}
                }
            }
            ran.push(command);
        }
        Ok(ran)
    }
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|w| (*w).to_owned()).collect()
}

fn systemctl(spec: &ServiceSpec) -> Vec<String> {
    if spec.system { words(&["systemctl"]) } else { words(&["systemctl", "--user"]) }
}

fn with(base: &[String], more: &[&str]) -> Vec<String> {
    base.iter().cloned().chain(more.iter().map(|w| (*w).to_owned())).collect()
}

/// Runs `command`: its code and its output, or what it said when it failed.
fn run(command: &[String]) -> Result<(i32, String), String> {
    let (program, args) = command.split_first().ok_or("nenhum comando")?;
    let output = Command::new(program).args(args).output().map_err(|e| format!("{program}: {e}"))?;
    let text =
        format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    if output.status.success() {
        Ok((output.status.code().unwrap_or(0), text))
    } else {
        Err(format!("{} falhou: {}", command.join(" "), text.trim()))
    }
}

/// `text` as UTF-16 with its mark, as the Task Scheduler reads its files.
fn utf16(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> ServiceSpec {
        ServiceSpec::new(
            "raven-ci",
            "dev.ravenrs.raven-ci",
            "Raven CI",
            PathBuf::from("/opt/Raven CI/raven-ci"),
        )
        .args(["--dados", "/srv/ci 100%", "servir", "--endereco", "0.0.0.0:8080"])
        .env("RUST_LOG", "info")
        .memory_max("1G")
        .logs(PathBuf::from("/var/log/raven-ci"))
    }

    #[test]
    fn the_unit_of_systemd_has_the_limit_the_restart_and_the_command_quoted() {
        let unit = spec().systemd_unit();
        assert!(unit.contains("MemoryMax=1G\n"), "{unit}");
        assert!(unit.contains("Restart=on-failure\n") && unit.contains("KillSignal=SIGTERM\n"));
        assert!(
            unit.contains(r#"ExecStart="/opt/Raven CI/raven-ci" "--dados" "/srv/ci 100%%" "servir""#),
            "{unit}"
        );
        assert!(unit.contains("Environment=\"RUST_LOG=info\"\n"));
        assert!(unit.contains("WantedBy=default.target"));
        assert!(spec().system(true).systemd_unit().contains("WantedBy=multi-user.target"));
        assert!(!ServiceSpec::new("a", "b", "c", PathBuf::from("/x")).systemd_unit().contains("MemoryMax"));
    }

    #[test]
    fn the_plist_of_launchd_keeps_it_alive_and_says_the_limit_is_not_enforced() {
        let plist = spec().launchd_plist();
        assert!(plist.contains("<key>Label</key><string>dev.ravenrs.raven-ci</string>"));
        assert!(plist.contains("<string>/opt/Raven CI/raven-ci</string>"));
        assert!(plist.contains("<key>RunAtLoad</key><true/>"));
        assert!(plist.contains("<key>SuccessfulExit</key><false/>"));
        assert!(plist.contains("O macOS não o aplica"));
        assert!(plist.contains("/var/log/raven-ci/raven-ci.log"));
    }

    #[test]
    fn the_task_of_windows_starts_at_logon_or_boot_and_again_when_it_falls() {
        let task = spec().windows_task_xml();
        assert!(
            task.contains("<LogonTrigger>") && task.contains("<RestartOnFailure><Interval>PT1M</Interval>")
        );
        assert!(task.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(task.contains("<Arguments>--dados &quot;/srv/ci 100%&quot; servir"), "{task}");
        let system = spec().system(true).windows_task_xml();
        assert!(system.contains("<BootTrigger>") && system.contains("S-1-5-18"));
        assert_eq!(&utf16("<a")[..4], &[0xFF, 0xFE, b'<', 0]);
    }

    #[test]
    fn a_limit_or_a_name_that_does_not_fit_is_refused() {
        assert!(spec().check().is_ok());
        assert!(spec().memory_max("muito").check().is_err());
        assert!(ServiceSpec::new("com espaço", "l", "d", PathBuf::from("/x")).check().is_err());
        assert!(spec().args(["a\nb"]).check().is_err());
    }

    #[test]
    fn each_system_plans_its_commands_and_writes_its_file() {
        let folder = std::env::temp_dir().join(format!("ravenrs-servico-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let spec = spec().logs(folder.join("logs"));
        for (system, expected) in [
            (ServiceSystem::Systemd, "systemctl --user enable --now raven-ci"),
            (ServiceSystem::Launchd, "launchctl bootstrap gui/UID"),
            (ServiceSystem::TaskScheduler, "schtasks /Create /TN raven-ci /XML"),
        ] {
            let manager = ServiceManager::new(system).dry_run(folder.clone());
            let done = manager.install(&spec).expect("planned");
            assert!(done.file.exists(), "{}", done.file.display());
            let planned: Vec<String> = done.commands.iter().map(|c| c.join(" ")).collect();
            assert!(planned.iter().any(|c| c.starts_with(expected)), "{system:?}: {planned:?}");
            assert!(manager.status(&spec).expect("status").installed);
            manager.uninstall(&spec).expect("removed");
            assert!(!done.file.exists());
        }
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn plutil_takes_the_plist() {
        let file = std::env::temp_dir().join(format!("ravenrs-servico-{}.plist", std::process::id()));
        std::fs::write(&file, spec().launchd_plist()).expect("written");
        let checked = Command::new("plutil").arg("-lint").arg(&file).output().expect("plutil");
        let _ = std::fs::remove_file(&file);
        assert!(checked.status.success(), "{}", String::from_utf8_lossy(&checked.stdout));
    }
}
