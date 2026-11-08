//! Rows that live on a server. The grid holds a few chunks of them and asks for more
//! only once scrolling settles, so that neither the program's memory nor the database
//! pays for rows nobody is looking at.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use ravenrs_reactive::State;

use super::model::{Row, SortDirection};
use super::query::GridQuery;

const DEFAULT_CHUNK: usize = 100;
const DEFAULT_MAX_CHUNKS: usize = 8;
const DEFAULT_IN_FLIGHT: usize = 2;
/// Seconds scrolling must rest before anything is asked for: a scrollbar dragged
/// across a million rows asks only for where it stopped.
const SCROLL_SETTLE: f64 = 0.12;
/// Seconds typing in a filter must rest before the data is asked for again.
const QUERY_SETTLE: f64 = 0.3;

/// What a [`RemoteRows`] loader must bring.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct GridRequest {
    /// The first row wanted, from the start of the result.
    pub offset: usize,
    /// How many rows, at most.
    pub limit: usize,
    /// Sort columns (zero-based) in priority order.
    pub sort: Vec<(usize, SortDirection)>,
    /// Filter text per column (index = column).
    pub filters: Vec<String>,
    /// What is being searched for.
    pub search: String,
    /// The row just before `offset`, when it is at hand. With it a query can continue
    /// from there (`WHERE key > ?`) and not walk past `offset` rows, which is what
    /// makes `OFFSET` slow on a big table.
    pub after: Option<Row>,
    /// Whether the total is still unknown. Answer `None` to leave it so: a total can
    /// cost the database more than all the rows ever shown.
    pub wants_total: bool,
}

/// What a [`RemoteRows`] loader brought.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct GridPage {
    /// The rows, at most as many as were asked for.
    pub rows: Vec<Row>,
    /// How many rows match in all, when it is known.
    pub total: Option<usize>,
    /// There are more rows after these. Without a total, the grid keeps going while this holds.
    pub has_more: bool,
    /// Aggregates per column, for the footer.
    pub totals: Vec<Option<f64>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl GridPage {
    /// These rows, with more after them or not.
    pub fn new(rows: Vec<Row>, has_more: bool) -> Self {
        Self { rows, has_more, ..Self::default() }
    }

    /// Says how many rows match in all.
    pub fn total(mut self, total: usize) -> Self {
        self.total = Some(total);
        self
    }

    /// Gives the aggregates per column for the footer.
    pub fn totals(mut self, totals: Vec<Option<f64>>) -> Self {
        self.totals = totals;
        self
    }
}

struct Chunk {
    rows: Vec<Row>,
    used: u64,
}

#[derive(Default)]
struct Held {
    chunks: BTreeMap<usize, Chunk>,
    in_flight: BTreeSet<usize>,
    /// Bumped when the query changes, so that an old answer is not taken for a new one.
    generation: u64,
    total: Option<usize>,
    /// One past the last row known to exist.
    end: usize,
    has_more: bool,
    totals: Vec<Option<f64>>,
    clock: u64,
    wanted: (usize, usize),
    moved_at: f64,
    query: Option<GridQuery>,
    queried_at: f64,
    error: Option<String>,
    started: bool,
    peak_rows: usize,
}

/// Answers one [`GridRequest`], now or later. An answer to a question nobody is asking
/// any more, because the filters changed meanwhile, is dropped.
#[derive(Clone)]
pub struct GridResponder {
    held: Rc<RefCell<Held>>,
    tick: State<u64>,
    generation: u64,
    chunk: usize,
    chunk_size: usize,
    max_chunks: usize,
}

