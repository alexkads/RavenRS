//! The grid's filter and number formats moved onto shared code. These tests hold frozen copies of
//! what the grid did before, and check that nothing a user could already type or see changed.
#![allow(missing_docs)]

use proptest::prelude::*;
use ravenrs_widgets::grid::query::{GridQuery, matches_filter, run_query, to_csv};
use ravenrs_widgets::grid::{GridColumn, NumberFormat, Row, Value};

/// `matches_filter` as it was when `grid::query` parsed every filter itself.
fn old_matches_filter(value: &Value, display: &str, filter: &str) -> bool {
    type NumTest = fn(f64, f64) -> bool;
    let f = filter.trim();
    if f.is_empty() {
        return true;
    }
    let num = |s: &str| s.trim().replace(',', "").parse::<f64>().ok();
    if let Some(n) = value.as_f64().filter(|_| matches!(value, Value::Number(_))) {
        if let Some((a, b)) = f.split_once("..")
            && let (Some(a), Some(b)) = (num(a), num(b))
        {
            return n >= a && n <= b;
        }
        let ops: [(&str, NumTest); 7] = [
            (">=", |n, x| n >= x),
            ("<=", |n, x| n <= x),
            ("<>", |n, x| (n - x).abs() > f64::EPSILON),
            ("!=", |n, x| (n - x).abs() > f64::EPSILON),
            (">", |n, x| n > x),
            ("<", |n, x| n < x),
            ("=", |n, x| (n - x).abs() < 1e-9),
        ];
        for (op, test) in ops {
            if let Some(x) = f.strip_prefix(op).and_then(num) {
                return test(n, x);
            }
        }
    }
    let text = f.to_lowercase();
    if let Value::Bool(b) = value {
        match text.as_str() {
            "yes" | "true" | "1" | "y" | "sim" => return *b,
            "no" | "false" | "0" | "n" | "não" | "nao" => return !*b,
            _ => {}
        }
    }
    display.to_lowercase().contains(&text)
}

/// `NumberFormat::format` as it was before the digits moved to `ravenrs_criteria::NumberStyle`.
fn old_format(f: &NumberFormat, n: f64) -> String {
    if !n.is_finite() {
        return String::new();
    }
    let fixed = format!("{:.*}", f.decimals, n.abs());
    let (int, frac) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let mut grouped = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            grouped.push(f.thousands);
        }
        grouped.push(c);
    }
    let sign = if n < 0.0 && fixed.chars().any(|c| c != '0' && c != '.') { "-" } else { "" };
    let mut out = format!("{sign}{}{grouped}", f.prefix);
    if !frac.is_empty() {
        out.push(f.decimal);
        out.push_str(frac);
    }
    out.push_str(&f.suffix);
    out
}

fn sample_values() -> Vec<(Value, &'static str)> {
    vec![
        (Value::Number(0.0), "0"),
        (Value::Number(1.0), "1"),
        (Value::Number(15.0), "15"),
        (Value::Number(-3.5), "-3.5"),
        (Value::Number(1234.5), "1,234.5"),
        (Value::Bool(true), "Yes"),
        (Value::Bool(false), "No"),
        (Value::Text("Brazil".into()), "Brazil"),
        (Value::Text("não".into()), "não"),
        (Value::Text("[draft] > 3".into()), "[draft] > 3"),
        (Value::Text("1..2".into()), "1..2"),
        (Value::Empty, ""),
    ]
}

#[test]
fn every_short_form_answers_as_before() {
    let filters = [
        "",
        "  ",
        ">",
        ">=15",
        "<=15",
        "<15",
        ">15",
        "=15",
        "= 15",
        "<>15",
        "!=15",
        " >= 1,234.5 ",
        "5..20",
        "5 .. 20",
        "20..5",
        "..5",
        "5..",
        "a..b",
        "1..2",
        "yes",
        "YES",
        "true",
        "1",
        "y",
        "sim",
        "no",
        "false",
        "0",
        "n",
        "não",
        "NÃO",
        "nao",
        "bra",
        "BRA",
        "=bra",
        ">bra",
        "[draft]",
        "[draft] > 3",
        "[",
        "]",
        "1e3",
        ">1e1",
        "NaN",
        "inf",
        ">inf",
    ];
    for filter in filters {
        for (value, display) in sample_values() {
            assert_eq!(
                matches_filter(&value, display, filter),
                old_matches_filter(&value, display, filter),
                "{filter:?} on {value:?}"
            );
        }
    }
}

/// Text that is not an expression over this grid's columns must stay a short filter.
#[test]
fn run_query_treats_non_expressions_as_before() {
    let columns = [GridColumn::text("Name"), GridColumn::number("Seats")];
    let rows: Vec<Row> = vec![
        vec!["[draft] plan".into(), 10.into()],
        vec!["[Seats] > 3".into(), 1.into()],
        vec!["[Nope] > 3".into(), 2.into()],
        vec!["Sum([Seats])".into(), 3.into()],
        vec!["[Seats] = ?x".into(), 4.into()],
        vec!["[Seats] >".into(), 5.into()],
        vec!["Foo([Seats])".into(), 6.into()],
    ];
    let filters = [
        "[draft]",
        "[DRAFT] PLAN",
        "[Nope] > 3",
        "Sum([Seats])",
        "[Seats] = ?x",
        "[Seats] >",
        "Foo([Seats])",
    ];
    for (i, filter) in filters.into_iter().enumerate() {
        let mut query = GridQuery::default();
        query.set_filter(0, filter.into());
        let want: Vec<usize> = (0..rows.len())
            .filter(|&r| old_matches_filter(&rows[r][0], &columns[0].display(&rows[r][0]), filter))
            .collect();
        assert_eq!(run_query(&columns, &rows, &query).matched, want, "{filter:?}");
        assert!(!want.is_empty(), "case {i} should match its own row");
    }
}

