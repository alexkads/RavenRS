//! Backups and the copies the app keeps by itself.
//!
//! A backup is a file the person keeps somewhere else. A copy is what the app
//! keeps by itself beside the database: one a day, and one before anything
//! replaces the data, so every replacement can be undone.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ravenrs::prelude::Date;
use serde::{Deserialize, Serialize};

use crate::db::{BackupContents, Database};

/// How many copies stay in the folder; the oldest go first.
pub const COPIES_KEPT: usize = 14;

/// After this many days without a backup, the settings page says so.
pub const BACKUP_IS_OLD_AFTER_DAYS: u64 = 30;

/// Why a copy was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyReason {
    /// The one made when the app opens, once a day.
    Daily,
    /// Before a backup replaced the data.
    BeforeRestore,
    /// Before the data was erased.
    BeforeErase,
    /// Before the sample data replaced the data.
    BeforeSample,
}

impl CopyReason {
    const ALL: [Self; 4] = [Self::Daily, Self::BeforeRestore, Self::BeforeErase, Self::BeforeSample];

    fn slug(self) -> &'static str {
        match self {
            Self::Daily => "diaria",
            Self::BeforeRestore => "antes-de-restaurar",
            Self::BeforeErase => "antes-de-apagar",
            Self::BeforeSample => "antes-do-exemplo",
        }
    }

    /// How the settings page words it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Daily => "Cópia do dia",
            Self::BeforeRestore => "Antes de restaurar um backup",
            Self::BeforeErase => "Antes de apagar tudo",
            Self::BeforeSample => "Antes dos dados de exemplo",
        }
    }
}

/// One copy in the folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptCopy {
    /// The file.
    pub path: PathBuf,
    /// Why it was made.
    pub reason: CopyReason,
    /// When, already worded.
    pub when: String,
}

/// The backup made last.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastBackup {
    /// Where it went.
    pub path: PathBuf,
    /// When, in seconds since 1970.
    pub seconds: u64,
}

/// A moment the way the app shows it, in this computer's time.
pub fn moment_label(moment: SystemTime) -> String {
    jiff::Timestamp::try_from(moment).map_or_else(
        |_| "data desconhecida".to_owned(),
        |stamp| stamp.to_zoned(jiff::tz::TimeZone::system()).strftime("%d/%m/%Y às %H:%M").to_string(),
    )
}

/// Where the copies of the database at `database` stay.
pub fn copies_folder(database: &Path) -> PathBuf {
    database.parent().unwrap_or_else(|| Path::new(".")).join("copias")
}

fn file_name(today: Date, reason: CopyReason, attempt: u32) -> String {
    let day = format!("{:04}-{:02}-{:02}", today.year(), today.month(), today.day());
    match attempt {
        0 => format!("financas-{day}-{}.db", reason.slug()),
        n => format!("financas-{day}-{}-{}.db", reason.slug(), n + 1),
    }
}

fn reason_of(name: &str) -> Option<CopyReason> {
    let stem = name.strip_prefix("financas-")?.strip_suffix(".db")?;
    // After the date, whose form is fixed, comes the reason and maybe a number.
    let rest = stem.get(11..)?;
    CopyReason::ALL.into_iter().find(|reason| {
        rest.strip_prefix(reason.slug()).is_some_and(|end| {
            end.is_empty() || end.strip_prefix('-').is_some_and(|n| n.parse::<u32>().is_ok())
        })
    })
}

/// Keeps a copy of the data, then removes the oldest beyond [`COPIES_KEPT`].
pub fn keep_copy(db: &Database, folder: &Path, today: Date, reason: CopyReason) -> Result<PathBuf, String> {
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let target = (0..)
        .map(|attempt| folder.join(file_name(today, reason, attempt)))
        .find(|path| !path.exists())
        .unwrap_or_else(|| folder.join("financas.db"));
    db.backup_to(&target).map_err(|e| e.to_string())?;
    prune(folder);
    Ok(target)
}

/// The copy of the day, if it was not made yet.
pub fn keep_daily_copy(db: &Database, folder: &Path, today: Date) -> Result<(), String> {
    if folder.join(file_name(today, CopyReason::Daily, 0)).exists() {
        return Ok(());
    }
    keep_copy(db, folder, today, CopyReason::Daily).map(|_| ())
}

