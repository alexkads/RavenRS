//! Tells about bills through the system's notifications, even with the window closed
//! behind the tray, when the UI does not run. So it works on its own thread, with its
//! own connection to the database and what it needs in a file beside it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ravenrs::native::SystemNotifier;
use ravenrs::prelude::Date;
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::model::{Bill, BillStatus};
use crate::money::format_brl;
use crate::paths;

/// How long after the app opens it first looks, for the database to be ready.
const FIRST_LOOK: Duration = Duration::from_secs(30);
/// How often it looks after that. It reads little, and an hour would miss the chosen time.
const NEXT_LOOK: Duration = Duration::from_secs(600);
/// How many bills a notification names; the rest it counts.
const NAMED: usize = 3;

/// The advance choices of the settings, in days, with `None` for overdue only.
pub const ADVANCE_DAYS: [Option<i32>; 5] = [None, Some(0), Some(1), Some(3), Some(7)];

/// What the person chose, and what was already told.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReminderSettings {
    /// Whether it tells at all.
    pub enabled: bool,
    /// Which of [`ADVANCE_DAYS`] it tells about.
    pub advance: usize,
    /// The hour of the day, from 0 to 23, from which it tells.
    pub hour: u8,
    /// Tells every day while a bill is unpaid, and not only the first time.
    pub every_day: bool,
    /// The last day it told, as text.
    pub last_day: String,
    /// The bills already told about, when not telling every day.
    pub told: Vec<i64>,
}

impl Default for ReminderSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            advance: 1,
            hour: 9,
            every_day: true,
            last_day: String::new(),
            told: Vec::new(),
        }
    }
}

/// The file beside the database.
pub fn settings_file(database: &Path) -> PathBuf {
    database.with_extension("avisos.json")
}

impl ReminderSettings {
    /// What is in the file. A missing or damaged one is the default choices.
    pub fn load(file: &Path) -> Self {
        std::fs::read(file).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }

    /// Writes the file whole, so that the other thread never reads half of it.
    pub fn save(&self, file: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let partial = file.with_extension("avisos.parcial");
        std::fs::write(&partial, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&partial, file).map_err(|e| e.to_string())
    }
}

/// A notification ready to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// Its first line.
    pub title: String,
    /// What it says.
    pub body: String,
    /// The bills it speaks of.
    pub bills: Vec<i64>,
}

/// What to tell now, if anything, about these unpaid bills.
pub fn notice_for(bills: &[Bill], today: Date, hour: u8, settings: &ReminderSettings) -> Option<Notice> {
    if !settings.enabled || hour < settings.hour || settings.last_day == today.to_string() {
        return None;
    }
    notice_about(bills, today, settings)
}

/// What it would say now, whatever the hour.
pub fn notice_about(bills: &[Bill], today: Date, settings: &ReminderSettings) -> Option<Notice> {
    let advance = ADVANCE_DAYS.get(settings.advance).copied().flatten();
    let limit = advance.map(|days| today.add_days(days));
    let wanted: Vec<&Bill> = bills
        .iter()
        .filter(|bill| bill.paid_on.is_none())
        .filter(|bill| bill.due < today || limit.is_some_and(|limit| bill.due <= limit))
        .filter(|bill| settings.every_day || !settings.told.contains(&bill.id))
        .collect();
    if wanted.is_empty() {
        return None;
    }
    let overdue = wanted.iter().filter(|bill| bill.status(today) == BillStatus::Overdue).count();
    let coming = wanted.len() - overdue;
    let title = match (overdue, coming) {
        (0, 1) => "1 conta a vencer".to_owned(),
        (0, coming) => format!("{coming} contas a vencer"),
        (1, 0) => "1 conta vencida".to_owned(),
        (overdue, 0) => format!("{overdue} contas vencidas"),
        (1, coming) => format!("1 conta vencida e {coming} a vencer"),
        (overdue, coming) => format!("{overdue} contas vencidas e {coming} a vencer"),
    };
    let mut names: Vec<String> = wanted
        .iter()
        .take(NAMED)
        .map(|bill| format!("{} ({})", bill.title(), format_brl(bill.amount_cents)))
        .collect();
    if wanted.len() > NAMED {
        names.push(format!("mais {}", wanted.len() - NAMED));
    }
    let total: i64 = wanted.iter().map(|bill| bill.amount_cents).sum();
    let body = format!("{}. Total: {}.", names.join(", "), format_brl(total));
    Some(Notice { title, body, bills: wanted.iter().map(|bill| bill.id).collect() })
}

/// Who the notifications are from, as the installer registers it.
pub fn notifier() -> SystemNotifier {
    SystemNotifier::new("Raven Finanças").bundle_id("dev.ravenrs.family-finance").opens_window(paths::APP_ID)
}

