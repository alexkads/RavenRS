//! A server answering HTTP on a free port, and agents as programs.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use raven_ci_core::agents::AgentWatch;
use raven_ci_core::{Ci, Executors, JobSpec, agents, api};
use raven_ci_plugin_api::{BuildRequest, BuildStatus, CiPlugin};
use ravenrs_app_server::{ApiOptions, LiveChannels};
use ravenrs_process::{Running, Spec};

/// A folder of its own, removed when dropped.
pub struct Folder(pub PathBuf);

impl Folder {
    pub fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("raven-ci-agente-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        Self(dir)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A server of Raven CI answering HTTP, with the plugins of the tests and `executors` of its own.
pub struct Server {
    pub ci: Arc<Ci>,
    pub url: String,
    pub _dir: Folder,
    runtime: Option<tokio::runtime::Runtime>,
    watch: Option<AgentWatch>,
    executors: Option<Executors>,
    serving: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
    address: std::net::SocketAddr,
    channels: LiveChannels,
}

/// The HTTP of `ci`, whose data is in `dir`, on `listener`.
fn serve(
    ci: Arc<Ci>,
    dir: &Path,
    channels: LiveChannels,
    listener: tokio::net::TcpListener,
) -> impl std::future::Future<Output = std::io::Result<()>> + Send + 'static {
    let options = ApiOptions::new().workers(2).open(api::OPEN_PREFIX).open(agents::JOIN_PATH).live(channels);
    let users = api::users_file(dir);
    ravenrs_app_server::serve_with(listener, options, move || api::service(ci.clone(), &users))
}

impl Server {
    pub fn start(name: &str, executors: usize, grace: Duration) -> Self {
        Self::with(name, executors, grace, Vec::new())
    }

    pub fn with(name: &str, executors: usize, grace: Duration, extra: Vec<Arc<dyn CiPlugin>>) -> Self {
        let dir = Folder::new(&format!("{name}-servidor"));
        let ci = Arc::new(Ci::open(&dir.0).expect("a core"));
        ci.add_plugin(Arc::new(raven_ci_plugin_format_toml::TomlFormatPlugin));
        ci.add_plugin(Arc::new(raven_ci_plugin_shell::ShellPlugin::new()));
        ci.add_plugin(Arc::new(raven_ci_plugin_workspace::WorkspacePlugin::new()));
        for plugin in extra {
            ci.add_plugin(plugin);
        }
        ci.apply_plugins().expect("on");
        // The first person is the administrator; an agent can never be.
        api::create_user(&api::users_file(&dir.0), "ana", "uma-senha-de-teste", "ci-admin").expect("ana");
        ci.set_agent_grace(grace);
        let channels = LiveChannels::checked_every(Duration::from_millis(200));
        ci.set_agent_live(channels.clone());
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("a runtime");
        let listener = runtime.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).expect("a port");
        let address = listener.local_addr().expect("an address");
        let url = format!("http://{address}");
        ci.set_agent_server(&url);
        let serving = Some(runtime.spawn(serve(ci.clone(), &dir.0, channels.clone(), listener)));
        let watch = Some(ci.watch_agents(Duration::from_millis(100)));
        let executors = Some(Executors::start(ci.clone(), executors, Duration::from_millis(50)));
        Self { ci, url, _dir: dir, runtime: Some(runtime), watch, executors, serving, address, channels }
    }

    /// Stops answering HTTP for `pause`, as a network that fell, and answers again on the same port.
    #[allow(dead_code, reason = "only some tests stop it")]
    pub fn pause(&mut self, pause: Duration) {
        let Some(runtime) = &self.runtime else { return };
        if let Some(serving) = self.serving.take() {
            serving.abort();
            let _ = runtime.block_on(serving);
        }
        std::thread::sleep(pause);
        let listener =
            runtime.block_on(tokio::net::TcpListener::bind(self.address)).expect("the same port again");
        self.serving =
            Some(runtime.spawn(serve(self.ci.clone(), &self._dir.0, self.channels.clone(), listener)));
    }

    /// Keeps a job whose `.raven-ci.toml` is `pipeline`.
    pub fn job(&self, name: &str, pipeline: &str) {
        self.ci.save_job(JobSpec::new(name, "toml", pipeline)).expect("the job");
    }

    pub fn request(&self, job: &str) -> u64 {
        self.ci.request_build(BuildRequest::new(job, "teste")).expect("asked").number
    }

    /// Waits until the build ends, for up to `within`; its status.
    pub fn wait_end(&self, job: &str, number: u64, within: Duration) -> BuildStatus {
        let until = Instant::now() + within;
        loop {
            let status = self.ci.build(job, number).expect("the build").status;
            if status.ended() || Instant::now() > until {
                return status;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Waits until `what` is true, for up to `within`.
    pub fn wait(&self, within: Duration, what: impl Fn(&Ci) -> bool) -> bool {
        let until = Instant::now() + within;
        while Instant::now() < until {
            if what(&self.ci) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    /// The text of every line of the build's log.
    pub fn log(&self, job: &str, number: u64) -> Vec<String> {
        self.ci.build_log(job, number, 0).expect("the log").into_iter().map(|line| line.text).collect()
    }

    pub fn connected(&self, agent: &str) -> bool {
        self.ci.agents().expect("agents").iter().any(|a| a.name == agent && a.connected)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(watch) = self.watch.take() {
            watch.stop();
        }
        if let Some(executors) = self.executors.take() {
            executors.stop();
        }
        self.ci.stop_launched_agents();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_secs(2));
        }
    }
}

/// The agent's program, as the build made it.
pub fn program() -> &'static str {
    env!("CARGO_BIN_EXE_raven-ci-agent")
}

/// An agent running as a program; stopped when dropped.
pub struct Agent {
    pub running: Option<Running>,
}

impl Agent {
    pub fn start(
        server: &Server,
        name: &str,
        dir: &Path,
        labels: &str,
        token: Option<&str>,
        extra: &[&str],
    ) -> Self {
        let mut spec = Spec::new(program())
            .args(["--servidor", &server.url, "--nome", name, "--pasta"])
            .arg(dir.as_os_str())
            .args(["--rotulos", labels])
            .args(extra.iter().copied());
        if let Some(token) = token {
            spec = spec.env(raven_ci_agent::TOKEN_VAR, token);
        }
        Self { running: Some(spec.spawn().expect("the agent starts")) }
    }

    pub fn pid(&self) -> u32 {
        self.running.as_ref().expect("running").control().pid()
    }

    /// Kills it at once, as a machine that fell: with `KILL`, nothing it can do about it.
    pub fn kill(&mut self) {
        if let Some(running) = self.running.take() {
            kill_now(running.control().pid());
            running.control().stop();
        }
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        if let Some(running) = self.running.take() {
            running.control().stop();
        }
    }
}

#[cfg(unix)]
fn kill_now(pid: u32) {
    let _ = std::process::Command::new("kill").args(["-KILL", &pid.to_string()]).status();
}

#[cfg(windows)]
fn kill_now(pid: u32) {
    let _ = std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).status();
}

/// A step that waits `seconds`, on every system.
pub fn sleep_step(seconds: u32) -> String {
    if cfg!(windows) {
        format!("ping -n {} 127.0.0.1 > NUL", seconds + 1)
    } else {
        format!("sleep {seconds}")
    }
}
