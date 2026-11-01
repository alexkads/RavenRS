//! The data grid query engine: filter → search → sort → group → summarize.
//! Pure functions over rows, independent of the UI.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

use super::model::{GridColumn, Row, SortDirection, Summary, Value};

/// What the user asked the grid to show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GridQuery {
    /// Sort columns in priority order.
    pub sort: Vec<(usize, SortDirection)>,
    /// Filter-row text per column (index = column).
    pub filters: Vec<String>,
    /// Text searched in every visible column.
    pub search: String,
    /// Grouping columns, outermost first.
    pub group_by: Vec<usize>,
    /// Keys of collapsed groups (see [`ViewRow::Group::key`]).
    pub collapsed: BTreeSet<String>,
    /// Hidden columns are ignored by search.
    pub hidden: Vec<bool>,
}

impl GridQuery {
    /// The filter text of `column` (zero-based), or `""`.
    pub fn filter(&self, column: usize) -> &str {
        self.filters.get(column).map_or("", String::as_str)
    }

    /// Sets the filter text of `column` (zero-based).
    pub fn set_filter(&mut self, column: usize, text: String) {
        if self.filters.len() <= column {
            self.filters.resize(column + 1, String::new());
        }
        self.filters[column] = text;
    }

    /// The column (zero-based) is hidden.
    pub fn is_hidden(&self, column: usize) -> bool {
        self.hidden.get(column).copied().unwrap_or(false)
    }

    /// Plain click: sort by this column only, cycling ascending → descending →
    /// none. With `add`, the column is added to (or cycled within) a multi-column sort.
    pub fn toggle_sort(&mut self, column: usize, add: bool) {
        let current = self.sort.iter().position(|(c, _)| *c == column);
        let next = match current.map(|i| self.sort[i].1) {
            None => Some(SortDirection::Ascending),
            Some(SortDirection::Ascending) => Some(SortDirection::Descending),
            Some(SortDirection::Descending) => None,
        };
        if !add {
            self.sort.clear();
            if let Some(d) = next {
                self.sort.push((column, d));
            }
            return;
        }
        match (current, next) {
            (Some(i), Some(d)) => self.sort[i].1 = d,
            (Some(i), None) => {
                self.sort.remove(i);
            }
            (None, Some(d)) => self.sort.push((column, d)),
            (None, None) => {}
        }
    }

    /// Adds the column to or removes it from the grouping; expands all groups.
    pub fn toggle_group(&mut self, column: usize) {
        if let Some(i) = self.group_by.iter().position(|c| *c == column) {
            self.group_by.remove(i);
        } else {
            self.group_by.push(column);
        }
        self.collapsed.clear();
    }

    /// Collapses or expands the group with this key.
    pub fn toggle_collapsed(&mut self, key: &str) {
        if !self.collapsed.remove(key) {
            self.collapsed.insert(key.to_owned());
        }
    }
}

/// A visible row of the grid: a group header or a data row.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewRow {
    /// A group header.
    Group {
        /// Nesting depth, 0 for the outermost groups.
        level: usize,
        /// The column grouped by (zero-based).
        column: usize,
        /// Display text of the group's value.
        label: String,
        /// Unique path of this group, used to collapse it.
        key: String,
        /// Number of data rows in the group.
        count: usize,
        /// The group's rows are hidden.
        collapsed: bool,
        /// Aggregates per column over the group's rows.
        summaries: Vec<Option<f64>>,
    },
    /// Index into the data rows.
    Data(usize),
}

/// The output of [`run_query`]: what the grid shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueryResult {
    /// Visible rows in display order.
    pub rows: Vec<ViewRow>,
    /// Matching data rows in display order (collapsed groups included).
    pub matched: Vec<usize>,
    /// Aggregates per column over all matching rows.
    pub totals: Vec<Option<f64>>,
}

/// Does `value` (shown as `display`) pass a filter-row expression?
///
/// Numbers accept `>`, `>=`, `<`, `<=`, `=`, `<>` and ranges `a..b`; everything
/// else is a case-insensitive "contains".
pub fn matches_filter(value: &Value, display: &str, filter: &str) -> bool {
    Filter::parse(filter).matches(value, || Cow::Borrowed(display))
}

type NumTest = fn(f64, f64) -> bool;

/// A filter-row expression parsed once per query instead of once per row.
#[derive(Debug)]
struct Filter {
    /// The comparison applied to numbers, when the text is one.
    number: Option<NumberTest>,
    /// The answer for booleans, when the text is a yes/no word.
    boolean: Option<bool>,
    /// The lowercased text for the "contains" fallback.
    text: String,
}

