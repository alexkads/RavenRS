//! How a grid is arranged, apart from its data: what a saved scenario holds.
//!
//! Columns go by title and not by position, so a scenario survives a column being
//! added or a window too narrow for some of them.

use serde::{Deserialize, Serialize};

use super::model::{GridColumn, SortDirection, Summary};
use super::query::GridQuery;

/// One column's place in a [`GridLayout`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ColumnLayout {
    /// Its title.
    pub title: String,
    /// Its width in logical pixels.
    pub width: f32,
    /// It is out of sight.
    pub hidden: bool,
    /// It stays in place at the left while the others scroll.
    pub pinned: bool,
}

/// One sorted column of a [`GridLayout`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct SortLayout {
    /// Its title.
    pub title: String,
    /// Largest first.
    pub descending: bool,
}

/// One column's filter in a [`GridLayout`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FilterLayout {
    /// Its title.
    pub title: String,
    /// What the filter row holds for it.
    pub text: String,
}

/// One column's summary in a [`GridLayout`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct SummaryLayout {
    /// Its title.
    pub title: String,
    /// `count`, `sum`, `average`, `min` or `max`; empty for none.
    pub kind: String,
    /// The formula that takes the place of the kind.
    pub formula: Option<String>,
}

fn summary_name(summary: Summary) -> &'static str {
    match summary {
        Summary::Count => "count",
        Summary::Average => "average",
        Summary::Min => "min",
        Summary::Max => "max",
        _ => "sum",
    }
}

fn summary_from(name: &str) -> Option<Summary> {
    match name {
        "count" => Some(Summary::Count),
        "sum" => Some(Summary::Sum),
        "average" => Some(Summary::Average),
        "min" => Some(Summary::Min),
        "max" => Some(Summary::Max),
        _ => None,
    }
}

/// Everything about how a grid is arranged: its columns in order, sorting, groups,
/// filters and search. Turn it into JSON to keep it.
///
/// ```
/// use ravenrs_widgets::GridLayout;
///
/// let kept = GridLayout::default().to_json();
/// assert_eq!(GridLayout::from_json(&kept), Ok(GridLayout::default()));
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct GridLayout {
    /// The columns, in the order they are shown.
    pub columns: Vec<ColumnLayout>,
    /// Sorted columns, in priority order.
    pub sort: Vec<SortLayout>,
    /// Grouped columns by title, outermost first.
    pub group_by: Vec<String>,
    /// Filters by column.
    pub filters: Vec<FilterLayout>,
    /// What is being searched for.
    pub search: String,
    /// The space to drop columns on and group by them is showing.
    pub group_panel: bool,
    /// Rows on a page, when it is in pages.
    pub page_size: Option<usize>,
    /// What each column sums up in group rows and in the footer.
    pub summaries: Vec<SummaryLayout>,
}

impl Default for GridLayout {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            sort: Vec::new(),
            group_by: Vec::new(),
            filters: Vec::new(),
            search: String::new(),
            group_panel: true,
            page_size: None,
            summaries: Vec::new(),
        }
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl GridLayout {
    /// The layout as JSON, to keep in a file or a database.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Reads what [`to_json`](Self::to_json) wrote. What an older version did not
    /// know takes its default.
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }
}

/// A named [`GridLayout`], for someone to come back to with one click.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct GridScenario {
    /// The name it was saved under.
    pub name: String,
    /// How the grid was arranged.
    pub layout: GridLayout,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl GridScenario {
    /// A scenario under this name.
    pub fn new(name: impl Into<String>, layout: GridLayout) -> Self {
        Self { name: name.into(), layout }
    }

    /// A list of scenarios as JSON, to keep in a file or a database.
    pub fn list_to_json(scenarios: &[Self]) -> String {
        serde_json::to_string(scenarios).unwrap_or_default()
    }

    /// Reads what [`list_to_json`](Self::list_to_json) wrote.
    pub fn list_from_json(text: &str) -> Result<Vec<Self>, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }
}

