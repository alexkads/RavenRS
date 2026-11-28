//! Bills to pay and to receive: a bill is settled with interest, fine and discount, in part or in
//! full, the cash book gets the entry in the same commit, a settlement is reversed by an entry of
//! the opposite sign, and a bill is split in installments to the cent (#859). Everything runs on
//! a real `MemoryDatabase` with the entities of `ravenrs_app_library::finance`.
use ravenrs::prelude::*;
use ravenrs_app::{MemoryDatabase, Object, ObjectSpace, Query, Value};
use ravenrs_app_library::finance::{self, COMMITMENT, LEDGER_ENTRY, SETTLEMENT, SettleInput};
use ravenrs_criteria::DateTime;

/// The control registered in the center.
pub const ID: &str = "FinanceDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Contas a pagar e receber";

const BILL: f64 = 1.0;

fn today() -> DateTime {
    DateTime::from_ymd(2026, 11, 21).unwrap_or(DateTime { days: 0, seconds: 0 })
}

fn brl(cents: i64) -> String {
    format!("R$ {},{:02}", cents / 100, (cents % 100).abs())
}

#[derive(Clone)]
struct Demo {
    database: MemoryDatabase,
    said: String,
}

impl Demo {
    fn new() -> Self {
        let database =
            MemoryDatabase::new(finance::entities()).unwrap_or_else(|e| panic!("the page's model: {e}"));
        let mut space = database.space();
        if let Ok(mut bill) = space.new_object(COMMITMENT) {
            bill.set("id", BILL);
            bill.set("description", "Luz");
            bill.set("kind", "payable");
            bill.set("amount", 10_000.0);
            bill.set("due_on", Value::Date(today()));
            let _ = space.insert(bill);
        }
        let _ = space.commit();
        Self { database, said: "Pronto.".to_owned() }
    }

    fn answer(&mut self, done: Result<String, ravenrs_app::SpaceError>) {
        self.said = match done {
            Ok(what) => what,
            Err(why) => format!("Recusado: {why}"),
        };
    }

    fn settle(&mut self, input: SettleInput) {
        let total = input.total();
        let mut space = self.database.space();
        let done = finance::settle(&mut space, &Value::Number(BILL), input, today())
            .map(|_| format!("Baixa de {} no caixa.", brl(total)));
        self.answer(done);
    }

    fn reverse_last(&mut self, reason: &str) {
        let space = self.database.space();
        let last = space
            .query(SETTLEMENT, &Query::all())
            .unwrap_or_default()
            .into_iter()
            .rfind(|settlement| settlement.get("reversed_on") == Value::Null);
        let Some(last) = last else {
            self.said = "Nenhuma baixa para estornar.".to_owned();
            return;
        };
        let mut space = self.database.space();
        let done = finance::reverse(&mut space, &last.get("id"), reason, today())
            .map(|()| "Baixa estornada.".to_owned());
        self.answer(done);
    }

    fn split(&mut self) {
        let mut space = self.database.space();
        let first = DateTime::from_ymd(2027, 1, 31).unwrap_or(DateTime { days: 0, seconds: 0 });
        let done = finance::split(&mut space, &Value::Number(BILL), 3, first)
            .map(|parts| format!("Parcelada em {}.", parts.len()));
        self.answer(done);
    }

    fn cancel(&mut self) {
        let mut space = self.database.space();
        let done = finance::cancel(&mut space, &Value::Number(BILL)).map(|()| "Conta cancelada.".to_owned());
        self.answer(done);
    }

    /// Prints the statement of the month and the position of what is open, as documents.
    fn print(&mut self) {
        let (culture, words) = (ravenrs_criteria::Culture::pt_br(), ravenrs_app_report::ReportWords::pt_br());
        let space = self.database.space();
        let from = DateTime { days: today().days - 30, seconds: 0 };
        let printed = finance::statement(&space, from, today()).and_then(|statement| {
            let position = finance::position(&space, today())?;
            Ok((
                ravenrs_app_report::statement_document(&statement, &culture, &words),
                ravenrs_app_report::position_document(&position, &culture, &words),
            ))
        });
        self.said = match printed {
            Ok((statement, position)) => format!(
                "Impresso: extrato com {} linhas e posição com {} linhas.",
                statement.page_text(0).len(),
                position.page_text(0).len()
            ),
            Err(error) => format!("Recusado: {error}"),
        };
    }