fn files_newest_first(folder: &Path) -> Vec<(PathBuf, CopyReason, SystemTime)> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut files: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let reason = reason_of(entry.file_name().to_str()?)?;
            Some((entry.path(), reason, entry.metadata().ok()?.modified().ok()?))
        })
        .collect();
    files.sort_by_key(|(path, _, when)| std::cmp::Reverse((*when, path.clone())));
    files
}

/// The copies there are, newest first.
pub fn copies(folder: &Path) -> Vec<KeptCopy> {
    files_newest_first(folder)
        .into_iter()
        .map(|(path, reason, when)| KeptCopy { path, reason, when: moment_label(when) })
        .collect()
}

fn prune(folder: &Path) {
    for (path, _, _) in files_newest_first(folder).into_iter().skip(COPIES_KEPT) {
        if let Err(e) = std::fs::remove_file(&path) {
            tracing::warn!(error = %e, path = %path.display(), "could not remove an old copy");
        }
    }
}

fn memory_file(database: &Path) -> PathBuf {
    database.parent().unwrap_or_else(|| Path::new(".")).join("ultimo-backup.json")
}

/// The backup made last, if the app knows of one.
pub fn last_backup(database: &Path) -> Option<LastBackup> {
    serde_json::from_slice(&std::fs::read(memory_file(database)).ok()?).ok()
}

/// How many whole days ago a backup was made.
pub fn days_since(seconds: u64, now: SystemTime) -> u64 {
    let now = now.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs());
    now.saturating_sub(seconds) / 86_400
}

/// Writes a backup and proves it reads back. It goes to a file beside the target
/// and moves over it at the end, so a failure leaves what was there as it was.
pub fn save_backup(
    db: &Database,
    database: Option<&Path>,
    target: &Path,
    now: SystemTime,
) -> Result<BackupContents, String> {
    if database == Some(target) {
        return Err("Esse é o próprio banco em uso. Escolha outro arquivo.".to_owned());
    }
    let draft = target.with_extension("db.parcial");
    // Left by a backup that was interrupted; `backup_to` refuses to write over anything.
    let _ = std::fs::remove_file(&draft);
    db.backup_to(&draft).map_err(|e| e.to_string())?;
    let contents = Database::inspect_backup(&draft).map_err(|e| {
        let _ = std::fs::remove_file(&draft);
        e.to_string()
    })?;
    std::fs::rename(&draft, target).map_err(|e| e.to_string())?;
    if let Some(database) = database {
        let seconds = now.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs());
        let memory = LastBackup { path: target.to_owned(), seconds };
        let written = serde_json::to_vec(&memory)
            .map_err(|e| e.to_string())
            .and_then(|text| std::fs::write(memory_file(database), text).map_err(|e| e.to_string()));
        if let Err(e) = written {
            tracing::warn!(error = %e, "could not remember the backup");
        }
    }
    Ok(contents)
}

/// Replaces the data with what is in `backup`, keeping a copy of it first. If the
/// copy cannot be made, nothing is replaced. Returns what came in and where the copy went.
pub fn restore(
    db: &Database,
    database: Option<&Path>,
    backup: &Path,
    today: Date,
) -> Result<(BackupContents, Option<PathBuf>), String> {
    if database == Some(backup) {
        return Err("Esse é o próprio banco em uso. Escolha um arquivo de backup.".to_owned());
    }
    let contents = Database::inspect_backup(backup).map_err(|e| e.to_string())?;
    let copy = database
        .map(|database| keep_copy(db, &copies_folder(database), today, CopyReason::BeforeRestore))
        .transpose()?;
    db.restore_from(backup).map_err(|e| e.to_string())?;
    Ok((contents, copy))
}