#[derive(Debug)]
enum NumberTest {
    Range(f64, f64),
    Compare(NumTest, f64),
}

impl Filter {
    fn parse(filter: &str) -> Self {
        let f = filter.trim();
        let num = |s: &str| s.trim().replace(',', "").parse::<f64>().ok();
        let mut number = None;
        if let Some((a, b)) = f.split_once("..")
            && let (Some(a), Some(b)) = (num(a), num(b))
        {
            number = Some(NumberTest::Range(a, b));
        }
        if number.is_none() {
            let ops: [(&str, NumTest); 7] = [
                (">=", |n, x| n >= x),
                ("<=", |n, x| n <= x),
                ("<>", |n, x| (n - x).abs() > f64::EPSILON),
                ("!=", |n, x| (n - x).abs() > f64::EPSILON),
                (">", |n, x| n > x),
                ("<", |n, x| n < x),
                ("=", |n, x| (n - x).abs() < 1e-9),
            ];
            number = ops.into_iter().find_map(|(op, test)| {
                f.strip_prefix(op).and_then(num).map(|x| NumberTest::Compare(test, x))
            });
        }
        let text = f.to_lowercase();
        let boolean = match text.as_str() {
            "yes" | "true" | "1" | "y" | "sim" => Some(true),
            "no" | "false" | "0" | "n" | "não" | "nao" => Some(false),
            _ => None,
        };
        Self { number, boolean, text }
    }

    /// `display` is only called when the value has to be matched as text.
    fn matches<'a>(&self, value: &Value, display: impl FnOnce() -> Cow<'a, str>) -> bool {
        if self.text.is_empty() {
            return true;
        }
        match (value, &self.number, self.boolean) {
            (Value::Number(n), Some(NumberTest::Range(a, b)), _) => *n >= *a && *n <= *b,
            (Value::Number(n), Some(NumberTest::Compare(test, x)), _) => test(*n, *x),
            (Value::Bool(b), _, Some(want)) => *b == want,
            _ => contains_lowercase(&display(), &self.text),
        }
    }
}

/// Case-insensitive "contains" where `needle` is already lowercase. ASCII text,
/// the common case, is compared in place without allocating.
fn contains_lowercase(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.is_ascii() && needle.is_ascii() {
        let (h, n) = (haystack.as_bytes(), needle.as_bytes());
        return h.windows(n.len()).any(|w| w.iter().zip(n).all(|(a, b)| a.to_ascii_lowercase() == *b));
    }
    haystack.to_lowercase().contains(needle)
}

/// A precomputed sort key, so sorting does not format or allocate per comparison.
/// Text becomes its rank among the column's distinct lowercase texts.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Key {
    Empty,
    Num(f64),
    Text(u32),
}

/// Sort keys of `column` for the rows in `indices`, in the same order.
fn keys_of(column: &GridColumn, c: usize, rows: &[Row], indices: &[usize]) -> Vec<Key> {
    let empty = Value::Empty;
    let mut texts: HashMap<Cow<'_, str>, u32> = HashMap::new();
    let mut keys: Vec<Key> = indices
        .iter()
        .map(|&r| match rows[r].get(c).unwrap_or(&empty) {
            Value::Number(n) => Key::Num(*n),
            Value::Bool(b) => Key::Num(f64::from(u8::from(*b))),
            v @ Value::Text(_) => {
                let next = texts.len() as u32;
                Key::Text(*texts.entry(column.display_cow(v)).or_insert(next))
            }
            _ => Key::Empty,
        })
        .collect();
    if !texts.is_empty() {
        // Distinct texts sorted case-insensitively; equal lowercase shares a rank.
        let mut distinct: Vec<(String, u32)> = texts.iter().map(|(t, &id)| (t.to_lowercase(), id)).collect();
        distinct.sort_unstable();
        let mut rank_of = vec![0u32; distinct.len()];
        let mut rank = 0;
        for (i, (text, id)) in distinct.iter().enumerate() {
            if i > 0 && *text != distinct[i - 1].0 {
                rank += 1;
            }
            rank_of[*id as usize] = rank;
        }
        for key in &mut keys {
            if let Key::Text(id) = key {
                *id = rank_of[*id as usize];
            }
        }
    }
    keys
}

fn cmp_keys(a: &Key, b: &Key) -> Ordering {
    match (a, b) {
        (Key::Empty, Key::Empty) => Ordering::Equal,
        // Empty values sort last in ascending order.
        (Key::Empty, _) => Ordering::Greater,
        (_, Key::Empty) => Ordering::Less,
        (Key::Num(x), Key::Num(y)) => x.total_cmp(y),
        (Key::Text(x), Key::Text(y)) => x.cmp(y),
        (Key::Num(_), Key::Text(_)) => Ordering::Less,
        (Key::Text(_), Key::Num(_)) => Ordering::Greater,
    }
}

