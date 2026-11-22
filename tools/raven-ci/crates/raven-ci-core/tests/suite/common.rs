use std::path::PathBuf;
use std::sync::Arc;

use raven_ci_core::{Ci, CiOptions, MemorySecrets};
use raven_ci_plugin_api::Point;
use raven_ci_plugin_api::testing::Fake;
use ravenrs_flows::ManualClock;

/// A core in memory with a clock the test moves.
pub fn ci_at(clock: &ManualClock) -> Ci {
    Ci::new(CiOptions { clock: Arc::new(clock.clone()), ..CiOptions::default() }).expect("a core")
}

/// A core with the fake step and the fake format on, ready to run builds.
pub fn ready() -> Ci {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(Fake::new(Point::Step)));
    ci.add_plugin(Arc::new(Fake::new(Point::PipelineFormat)));
    ci.apply_plugins().expect("on");
    ci
}

/// A core whose data is in a folder of its own, removed when it is dropped.
pub struct OnDisk {
    pub dir: PathBuf,
}

impl OnDisk {
    pub fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("raven-ci-teste-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self { dir }
    }

    pub fn open(&self, secrets: MemorySecrets) -> Ci {
        Ci::new(CiOptions {
            data_dir: Some(self.dir.clone()),
            secrets: Arc::new(secrets),
            ..CiOptions::default()
        })
        .expect("a core on disk")
    }
}

impl Drop for OnDisk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The folder of the plugins of the tests, and the one with the example plugin.
pub fn test_plugins() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/plugins")
}

pub fn example_plugins() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
}

/// Whether Python 3 is here; the tests of external plugins say they skip when it is not.
pub fn python() -> bool {
    let found = ravenrs_process::resolve(if cfg!(windows) { "python" } else { "python3" }).is_some();
    if !found {
        eprintln!("pulado: sem Python 3 nesta máquina, os plugins externos de exemplo não rodam");
    }
    found
}