fn look(database: &Path, file: &Path) -> Result<(), String> {
    let settings = ReminderSettings::load(file);
    if !settings.enabled {
        return Ok(());
    }
    let today = crate::today();
    let hour = jiff::Zoned::now().hour();
    let hour = u8::try_from(hour).unwrap_or_default();
    let bills = Database::open(database).and_then(|db| db.open_bills()).map_err(|e| e.to_string())?;
    let Some(notice) = notice_for(&bills, today, hour, &settings) else {
        return Ok(());
    };
    notifier().send(&notice.title, &notice.body)?;
    // Read again, for what was changed on screen while it was telling.
    let mut settings = ReminderSettings::load(file);
    settings.last_day = today.to_string();
    let still_open = |id: &i64| bills.iter().any(|bill| bill.id == *id);
    settings.told.retain(still_open);
    settings
        .told
        .extend(notice.bills.iter().filter(|id| !settings.told.contains(id)).copied().collect::<Vec<_>>());
    settings.save(file)
}

/// Starts looking, for as long as the app runs.
pub fn watch(database: PathBuf) {
    let file = settings_file(&database);
    let started = std::thread::Builder::new().name("raven-avisos".to_owned()).spawn(move || {
        std::thread::sleep(FIRST_LOOK);
        loop {
            if let Err(e) = look(&database, &file) {
                tracing::warn!(error = %e, "could not tell about the bills");
            }
            std::thread::sleep(NEXT_LOOK);
        }
    });
    if let Err(e) = started {
        tracing::warn!(error = %e, "no notifications about bills");
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "tests fail loudly")]

    use super::*;

    fn day(n: u8) -> Date {
        Date::new(2026, 11, n).expect("valid date")
    }

    fn bills() -> Vec<Bill> {
        let db = Database::in_memory().expect("database");
        crate::seed::load_demo(&db, day(15)).expect("seed");
        db.open_bills().expect("bills")
    }

    #[test]
    #[ignore = "shows a real notification"]
    fn real_notification() {
        let started = std::time::Instant::now();
        notifier().send("Raven Finanças", "Teste do aviso do sistema.").expect("the system took it");
        println!("levou {:?}", started.elapsed());
    }

    #[test]
    fn it_tells_once_a_day_from_the_chosen_hour_and_never_when_turned_off() {
        let (bills, settings) = (bills(), ReminderSettings::default());
        assert!(notice_for(&bills, day(15), 8, &settings).is_none(), "too early");
        let notice = notice_for(&bills, day(15), 9, &settings).expect("the hour came");
        assert!(notice.title.contains("vencida"), "{}", notice.title);
        assert!(notice.body.contains("Total: R$"), "{}", notice.body);

        let told = ReminderSettings { last_day: day(15).to_string(), ..settings.clone() };
        assert!(notice_for(&bills, day(15), 20, &told).is_none(), "already told today");
        assert!(notice_for(&bills, day(16), 9, &told).is_some(), "a new day");
        let off = ReminderSettings { enabled: false, ..settings };
        assert!(notice_for(&bills, day(15), 9, &off).is_none());
    }

    #[test]
    fn the_advance_chooses_which_bills_and_each_can_be_told_only_once() {
        let bills = bills();
        let only_overdue = ReminderSettings { advance: 0, ..ReminderSettings::default() };
        let week = ReminderSettings { advance: 4, ..ReminderSettings::default() };
        let few = notice_about(&bills, day(15), &only_overdue).expect("overdue bills");
        let many = notice_about(&bills, day(15), &week).expect("bills");
        assert!(few.bills.len() < many.bills.len());
        assert!(!few.title.contains("a vencer"), "{}", few.title);
        assert!(many.title.contains("a vencer"), "{}", many.title);

        let once = ReminderSettings { every_day: false, told: many.bills.clone(), ..week };
        assert!(notice_about(&bills, day(15), &once).is_none(), "nothing new");
    }

    #[test]
    fn the_choices_survive_in_their_file_and_a_damaged_one_is_the_default() {
        let folder = std::env::temp_dir().join(format!("raven-avisos-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("folder");
        let file = settings_file(&folder.join("financas.db"));
        assert_eq!(ReminderSettings::load(&file), ReminderSettings::default());
        let chosen = ReminderSettings { hour: 20, advance: 3, ..ReminderSettings::default() };
        chosen.save(&file).expect("saved");
        assert_eq!(ReminderSettings::load(&file), chosen);
        std::fs::write(&file, b"nope").expect("damaged");
        assert_eq!(ReminderSettings::load(&file), ReminderSettings::default());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