impl fmt::Debug for GridResponder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GridResponder").field("chunk", &self.chunk).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl GridResponder {
    /// Gives the rows. Call it on the UI thread.
    pub fn send(self, page: GridPage) {
        {
            let mut held = self.held.borrow_mut();
            if held.generation != self.generation {
                return;
            }
            held.in_flight.remove(&self.chunk);
            held.error = None;
            let start = self.chunk * self.chunk_size;
            held.end = held.end.max(start + page.rows.len());
            held.has_more = page.has_more;
            if page.total.is_some() {
                held.total = page.total;
            } else if !page.has_more {
                held.total = Some(start + page.rows.len());
            }
            if !page.totals.is_empty() {
                held.totals = page.totals;
            }
            held.clock += 1;
            let used = held.clock;
            held.chunks.insert(self.chunk, Chunk { rows: page.rows, used });
            evict(&mut held, self.max_chunks, self.chunk_size);
            let rows: usize = held.chunks.values().map(|c| c.rows.len()).sum();
            held.peak_rows = held.peak_rows.max(rows);
        }
        self.tick.update(|t| *t += 1);
    }

    /// Says what went wrong. The grid offers to try again.
    pub fn fail(self, message: impl Into<String>) {
        {
            let mut held = self.held.borrow_mut();
            if held.generation != self.generation {
                return;
            }
            held.in_flight.remove(&self.chunk);
            held.error = Some(message.into());
        }
        self.tick.update(|t| *t += 1);
    }
}

/// Drops the chunks used longest ago, never one on the screen.
fn evict(held: &mut Held, max_chunks: usize, chunk_size: usize) {
    let (first, last) = (held.wanted.0 / chunk_size.max(1), held.wanted.1 / chunk_size.max(1));
    while held.chunks.len() > max_chunks.max(last.saturating_sub(first) + 1) {
        let oldest = held
            .chunks
            .iter()
            .filter(|(chunk, _)| **chunk < first || **chunk > last)
            .min_by_key(|(_, c)| c.used)
            .map(|(chunk, _)| *chunk);
        let Some(oldest) = oldest else { return };
        held.chunks.remove(&oldest);
    }
}

type Loader = Rc<dyn Fn(GridRequest, GridResponder)>;

/// The rows of a [`DataGrid`](crate::DataGrid) on a server. The grid asks `loader` for
/// a chunk at a time, only for what is on the screen, at most two at once, and keeps a
/// limited number of them, so a table of any size costs the same to look at.
///
/// ```
/// use ravenrs_widgets::{DataGrid, GridColumn, GridPage, RemoteRows, Value};
///
/// let rows = RemoteRows::new(|request, answer| {
///     // Ask the database here, off the UI thread if it takes a while.
///     let chunk = (request.offset..request.offset + request.limit)
///         .map(|n| vec![Value::Number(n as f64)])
///         .collect();
///     answer.send(GridPage::new(chunk, true));
/// });
/// let grid = DataGrid::remote(rows, [GridColumn::number("N")]);
/// ```
#[derive(Clone)]
pub struct RemoteRows {
    held: Rc<RefCell<Held>>,
    tick: State<u64>,
    loader: Loader,
    chunk_size: usize,
    max_chunks: usize,
    in_flight: usize,
}

impl fmt::Debug for RemoteRows {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteRows").field("chunk_size", &self.chunk_size).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl RemoteRows {
    /// Rows that `loader` brings, in chunks of 100, keeping 8 of them.
    pub fn new(loader: impl Fn(GridRequest, GridResponder) + 'static) -> Self {
        Self {
            held: Rc::new(RefCell::new(Held { has_more: true, ..Held::default() })),
            tick: State::new(0),
            loader: Rc::new(loader),
            chunk_size: DEFAULT_CHUNK,
            max_chunks: DEFAULT_MAX_CHUNKS,
            in_flight: DEFAULT_IN_FLIGHT,
        }
    }

    /// How many rows to ask for at a time.
    pub fn chunk_size(mut self, rows: usize) -> Self {
        self.chunk_size = rows.max(1);
        self
    }

    /// How many chunks to keep. The memory it takes is this times the chunk size, at most.
    pub fn max_chunks(mut self, chunks: usize) -> Self {
        self.max_chunks = chunks.max(1);
        self
    }

