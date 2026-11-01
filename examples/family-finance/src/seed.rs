//! A family's last year of money, generated around today, so the app opens
//! with something to look at. The same day always gives the same data.

use ravenrs::prelude::{Date, Frequency, RecurrenceEnd, RecurrenceRule};

use crate::db::{Database, DbResult};
use crate::model::{AccountKind, BillInput, Flow, Recurrence, ScheduleInput, TransactionInput, shift_months};

/// Months of history to generate.
const HISTORY_MONTHS: i32 = 13;

/// A small deterministic generator, so demos and tests are repeatable.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next() % items.len() as u64) as usize]
    }
}

/// Fills an empty database with accounts, categories, a year of transactions and bills.
pub fn load_demo(db: &Database, today: Date) -> DbResult<()> {
    db.atomically(|db| {
        let (ana, bruno, lia) = (db.add_member("Ana")?, db.add_member("Bruno")?, db.add_member("Lia")?);

        let checking = db.add_account("Conta corrente", AccountKind::Checking, 4_200_00)?;
        let savings = db.add_account("Poupança", AccountKind::Savings, 18_000_00)?;
        let wallet = db.add_account("Carteira", AccountKind::Cash, 600_00)?;

        let housing = db.add_category("Moradia", Flow::Expense, 2_400_00)?;
        let groceries = db.add_category("Mercado", Flow::Expense, 1_500_00)?;
        let utilities = db.add_category("Contas da casa", Flow::Expense, 650_00)?;
        let transport = db.add_category("Transporte", Flow::Expense, 700_00)?;
        let health = db.add_category("Saúde", Flow::Expense, 600_00)?;
        let school = db.add_category("Educação", Flow::Expense, 1_100_00)?;
        let fun = db.add_category("Lazer", Flow::Expense, 500_00)?;
        let streaming = db.add_category("Assinaturas", Flow::Expense, 120_00)?;
        let salary = db.add_category("Salário", Flow::Income, 0)?;
        let freelance = db.add_category("Trabalhos extras", Flow::Income, 0)?;
        let yields = db.add_category("Rendimentos", Flow::Income, 0)?;

        let mut rng = Rng(0x5eed_f00d_cafe_0001 ^ today.to_days() as u64);
        let this_month = today.first_of_month();
        let spend = |date: Date, text: &str, cents: i64, account: i64, category: i64, member: Option<i64>| {
            // Nothing is recorded in the future: those are still bills to pay.
            if date > today {
                return Ok(());
            }
            db.add_transaction(&TransactionInput {
                date,
                description: text.to_owned(),
                amount_cents: cents,
                flow: Flow::Expense,
                account_id: account,
                category_id: category,
                member_id: member,
            })
            .map(|_| ())
        };
        let earn = |date: Date, text: &str, cents: i64, account: i64, category: i64, member: Option<i64>| {
            if date > today {
                return Ok(());
            }
            db.add_transaction(&TransactionInput {
                date,
                description: text.to_owned(),
                amount_cents: cents,
                flow: Flow::Income,
                account_id: account,
                category_id: category,
                member_id: member,
            })
            .map(|_| ())
        };

        for back in (0..HISTORY_MONTHS).rev() {
            let month = this_month.add_months(-back);
            let day = |d: u8| shift_months(month, 0, d);

            earn(day(5), "Salário Ana", 5_800_00, checking, salary, Some(ana))?;
            earn(day(7), "Salário Bruno", 4_200_00, checking, salary, Some(bruno))?;
            earn(day(28), "Rendimento da poupança", rng.range(90_00, 130_00), savings, yields, None)?;
            if back % 3 == 1 {
                earn(
                    day(18),
                    "Projeto de design",
                    rng.range(900_00, 2_400_00),
                    checking,
                    freelance,
                    Some(bruno),
                )?;
            }

            // Monthly bills of past months are recorded as paid further down; these are the day-to-day ones.
            for _ in 0..rng.range(6, 9) {
                let place = *rng.pick(&[
                    "Supermercado Boa Compra",
                    "Feira do bairro",
                    "Padaria Central",
                    "Hortifruti",
                ]);
                spend(
                    day(rng.range(1, 28) as u8),
                    place,
                    rng.range(60_00, 340_00),
                    checking,
                    groceries,
                    Some(ana),
                )?;
            }
            for _ in 0..rng.range(3, 5) {
                let (text, account, lo, hi) = *rng.pick(&[
                    ("Combustível", checking, 90_00, 260_00),
                    ("Aplicativo de transporte", checking, 15_00, 60_00),
                    ("Estacionamento", wallet, 8_00, 25_00),
                    ("Passagem de ônibus", wallet, 5_00, 15_00),
                ]);
                let who = *rng.pick(&[ana, bruno]);
                spend(day(rng.range(1, 28) as u8), text, rng.range(lo, hi), account, transport, Some(who))?;
            }
            for _ in 0..rng.range(2, 4) {
                let text =
                    *rng.pick(&["Restaurante", "Cinema", "Sorveteria", "Passeio no parque", "Livraria"]);
                let who = *rng.pick(&[ana, bruno, lia]);
                spend(day(rng.range(1, 28) as u8), text, rng.range(30_00, 240_00), checking, fun, Some(who))?;
            }
            if back % 2 == 0 {
                let text = *rng.pick(&["Farmácia", "Consulta pediátrica", "Exames de rotina", "Dentista"]);
                spend(
                    day(rng.range(1, 28) as u8),
                    text,
                    rng.range(45_00, 520_00),
                    checking,
                    health,
                    Some(lia),
                )?;
            }
        }

        // Recurring bills: past months paid, the rest still open.
        let plan: [(&str, i64, i64, u8, Option<i64>); 8] = [
            ("Aluguel", 2_150_00, housing, 10, None),
            ("Condomínio", 480_00, housing, 12, None),
            ("Energia elétrica", 286_40, utilities, 15, None),
            ("Água e esgoto", 112_75, utilities, 18, None),
            ("Internet", 129_90, utilities, 20, None),
            ("Escola da Lia", 1_040_00, school, 8, Some(lia)),
            ("Plano de saúde", 780_00, health, 3, None),
            ("Streaming de vídeo", 55_90, streaming, 25, None),
        ];
        for (index, (text, cents, category, due_day, member)) in plan.into_iter().enumerate() {
            let first = shift_months(this_month.add_months(1 - HISTORY_MONTHS), 0, due_day);
            let ids = db.add_bill(&BillInput {
                description: text.to_owned(),
                category_id: category,
                member_id: member,
                amount_cents: cents,
                due: first,
                recurrence: Recurrence::Monthly,
                installments: 1,
                barcode: if index < 5 { boleto_digits(&mut rng) } else { String::new() },
            })?;
            let mut current = ids.first().copied();
            while let Some(id) = current {
                let Some(bill) = db.bill(id)? else { break };
                // Two bills stay overdue on purpose, so the app has something to warn about.
                let overdue_on_purpose = index >= 6 && bill.due >= today.add_days(-30);
                if bill.due >= today || overdue_on_purpose {
                    break;
                }
                let paid = bill.due.add_days(-(rng.range(0, 3) as i32));
                db.pay_bill(id, checking, paid, bill.amount_cents)?;
                current = db
                    .bills()?
                    .into_iter()
                    .find(|b| b.description == bill.description && b.due > bill.due)
                    .map(|b| b.id);
            }
        }

        // A purchase split into installments, some already paid.
        let fridge = db.add_bill(&BillInput {
            description: "Geladeira".to_owned(),
            category_id: housing,
            member_id: None,
            amount_cents: 4_299_00,
            due: shift_months(this_month.add_months(-3), 0, 22),
            recurrence: Recurrence::None,
            installments: 10,
            barcode: String::new(),
        })?;
        for id in fridge {
            if let Some(bill) = db.bill(id)?
                && bill.due < today
            {
                db.pay_bill(id, checking, bill.due, bill.amount_cents)?;
            }
        }

        // What is planned ahead, so the agenda has receipts and payments to confirm.
        let plan = |text: &str, flow, cents, category, start, rule| {
            db.add_schedule(&ScheduleInput {
                description: String::from(text),
                flow,
                amount_cents: cents,
                account_id: checking,
                category_id: category,
                member_id: None,
                start,
                rule,
            })
        };
        plan(
            "Aluguel da garagem",
            Flow::Income,
            350_00,
            freelance,
            today.add_days(2),
            RecurrenceRule::new(Frequency::Monthly),
        )?;
        plan(
            "Aula de natação",
            Flow::Expense,
            90_00,
            health,
            today.add_days(1),
            RecurrenceRule::new(Frequency::Weekly).end(RecurrenceEnd::After(12)),
        )?;
        plan(
            "Décimo terceiro",
            Flow::Income,
            3_200_00,
            salary,
            today.add_days(16),
            RecurrenceRule::new(Frequency::Once),
        )?;
        plan(
            "Seguro do carro",
            Flow::Expense,
            210_00,
            transport,
            today.add_days(9),
            RecurrenceRule::new(Frequency::Monthly).end(RecurrenceEnd::After(6)),
        )?;

        // A yearly bill and a one-off one, ahead of today.
        db.add_bill(&BillInput {
            description: "IPVA".to_owned(),
            category_id: transport,
            member_id: Some(bruno),
            amount_cents: 1_865_30,
            due: today.add_days(45),
            recurrence: Recurrence::Yearly,
            installments: 1,
            barcode: boleto_digits(&mut rng),
        })?;
        db.add_bill(&BillInput {
            description: "Material escolar".to_owned(),
            category_id: school,
            member_id: Some(lia),
            amount_cents: 389_90,
            due: today.add_days(6),
            recurrence: Recurrence::None,
            installments: 1,
            barcode: String::new(),
        })?;
        Ok(())
    })
}

/// What an empty start needs: one account and the usual categories.
pub fn load_defaults(db: &Database) -> DbResult<()> {
    db.atomically(|db| {
        db.add_account("Conta corrente", AccountKind::Checking, 0)?;
        for name in ["Moradia", "Mercado", "Contas da casa", "Transporte", "Saúde", "Educação", "Lazer"] {
            db.add_category(name, Flow::Expense, 0)?;
        }
        for name in ["Salário", "Outras receitas"] {
            db.add_category(name, Flow::Income, 0)?;
        }
        Ok(())
    })
}

/// 44 made-up digits, the length of a Brazilian payment slip's barcode.
fn boleto_digits(rng: &mut Rng) -> String {
    (0..44).map(|_| char::from(b'0' + rng.range(0, 9) as u8)).collect()
}
