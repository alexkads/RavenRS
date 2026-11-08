//! Bills to pay: adding them in installments, paying, reopening and moving them.

use ravenrs::prelude::Date;
use rusqlite::{OptionalExtension, Row, params};

use super::{Database, DbError, DbResult, date_at};
use crate::model::{Bill, BillInput, Flow, Recurrence, TransactionInput, parse_date, shift_months};
use crate::money::split_installments;

const BILL_COLUMNS: &str = "b.id, b.description, b.category_id, c.name, b.member_id, COALESCE(m.name, ''),
    b.amount_cents, b.due, b.recurrence, b.anchor_day, b.installment, b.installments, b.barcode, b.paid_on,
    b.transaction_id";

const BILL_JOINS: &str =
    "FROM bills b JOIN categories c ON c.id = b.category_id LEFT JOIN members m ON m.id = b.member_id";

impl Database {
    /// Saves a bill, or one bill per installment with the total split so it adds up exactly.
    /// Returns the keys.
    pub fn add_bill(&self, input: &BillInput) -> DbResult<Vec<i64>> {
        let parts = input.installments.max(1);
        let anchor = input.due.day();
        // Installments end by themselves, so they never repeat.
        let recurrence = if parts > 1 { Recurrence::None } else { input.recurrence };
        self.atomically(|db| {
            let mut ids = Vec::with_capacity(parts as usize);
            for (i, amount) in split_installments(input.amount_cents, parts).into_iter().enumerate() {
                db.conn.execute(
                    "INSERT INTO bills (description, category_id, member_id, amount_cents, due, recurrence,
                                        anchor_day, installment, installments, barcode)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        input.description.trim(),
                        input.category_id,
                        input.member_id,
                        amount,
                        shift_months(input.due, i as i32, anchor).to_string(),
                        recurrence.code(),
                        anchor,
                        i as u32 + 1,
                        parts,
                        input.barcode.trim(),
                    ],
                )?;
                ids.push(db.conn.last_insert_rowid());
            }
            Ok(ids)
        })
    }

    /// Changes one unpaid bill.
    pub fn update_bill(&self, id: i64, input: &BillInput) -> DbResult<()> {
        self.conn.execute(
            "UPDATE bills SET description = ?2, category_id = ?3, member_id = ?4, amount_cents = ?5, due = ?6,
                    recurrence = ?7, anchor_day = ?8, barcode = ?9
             WHERE id = ?1",
            params![
                id,
                input.description.trim(),
                input.category_id,
                input.member_id,
                input.amount_cents,
                input.due.to_string(),
                input.recurrence.code(),
                input.due.day(),
                input.barcode.trim(),
            ],
        )?;
        Ok(())
    }

    /// Removes a bill; a paid one has to be reopened first, so its transaction is not left behind.
    pub fn delete_bill(&self, id: i64) -> DbResult<()> {
        if self.bill(id)?.is_some_and(|b| b.paid_on.is_some()) {
            return Err(DbError::Rule(
                "Esta conta já foi paga. Desfaça o pagamento antes de excluir.".to_owned(),
            ));
        }
        self.conn.execute("DELETE FROM bills WHERE id = ?1", [id])?;
        Ok(())
    }

    /// One bill.
    pub fn bill(&self, id: i64) -> DbResult<Option<Bill>> {
        let sql = format!("SELECT {BILL_COLUMNS} {BILL_JOINS} WHERE b.id = ?1");
        Ok(self.conn.prepare_cached(&sql)?.query_row([id], bill_from_row).optional()?)
    }

    /// Every bill by due date, oldest first.
    pub fn bills(&self) -> DbResult<Vec<Bill>> {
        let sql = format!("SELECT {BILL_COLUMNS} {BILL_JOINS} ORDER BY b.due, b.id");
        let mut query = self.conn.prepare_cached(&sql)?;
        let rows = query.query_map([], bill_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Unpaid bills by due date, oldest first.
    pub fn open_bills(&self) -> DbResult<Vec<Bill>> {
        let sql = format!("SELECT {BILL_COLUMNS} {BILL_JOINS} WHERE b.paid_on IS NULL ORDER BY b.due, b.id");
        let mut query = self.conn.prepare_cached(&sql)?;
        let rows = query.query_map([], bill_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Pays a bill from an account: records the expense, marks the bill paid and,
    /// when the bill repeats, schedules the next one. All of it or none of it.
    /// Returns the new transaction's key.
    pub fn pay_bill(&self, id: i64, account_id: i64, paid_on: Date, amount_cents: i64) -> DbResult<i64> {
        self.atomically(|db| {
            let Some(bill) = db.bill(id)? else {
                return Err(DbError::Rule("Esta conta não existe mais.".to_owned()));
            };
            if bill.paid_on.is_some() {
                return Err(DbError::Rule("Esta conta já foi paga.".to_owned()));
            }
            let transaction = db.add_transaction(&TransactionInput {
                date: paid_on,
                description: bill.title(),
                amount_cents,
                flow: Flow::Expense,
                account_id,
                category_id: bill.category_id,
                member_id: bill.member_id,
            })?;
            db.conn.execute(
                "UPDATE bills SET paid_on = ?2, transaction_id = ?3 WHERE id = ?1",
                params![id, paid_on.to_string(), transaction],
            )?;
            db.schedule_next(&bill)?;
            Ok(transaction)
        })
    }

    /// Undoes a payment: the bill is open again and the expense goes away.
    pub fn reopen_bill(&self, id: i64) -> DbResult<()> {
        self.atomically(|db| {
            let transaction = db.bill(id)?.and_then(|b| b.transaction_id);
            db.conn.execute("UPDATE bills SET paid_on = NULL, transaction_id = NULL WHERE id = ?1", [id])?;
            if let Some(transaction) = transaction {
                db.conn.execute("DELETE FROM transactions WHERE id = ?1", [transaction])?;
            }
            Ok(())
        })
    }

    /// Moves an unpaid bill to another day and person. It comes back on the new day from then on.
    pub fn move_bill(&self, id: i64, due: Date, member_id: Option<i64>) -> DbResult<()> {
        self.conn.execute(
            "UPDATE bills SET due = ?2, anchor_day = ?3, member_id = ?4 WHERE id = ?1 AND paid_on IS NULL",
            params![id, due.to_string(), due.day(), member_id],
        )?;
        Ok(())
    }

    fn schedule_next(&self, bill: &Bill) -> DbResult<()> {
        let months = match bill.recurrence {
            Recurrence::None => return Ok(()),
            Recurrence::Monthly => 1,
            Recurrence::Yearly => 12,
        };
        let next = shift_months(bill.due, months, bill.anchor_day);
        // Paying, undoing and paying again must not schedule the next one twice.
        let exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM bills WHERE description = ?1 AND due = ?2",
            params![bill.description, next.to_string()],
            |r| r.get(0),
        )?;
        if exists == 0 {
            self.conn.execute(
                "INSERT INTO bills (description, category_id, member_id, amount_cents, due, recurrence, anchor_day)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    bill.description,
                    bill.category_id,
                    bill.member_id,
                    bill.amount_cents,
                    next.to_string(),
                    bill.recurrence.code(),
                    bill.anchor_day
                ],
            )?;
        }
        Ok(())
    }
}

fn bill_from_row(r: &Row<'_>) -> rusqlite::Result<Bill> {
    Ok(Bill {
        id: r.get(0)?,
        description: r.get(1)?,
        category_id: r.get(2)?,
        category: r.get(3)?,
        member_id: r.get(4)?,
        member: r.get(5)?,
        amount_cents: r.get(6)?,
        due: date_at(r, 7)?,
        recurrence: Recurrence::from_code(&r.get::<_, String>(8)?),
        anchor_day: r.get(9)?,
        installment: r.get(10)?,
        installments: r.get(11)?,
        barcode: r.get(12)?,
        paid_on: r.get::<_, Option<String>>(13)?.as_deref().and_then(parse_date),
        transaction_id: r.get(14)?,
    })
}
