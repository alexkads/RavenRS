//! A summary of what is overdue and what falls due in the next days, for an entity that says it has
//! a due date, as each person may see it (#860).
//!
//! The summary is worked out from records, apart from any screen: [`digest_of`] counts, orders,
//! names and adds them up; [`due_digest_for`] reads them through the person's space and leaves the
//! amounts out when they may not read that field. What each person wants, on or off and how many
//! days ahead, is a setting of their own.

use ravenrs_criteria::{Criteria, DateTime, Scope, Value};

use crate::{EntityInfo, Object, ObjectSpace, Operation, Permissions, Query, SettingScope, SettingsStore};

/// The words of the text around the entity's own, in a language.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct DigestWords {
    /// After the count of overdue ones, for one and for several: "overdue".
    pub overdue: (String, String),
    /// After the count of the coming ones: "coming".
    pub coming: String,
    /// Between the two counts: "and".
    pub and: String,
    /// Before how many more there are: "more".
    pub more: String,
    /// Before the total: "Total".
    pub total: String,
    /// Before an amount: "$".
    pub money_prefix: String,
    /// Between whole units and cents.
    pub decimal: char,
    /// Between thousands.
    pub thousands: char,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DigestWords {
    /// In English, with dollars.
    pub fn english() -> Self {
        Self {
            overdue: ("overdue".to_owned(), "overdue".to_owned()),
            coming: "coming".to_owned(),
            and: "and".to_owned(),
            more: "more".to_owned(),
            total: "Total".to_owned(),
            money_prefix: "$".to_owned(),
            decimal: '.',
            thousands: ',',
        }
    }

    /// In Brazilian Portuguese, with reais, for a word like "conta": "1 conta vencida e 2 a
    /// vencer". A masculine word changes `overdue` to "vencido" and "vencidos".
    pub fn portuguese() -> Self {
        Self {
            overdue: ("vencida".to_owned(), "vencidas".to_owned()),
            coming: "a vencer".to_owned(),
            and: "e".to_owned(),
            more: "mais".to_owned(),
            total: "Total".to_owned(),
            money_prefix: "R$ ".to_owned(),
            decimal: ',',
            thousands: '.',
        }
    }

    /// The words of a catalog, in its language, with its money format. What it does not have
    /// comes from English. The adjective agrees with a feminine word in Portuguese, as "conta":
    /// a masculine one changes `overdue`.
    pub fn from_texts(texts: &ravenrs_app_i18n::Texts) -> Self {
        let char_of = |key: &str, or: char| texts.get(key).chars().next().unwrap_or(or);
        Self {
            overdue: (texts.get("ui.due_overdue_one"), texts.get("ui.due_overdue_many")),
            coming: texts.get("ui.due_coming"),
            and: texts.get("ui.due_and"),
            more: texts.get("ui.due_more"),
            total: texts.get("ui.due_total"),
            money_prefix: texts.get("format.money_prefix"),
            decimal: char_of("format.decimal", '.'),
            thousands: char_of("format.thousands", ','),
        }
    }

    /// An amount in cents, as people read it: "R$ 1.480,00".
    pub fn money(&self, cents: i64) -> String {
        let sign = if cents < 0 { "-" } else { "" };
        let cents = cents.unsigned_abs();
        let whole = (cents / 100).to_string();
        let mut grouped = String::new();
        for (at, digit) in whole.chars().enumerate() {
            if at > 0 && (whole.len() - at).is_multiple_of(3) {
                grouped.push(self.thousands);
            }
            grouped.push(digit);
        }
        format!("{sign}{}{grouped}{}{:02}", self.money_prefix, self.decimal, cents % 100)
    }
}

/// What a summary says, and of which records.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Digest {
    /// "1 conta vencida e 2 a vencer".
    pub title: String,
    /// The names, how many more, and the total when there is one.
    pub body: String,
    /// How many are overdue.
    pub overdue: usize,
    /// How many fall due in the days ahead.
    pub coming: usize,
    /// The keys of the records it is about, by due date.
    pub keys: Vec<Value>,
}

fn due_day(record: &Object, field: &str) -> Option<i64> {
    match record.get(field) {
        Value::Date(date) => Some(date.days),
        _ => None,
    }
}

