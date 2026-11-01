//! Property tests for the widgets crate's pure logic: dates, the data grid
//! query engine, number formatting and text editing.

use std::collections::BTreeSet;

use proptest::prelude::*;
use ravenrs_core::{Event, Key, Modifiers, Point, PointerButton, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::grid::query::{GridQuery, ViewRow, matches_filter, run_query};
use ravenrs_widgets::grid::{GridColumn, NumberFormat, Row, SortDirection, Summary, Value};
use ravenrs_widgets::{Date, TextField, Ui, VStack};

fn date() -> impl Strategy<Value = Date> {
    (1600i32..2400, 1u8..=12, 1u8..=31).prop_filter_map("valid date", |(y, m, d)| Date::new(y, m, d))
}

fn rows() -> impl Strategy<Value = Vec<Row>> {
    let country = prop_oneof![Just("Brazil"), Just("Chile"), Just("Peru"), Just("")];
    proptest::collection::vec(("[a-z]{0,6}", country, -1000i32..1000, any::<bool>()), 0..60).prop_map(|rs| {
        rs.into_iter()
            .map(|(name, c, n, b)| {
                let country = if c.is_empty() { Value::Empty } else { Value::from(c) };
                vec![Value::from(name), country, Value::from(n), Value::from(b)]
            })
            .collect()
    })
}

fn columns() -> Vec<GridColumn> {
    vec![
        GridColumn::text("Name"),
        GridColumn::text("Country"),
        GridColumn::number("N").summary(Summary::Sum),
        GridColumn::boolean("Flag"),
    ]
}

fn sort_key(v: &Value) -> (u8, f64, String) {
    match v {
        Value::Empty => (2, 0.0, String::new()),
        Value::Number(n) => (0, *n, String::new()),
        Value::Bool(b) => (0, f64::from(u8::from(*b)), String::new()),
        Value::Text(s) => (1, 0.0, s.to_lowercase()),
        // `Value` is non-exhaustive: future kinds sort after everything else here.
        _ => (3, 0.0, String::new()),
    }
}

proptest! {
    #[test]
    fn days_round_trip_and_weekdays_advance(d in date(), k in -100_000i32..100_000) {
        prop_assert_eq!(Date::from_days(d.to_days()), d);
        let later = d.add_days(k);
        prop_assert_eq!(later.to_days() - d.to_days(), i64::from(k));
        prop_assert_eq!((i64::from(d.weekday()) + i64::from(k)).rem_euclid(7), i64::from(later.weekday()));
    }

    #[test]
    fn adding_months_keeps_valid_dates(d in date(), m in -240i32..240) {
        let moved = d.add_months(m);
        prop_assert!(Date::new(moved.year(), moved.month(), moved.day()).is_some());
        prop_assert!(moved.day() <= d.day());
        let months = |x: Date| i64::from(x.year()) * 12 + i64::from(x.month());
        prop_assert_eq!(months(moved) - months(d), i64::from(m));
    }

    #[test]
    fn filtered_rows_all_match_and_none_are_lost(data in rows(), f in prop_oneof![Just(">0"), Just("<=10"), Just("-5..5"), Just("a"), Just("yes"), Just("")]) {
        let cols = columns();
        let mut q = GridQuery::default();
        let column = if f.chars().any(|c| c.is_ascii_digit()) { 2 } else if f == "yes" { 3 } else { 0 };
        q.set_filter(column, f.to_string());
        let r = run_query(&cols, &data, &q);
        let expected: BTreeSet<usize> = (0..data.len())
            .filter(|&i| matches_filter(&data[i][column], &cols[column].display(&data[i][column]), f))
            .collect();
        prop_assert_eq!(r.matched.iter().copied().collect::<BTreeSet<_>>(), expected);
        prop_assert_eq!(r.matched.len(), r.rows.len(), "no grouping: one view row per match");
    }

    #[test]
    fn sorting_orders_rows_and_keeps_every_row(data in rows(), column in 0usize..4, desc in any::<bool>()) {
        let cols = columns();
        let dir = if desc { SortDirection::Descending } else { SortDirection::Ascending };
        let q = GridQuery { sort: vec![(column, dir)], ..Default::default() };
        let r = run_query(&cols, &data, &q);
        prop_assert_eq!(r.matched.iter().copied().collect::<BTreeSet<_>>(), (0..data.len()).collect());
        for w in r.matched.windows(2) {
            let (a, b) = (sort_key(&data[w[0]][column]), sort_key(&data[w[1]][column]));
            let ordered = match dir {
                SortDirection::Ascending => a.partial_cmp(&b) != Some(std::cmp::Ordering::Greater),
                SortDirection::Descending => a.partial_cmp(&b) != Some(std::cmp::Ordering::Less),
            };
            prop_assert!(ordered, "{a:?} then {b:?} is out of {dir:?} order");
        }
    }

    #[test]
    fn groups_partition_the_rows_and_sum_correctly(data in rows(), collapse_first in any::<bool>()) {
        let cols = columns();
        let mut q = GridQuery { group_by: vec![1], ..Default::default() };
        let r = run_query(&cols, &data, &q);
        let groups: Vec<(String, usize, Option<f64>)> = r.rows.iter().filter_map(|row| match row {
            ViewRow::Group { key, count, summaries, .. } => Some((key.clone(), *count, summaries[2])),
            ViewRow::Data(_) => None,
        }).collect();
        prop_assert_eq!(groups.iter().map(|g| g.1).sum::<usize>(), data.len());
        let total: f64 = groups.iter().filter_map(|g| g.2).sum();
        prop_assert!((total - r.totals[2].unwrap_or(0.0)).abs() < 1e-6);
        if collapse_first && let Some((key, count, _)) = groups.first() {
            q.toggle_collapsed(key);
            let collapsed = run_query(&cols, &data, &q);
            prop_assert_eq!(collapsed.rows.len(), r.rows.len() - count);
        }
    }

    #[test]
    fn csv_has_a_header_and_one_line_per_match(data in rows()) {
        let cols = columns();
        let q = GridQuery::default();
        let r = run_query(&cols, &data, &q);
        let csv = ravenrs_widgets::grid::query::to_csv(&cols, &data, &q, &r.matched);
        prop_assert_eq!(csv.matches("\r\n").count(), data.len() + 1);
    }

    #[test]
    fn search_matches_a_lowercase_contains_on_every_visible_column(
        names in proptest::collection::vec("[a-zA-ZçÇãÃéÉİß ]{0,8}", 0..40),
        needle in "[a-zA-ZçÇãÃéÉ]{0,3}",
        hide_name in any::<bool>(),
    ) {
        let cols = vec![GridColumn::text("Name"), GridColumn::number("N")];
        let data: Vec<Row> = names.iter().enumerate().map(|(i, n)| vec![Value::from(n.as_str()), Value::from(i)]).collect();
        let mut q = GridQuery { search: needle.clone(), ..GridQuery::default() };
        if hide_name {
            q.hidden = vec![true, false];
        }
        let expected: Vec<usize> = (0..data.len())
            .filter(|&r| {
                let s = needle.trim().to_lowercase();
                s.is_empty()
                    || (!hide_name && names[r].to_lowercase().contains(&s))
                    || r.to_string().contains(&s)
            })
            .collect();
        prop_assert_eq!(run_query(&cols, &data, &q).matched, expected);
    }

    #[test]
    fn text_filter_matches_a_lowercase_contains(
        names in proptest::collection::vec("[a-zA-ZçÇãÃéÉ ]{0,8}", 0..40),
        filter in "[a-zA-ZçÇãÃéÉ]{1,3}",
    ) {
        for n in &names {
            let want = n.to_lowercase().contains(&filter.to_lowercase());
            prop_assert_eq!(matches_filter(&Value::from(n.as_str()), n, &filter), want, "{:?} in {:?}", filter, n);
        }
    }

    #[test]
    fn number_format_round_trips_through_parse(n in -1.0e9f64..1.0e9, decimals in 0usize..4) {
        let col = GridColumn::number("X").number_format(NumberFormat { decimals, ..NumberFormat::default() });
        let text = col.display(&Value::Number(n));
        let parsed = col.parse(&text, &Value::Number(0.0)).as_f64().unwrap_or(f64::NAN);
        let step = 10f64.powi(-(decimals as i32));
        prop_assert!((parsed - n).abs() <= step / 2.0 + 1e-6 * n.abs().max(1.0), "{n} -> {text} -> {parsed}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Typing any text into a field (through the UI, as a user would) stores
    /// exactly that text, and Backspace removes one character at a time.
    #[test]
    fn text_field_stores_what_was_typed(text in "[a-zA-Z0-9çãé ]{0,16}", erase in 0usize..20) {
        let value = State::new(String::new());
        let mut ui = Ui::new(VStack::new((TextField::new(value.binding()),)), Theme::raven_dark());
        ui.set_viewport(Size::new(400.0, 200.0), 1.0);
        ui.update();
        let at = Point::new(50.0, 20.0);
        ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
        ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
        ui.update();
        for c in text.chars() {
            let key = if c == ' ' { Key::Space } else { Key::Character(c.to_string()) };
            ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
        }
        prop_assert_eq!(value.get(), text.clone());
        for _ in 0..erase {
            ui.handle_event(&Event::KeyDown { key: Key::Backspace, modifiers: Modifiers::default() });
        }
        let kept: String = text.chars().take(text.chars().count().saturating_sub(erase)).collect();
        prop_assert_eq!(value.get(), kept);
    }
}