/// False when no number this column shows can contain `needle`, so search can
/// skip formatting its numbers.
fn numbers_may_contain(column: &GridColumn, needle: &str) -> bool {
    if column.format.is_some() {
        return true;
    }
    let (extra, thousands, decimal) = match &column.number {
        Some(f) => (format!("{}{}", f.prefix, f.suffix).to_lowercase(), f.thousands, f.decimal),
        None => (String::new(), ',', '.'),
    };
    needle.chars().all(|ch| {
        ch.is_ascii_digit() || "-.infa".contains(ch) || ch == thousands || ch == decimal || extra.contains(ch)
    })
}

fn summarize(summary: Summary, column: usize, rows: &[Row], indices: &[usize]) -> Option<f64> {
    let values = indices.iter().filter_map(|&i| rows[i].get(column)).filter(|v| !v.is_empty());
    match summary {
        Summary::Count => Some(values.count() as f64),
        Summary::Sum => Some(values.filter_map(Value::as_f64).sum()),
        Summary::Average => {
            let (sum, n) = values.filter_map(Value::as_f64).fold((0.0, 0usize), |(s, n), v| (s + v, n + 1));
            (n > 0).then(|| sum / n as f64)
        }
        Summary::Min => values.filter_map(Value::as_f64).reduce(f64::min),
        Summary::Max => values.filter_map(Value::as_f64).reduce(f64::max),
    }
}

fn summaries(columns: &[GridColumn], rows: &[Row], indices: &[usize]) -> Vec<Option<f64>> {
    columns
        .iter()
        .enumerate()
        .map(|(c, col)| col.summary.and_then(|s| summarize(s, c, rows, indices)))
        .collect()
}

/// Runs the whole pipeline.
pub fn run_query(columns: &[GridColumn], rows: &[Row], query: &GridQuery) -> QueryResult {
    let search = query.search.trim().to_lowercase();
    let active_filters: Vec<(usize, Filter)> = (0..columns.len())
        .map(|c| (c, query.filter(c)))
        .filter(|(c, f)| !f.trim().is_empty() && columns[*c].filterable)
        .map(|(c, f)| (c, Filter::parse(f)))
        .collect();
    let searched: Vec<(usize, bool)> = (0..columns.len())
        .filter(|&c| !query.is_hidden(c))
        .map(|c| (c, numbers_may_contain(&columns[c], &search)))
        .collect();
    let empty = Value::Empty;
    let cell = |r: usize, c: usize| rows[r].get(c).unwrap_or(&empty);

    let mut indices: Vec<usize> = (0..rows.len())
        .filter(|&r| {
            active_filters.iter().all(|(c, f)| {
                let v = cell(r, *c);
                f.matches(v, || columns[*c].display_cow(v))
            })
        })
        .filter(|&r| {
            search.is_empty()
                || searched.iter().any(|&(c, numbers)| {
                    let v = cell(r, c);
                    (numbers || !matches!(v, Value::Number(_)))
                        && contains_lowercase(&columns[c].display_cow(v), &search)
                })
        })
        .collect();

    // Group columns sort first (in their sort direction if they have one).
    let mut order: Vec<(usize, SortDirection)> = query
        .group_by
        .iter()
        .map(|&g| (g, query.sort.iter().find(|(c, _)| *c == g).map_or(SortDirection::Ascending, |s| s.1)))
        .collect();
    order.extend(query.sort.iter().filter(|(c, _)| !query.group_by.contains(c)).copied());
    order.retain(|(c, _)| *c < columns.len());
    if !order.is_empty() {
        // Keys only for matching rows, indexed by position in `indices`.
        let keys: Vec<Vec<Key>> =
            order.iter().map(|&(c, _)| keys_of(&columns[c], c, rows, &indices)).collect();
        let mut positions: Vec<usize> = (0..indices.len()).collect();
        // Ties fall back to the original position, so the result matches a stable sort.
        positions.sort_unstable_by(|&a, &b| {
            for (k, (_, dir)) in order.iter().enumerate() {
                let o = cmp_keys(&keys[k][a], &keys[k][b]);
                if o != Ordering::Equal {
                    return dir.apply(o);
                }
            }
            a.cmp(&b)
        });
        indices = positions.into_iter().map(|p| indices[p]).collect();
    }

    let totals = summaries(columns, rows, &indices);
    let mut out = Vec::with_capacity(indices.len());
    let groups: Vec<usize> = query.group_by.iter().copied().filter(|&g| g < columns.len()).collect();
    emit_groups(columns, rows, query, &groups, 0, &indices, "", &mut out);
    QueryResult { rows: out, matched: indices, totals }
}

