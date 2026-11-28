//! The finance reports printed: what the services read becomes a document, in the culture's money
//! and dates (#859).
#![allow(clippy::expect_used)]

use ravenrs_app::{MemoryDatabase, ObjectSpace, Value};
use ravenrs_app_library::finance::{
    self, COMMITMENT, SettleInput, annual, forecast, position, settle, statement,
};
use ravenrs_app_report::{
    ReportWords, annual_document, forecast_document, position_document, statement_document,
};
use ravenrs_criteria::{Culture, DateTime};

fn on(days: i64) -> DateTime {
    let today = ravenrs_app::local_now();
    DateTime { days: today.days + days, seconds: 0 }
}

fn text(document: &ravenrs_report::Document) -> String {
    // The heads of the columns print in capitals: what is said is compared without case.
    (0..document.pages.len())
        .flat_map(|page| document.page_text(page))
        .collect::<Vec<_>>()
        .join(" | ")
        .to_lowercase()
}

#[test]
fn the_statement_year_position_and_forecast_print_in_reais() {
    let database = MemoryDatabase::new(finance::entities()).expect("entities fit");
    let mut space = database.space();
    for (id, kind, description, cents, due) in
        [(1.0, "payable", "Aluguel", 150_000.0, -2), (2.0, "receivable", "Dízimo", 48_000.0, 5)]
    {
        let mut bill = space.new_object(COMMITMENT).expect("known");
        bill.set("id", id);
        bill.set("kind", kind);
        bill.set("description", description);
        bill.set("amount", cents);
        bill.set("due_on", Value::Date(on(due)));
        bill.set("status", "open");
        space.insert(bill).expect("waits");
    }
    space.commit().expect("kept");
    settle(&mut space, &Value::Number(2.0), SettleInput::new(on(0), 48_000).interest(500), on(0))
        .expect("settled");

    let (culture, words) = (Culture::pt_br(), ReportWords::pt_br());
    let printed =
        text(&statement_document(&statement(&space, on(-30), on(0)).expect("reads"), &culture, &words));
    assert!(printed.contains("extrato"), "{printed}");
    assert!(printed.contains("saldo anterior") && printed.contains("saldo final"), "{printed}");
    assert!(printed.contains("485,00"), "the entry, with the interest: {printed}");

    let (year_now, month_now, _) = on(0).ymd();
    let year = annual(&space, year_now).expect("reads");
    let printed = text(&annual_document(year_now, &year, &culture, &words));
    assert!(
        printed.contains("entradas") && printed.contains(&format!("{month_now:02}/{year_now}")),
        "{printed}"
    );

    let printed = text(&position_document(&position(&space, on(0)).expect("reads"), &culture, &words));
    assert!(
        printed.contains("a pagar") && printed.contains("1.500,00"),
        "the rent, open and overdue: {printed}"
    );

    let printed = text(&forecast_document(&forecast(&space, on(0), on(0)).expect("reads"), &culture, &words));
    assert!(printed.contains("aluguel"), "{printed}");
}
