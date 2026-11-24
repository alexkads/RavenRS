//! Backup and restore of the folder of data of Raven CI (O-04 of #803, #917).
//!
//! # Its role
//!
//! Moving the server to another machine, or coming back from a disk lost, without losing jobs,
//! builds with their logs and attachments, people, credentials or the data of the plugins. A
//! backup is made **with the server running**; a restore needs it stopped.
//!
//! # How it works
//!
//! - **[`create`]** copies each `.sqlite` of the folder of data with `VACUUM INTO`
//!   (`ravenrs_app_sqlite::Backups` over a connection of its own that only reads), which gives a
//!   consistent copy even with a build writing, checked and stamped with the application and the
//!   version. The other files are copied as they are. Left out: the folders of builds (`espacos`),
//!   the caches of plugins that come back by themselves (the repositories of `git`, the locks of
//!   `lock`), the files of SQLite's journal, the lock of the server, the backups themselves and the
//!   folders a restore kept. **The key of the vault** (`cofre.chave`) goes in only when asked
//!   (`with_key`): a backup with it opens every credential, so it is kept apart by default.
//! - **The manifest** `raven-ci-backup.json` says the version, when, whether the key is in it, and
//!   each file with its size and whether it is a database.
//! - **[`inspect`]** reads the manifest and checks each database by its stamp and `quick_check`.
//! - **[`restore`]** checks everything first, refuses while a server holds the folder
//!   ([`ServerLock`]), moves what is there to `antes-da-restauracao-<quando>/`, and only then puts
//!   the backup in place, the databases without their stamp. A backup with no key leaves the key
//!   that is there where it is.
//! - **[`ServerLock`]**: `raven-ci servir` holds `servidor.trava` while it runs, so a second server
//!   on the same folder, or a restore, is refused saying why.

use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use raven_ci_plugin_api::CiError;
use ravenrs_app_sqlite::{BackupStamp, SqliteBackups, unstamped_copy};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::credentials::KEY_FILE;
use crate::lifecycle::WORKSPACES_DIR;

/// The manifest of a backup.
pub const MANIFEST: &str = "raven-ci-backup.json";
/// Where `raven-ci backup criar` puts a backup when no folder is said.
pub const BACKUPS_DIR: &str = "backups";
/// The lock a server holds on its folder of data.
pub const LOCK_FILE: &str = "servidor.trava";
/// The start of the folder a restore keeps what was there in.
pub const BEFORE_RESTORE: &str = "antes-da-restauracao-";
/// The application in the stamp of each database.
const APPLICATION: &str = "Raven CI";

/// Paths of the folder of data a backup leaves out: caches that come back by themselves.
const LEFT_OUT: &[&str] = &["dados-dos-plugins/git/fontes", "dados-dos-plugins/lock/travas"];

/// One file of a backup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackedUp {
    /// Its path in the folder of data, with `/`.
    pub caminho: String,
    /// Its size in the backup, in bytes.
    pub tamanho: u64,
    /// Whether it is a database, copied with its stamp.
    pub banco: bool,
}

/// The manifest of a backup: what it holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Always `Raven CI`.
    pub aplicacao: String,
    /// The version that made it.
    pub versao: String,
    /// When, in milliseconds since 1970.
    pub feito_ms: u64,
    /// Whether the key of the vault is in it.
    pub com_chave: bool,
    /// Its files.
    pub arquivos: Vec<BackedUp>,
}

/// What a restore did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restored {
    /// The manifest of the backup put back.
    pub manifest: Manifest,
    /// Where what was there before went, when there was something.
    pub before: Option<PathBuf>,
    /// Whether the key that was there stayed, because the backup had none.
    pub kept_key: bool,
}

/// The lock a server holds on its folder of data while it runs: a second server, or a restore,
/// sees it and stops. It goes when this is dropped, or when the process ends however it ends.
#[derive(Debug)]
pub struct ServerLock {
    _file: File,
}

impl ServerLock {
    /// Takes the lock of `data`, or says another server holds it.
    pub fn take(data: &Path) -> Result<Self, CiError> {
        std::fs::create_dir_all(data).map_err(|e| failed(data, &e))?;
        let path = data.join(LOCK_FILE);
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|e| failed(&path, &e))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(CiError::unavailable(format!(
                "outro servidor do Raven CI já usa a pasta de dados {}: pare-o antes",
                data.display()
            ))),
            Err(TryLockError::Error(e)) => Err(failed(&path, &e)),
        }
    }
}

fn failed(path: &Path, e: &dyn std::fmt::Display) -> CiError {
    CiError::failed(format!("{}: {e}", path.display()))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
}

/// The path of `path` inside `root`, with `/`.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// The name of the folder of data a command makes in its user's home when it is not told where:
/// a folder of that name inside the folder of data is another one, and stays out (#952).
const NESTED_DATA: &str = ".raven-ci";