#[allow(clippy::too_many_arguments)]
fn emit_groups(
    columns: &[GridColumn],
    rows: &[Row],
    query: &GridQuery,
    groups: &[usize],
    level: usize,
    indices: &[usize],
    parent_key: &str,
    out: &mut Vec<ViewRow>,
) {
    let Some(&column) = groups.get(level) else {
        out.extend(indices.iter().map(|&i| ViewRow::Data(i)));
        return;
    };
    let empty = Value::Empty;
    let label_of = |r: usize| columns[column].display_cow(rows[r].get(column).unwrap_or(&empty));
    let mut start = 0;
    while start < indices.len() {
        let label = label_of(indices[start]).into_owned();
        let mut end = start + 1;
        while end < indices.len() && *label_of(indices[end]) == *label {
            end += 1;
        }
        let run = &indices[start..end];
        let key = format!("{parent_key}\u{1f}{column}:{label}");
        let collapsed = query.collapsed.contains(&key);
        out.push(ViewRow::Group {
            level,
            column,
            label: label.clone(),
            key: key.clone(),
            count: run.len(),
            collapsed,
            summaries: summaries(columns, rows, run),
        });
        if !collapsed {
            emit_groups(columns, rows, query, groups, level + 1, run, &key, out);
        }
        start = end;
    }
}