    fn change_paid(&mut self) {
        let mut space = self.database.space();
        let done = space
            .find(COMMITMENT, &Value::Number(BILL))
            .and_then(|found| found.ok_or_else(|| ravenrs_app::SpaceError::Refused("a conta sumiu".into())))
            .and_then(|mut bill: Object| {
                bill.set("amount", 5_000.0);
                space.save(bill)?;
                space.commit()
            })
            .map(|()| "Valor mudado.".to_owned());
        self.answer(done);
    }

    fn lines(&self) -> Vec<String> {
        let space = self.database.space();
        let mut lines = Vec::new();
        for bill in space.query(COMMITMENT, &Query::all()).unwrap_or_default() {
            lines.push(format!(
                "{}: {}, {}, saldo {}",
                bill.text("description"),
                bill.text("status"),
                brl(bill.cents("amount").unwrap_or_default()),
                brl(bill.cents("balance").unwrap_or_default()),
            ));
        }
        let entries = space.query(LEDGER_ENTRY, &Query::all()).unwrap_or_default();
        let total: i64 = entries.iter().filter_map(|entry| entry.cents("amount")).sum();
        lines.push(format!("Caixa: {} lançamento(s), {}", entries.len(), brl(total)));
        let plan = finance::plan(
            10_000,
            3,
            DateTime::from_ymd(2027, 1, 31).unwrap_or(DateTime { days: 0, seconds: 0 }),
        );
        let preview: Vec<String> = plan
            .iter()
            .map(|(due, cents)| {
                let (_, month, day) = due.ymd();
                format!("{day:02}/{month:02} {}", brl(*cents))
            })
            .collect();
        lines.push(format!("Prévia em 3: {}", preview.join(", ")));
        lines
    }
}

/// Every public feature of SettleInput, with the services of the finances, on one page.
#[derive(Debug, Default)]
pub struct FinancePage;

impl View for FinancePage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let demo = cx.use_state(Demo::new);
        let now = demo.get();
        let step = |label: &str, run: fn(&mut Demo)| {
            let demo = demo.clone();
            Button::new(label).on_click(move || demo.update(run))
        };
        let lines: Vec<Text> = now.lines().into_iter().map(Text::new).collect();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("A baixa lança no caixa no mesmo commit, com juros, multa e desconto. O estorno lança o contrário sem apagar nada. Conta paga não muda direto, e a parcela fecha no centavo.")
                    .foreground(Semantic::InkMuted),
                HStack::new((
                    step("Baixa parcial de 40", |d| d.settle(SettleInput::new(today(), 4_000))),
                    step("Baixar com juros, multa e desconto", |d| {
                        let open = d
                            .database
                            .space()
                            .find(COMMITMENT, &Value::Number(BILL))
                            .ok()
                            .flatten()
                            .and_then(|bill| bill.cents("balance"))
                            .unwrap_or_default();
                        d.settle(SettleInput::new(today(), open).interest(300).fine(200).discount(100));
                    }),
                    step("Baixa com data de amanhã", |d| {
                        let tomorrow = DateTime { days: today().days + 1, seconds: 0 };
                        d.settle(SettleInput::new(tomorrow, 1_000));
                    }),
                    step("Mudar o valor da conta", Demo::change_paid),
                ))
                .spacing(8.0),
                HStack::new((
                    step("Estornar sem motivo", |d| d.reverse_last("")),
                    step("Estornar a última baixa", |d| d.reverse_last("pago em duplicidade")),
                    step("Parcelar em 3", Demo::split),
                    step("Cancelar a conta", Demo::cancel),
                    step("Imprimir extrato e posição", Demo::print),
                    step("Recomeçar", |d| *d = Demo::new()),
                ))
                .spacing(8.0),
                Text::new(now.said.clone()),
                VStack::new(lines).spacing(4.0).align(Align::Start),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