/// Whether the file or folder at `name` (relative, with `/`) stays out of a backup.
fn left_out(name: &str, with_key: bool) -> bool {
    let first = name.split('/').next().unwrap_or_default();
    let last = name.rsplit('/').next().unwrap_or_default();
    first == BACKUPS_DIR
        || first.starts_with(BEFORE_RESTORE)
        || name.split('/').any(|part| part == WORKSPACES_DIR)
        || name == LOCK_FILE
        // The key of the vault: the one at the root only with `--com-chave`, and one deeper never,
        // since it opens a folder of data that is not this one (#952).
        || (last == KEY_FILE && (name != KEY_FILE || !with_key))
        // A folder of data made inside this one by a command run with no `--dados` (#952).
        || name.split('/').any(|part| part == NESTED_DATA)
        || [".sqlite-wal", ".sqlite-shm", ".sqlite-journal"].iter().any(|end| last.ends_with(end))
        || LEFT_OUT.iter().any(|out| name == *out || name.starts_with(&format!("{out}/")))
}

/// Every file of `dir` a backup takes, by its path relative to `root`.
fn files(root: &Path, dir: &Path, with_key: bool, out: &mut Vec<PathBuf>) -> Result<(), CiError> {
    let mut entries: Vec<_> =
        std::fs::read_dir(dir).map_err(|e| failed(dir, &e))?.filter_map(Result::ok).collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if left_out(&relative(root, &path), with_key) {
            continue;
        }
        let kind = entry.file_type().map_err(|e| failed(&path, &e))?;
        if kind.is_dir() {
            files(root, &path, with_key, out)?;
        } else if kind.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn stamp(version: &str) -> BackupStamp {
    BackupStamp::new(APPLICATION).version(version)
}

/// Makes a backup of the folder of data `data` in the folder `target`, which must not exist or be
/// empty; `version` is the version of Raven CI. See the [module](self).
pub fn create(data: &Path, target: &Path, with_key: bool, version: &str) -> Result<Manifest, CiError> {
    if !data.join(crate::DATABASE_FILE).exists() {
        return Err(CiError::settings(
            "dados",
            format!(
                "{} não é uma pasta de dados do Raven CI: não tem {}",
                data.display(),
                crate::DATABASE_FILE
            ),
        ));
    }
    if target.exists() && std::fs::read_dir(target).map_err(|e| failed(target, &e))?.next().is_some() {
        return Err(CiError::settings(
            "pasta",
            format!("{} já tem arquivos: um backup vai numa pasta vazia", target.display()),
        ));
    }
    if with_key && !data.join(KEY_FILE).exists() {
        return Err(CiError::settings("com-chave", format!("não há {KEY_FILE} em {}", data.display())));
    }
    let mut found = Vec::new();
    files(data, data, with_key, &mut found)?;
    std::fs::create_dir_all(target).map_err(|e| failed(target, &e))?;
    let mut arquivos = Vec::new();
    for path in found {
        let name = relative(data, &path);
        let to = target.join(&name);
        if let Some(folder) = to.parent() {
            std::fs::create_dir_all(folder).map_err(|e| failed(folder, &e))?;
        }
        let banco = name.ends_with(".sqlite");
        if banco {
            let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|e| failed(&path, &e))?;
            Rc::new(connection).backups(stamp(version)).save(&to).map_err(|e| failed(&path, &e))?;
        } else {
            std::fs::copy(&path, &to).map_err(|e| failed(&path, &e))?;
            #[cfg(unix)]
            {
                // The key and the vault are only their owner's to read, in the backup too.
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&path).map_err(|e| failed(&path, &e))?.permissions().mode();
                std::fs::set_permissions(&to, std::fs::Permissions::from_mode(mode))
                    .map_err(|e| failed(&to, &e))?;
            }
        }
        let tamanho = std::fs::metadata(&to).map_err(|e| failed(&to, &e))?.len();
        arquivos.push(BackedUp { caminho: name, tamanho, banco });
    }
    let manifest = Manifest {
        aplicacao: APPLICATION.to_owned(),
        versao: version.to_owned(),
        feito_ms: now_ms(),
        com_chave: with_key,
        arquivos,
    };
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| CiError::failed(e.to_string()))?;
    std::fs::write(target.join(MANIFEST), text).map_err(|e| failed(target, &e))?;
    Ok(manifest)
}