/// Rows as tab-separated text, the way spreadsheets copy and paste: the
/// shown (formatted) values of the `visible` columns, no header.
pub fn to_tsv(columns: &[GridColumn], rows: &[Row], visible: &[usize], selected: &[usize]) -> String {
    let empty = Value::Empty;
    let clean = |s: String| s.replace(['\t', '\n', '\r'], " ");
    selected
        .iter()
        .filter_map(|&r| rows.get(r))
        .map(|row| {
            visible
                .iter()
                .map(|&c| clean(columns[c].display(row.get(c).unwrap_or(&empty))))
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Matching rows as CSV (RFC 4180), visible columns only, in display order.
pub fn to_csv(columns: &[GridColumn], rows: &[Row], query: &GridQuery, matched: &[usize]) -> String {
    let quote = |s: &str| {
        if s.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_owned()
        }
    };
    let visible: Vec<usize> = (0..columns.len()).filter(|&c| !query.is_hidden(c)).collect();
    let mut out = visible.iter().map(|&c| quote(&columns[c].title)).collect::<Vec<_>>().join(",");
    out.push_str("\r\n");
    let empty = Value::Empty;
    for &r in matched {
        let line = visible
            .iter()
            .map(|&c| {
                let v = rows[r].get(c).unwrap_or(&empty);
                // Raw numbers export unformatted so spreadsheets read them as numbers.
                match v {
                    Value::Number(n) => n.to_string(),
                    _ => quote(&columns[c].display(v)),
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&line);
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::model::Summary;

    fn columns() -> Vec<GridColumn> {
        vec![
            GridColumn::text("Name"),
            GridColumn::text("Country"),
            GridColumn::number("Seats").summary(Summary::Sum),
            GridColumn::boolean("Active"),
        ]
    }

    fn rows() -> Vec<Row> {
        vec![
            vec!["Acme".into(), "Brazil".into(), 10.into(), true.into()],
            vec!["Globex".into(), "Chile".into(), 25.into(), false.into()],
            vec!["Initech".into(), "Brazil".into(), 5.into(), true.into()],
            vec!["umbrella".into(), "Argentina".into(), 40.into(), true.into()],
            vec!["Hooli".into(), Value::Empty, 15.into(), false.into()],
        ]
    }

    fn names(result: &QueryResult, rows: &[Row]) -> Vec<String> {
        result
            .rows
            .iter()
            .map(|r| match r {
                ViewRow::Data(i) => match &rows[*i][0] {
                    Value::Text(s) => s.clone(),
                    _ => String::new(),
                },
                ViewRow::Group { label, count, .. } => format!("[{label} {count}]"),
            })
            .collect()
    }

    #[test]
    fn filters_with_operators_ranges_and_text() {
        let (c, r) = (columns(), rows());
        let mut q = GridQuery::default();
        q.set_filter(2, ">=15".into());
        assert_eq!(names(&run_query(&c, &r, &q), &r), ["Globex", "umbrella", "Hooli"]);
        q.set_filter(2, "5..12".into());
        assert_eq!(names(&run_query(&c, &r, &q), &r), ["Acme", "Initech"]);
        q.set_filter(2, String::new());
        q.set_filter(1, "BRA".into());
        q.set_filter(3, "yes".into());
        assert_eq!(names(&run_query(&c, &r, &q), &r), ["Acme", "Initech"]);
    }

    #[test]
    fn search_ignores_hidden_columns() {
        let (c, r) = (columns(), rows());
        let mut q = GridQuery { search: "chile".into(), ..Default::default() };
        assert_eq!(names(&run_query(&c, &r, &q), &r), ["Globex"]);
        q.hidden = vec![false, true];
        assert!(run_query(&c, &r, &q).rows.is_empty());
    }

    #[test]
    fn multi_column_sort_is_case_insensitive_with_empties_last() {
        let (c, r) = (columns(), rows());
        let mut q = GridQuery::default();
        q.toggle_sort(1, false);
        q.toggle_sort(2, true);
        q.toggle_sort(2, true); // second click: descending
        assert_eq!(q.sort, vec![(1, SortDirection::Ascending), (2, SortDirection::Descending)]);
        assert_eq!(names(&run_query(&c, &r, &q), &r), ["umbrella", "Acme", "Initech", "Globex", "Hooli"]);
        q.toggle_sort(0, false);
        assert_eq!(q.sort, vec![(0, SortDirection::Ascending)], "plain click replaces the sort");
        assert_eq!(names(&run_query(&c, &r, &q), &r), ["Acme", "Globex", "Hooli", "Initech", "umbrella"]);
    }

    #[test]
    fn groups_with_counts_summaries_and_collapse() {
        let (c, r) = (columns(), rows());
        let mut q = GridQuery::default();
        q.toggle_group(1);
        let result = run_query(&c, &r, &q);
        assert_eq!(
            names(&result, &r),
            [
                "[Argentina 1]",
                "umbrella",
                "[Brazil 2]",
                "Acme",
                "Initech",
                "[Chile 1]",
                "Globex",
                "[ 1]",
                "Hooli"
            ]
        );
        let brazil = result.rows.iter().find_map(|row| match row {
            ViewRow::Group { label, key, summaries, .. } if label == "Brazil" => {
                Some((key.clone(), summaries[2]))
            }
            _ => None,
        });
        let (key, seats) = brazil.expect("Brazil group");
        assert_eq!(seats, Some(15.0));
        assert_eq!(result.totals[2], Some(95.0));
        q.toggle_collapsed(&key);
        assert_eq!(
            names(&run_query(&c, &r, &q), &r),
            ["[Argentina 1]", "umbrella", "[Brazil 2]", "[Chile 1]", "Globex", "[ 1]", "Hooli"]
        );
    }

    #[test]
    fn nested_groups() {
        let (c, r) = (columns(), rows());
        let q = GridQuery { group_by: vec![3, 1], ..Default::default() };
        let result = run_query(&c, &r, &q);
        let levels: Vec<usize> = result
            .rows
            .iter()
            .filter_map(|row| if let ViewRow::Group { level, .. } = row { Some(*level) } else { None })
            .collect();
        assert_eq!(levels, [0, 1, 1, 0, 1, 1]);
    }

    #[test]
    fn csv_quotes_and_keeps_raw_numbers() {
        let c = vec![GridColumn::text("Name"), GridColumn::currency("Amount", "$", 2)];
        let r = vec![vec!["Smith, \"J\"".into(), 1234.5.into()]];
        let q = GridQuery::default();
        let csv = to_csv(&c, &r, &q, &[0]);
        assert_eq!(csv, "Name,Amount\r\n\"Smith, \"\"J\"\"\",1234.5\r\n");
    }

    #[test]
    fn hundred_thousand_rows_query_quickly() {
        let c = columns();
        let r: Vec<Row> = (0..100_000)
            .map(|i| {
                vec![
                    format!("Company {i}").into(),
                    ["Brazil", "Chile", "Peru"][i % 3].into(),
                    (i % 97).into(),
                    (i % 2 == 0).into(),
                ]
            })
            .collect();
        let q =
            GridQuery { sort: vec![(0, SortDirection::Descending)], group_by: vec![1], ..Default::default() };
        let start = std::time::Instant::now();
        let result = run_query(&c, &r, &q);
        assert_eq!(result.matched.len(), 100_000);
        assert_eq!(result.rows.len(), 100_003);
        // Generous bound for debug builds on CI-class machines.
        assert!(start.elapsed().as_secs_f32() < 5.0, "took {:?}", start.elapsed());
    }
}