/// What the grid keeps about its columns beyond [`GridQuery`].
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Arrangement {
    /// Column indices in the order they are shown.
    pub order: Vec<usize>,
    /// Per column, whether it stays in place at the left.
    pub pinned: Vec<bool>,
    pub group_panel: bool,
    pub page_size: Option<usize>,
    /// Per column, its summary.
    pub summaries: Vec<Option<Summary>>,
    /// Per column, the formula that takes the place of its summary.
    pub formulas: Vec<Option<String>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Arrangement {
    pub fn new(columns: &[GridColumn], group_panel: bool, page_size: Option<usize>) -> Self {
        Self {
            order: (0..columns.len()).collect(),
            pinned: columns.iter().map(|c| c.pinned).collect(),
            group_panel,
            page_size,
            summaries: columns.iter().map(|c| c.summary).collect(),
            formulas: columns.iter().map(|c| c.formula.clone()).collect(),
        }
    }

    /// Puts `column` before `before`, or last.
    pub fn move_column(&mut self, column: usize, before: Option<usize>) {
        self.order.retain(|c| *c != column);
        let at = before.and_then(|b| self.order.iter().position(|c| *c == b)).unwrap_or(self.order.len());
        self.order.insert(at, column);
    }

    /// The columns to show, the pinned ones first. A grouped column shows in its groups.
    pub fn visible(&self, query: &GridQuery) -> Vec<usize> {
        let shown = |c: &usize| !query.is_hidden(*c) && !query.group_by.contains(c);
        let pinned = |c: &usize| self.pinned.get(*c).copied().unwrap_or(false);
        let mut out: Vec<usize> = self.order.iter().filter(|c| shown(c) && pinned(c)).copied().collect();
        out.extend(self.order.iter().filter(|c| shown(c) && !pinned(c)));
        out
    }
}

/// The layout of a grid as it stands.
pub(crate) fn capture(
    columns: &[GridColumn],
    query: &GridQuery,
    widths: &[f32],
    arrangement: &Arrangement,
) -> GridLayout {
    let title = |c: usize| columns.get(c).map_or(String::new(), |col| col.title.clone());
    GridLayout {
        columns: arrangement
            .order
            .iter()
            .map(|&c| ColumnLayout {
                title: title(c),
                width: widths.get(c).copied().unwrap_or_default(),
                hidden: query.is_hidden(c),
                pinned: arrangement.pinned.get(c).copied().unwrap_or(false),
            })
            .collect(),
        sort: query
            .sort
            .iter()
            .map(|(c, d)| SortLayout { title: title(*c), descending: *d == SortDirection::Descending })
            .collect(),
        group_by: query.group_by.iter().map(|c| title(*c)).collect(),
        filters: (0..columns.len())
            .filter(|c| !query.filter(*c).trim().is_empty())
            .map(|c| FilterLayout { title: title(c), text: query.filter(c).to_owned() })
            .collect(),
        search: query.search.clone(),
        group_panel: arrangement.group_panel,
        page_size: arrangement.page_size,
        summaries: (0..columns.len())
            .map(|c| SummaryLayout {
                title: title(c),
                kind: arrangement.summaries[c].map_or_else(String::new, |s| summary_name(s).to_owned()),
                formula: arrangement.formulas[c].clone(),
            })
            .collect(),
    }
}

/// Arranges a grid as `layout` says. A column it does not name stays where it is, at
/// the end, and a name it has that the grid does not is ignored.
pub(crate) fn apply(
    layout: &GridLayout,
    columns: &[GridColumn],
    query: &mut GridQuery,
    widths: &mut [f32],
    arrangement: &mut Arrangement,
) {
    let index = |title: &str| columns.iter().position(|c| c.title == title);
    let named: Vec<(usize, &ColumnLayout)> =
        layout.columns.iter().filter_map(|c| Some((index(&c.title)?, c))).collect();
    if !named.is_empty() {
        let mut order: Vec<usize> = named.iter().map(|(c, _)| *c).collect();
        order.extend((0..columns.len()).filter(|c| !named.iter().any(|(n, _)| n == c)));
        arrangement.order = order;
        for (c, column) in named {
            if column.width > 0.0 {
                widths[c] = column.width.max(columns[c].min_width);
            }
            if query.hidden.len() <= c {
                query.hidden.resize(c + 1, false);
            }
            query.hidden[c] = column.hidden;
            arrangement.pinned[c] = column.pinned;
        }
    }
    query.sort = layout
        .sort
        .iter()
        .filter_map(|s| {
            let direction = if s.descending { SortDirection::Descending } else { SortDirection::Ascending };
            Some((index(&s.title)?, direction))
        })
        .collect();
    query.group_by = layout.group_by.iter().filter_map(|t| index(t)).collect();
    query.collapsed.clear();
    query.filters.clear();
    for filter in &layout.filters {
        if let Some(c) = index(&filter.title) {
            query.set_filter(c, filter.text.clone());
        }
    }
    query.search.clone_from(&layout.search);
    arrangement.group_panel = layout.group_panel;
    arrangement.page_size = layout.page_size;
    for summary in &layout.summaries {
        if let Some(c) = index(&summary.title) {
            arrangement.summaries[c] = summary_from(&summary.kind);
            arrangement.formulas[c].clone_from(&summary.formula);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns() -> Vec<GridColumn> {
        vec![GridColumn::text("A"), GridColumn::text("B"), GridColumn::number("C")]
    }

    #[test]
    fn a_layout_comes_back_as_it_was() {
        let columns = columns();
        let mut arrangement = Arrangement::new(&columns, true, Some(50));
        arrangement.move_column(2, Some(0));
        arrangement.pinned[1] = true;
        let mut query = GridQuery { search: "x".into(), group_by: vec![1], ..GridQuery::default() };
        query.sort.push((2, SortDirection::Descending));
        query.set_filter(0, ">1".into());
        let widths = vec![100.0, 200.0, 300.0];
        let saved = capture(&columns, &query, &widths, &arrangement);
        let read = GridLayout::from_json(&saved.to_json()).expect("json");
        let (mut q, mut w, mut a) =
            (GridQuery::default(), vec![1.0, 1.0, 1.0], Arrangement::new(&columns, false, None));
        apply(&read, &columns, &mut q, &mut w, &mut a);
        assert_eq!(a.order, [2, 0, 1]);
        assert_eq!(a.pinned, [false, true, false]);
        assert_eq!(a.page_size, Some(50));
        assert!(a.group_panel);
        assert_eq!(w, [100.0, 200.0, 300.0]);
        assert_eq!(q.group_by, [1]);
        assert_eq!(q.sort, [(2, SortDirection::Descending)]);
        assert_eq!(q.filter(0), ">1");
        assert_eq!(q.search, "x");
    }

    #[test]
    fn a_scenario_survives_columns_that_changed() {
        let layout = GridLayout {
            columns: vec![
                ColumnLayout { title: "C".into(), width: 90.0, ..ColumnLayout::default() },
                ColumnLayout { title: "Gone".into(), width: 90.0, ..ColumnLayout::default() },
            ],
            group_by: vec!["Gone".into(), "B".into()],
            ..GridLayout::default()
        };
        let columns = columns();
        let (mut q, mut w, mut a) =
            (GridQuery::default(), vec![1.0, 1.0, 1.0], Arrangement::new(&columns, true, None));
        apply(&layout, &columns, &mut q, &mut w, &mut a);
        assert_eq!(a.order, [2, 0, 1], "what it does not name goes last");
        assert_eq!(q.group_by, [1]);
    }

    #[test]
    fn what_an_older_version_did_not_know_takes_its_default() {
        let layout = GridLayout::from_json(r#"{"search":"x"}"#).expect("json");
        assert!(layout.group_panel);
        assert_eq!(layout.search, "x");
    }

    #[test]
    fn pinned_columns_come_first_and_grouped_ones_leave() {
        let columns = columns();
        let mut arrangement = Arrangement::new(&columns, true, None);
        arrangement.pinned[2] = true;
        let query = GridQuery { group_by: vec![1], ..GridQuery::default() };
        assert_eq!(arrangement.visible(&query), [2, 0]);
    }

    #[test]
    fn formulas_go_with_the_layout() {
        let columns = columns();
        let mut arrangement = Arrangement::new(&columns, true, None);
        arrangement.formulas[2] = Some("Sum([C]) / Count()".into());
        arrangement.summaries[1] = Some(Summary::Count);
        let saved = capture(&columns, &GridQuery::default(), &[1.0, 1.0, 1.0], &arrangement);
        let read = GridLayout::from_json(&saved.to_json()).expect("json");
        let (mut q, mut w, mut a) =
            (GridQuery::default(), vec![1.0, 1.0, 1.0], Arrangement::new(&columns, true, None));
        apply(&read, &columns, &mut q, &mut w, &mut a);
        assert_eq!(a.formulas[2].as_deref(), Some("Sum([C]) / Count()"));
        assert_eq!(a.summaries[1], Some(Summary::Count));
    }

    #[test]
    fn scenarios_go_to_json_and_back() {
        let list = vec![GridScenario::new("Por conta", GridLayout::default())];
        assert_eq!(GridScenario::list_from_json(&GridScenario::list_to_json(&list)), Ok(list));
    }
}
