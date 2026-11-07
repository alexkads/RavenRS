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

/// A kind of notice the Telegram bot sends, which the chat can turn off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelegramNotice {
    /// Bills overdue or coming, with the advance and the hour chosen in Ajustes.
    Bills,
    /// A category's spending past its monthly limit, once per category and month.
    Budget,
    /// Last week's income and spending, on Mondays.
    Weekly,
}

impl TelegramNotice {
    /// Every kind, in the order `/avisos` lists them.
    pub const ALL: [Self; 3] = [Self::Bills, Self::Budget, Self::Weekly];

    /// The word for it in a chat command: `/avisos contas desligar`.
    pub fn word(self) -> &'static str {
        match self {
            Self::Bills => "contas",
            Self::Budget => "orcamento",
            Self::Weekly => "resumo",
        }
    }

    /// Its name, as the chat shows it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Bills => "Contas a vencer",
            Self::Budget => "Orçamento estourado",
            Self::Weekly => "Resumo semanal",
        }
    }
}

/// What the Telegram bot tells and has told, in its own file beside the database: the
/// system notifications keep theirs, so turning one off leaves the other.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TelegramNotices {
    /// Whether it tells about bills.
    pub bills: bool,
    /// Whether it tells about budgets.
    pub budget: bool,
    /// Whether it sends the weekly summary.
    pub weekly: bool,
    /// The last day it told about bills.
    pub bills_day: String,
    /// The budgets already told about, as `YYYY-MM:category`.
    pub budget_told: Vec<String>,
    /// The Monday it last sent the summary.
    pub weekly_day: String,
}

impl Default for TelegramNotices {
    fn default() -> Self {
        Self {
            bills: true,
            budget: true,
            weekly: true,
            bills_day: String::new(),
            budget_told: Vec::new(),
            weekly_day: String::new(),
        }
    }
}

impl TelegramNotices {
    /// The file beside the database.
    pub fn file(database: &Path) -> PathBuf {
        database.with_extension("telegram-avisos.json")
    }

    /// What is in the file. A missing or damaged one is the default: everything on.
    pub fn load(file: &Path) -> Self {
        std::fs::read(file).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }

    /// Writes the file whole.
    pub fn save(&self, file: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let partial = file.with_extension("telegram-avisos.parcial");
        std::fs::write(&partial, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&partial, file).map_err(|e| e.to_string())
    }

    /// Whether `kind` is on.
    pub fn is_on(&self, kind: TelegramNotice) -> bool {
        match kind {
            TelegramNotice::Bills => self.bills,
            TelegramNotice::Budget => self.budget,
            TelegramNotice::Weekly => self.weekly,
        }
    }

    /// Turns `kind` on or off.
    pub fn set(&mut self, kind: TelegramNotice, on: bool) {
        match kind {
            TelegramNotice::Bills => self.bills = on,
            TelegramNotice::Budget => self.budget = on,
            TelegramNotice::Weekly => self.weekly = on,
        }
    }

    /// What to tell now, from the hour chosen in Ajustes, each at most once: bills once a
    /// day, a budget once a month, the summary once a week. Marks what it returns as told.
    pub fn due(
        &mut self,
        db: &Database,
        today: Date,
        hour: u8,
        chosen: &ReminderSettings,
    ) -> crate::db::DbResult<Vec<(TelegramNotice, String)>> {
        let mut said = Vec::new();
        if hour < chosen.hour {
            return Ok(said);
        }
        let day = today.to_string();
        if self.bills && self.bills_day != day {
            let settings = ReminderSettings { every_day: true, ..chosen.clone() };
            if let Some(notice) = notice_about(&db.open_bills()?, today, &settings) {
                said.push((TelegramNotice::Bills, format!("{}: {}", notice.title, notice.body)));
            }
            self.bills_day = day.clone();
        }
        if self.budget {
            let month = &day[..7];
            self.budget_told.retain(|told| told.starts_with(month));
            for line in db.budget_lines(today)? {
                let key = format!("{month}:{}", line.category_id);
                if line.budget_cents > 0
                    && line.spent_cents > line.budget_cents
                    && !self.budget_told.contains(&key)
                {
                    said.push((
                        TelegramNotice::Budget,
                        format!(
                            "Orçamento estourado: {} já gastou {} de {} neste mês ({}%).",
                            line.category,
                            format_brl(line.spent_cents),
                            format_brl(line.budget_cents),
                            line.spent_cents * 100 / line.budget_cents
                        ),
                    ));
                    self.budget_told.push(key);
                }
            }
        }
        if self.weekly && today.weekday() == 1 && self.weekly_day != day {
            let (from, to) = (today.add_days(-7), today);
            let totals = db.totals_between(from, to)?;
            let short = |date: Date| format!("{:02}/{:02}", date.day(), date.month());
            said.push((
                TelegramNotice::Weekly,
                format!(
                    "Resumo da semana ({} a {}): gastos {}, receitas {}.",
                    short(from),
                    short(to.add_days(-1)),
                    format_brl(totals.expense_cents),
                    format_brl(totals.income_cents)
                ),
            ));
            self.weekly_day = day;
        }
        Ok(said)
    }
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
    fn the_telegram_tells_each_thing_once_from_the_chosen_hour() {
        use crate::model::{AccountKind, Flow, TransactionInput};
        let db = Database::in_memory().expect("database");
        crate::seed::load_demo(&db, day(15)).expect("seed");
        let market = db.add_category("Feira", Flow::Expense, 100_00).expect("category");
        let bank = db.add_account("Conta da feira", AccountKind::Checking, 0).expect("account");
        let spend = |cents| {
            db.add_transaction(&TransactionInput {
                date: day(10),
                description: "feira".into(),
                amount_cents: cents,
                flow: Flow::Expense,
                account_id: bank,
                category_id: market,
                member_id: None,
            })
            .expect("transaction")
        };
        spend(150_00);
        let chosen = ReminderSettings::default();
        let monday = day(16);
        assert_eq!(monday.weekday(), 1);

        let mut notices = TelegramNotices::default();
        assert!(notices.due(&db, monday, 8, &chosen).expect("read").is_empty(), "too early");
        let said = notices.due(&db, monday, 9, &chosen).expect("read");
        let kinds: Vec<TelegramNotice> = said.iter().map(|(kind, _)| *kind).collect();
        assert!(kinds.contains(&TelegramNotice::Bills), "{said:?}");
        assert!(kinds.contains(&TelegramNotice::Weekly), "{said:?}");
        let feira = said.iter().find(|(_, text)| text.contains("Feira")).expect("the budget past its limit");
        assert_eq!(feira.1, "Orçamento estourado: Feira já gastou R$ 150,00 de R$ 100,00 neste mês (150%).");
        assert!(said.iter().any(|(_, text)| text.starts_with("Resumo da semana (09/11 a 15/11): gastos R$")));

        assert!(notices.due(&db, monday, 20, &chosen).expect("read").is_empty(), "each once");
        spend(10_00);
        let tuesday = notices.due(&db, day(17), 9, &chosen).expect("read");
        assert!(
            tuesday.iter().all(|(kind, _)| *kind == TelegramNotice::Bills),
            "bills daily, the rest not: {tuesday:?}"
        );

        let mut off = TelegramNotices::default();
        for kind in TelegramNotice::ALL {
            off.set(kind, false);
        }
        assert!(off.due(&db, monday, 9, &chosen).expect("read").is_empty(), "turned off");
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