/// The manifest of the backup in `folder`, with each of its files there and each database checked
/// by its stamp and its `quick_check`.
pub fn inspect(folder: &Path) -> Result<Manifest, CiError> {
    let not_one = || {
        CiError::settings(
            "pasta",
            format!("{} não é um backup do Raven CI: falta {MANIFEST}", folder.display()),
        )
    };
    let text = std::fs::read_to_string(folder.join(MANIFEST)).map_err(|_| not_one())?;
    let manifest: Manifest = serde_json::from_str(&text).map_err(|_| not_one())?;
    if manifest.aplicacao != APPLICATION {
        return Err(not_one());
    }
    // A connection of its own, empty: the stamp, the migrations (none) and the tables are checked.
    let empty = Rc::new(Connection::open_in_memory().map_err(|e| CiError::failed(e.to_string()))?);
    for file in &manifest.arquivos {
        let path = folder.join(&file.caminho);
        if !path.is_file() {
            return Err(CiError::settings(
                "pasta",
                format!("o backup diz {}, que não está lá", file.caminho),
            ));
        }
        if file.banco {
            empty
                .backups(stamp(&manifest.versao))
                .inspect(&path)
                .map_err(|e| CiError::settings("pasta", format!("{}: {e}", file.caminho)))?;
            let ok: String = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .and_then(|db| db.query_row("PRAGMA quick_check", [], |row| row.get(0)))
                .map_err(|e| CiError::settings("pasta", format!("{}: {e}", file.caminho)))?;
            if ok != "ok" {
                return Err(CiError::settings("pasta", format!("{}: danificado ({ok})", file.caminho)));
            }
        }
    }
    Ok(manifest)
}

/// Puts the backup in `folder` back as the folder of data `data`, which no server may be using.
/// See the [module](self).
pub fn restore(data: &Path, folder: &Path) -> Result<Restored, CiError> {
    let manifest = inspect(folder)?;
    let _lock = ServerLock::take(data).map_err(|_| {
        CiError::unavailable(format!(
            "um servidor do Raven CI está usando {}: pare-o (systemctl stop raven-ci) e restaure de novo",
            data.display()
        ))
    })?;
    // What is there now goes aside, all of it, before anything is put back.
    let mut there: Vec<_> = std::fs::read_dir(data)
        .map_err(|e| failed(data, &e))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            let name = relative(data, path);
            name != LOCK_FILE && name != BACKUPS_DIR && !name.starts_with(BEFORE_RESTORE)
        })
        .collect();
    there.sort();
    let kept_key = !manifest.com_chave && data.join(KEY_FILE).exists();
    let before = if there.is_empty() {
        None
    } else {
        let aside = data.join(format!("{BEFORE_RESTORE}{}", now_ms()));
        std::fs::create_dir_all(&aside).map_err(|e| failed(&aside, &e))?;
        for path in &there {
            let to = aside.join(path.file_name().unwrap_or_default());
            std::fs::rename(path, &to).map_err(|e| failed(path, &e))?;
        }
        if kept_key {
            std::fs::copy(aside.join(KEY_FILE), data.join(KEY_FILE)).map_err(|e| failed(&aside, &e))?;
        }
        Some(aside)
    };
    for file in &manifest.arquivos {
        let from = folder.join(&file.caminho);
        let to = data.join(&file.caminho);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| failed(parent, &e))?;
        }
        if file.banco {
            unstamped_copy(&from, &to).map_err(|e| failed(&from, &e))?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| failed(&from, &e))?;
        }
    }
    Ok(Restored { manifest, before, kept_key })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caches_journals_folders_of_builds_and_the_key_stay_out_unless_asked() {
        for name in [
            "espacos/site/1/x",
            "dados-dos-plugins/agent-local/agentes/a/dados/espacos/j/1/f",
            "dados-dos-plugins/git/fontes/abc/HEAD",
            "raven-ci.sqlite-wal",
            "servidor.trava",
            "backups/raven-ci-1/raven-ci.sqlite",
            "antes-da-restauracao-1/raven-ci.sqlite",
            "cofre.chave",
        ] {
            assert!(left_out(name, false), "{name}");
        }
        for name in
            ["raven-ci.sqlite", "usuarios.sqlite", "cofre/x.json", "dados-dos-plugins/junit/plugin.sqlite"]
        {
            assert!(!left_out(name, false), "{name}");
        }
        assert!(!left_out("cofre.chave", true));
    }

    #[test]
    fn a_key_deeper_than_the_root_and_a_folder_of_data_inside_never_go_even_with_the_key() {
        for with_key in [false, true] {
            for name in [
                "sub/cofre.chave",
                ".raven-ci",
                ".raven-ci/raven-ci.sqlite",
                ".raven-ci/cofre.chave",
                "dados-dos-plugins/x/.raven-ci/raven-ci.sqlite",
            ] {
                assert!(left_out(name, with_key), "{name}, com a chave: {with_key}");
            }
        }
        assert!(!left_out("cofre.chave", true), "the key of the root, when asked");
    }
}
