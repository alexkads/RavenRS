//! The verb that clicks twice, `double-click` or `duplo-clique`, as a person opens a row of a grid
//! (#921): read from a `.raventest` file in both languages, and played on a session.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

use ravenrs_e2e::E2e;
use ravenrs_e2e_wire::{Command, Kind, Scenario};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{BuildCx, DataGrid, GridColumn, GridData, NodeId, Value, View};

/// A grid of two jobs that says which one a double click opened.
struct Screen {
    opened: State<String>,
}

impl View for Screen {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let rows: Vec<Vec<Value>> = vec![vec!["site".into()], vec!["loja".into()]];
        let opened = self.opened.clone();
        cx.child(
            &DataGrid::new(GridData::new(rows), [GridColumn::text("Job").fill()])
                .key_column(0)
                .on_row_open(move |row: &Vec<Value>| opened.set(row[0].key_text()))
                .height(200.0),
        )
    }
}

#[test]
fn double_click_reads_in_both_languages() {
    assert_eq!(Kind::read("double-click"), Some(Kind::DoubleClick));
    assert_eq!(Kind::read("duplo-clique"), Some(Kind::DoubleClick));
    assert_eq!(Kind::DoubleClick.word(), "double-click");
    let scenario =
        Scenario::parse("scenario: abrir\nopen Jobs | Abrir\nduplo-clique #row:loja | Abrir a loja\n")
            .expect("a scenario");
    let command = scenario.blocks[1].command().expect("a command");
    assert_eq!(command, Command::DoubleClick { target: "#row:loja".to_owned() });
}

#[test]
fn a_double_click_opens_the_row_of_a_grid() {
    let opened = State::new(String::new());
    let mut e2e = E2e::new(Screen { opened: opened.clone() }, Theme::raven_dark());
    let scenario =
        Scenario::parse("scenario: abrir\nopen Jobs | Open\ndouble-click #row:loja | Open the store\n")
            .expect("a scenario");
    scenario.play(&mut e2e).expect("it plays");
    assert_eq!(opened.get(), "loja");
}