    /// How many questions the server gets at once.
    pub fn max_in_flight(mut self, requests: usize) -> Self {
        self.in_flight = requests.max(1);
        self
    }

    /// The most rows it has held at any moment.
    pub fn peak_rows(&self) -> usize {
        self.held.borrow().peak_rows
    }

    /// Forgets everything and asks again, as when the data changed on the server.
    pub fn refresh(&self) {
        {
            let mut held = self.held.borrow_mut();
            held.generation += 1;
            held.chunks.clear();
            held.in_flight.clear();
            held.total = None;
            held.end = 0;
            held.has_more = true;
            held.started = false;
            held.error = None;
        }
        self.tick.update(|t| *t += 1);
    }

    /// Subscribes the build to what arrives.
    pub(crate) fn watch(&self) {
        self.tick.get();
    }

    /// How many rows the grid shows: all of them when the total is known, or what has
    /// come so far and a little more, so that the scrollbar has somewhere to go.
    pub(crate) fn len(&self) -> usize {
        let held = self.held.borrow();
        match held.total {
            Some(total) => total,
            None if held.has_more => held.end + self.chunk_size,
            None => held.end,
        }
    }

    #[cfg(test)]
    pub(crate) fn total(&self) -> Option<usize> {
        self.held.borrow().total
    }

    pub(crate) fn error(&self) -> Option<String> {
        self.held.borrow().error.clone()
    }

    pub(crate) fn totals(&self) -> Vec<Option<f64>> {
        self.held.borrow().totals.clone()
    }

    /// The row at `index`, if it is here.
    /// Every row at hand, with its index.
    pub(crate) fn held_rows(&self) -> Vec<(usize, Row)> {
        let held = self.held.borrow();
        let mut rows = Vec::new();
        for (chunk, content) in &held.chunks {
            let start = chunk * self.chunk_size;
            rows.extend(content.rows.iter().enumerate().map(|(i, row)| (start + i, row.clone())));
        }
        rows
    }

    pub(crate) fn row(&self, index: usize) -> Option<Row> {
        let mut held = self.held.borrow_mut();
        held.clock += 1;
        let used = held.clock;
        let chunk = held.chunks.get_mut(&(index / self.chunk_size))?;
        chunk.used = used;
        chunk.rows.get(index % self.chunk_size).cloned()
    }

    /// Takes note of what is on the screen and asks for what is missing, if things have
    /// settled. Returns when to look again, if something is still waiting to settle.
    pub(crate) fn want(&self, first: usize, last: usize, query: &GridQuery, now: f64) -> Option<f64> {
        let mut requests = Vec::new();
        let wake = {
            let mut held = self.held.borrow_mut();
            let same_query = held.query.as_ref().is_some_and(|q| {
                q.sort == query.sort && q.filters == query.filters && q.search == query.search
            });
            if !same_query {
                let was_running = held.query.is_some();
                held.query = Some(query.clone());
                if was_running {
                    held.generation += 1;
                    held.chunks.clear();
                    held.in_flight.clear();
                    held.total = None;
                    held.end = 0;
                    held.has_more = true;
                    held.queried_at = now;
                    held.started = true;
                    held.error = None;
                }
            }
            if held.wanted != (first, last) {
                held.wanted = (first, last);
                held.moved_at = now;
            }
            // The very first screen has nothing to wait for.
            let ready = (!held.started)
                || (now - held.moved_at >= SCROLL_SETTLE && now - held.queried_at >= QUERY_SETTLE);
            if !ready {
                Some((held.moved_at + SCROLL_SETTLE).max(held.queried_at + QUERY_SETTLE))
            } else if held.error.is_some() {
                None
            } else {
                held.started = true;
                let size = self.chunk_size;
                let end = last.max(first + 1) - 1;
                // One chunk past the screen, so that a slow scroll never sees a blank row.
                let until = end / size + 1;
                let limit = held.total.map(|t| t.div_ceil(size));
                for chunk in first / size..=until {
                    if limit.is_some_and(|l| chunk >= l)
                        || held.chunks.contains_key(&chunk)
                        || held.in_flight.contains(&chunk)
                        || held.in_flight.len() >= self.in_flight
                    {
                        continue;
                    }
                    held.in_flight.insert(chunk);
                    let after = chunk
                        .checked_sub(1)
                        .and_then(|before| held.chunks.get(&before))
                        .and_then(|c| c.rows.last().cloned());
                    let request = GridRequest {
                        offset: chunk * size,
                        limit: size,
                        sort: query.sort.clone(),
                        filters: query.filters.clone(),
                        search: query.search.clone(),
                        after,
                        wants_total: held.total.is_none(),
                    };
                    requests.push((chunk, request, held.generation));
                }
                None
            }
        };
        for (chunk, request, generation) in requests {
            (self.loader)(
                request,
                GridResponder {
                    held: self.held.clone(),
                    tick: self.tick.clone(),
                    generation,
                    chunk,
                    chunk_size: self.chunk_size,
                    max_chunks: self.max_chunks,
                },
            );
        }
        wake
    }