fn expression_grid() -> (Vec<GridColumn>, Vec<Row>) {
    let columns = vec![
        GridColumn::text("Name"),
        GridColumn::text("Home Country"),
        GridColumn::currency("Seats", "$", 2),
        GridColumn::boolean("Active"),
    ];
    let rows = vec![
        vec!["Acme".into(), "Brazil".into(), 10.into(), true.into()],
        vec!["Globex".into(), "Chile".into(), 25.into(), false.into()],
        vec!["Initech".into(), "Brazil".into(), 50.into(), false.into()],
        vec!["Hooli".into(), Value::Empty, Value::Empty],
    ];
    (columns, rows)
}

fn matched(filter: &str, column: usize) -> Vec<usize> {
    let (columns, rows) = expression_grid();
    let mut query = GridQuery::default();
    query.set_filter(column, filter.into());
    run_query(&columns, &rows, &query).matched
}

#[test]
fn filter_row_accepts_expressions_over_column_titles() {
    assert_eq!(matched("[Seats] > 20", 2), [1, 2]);
    assert_eq!(matched("[Seats] > 20 And [Home Country] = 'Brazil'", 0), [2]);
    assert_eq!(matched("[Active] Or [Seats] = 50", 1), [0, 2]);
    assert_eq!(matched("[Name] Like 'G%'", 0), [1]);
    assert_eq!(matched("Upper([Name]) = 'ACME'", 0), [0]);
    assert_eq!(matched("[Home Country] Is Null", 0), [3]);
    assert_eq!(matched("[Seats] Between(10, 25)", 0), [0, 1]);
    assert_eq!(matched("[Active] Is Null", 0), [3], "a short row reads as empty");
}

#[test]
fn expressions_combine_with_short_filters_and_search() {
    let (columns, rows) = expression_grid();
    let mut query = GridQuery::default();
    query.set_filter(0, "[Seats] >= 10".into());
    query.set_filter(1, "bra".into());
    assert_eq!(run_query(&columns, &rows, &query).matched, [0, 2]);
    query.search = "initech".into();
    assert_eq!(run_query(&columns, &rows, &query).matched, [2]);
}

#[test]
fn expressions_read_raw_values_and_a_failure_matches_nothing() {
    // The cell shows "$10.00"; the expression sees 10.
    assert_eq!(matched("[Seats] = 10", 0), [0]);
    assert_eq!(matched("[Name] / 0 > 1", 0), Vec::<usize>::new());
}

#[test]
fn a_column_that_cannot_filter_ignores_its_expression() {
    let (mut columns, rows) = expression_grid();
    columns[0] = columns[0].clone().filterable(false);
    let mut query = GridQuery::default();
    query.set_filter(0, "[Seats] > 20".into());
    assert_eq!(run_query(&columns, &rows, &query).matched.len(), rows.len());
}

#[test]
fn csv_quotes_titles_and_fields_as_before() {
    let columns = [GridColumn::text("Name, full"), GridColumn::text("Notes"), GridColumn::number("N")];
    let rows: Vec<Row> = vec![
        vec!["Ana".into(), "say \"hi\"".into(), 1234.5.into()],
        vec!["a\nb".into(), "c\rd".into(), Value::Empty],
    ];
    let query = GridQuery::default();
    assert_eq!(
        to_csv(&columns, &rows, &query, &[0, 1]),
        "\"Name, full\",Notes,N\r\nAna,\"say \"\"hi\"\"\",1234.5\r\n\"a\nb\",\"c\rd\",\r\n"
    );
}

proptest! {
    #[test]
    fn short_filters_match_the_old_parser(
        n in -1e6f64..1e6,
        text in "[a-zA-Z0-9 ,.<>=!-]{0,8}",
        filter in "(>=|<=|<>|!=|>|<|=)?[0-9 ,.a-z-]{0,8}(\\.\\.[0-9 ,.-]{0,5})?",
    ) {
        for (value, display) in [
            (Value::Number(n), n.to_string()),
            (Value::Text(text.clone()), text.clone()),
            (Value::Bool(n > 0.0), if n > 0.0 { "Yes".to_owned() } else { "No".to_owned() }),
            (Value::Empty, String::new()),
        ] {
            prop_assert_eq!(
                matches_filter(&value, &display, &filter),
                old_matches_filter(&value, &display, &filter),
                "{:?} on {:?}", filter, value
            );
        }
    }

    #[test]
    fn number_format_matches_the_old_digits(
        n in prop_oneof![-1e12f64..1e12, -2.0f64..2.0, Just(f64::NAN), Just(f64::INFINITY), Just(-0.0)],
        decimals in 0usize..5,
        thousands in prop::sample::select(vec![',', '.', ' ', '\u{a0}', '’']),
        decimal in prop::sample::select(vec!['.', ',']),
        prefix in prop::sample::select(vec!["", "$", "R$ "]),
        suffix in prop::sample::select(vec!["", "%", " un"]),
    ) {
        let f = NumberFormat { decimals, prefix: prefix.into(), suffix: suffix.into(), thousands, decimal };
        prop_assert_eq!(f.format(n), old_format(&f, n));
    }
}