/// The summary of these records at `today`: the pending ones due before it, and those due up to
/// `advance_days` after it, by due date. With `show_amounts` off, no amount is in the text; a record
/// whose amount is empty, as permissions leave it, takes the total out instead of adding nothing.
pub fn digest_of(
    entity: &EntityInfo,
    records: &[Object],
    today: DateTime,
    advance_days: u32,
    show_amounts: bool,
    words: &DigestWords,
) -> Option<Digest> {
    let digest = entity.due_digest.as_ref()?;
    let environment = crate::environment();
    let pending = digest.pending.as_deref().and_then(|text| Criteria::parse(text).ok());
    let limit = today.days + i64::from(advance_days);
    let mut wanted: Vec<(i64, &Object)> = records
        .iter()
        .filter(|record| {
            pending.as_ref().is_none_or(|criteria| Scope::new(&environment).row(*record).matches(criteria))
        })
        .filter_map(|record| due_day(record, &digest.field).map(|day| (day, record)))
        .filter(|(day, _)| *day <= limit)
        .collect();
    if wanted.is_empty() {
        return None;
    }
    wanted.sort_by_key(|(day, _)| *day);
    let overdue = wanted.iter().filter(|(day, _)| *day < today.days).count();
    let coming = wanted.len() - overdue;
    let (one, many) = &digest.words;
    let noun = |count: usize| if count == 1 { one } else { many };
    let title = match (overdue, coming) {
        (0, coming) => format!("{coming} {} {}", noun(coming), words.coming),
        (overdue, 0) => {
            let adjective = if overdue == 1 { &words.overdue.0 } else { &words.overdue.1 };
            format!("{overdue} {} {adjective}", noun(overdue))
        }
        (overdue, coming) => {
            let adjective = if overdue == 1 { &words.overdue.0 } else { &words.overdue.1 };
            format!("{overdue} {} {adjective} {} {coming} {}", noun(overdue), words.and, words.coming)
        }
    };
    let label = digest.label.clone().or_else(|| entity.default_property().map(|field| field.name.clone()));
    let amount_of = |record: &Object| -> Option<i64> {
        let field = digest.amount.as_ref()?;
        record.cents(field)
    };
    let amounts = show_amounts && digest.amount.is_some();
    let mut names: Vec<String> = wanted
        .iter()
        .take(digest.named)
        .map(|(_, record)| {
            let name = label.as_ref().map(|field| record.text(field)).unwrap_or_default();
            match amount_of(record) {
                Some(cents) if amounts => format!("{name} ({})", words.money(cents)),
                _ => name,
            }
        })
        .collect();
    if wanted.len() > digest.named {
        names.push(format!("{} {}", words.more, wanted.len() - digest.named));
    }
    let mut body = format!("{}.", names.join(", "));
    if amounts {
        let all: Option<Vec<i64>> = wanted.iter().map(|(_, record)| amount_of(record)).collect();
        if let Some(all) = all {
            body.push_str(&format!(" {}: {}.", words.total, words.money(all.iter().sum())));
        }
    } else if digest.amount.is_some()
        && let Some(hidden) = &digest.hidden_amounts
    {
        body.push(' ');
        body.push_str(hidden);
    }
    let key = entity.key_field().map(|field| field.name.clone());
    let keys = wanted
        .iter()
        .map(|(_, record)| key.as_ref().map_or(Value::Null, |field| record.get(field)))
        .collect();
    Some(Digest { title, body, overdue, coming, keys })
}

/// What a person wants of the summary of an entity: on or off, and how many days ahead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct DigestPreference {
    /// Whether they get it.
    pub enabled: bool,
    /// How many days ahead it looks: 0, 1, 3 or 7.
    pub advance_days: u32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DigestPreference {
    /// On or off, so many days ahead.
    pub fn new(enabled: bool, advance_days: u32) -> Self {
        Self { enabled, advance_days }
    }
}

impl Default for DigestPreference {
    fn default() -> Self {
        Self { enabled: true, advance_days: 3 }
    }
}

/// The days ahead a person can choose.
pub const DIGEST_ADVANCE_DAYS: [u32; 4] = [0, 1, 3, 7];

fn preference_name(entity: &str) -> String {
    format!("due-digest/{entity}")
}

/// What `user` wants of the summary of `entity`; on, three days ahead, if they never said.
pub fn digest_preference(store: &dyn SettingsStore, user: &str, entity: &str) -> DigestPreference {
    store
        .get(&SettingScope::User(user.to_owned()), &preference_name(entity))
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .map_or_else(DigestPreference::default, |kept| {
            let default = DigestPreference::default();
            DigestPreference {
                enabled: kept.get("enabled").and_then(serde_json::Value::as_bool).unwrap_or(default.enabled),
                advance_days: kept
                    .get("advance_days")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|days| u32::try_from(days).ok())
                    .filter(|days| DIGEST_ADVANCE_DAYS.contains(days))
                    .unwrap_or(default.advance_days),
            }
        })
}

/// Keeps what `user` wants of the summary of `entity`. Only they say it: `actor` is who asks, and
/// anyone else is refused, as is a number of days nobody can choose.
pub fn set_digest_preference(
    store: &dyn SettingsStore,
    actor: &str,
    user: &str,
    entity: &str,
    preference: DigestPreference,
) -> Result<(), String> {
    if actor != user {
        return Err("each person says only what they want".to_owned());
    }
    if !DIGEST_ADVANCE_DAYS.contains(&preference.advance_days) {
        return Err(format!("the days ahead are one of {DIGEST_ADVANCE_DAYS:?}"));
    }
    let text = serde_json::json!({ "enabled": preference.enabled, "advance_days": preference.advance_days })
        .to_string();
    store.set(&SettingScope::User(user.to_owned()), &preference_name(entity), &text)
}

/// The summary of `entity` for this person, as their space shows the records: nothing if they
/// cannot read the entity or turned it off, and no amount if they cannot read that field.
pub fn due_digest_for(
    space: &dyn ObjectSpace,
    entity: &EntityInfo,
    permissions: &Permissions,
    today: DateTime,
    preference: DigestPreference,
    words: &DigestWords,
) -> Option<Digest> {
    let digest = entity.due_digest.as_ref()?;
    if !preference.enabled || !permissions.can(Operation::Read, &entity.name, None) {
        return None;
    }
    let show_amounts = digest
        .amount
        .as_ref()
        .is_some_and(|field| permissions.can_field(Operation::Read, &entity.name, field, None));
    let records = space.query(&entity.name, &Query::all()).ok()?;
    digest_of(entity, &records, today, preference.advance_days, show_amounts, words)
}