    /// Clears a failure, so that the next look asks again.
    pub(crate) fn retry(&self) {
        self.held.borrow_mut().error = None;
        self.tick.update(|t| *t += 1);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::super::model::Value;
    use super::*;

    const SERVER_ROWS: usize = 1_000_000;

    /// A server that answers at once, and counts what it was asked.
    fn server(calls: &Rc<Cell<usize>>) -> RemoteRows {
        let calls = calls.clone();
        RemoteRows::new(move |request, answer| {
            calls.set(calls.get() + 1);
            let end = (request.offset + request.limit).min(SERVER_ROWS);
            let rows = (request.offset..end).map(|n| vec![Value::Number(n as f64)]).collect();
            answer.send(GridPage::new(rows, end < SERVER_ROWS));
        })
    }

    #[test]
    fn it_asks_only_for_what_is_on_the_screen() {
        let calls = Rc::new(Cell::new(0));
        let rows = server(&calls);
        rows.want(0, 30, &GridQuery::default(), 0.0);
        assert!(calls.get() <= 2, "{} questions", calls.get());
        assert_eq!(rows.row(5), Some(vec![Value::Number(5.0)]));
        assert_eq!(rows.row(5000), None);
    }

    #[test]
    fn dragging_the_scrollbar_across_everything_asks_only_where_it_stopped() {
        let calls = Rc::new(Cell::new(0));
        let rows = server(&calls);
        let q = GridQuery::default();
        rows.want(0, 30, &q, 0.0);
        let before = calls.get();
        for step in 1..500 {
            let at = step * 1000;
            assert!(rows.want(at, at + 30, &q, f64::from(step as u32) * 0.016).is_some());
        }
        assert_eq!(calls.get(), before, "nothing while it moves");
        rows.want(499_000, 499_030, &q, 100.0);
        assert!(calls.get() - before <= 2);
        assert!(rows.row(499_010).is_some());
    }

    #[test]
    fn memory_stays_within_its_limit_however_far_it_goes() {
        let calls = Rc::new(Cell::new(0));
        let rows = server(&calls).chunk_size(100).max_chunks(4);
        let q = GridQuery::default();
        let mut now = 0.0;
        for at in (0..200_000).step_by(100) {
            now += 1.0;
            rows.want(at, at + 30, &q, now);
            now += 1.0;
            rows.want(at, at + 30, &q, now);
        }
        assert!(rows.peak_rows() <= 400, "{} rows held", rows.peak_rows());
    }

    #[test]
    fn typing_in_a_filter_asks_once_it_rests() {
        let calls = Rc::new(Cell::new(0));
        let rows = server(&calls);
        let mut q = GridQuery::default();
        rows.want(0, 30, &q, 0.0);
        let before = calls.get();
        for (i, text) in ["a", "ab", "abc"].iter().enumerate() {
            q.search = (*text).to_owned();
            rows.want(0, 30, &q, 1.0 + i as f64 * 0.05);
        }
        assert_eq!(calls.get(), before);
        rows.want(0, 30, &q, 5.0);
        assert!(calls.get() > before);
    }

    #[test]
    fn an_answer_to_an_old_question_is_dropped() {
        let waiting: Rc<RefCell<Vec<GridResponder>>> = Rc::default();
        let held = waiting.clone();
        let rows = RemoteRows::new(move |_, answer| held.borrow_mut().push(answer)).max_in_flight(1);
        let mut q = GridQuery::default();
        rows.want(0, 30, &q, 0.0);
        let old = waiting.borrow_mut().remove(0);
        q.search = "new".into();
        rows.want(0, 30, &q, 1.0);
        rows.want(0, 30, &q, 5.0);
        old.send(GridPage::new(vec![vec![Value::Text("stale".into())]], false));
        assert_eq!(rows.row(0), None);
        assert_eq!(rows.total(), None);
    }

    #[test]
    fn the_server_gets_a_limited_number_of_questions_at_once() {
        let waiting: Rc<RefCell<Vec<GridResponder>>> = Rc::default();
        let held = waiting.clone();
        let rows =
            RemoteRows::new(move |_, answer| held.borrow_mut().push(answer)).chunk_size(10).max_in_flight(2);
        rows.want(0, 100, &GridQuery::default(), 0.0);
        assert_eq!(waiting.borrow().len(), 2);
    }

    #[test]
    fn without_a_total_it_grows_as_rows_arrive_and_stops_at_the_end() {
        let rows = RemoteRows::new(|request, answer| {
            let n = if request.offset >= 100 { 40 } else { 100 };
            let page = (0..n).map(|i| vec![Value::Number(f64::from(i))]).collect();
            answer.send(GridPage::new(page, n == 100));
        });
        let q = GridQuery::default();
        rows.want(0, 10, &q, 0.0);
        rows.want(95, 110, &q, 1.0);
        rows.want(95, 110, &q, 2.0);
        assert_eq!(rows.total(), Some(140), "a short page was the last");
        assert_eq!(rows.len(), 140);
    }

    #[test]
    fn it_can_continue_from_the_row_before_instead_of_counting_rows() {
        let seen: Rc<RefCell<Vec<GridRequest>>> = Rc::default();
        let log = seen.clone();
        let rows = RemoteRows::new(move |request, answer| {
            log.borrow_mut().push(request.clone());
            let page = (request.offset..request.offset + request.limit)
                .map(|n| vec![Value::Number(n as f64)])
                .collect();
            answer.send(GridPage::new(page, true));
        });
        let q = GridQuery::default();
        rows.want(0, 10, &q, 0.0);
        rows.want(150, 160, &q, 1.0);
        rows.want(150, 160, &q, 2.0);
        let next = seen.borrow().iter().find(|r| r.offset == 200).cloned().expect("the next chunk");
        assert_eq!(next.after, Some(vec![Value::Number(199.0)]));
    }

    #[test]
    fn a_failure_waits_for_someone_to_try_again() {
        let calls = Rc::new(Cell::new(0));
        let count = calls.clone();
        let rows = RemoteRows::new(move |_, answer| {
            count.set(count.get() + 1);
            answer.fail("no connection");
        })
        .max_in_flight(1);
        let q = GridQuery::default();
        rows.want(0, 10, &q, 0.0);
        assert_eq!(rows.error().as_deref(), Some("no connection"));
        rows.want(0, 10, &q, 9.0);
        assert_eq!(calls.get(), 1, "it does not hammer a server that is down");
        rows.retry();
        rows.want(0, 10, &q, 10.0);
        assert_eq!(calls.get(), 2);
    }
}