/// The whole of what a backup holds, in a sentence.
pub fn contents_label(contents: &BackupContents) -> String {
    let count = |n: i64, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    format!(
        "{}, {} e {}",
        count(contents.transactions, "lançamento", "lançamentos"),
        count(contents.bills, "conta a pagar", "contas a pagar"),
        count(contents.accounts, "conta", "contas"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(n: u8) -> Date {
        Date::new(2026, 11, n).expect("valid date")
    }

    fn folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("family-finance-backup-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("folder");
        folder
    }

    #[test]
    fn the_name_of_a_copy_says_why_it_was_made() {
        for reason in CopyReason::ALL {
            assert_eq!(reason_of(&file_name(day(1), reason, 0)), Some(reason));
            assert_eq!(reason_of(&file_name(day(1), reason, 3)), Some(reason));
        }
        assert_eq!(reason_of("financas.db"), None);
        assert_eq!(reason_of("financas-2026-11-01-outra-coisa.db"), None);
        assert_eq!(reason_of("foto.png"), None);
    }

    #[test]
    fn one_daily_copy_a_day_and_the_oldest_go() {
        let at = folder("daily");
        let db = Database::in_memory().expect("database");
        keep_daily_copy(&db, &at, day(1)).expect("copy");
        keep_daily_copy(&db, &at, day(1)).expect("again");
        assert_eq!(copies(&at).len(), 1);
        keep_daily_copy(&db, &at, day(2)).expect("next day");
        assert_eq!(copies(&at).len(), 2);

        for _ in 0..COPIES_KEPT + 3 {
            keep_copy(&db, &at, day(3), CopyReason::BeforeErase).expect("copy");
        }
        assert_eq!(copies(&at).len(), COPIES_KEPT);
        std::fs::remove_dir_all(&at).expect("clean up");
    }

    #[test]
    fn a_backup_is_remembered_and_never_goes_over_the_database_in_use() {
        let at = folder("save");
        let database = at.join("financas.db");
        let db = Database::open(&database).expect("database");
        let now = SystemTime::now();
        assert_eq!(last_backup(&database), None);
        assert!(save_backup(&db, Some(&database), &database, now).is_err());

        let target = at.join("meu-backup.db");
        save_backup(&db, Some(&database), &target, now).expect("backup");
        // Over one that exists, which the system's dialog has already asked about.
        save_backup(&db, Some(&database), &target, now).expect("again");
        assert_eq!(last_backup(&database).expect("remembered").path, target);
        assert!(!at.join("meu-backup.db.parcial").exists());
        drop(db);
        std::fs::remove_dir_all(&at).expect("clean up");
    }

    #[test]
    fn restoring_keeps_a_copy_that_brings_everything_back() {
        let at = folder("restore");
        let database = at.join("financas.db");
        let db = Database::open(&database).expect("database");
        db.add_account("Antes", crate::model::AccountKind::Cash, 1_00).expect("account");
        let target = at.join("backup.db");
        save_backup(&db, Some(&database), &target, SystemTime::now()).expect("backup");

        db.add_account("Depois", crate::model::AccountKind::Cash, 2_00).expect("account");
        let (contents, copy) = restore(&db, Some(&database), &target, day(1)).expect("restore");
        assert_eq!(contents.accounts, 1);
        assert_eq!(db.accounts().expect("accounts").len(), 1);

        let copy = copy.expect("a copy was kept");
        assert_eq!(copies(&copies_folder(&database))[0].reason, CopyReason::BeforeRestore);
        restore(&db, Some(&database), &copy, day(1)).expect("undo");
        assert_eq!(db.accounts().expect("accounts").len(), 2, "going back works the same way");
        drop(db);
        std::fs::remove_dir_all(&at).expect("clean up");
    }

    #[test]
    fn a_bad_file_touches_nothing_and_leaves_no_copy() {
        let at = folder("bad");
        let database = at.join("financas.db");
        let db = Database::open(&database).expect("database");
        db.add_account("Banco", crate::model::AccountKind::Cash, 1_00).expect("account");
        let bad = at.join("foto.db");
        std::fs::write(&bad, b"not a database at all, just some text").expect("file");
        assert!(restore(&db, Some(&database), &bad, day(1)).is_err());
        assert!(restore(&db, Some(&database), &database, day(1)).is_err(), "nor the file in use");
        assert_eq!(db.accounts().expect("accounts").len(), 1);
        assert!(copies(&copies_folder(&database)).is_empty());
        drop(db);
        std::fs::remove_dir_all(&at).expect("clean up");
    }

    #[test]
    fn the_sentence_counts_with_the_right_words() {
        let contents = BackupContents { version: 2, transactions: 1, bills: 2, accounts: 1, last_date: None };
        assert_eq!(contents_label(&contents), "1 lançamento, 2 contas a pagar e 1 conta");
    }

    #[test]
    fn days_since_a_backup() {
        let start = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(10 * 86_400);
        assert_eq!(days_since(0, start), 10);
        assert_eq!(days_since(10 * 86_400 + 5, start), 0, "a clock that went back");
    }
}
