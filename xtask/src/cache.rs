//! The compiler cache the checks put in front of rustc, so a new worktree does not compile again
//! what another one compiled. Each worktree keeps its own build directory; the cache takes the
//! checkout and build paths out of what it compiles and restores what another worktree compiled.
//!
//! - By default `ravenrs-xcache`, this repository's own (crates/ravenrs-xcache): libraries and
//!   proc macros. It is built first, and incremental compilation is turned off, since an
//!   incremental compile cannot be kept. At the end the cache is cut down to `RAVENRS_XCACHE_MAX`
//!   (20G), the results used longest ago going first.
//! - With `RAVENRS_XCACHE=0`, Kache when it is installed (`RAVENRS_KACHE=0` leaves it out too).
//! - A `RUSTC_WRAPPER` already set is kept as it is.

use std::path::PathBuf;
use std::process::Command;

use crate::run::Check;

/// Which cache the check runs with.
#[derive(Debug)]
pub enum Cache {
    /// None: `RUSTC_WRAPPER` was set already, or the caches were turned off.
    None,
    /// `ravenrs-xcache`, at this path.
    Xcache(PathBuf),
    /// Kache, from the `PATH`.
    Kache,
}

fn off(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == "0")
}

impl Cache {
    /// Builds and turns on the cache for every command of `check`.
    pub fn turn_on(check: &mut Check) -> Result<Cache, crate::run::Failed> {
        if std::env::var_os("RUSTC_WRAPPER").is_some() {
            return Ok(Cache::None);
        }
        if !off("RAVENRS_XCACHE") {
            check.run("cargo", &["build", "-q", "-p", "ravenrs-xcache", "--release"])?;
            let xcache =
                check.target.join("release").join(format!("ravenrs-xcache{}", std::env::consts::EXE_SUFFIX));
            check.set_env("RUSTC_WRAPPER", xcache.as_os_str());
            check.set_env("CARGO_INCREMENTAL", "0");
            let _ = Command::new(&xcache).args(["stats", "--reset"]).output();
            let version = Command::new(&xcache).arg("--version").output().ok();
            let version =
                version.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned()).unwrap_or_default();
            println!("compiler cache: {version} (crates/ravenrs-xcache)");
            return Ok(Cache::Xcache(xcache));
        }
        if !off("RAVENRS_KACHE")
            && Command::new("kache").arg("--version").output().is_ok_and(|o| o.status.success())
        {
            check.set_env("RUSTC_WRAPPER", "kache");
            println!("compiler cache: kache (RAVENRS_KACHE=0 turns it off)");
            return Ok(Cache::Kache);
        }
        Ok(Cache::None)
    }

    /// What the cache did in this check, and the cut that keeps it from taking the disk.
    pub fn report(&self, check: &Check) {
        match self {
            Cache::None => {}
            Cache::Kache => {
                println!("\nCompiler cache (kache stats --last-build):");
                let _ = check.command("kache", &["stats", "--last-build"]).status();
            }
            Cache::Xcache(xcache) => {
                println!();
                let _ = Command::new(xcache).arg("stats").status();
                // Every worktree fills the same cache.
                let max = std::env::var("RAVENRS_XCACHE_MAX").unwrap_or_else(|_| "20G".to_owned());
                let _ = Command::new(xcache).args(["gc", "--max-size", &max]).status();
            }
        }
    }
}
