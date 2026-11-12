//! Payments and receipts planned on a rule, and the days they fall on.

use ravenrs::prelude::{Date, Frequency, RecurrenceRule};
use rusqlite::{OptionalExtension, params};

use super::{Database, DbError, DbResult, date_at};
use crate::model::{Flow, Occurrence, Schedule, ScheduleInput, TransactionInput, parse_date};

impl Database {
    /// Saves something planned and returns the key.
    pub fn add_schedule(&self, input: &ScheduleInput) -> DbResult<i64> {
        self.conn.execute(
            "INSERT INTO schedules (description, flow, amount_cents, account_id, category_id, member_id, start, rule)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                input.description.trim(),
                input.flow.code(),
                input.amount_cents,
                input.account_id,
                input.category_id,
                input.member_id,
                input.start.to_string(),
                input.rule.to_text(),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Changes what is planned. Days already confirmed or moved stay as they are.
    pub fn update_schedule(&self, id: i64, input: &ScheduleInput) -> DbResult<()> {
        self.conn.execute(
            "UPDATE schedules SET description = ?2, flow = ?3, amount_cents = ?4, account_id = ?5,
                    category_id = ?6, member_id = ?7, start = ?8, rule = ?9 WHERE id = ?1",
            params![
                id,
                input.description.trim(),
                input.flow.code(),
                input.amount_cents,
                input.account_id,
                input.category_id,
                input.member_id,
                input.start.to_string(),
                input.rule.to_text(),
            ],
        )?;
        Ok(())
    }

    /// Stops planning it. What already became a transaction stays.
    pub fn delete_schedule(&self, id: i64) -> DbResult<()> {
        self.conn.execute("DELETE FROM schedules WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Everything planned.
    pub fn schedules(&self) -> DbResult<Vec<Schedule>> {
        let mut query = self.conn.prepare_cached(
            "SELECT s.id, s.description, s.flow, s.amount_cents, s.account_id, s.category_id, s.member_id,
                    s.start, s.rule, a.name, c.name, COALESCE(m.name, '')
             FROM schedules s JOIN accounts a ON a.id = s.account_id JOIN categories c ON c.id = s.category_id
             LEFT JOIN members m ON m.id = s.member_id ORDER BY s.start, s.id",
        )?;
        let rows = query.query_map([], |r| {
            let rule: String = r.get(8)?;
            Ok(Schedule {
                id: r.get(0)?,
                input: ScheduleInput {
                    description: r.get(1)?,
                    flow: Flow::from_code(&r.get::<_, String>(2)?),
                    amount_cents: r.get(3)?,
                    account_id: r.get(4)?,
                    category_id: r.get(5)?,
                    member_id: r.get(6)?,
                    start: date_at(r, 7)?,
                    // A rule the app cannot read happens once rather than hide what was planned.
                    rule: RecurrenceRule::parse(&rule).unwrap_or(RecurrenceRule::new(Frequency::Once)),
                },
                account: r.get(9)?,
                category: r.get(10)?,
                member: r.get(11)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// One thing planned.
    pub fn schedule(&self, id: i64) -> DbResult<Option<Schedule>> {
        Ok(self.schedules()?.into_iter().find(|s| s.id == id))
    }

    /// What falls on `from..=to`, by the day it falls on: each rule's days, with
    /// those moved in or out of the range accounted for. Skipped days are left out.
    pub fn occurrences(&self, from: Date, to: Date) -> DbResult<Vec<Occurrence>> {
        let mut query = self.conn.prepare_cached(
            "SELECT day, moved_to, skipped, transaction_id FROM schedule_days WHERE schedule_id = ?1",
        )?;
        let mut found = Vec::new();
        for schedule in self.schedules()? {
            let marks = query.query_map([schedule.id], |r| {
                Ok((
                    date_at(r, 0)?,
                    r.get::<_, Option<String>>(1)?.as_deref().and_then(parse_date),
                    r.get::<_, bool>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            })?;
            let marks: Vec<_> = marks.collect::<Result<_, _>>()?;
            let make = |day: Date, when: Date, transaction_id: Option<i64>| Occurrence {
                schedule_id: schedule.id,
                day,
                when,
                transaction_id,
                schedule: schedule.clone(),
            };
            for day in schedule.input.rule.occurrences(schedule.input.start, from, to) {
                if !marks.iter().any(|mark| mark.0 == day) {
                    found.push(make(day, day, None));
                }
            }
            for (day, moved_to, skipped, transaction_id) in marks {
                let when = moved_to.unwrap_or(day);
                if !skipped && when >= from && when <= to {
                    found.push(make(day, when, transaction_id));
                }
            }
        }
        found.sort_by_key(|o| (o.when, o.schedule_id));
        Ok(found)
    }

    /// Records that a planned day happened: the transaction goes in and the day is done.
    pub fn confirm_occurrence(
        &self,
        schedule_id: i64,
        day: Date,
        account_id: i64,
        when: Date,
        amount_cents: i64,
    ) -> DbResult<i64> {
        self.atomically(|db| {
            let Some(schedule) = db.schedule(schedule_id)? else {
                return Err(DbError::Rule("Esta programação não existe mais.".to_owned()));
            };
            let done: Option<Option<i64>> = db
                .conn
                .query_row(
                    "SELECT transaction_id FROM schedule_days WHERE schedule_id = ?1 AND day = ?2",
                    params![schedule_id, day.to_string()],
                    |r| r.get(0),
                )
                .optional()?;
            if done.flatten().is_some() {
                return Err(DbError::Rule("Este dia já foi confirmado.".to_owned()));
            }
            let transaction = db.add_transaction(&TransactionInput {
                date: when,
                description: schedule.input.description.clone(),
                amount_cents,
                flow: schedule.input.flow,
                account_id,
                category_id: schedule.input.category_id,
                member_id: schedule.input.member_id,
            })?;
            db.mark_day(schedule_id, day, Some(when), false, Some(transaction))?;
            Ok(transaction)
        })
    }

    /// Undoes a confirmation: the transaction goes away and the day is ahead again.
    pub fn undo_occurrence(&self, transaction_id: i64) -> DbResult<()> {
        self.delete_transaction(transaction_id)
    }

    /// Leaves one day out, keeping the rule.
    pub fn skip_occurrence(&self, schedule_id: i64, day: Date) -> DbResult<()> {
        self.mark_day(schedule_id, day, None, true, None)
    }

    /// Moves one day of a rule, keeping the others where they are.
    pub fn move_occurrence(&self, schedule_id: i64, day: Date, to: Date) -> DbResult<()> {
        self.mark_day(schedule_id, day, Some(to), false, None)
    }

    fn mark_day(
        &self,
        schedule_id: i64,
        day: Date,
        moved_to: Option<Date>,
        skipped: bool,
        transaction_id: Option<i64>,
    ) -> DbResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO schedule_days (schedule_id, day, moved_to, skipped, transaction_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![schedule_id, day.to_string(), moved_to.map(|d| d.to_string()), skipped, transaction_id],
        )?;
        Ok(())
    }
}
