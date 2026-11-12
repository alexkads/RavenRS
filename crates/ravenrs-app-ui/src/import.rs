//! Records from a spreadsheet, through the model: a table read from CSV or XLSX, which column goes
//! to which field, a preview that says what each line will do, and the writing, which goes through
//! the unit of work, so rules, permissions and the audit apply to every record as to one typed in.
//!
//! The writing is all or nothing for each batch: a line the storage refuses throws the batch away
//! and says which line it was. The screen around it is [`ImportWizard`], behind the
//! [`ids::IMPORT`](crate::ids::IMPORT) action of every list.

use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs_app::{CONTEXT_SAVE, EntityInfo, FieldInfo, FieldKind, Object, ObjectSpace, Query, Value, key_text};
use ravenrs_app_i18n::{Texts, format};

use crate::session::Words;
use crate::values::{display, value_of_text};

macro_rules! import_words {
    ($($(#[doc = $doc:literal])* $name:ident,)*) => {
        /// The words of importing records, from the `import.*` texts.
        #[derive(Clone, Debug)]
        #[non_exhaustive]
        #[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
        pub struct ImportWords {
            $($(#[doc = $doc])* pub $name: String,)*
        }

        impl ImportWords {
            /// The texts of a catalog, in its language.
            pub fn from_texts(texts: &Texts) -> Self {
                Self { $($name: texts.get(concat!("import.", stringify!($name))),)* }
            }
        }
    };
}

import_words! {
    /// The action on a list.
    action,
    /// The title of the window, where `{0}` is what is imported.
    title,
    /// The step that picks the file.
    step_file,
    /// The step that matches columns to fields.
    step_columns,
    /// The step that shows what will happen.
    step_preview,
    /// What file to pick.
    pick,
    /// The same, where only CSV is read.
    pick_csv,
    /// Opens the chooser.
    choose,
    /// Leaving the first step with no file.
    no_file,
    /// A file with only the names of its columns.
    empty,
    /// A file that cannot be read, where `{0}` says why.
    unreadable,
    /// An XLSX where the application reads only CSV.
    xlsx_off,
    /// How many lines came from which file.
    rows,
    /// The header of the columns of the file.
    column,
    /// The header of the fields.
    field,
    /// The header of the first line's values.
    example,
    /// A column that goes nowhere.
    ignore,
    /// Leaving the columns with none matched.
    no_column,
    /// A field matched twice, where `{0}` is the field.
    twice,
    /// Which field finds a record that already exists.
    match_on,
    /// Not looking for records that exist.
    no_match,
    /// What to do with one that exists.
    duplicates,
    /// Leaves it as it is.
    skip,
    /// Updates it.
    update,
    /// Counts it as an error.
    refuse,
    /// Imports the rest when some lines have errors.
    skip_invalid,
    /// The header of the line numbers.
    line,
    /// The header of what each line does.
    status,
    /// A line that creates a record.
    new,
    /// A line that updates one.
    updates,
    /// A line left out.
    skipped,
    /// A line for a record that exists, where `{0}` is what found it.
    duplicate,
    /// A line repeating another of the file, where `{0}` is that line.
    repeated,
    /// What the preview adds up to.
    summary,
    /// Why it cannot go on, where `{0}` is how many lines are wrong.
    has_errors,
    /// The last button.
    finish,
    /// What it did, where `{0}` is how many records.
    done,
    /// Why nothing was kept, where `{0}` is the line and `{1}` the reason.
    failed,
    /// Not a number.
    not_a_number,
    /// Not a whole number.
    not_whole,
    /// Not a date.
    not_a_date,
    /// Not one of the options.
    not_an_option,
    /// Neither yes nor no.
    not_yes_no,
    /// Not a color.
    not_a_color,
    /// Not a unique identifier.
    not_a_guid,
    /// No record has that name.
    not_found,
    /// More than one has it.
    ambiguous,
    /// A field no spreadsheet can fill.
    not_importable,
}

impl Default for ImportWords {
    fn default() -> Self {
        Self::from_texts(&Texts::new(ravenrs_app_i18n::Language::English))
    }
}

/// The lines of a spreadsheet: the names of its columns, from its first line, and the cells of
/// the others as text, each with its number in the file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportTable {
    /// The names of the columns.
    pub headers: Vec<String>,
    /// The cells of each line after the first, as many as there are headers.
    pub rows: Vec<Vec<String>>,
    /// The number of each line in the file, counting the names as line 1.
    pub lines: Vec<usize>,
}

impl ImportTable {
    /// From lines of cells, the first being the names. Empty lines are left out.
    pub fn from_lines(lines: Vec<Vec<String>>) -> Self {
        let mut numbered =
            lines.into_iter().enumerate().filter(|(_, cells)| cells.iter().any(|cell| !cell.trim().is_empty()));
        let Some((_, headers)) = numbered.next() else { return Self::default() };
        let headers: Vec<String> = headers.iter().map(|name| name.trim().to_owned()).collect();
        let (mut rows, mut numbers) = (Vec::new(), Vec::new());
        for (at, mut cells) in numbered {
            cells.resize(headers.len(), String::new());
            rows.push(cells);
            numbers.push(at + 1);
        }
        Self { headers, rows, lines: numbers }
    }

    /// From the bytes of a CSV file, in UTF-8 or Latin-1, with `;`, `,` or tabs between cells.
    pub fn from_csv(bytes: &[u8]) -> Self {
        let text = ravenrs_core::csv::text_of(bytes);
        Self::from_lines(ravenrs_core::csv::read_any(&text))
    }

    /// From the first sheet of an XLSX file. Numbers come with `decimal` as their decimal mark, as
    /// the person would type them, and dates as `yyyy-mm-dd`.
    #[cfg(feature = "import-xlsx")]
    pub fn from_xlsx(bytes: &[u8], decimal: char) -> Result<Self, String> {
        use ravenrs_sheet::base::{CellRef, Value as Sheet};
        let (book, _) = ravenrs_sheet::xlsx::read_xlsx(bytes).map_err(|error| error.to_string())?;
        let Some(sheet) = book.sheet(0) else { return Ok(Self::default()) };
        let Some(used) = sheet.used_range() else { return Ok(Self::default()) };
        let mut lines = Vec::new();
        for row in 0..=used.end.row {
            let mut cells = Vec::new();
            for col in 0..=used.end.col {
                let at = CellRef { row, col };
                let text = match sheet.value(at) {
                    Sheet::Empty => String::new(),
                    Sheet::Number(number) => {
                        let shown = ravenrs_sheet::engine::display::edit_text(&book, sheet, at);
                        // A plain number goes with the mark of the language; a date as it showed.
                        if shown.parse::<f64>().is_ok() {
                            number.to_string().replace('.', &decimal.to_string())
                        } else {
                            shown
                        }
                    }
                    Sheet::Text(text) => text.to_string(),
                    Sheet::Bool(on) => on.to_string(),
                    _ => sheet.input_text(at),
                };
                cells.push(text);
            }
            lines.push(cells);
        }
        Ok(Self::from_lines(lines))
    }

    /// From a file, by its extension: `.xlsx` when the application can read it, CSV otherwise.
    pub fn read(name: &str, bytes: &[u8], words: &Words) -> Result<Self, String> {
        let xlsx = name.to_lowercase().ends_with(".xlsx");
        #[cfg(feature = "import-xlsx")]
        let table = if xlsx {
            Self::from_xlsx(bytes, words.decimal)
                .map_err(|error| format(&words.import.unreadable, &[&error]))?
        } else {
            Self::from_csv(bytes)
        };
        #[cfg(not(feature = "import-xlsx"))]
        let table = if xlsx { return Err(words.import.xlsx_off.clone()) } else { Self::from_csv(bytes) };
        if table.rows.is_empty() {
            return Err(words.import.empty.clone());
        }
        Ok(table)
    }

    /// Whether XLSX files can be read, with the `import-xlsx` feature.
    pub fn reads_xlsx() -> bool {
        cfg!(feature = "import-xlsx")
    }
}

/// What to do with a line for a record that already exists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Duplicates {
    /// Leave the record as it is, and the line out.
    #[default]
    Skip,
    /// Write what the line says over the record.
    Update,
    /// Count the line as an error.
    Refuse,
}

/// How a table becomes records.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportPlan {
    /// The field each column goes to, by name; nothing leaves the column out.
    pub columns: Vec<Option<String>>,
    /// The field that finds a record that already exists; nothing does not look.
    pub match_on: Option<String>,
    /// What to do with one.
    pub duplicates: Duplicates,
    /// Imports the other lines when some have errors, instead of refusing all.
    pub skip_invalid: bool,
    /// How many records each commit keeps; 0 keeps them all in one.
    pub batch: usize,
}

/// Letters without their accents and nothing but letters and digits, for matching names.
fn plain(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

/// Whether a field can take a cell of a spreadsheet.
pub fn importable(field: &FieldInfo) -> bool {
    field.calculated.is_none()
        && (field.is_key || !field.read_only)
        && !matches!(field.kind, FieldKind::Collection(_) | FieldKind::Image | FieldKind::File)
}

impl ImportPlan {
    /// Each column to the field whose caption or name it has, ignoring case, accents and spaces,
    /// and records found by the key when a column has it.
    pub fn suggested(entity: &EntityInfo, headers: &[String]) -> Self {
        let mut taken: Vec<&str> = Vec::new();
        let columns = headers
            .iter()
            .map(|header| {
                let wanted = plain(header);
                let found = entity
                    .fields
                    .iter()
                    .filter(|field| importable(field) && !taken.contains(&field.name.as_str()))
                    .find(|field| plain(&field.caption) == wanted || plain(&field.name) == wanted)?;
                taken.push(&found.name);
                Some(found.name.clone())
            })
            .collect::<Vec<_>>();
        let key = entity.key_field().map(|key| key.name.clone());
        let match_on = key.filter(|key| columns.iter().flatten().any(|name| name == key));
        Self { columns, match_on, ..Self::default() }
    }

    /// What is wrong with the matching: no column, or a field twice.
    pub fn problem(&self, entity: &EntityInfo, words: &Words) -> Option<String> {
        let mapped: Vec<&String> = self.columns.iter().flatten().collect();
        if mapped.is_empty() {
            return Some(words.import.no_column.clone());
        }
        let twice = mapped.iter().enumerate().find(|(at, name)| mapped[..*at].contains(name))?;
        let caption = entity.fields.iter().find(|field| field.name == **twice.1).map_or(twice.1.as_str(), |f| &f.caption);
        Some(format(&words.import.twice, &[caption]))
    }
}

/// What a lookup by name found.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Found {
    /// The key of the one record.
    One(Value),
    /// Nothing.
    None,
    /// More than one.
    Many,
}

/// The records references can point to, by the name people know them by and by their key.
#[derive(Clone, Debug, Default)]
pub struct References {
    names: BTreeMap<String, BTreeMap<String, Vec<Value>>>,
    keys: BTreeMap<String, BTreeMap<String, Value>>,
}

impl References {
    /// Reads the records the reference fields of `fields` point to, as the space lets them be read.
    pub fn read<'a>(fields: impl IntoIterator<Item = &'a FieldInfo>, space: &dyn ObjectSpace) -> Self {
        let mut references = Self::default();
        let entities = space.entities();
        for field in fields {
            let FieldKind::Reference(target) = &field.kind else { continue };
            if references.names.contains_key(target) {
                continue;
            }
            let Some(info) = entities.iter().find(|entity| entity.name == *target) else { continue };
            let (mut names, mut keys) = (BTreeMap::<String, Vec<Value>>::new(), BTreeMap::new());
            let key_name = info.key_field().map(|key| key.name.clone()).unwrap_or_default();
            for record in space.query(target, &Query::all()).unwrap_or_default() {
                let key = record.get(&key_name);
                if let Some(named) = info.default_property() {
                    let name = display(named, &record.get(&named.name)).trim().to_lowercase();
                    if !name.is_empty() {
                        names.entry(name).or_default().push(key.clone());
                    }
                }
                if let Some(text) = key_text(&key) {
                    keys.insert(text, key);
                }
            }
            references.names.insert(target.clone(), names);
            references.keys.insert(target.clone(), keys);
        }
        references
    }

    /// The record of `entity` called `name`, or else the one whose key it is.
    pub(crate) fn find(&self, entity: &str, name: &str) -> Found {
        let wanted = name.trim().to_lowercase();
        match self.names.get(entity).and_then(|names| names.get(&wanted)).map(Vec::as_slice) {
            Some([key]) => Found::One(key.clone()),
            Some([_, _, ..]) => Found::Many,
            _ => {
                let by_key = self.keys.get(entity).and_then(|keys| {
                    keys.get(name.trim()).or_else(|| name.trim().parse::<f64>().ok().and_then(|n| keys.get(&n.to_string())))
                });
                by_key.map_or(Found::None, |key| Found::One(key.clone()))
            }
        }
    }
}

/// What a line of the file does.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum RowOutcome {
    /// Creates a record.
    New,
    /// Updates one.
    Update,
    /// Is left out, for this reason.
    Skip(String),
    /// Cannot be imported, for these reasons.
    Error(Vec<String>),
}

/// A line of the file and what it does.
#[derive(Clone, Debug, PartialEq)]
pub struct RowReport {
    /// Its number in the file.
    pub line: usize,
    /// What it does.
    pub outcome: RowOutcome,
}

/// What each line of a table will do, worked out without writing anything.
#[derive(Clone, Debug, Default)]
pub struct ImportPreview {
    /// One per line.
    pub rows: Vec<RowReport>,
    /// The record each line writes, when it writes one.
    records: Vec<Option<Object>>,
}

impl ImportPreview {
    fn count(&self, wanted: impl Fn(&RowOutcome) -> bool) -> usize {
        self.rows.iter().filter(|row| wanted(&row.outcome)).count()
    }

    /// Lines that create a record.
    pub fn new_count(&self) -> usize {
        self.count(|outcome| matches!(outcome, RowOutcome::New))
    }

    /// Lines that update one.
    pub fn update_count(&self) -> usize {
        self.count(|outcome| matches!(outcome, RowOutcome::Update))
    }

    /// Lines left out.
    pub fn skip_count(&self) -> usize {
        self.count(|outcome| matches!(outcome, RowOutcome::Skip(_)))
    }

    /// Lines with errors.
    pub fn error_count(&self) -> usize {
        self.count(|outcome| matches!(outcome, RowOutcome::Error(_)))
    }

    /// The line and first reason of the first error.
    fn first_error(&self) -> Option<(usize, String)> {
        self.rows.iter().find_map(|row| match &row.outcome {
            RowOutcome::Error(reasons) => Some((row.line, reasons.first().cloned().unwrap_or_default())),
            _ => None,
        })
    }
}

fn field_of<'a>(entity: &'a EntityInfo, name: &str) -> Option<&'a FieldInfo> {
    entity.fields.iter().find(|field| field.name == name)
}

/// The errors of the rules of the entity for this record, as a form saving it would find them.
fn broken(entity: &EntityInfo, record: &Object, space: &dyn ObjectSpace, words: &Words) -> Vec<String> {
    ravenrs_app::validate(entity, record, CONTEXT_SAVE, space, &words.rules)
        .into_iter()
        .filter(ravenrs_app::Violation::is_error)
        .map(|violation| violation.message)
        .collect()
}

/// What each line of `table` will do, by `plan`, reading the space but writing nothing: what each
/// cell is in its field, which record already exists, and what the rules say of the result.
pub fn preview(
    entity: &EntityInfo,
    table: &ImportTable,
    plan: &ImportPlan,
    space: &dyn ObjectSpace,
    words: &Words,
) -> ImportPreview {
    let mapped: Vec<(usize, &FieldInfo)> = plan
        .columns
        .iter()
        .enumerate()
        .filter_map(|(at, name)| Some((at, field_of(entity, name.as_deref()?)?)))
        .collect();
    let references = References::read(mapped.iter().map(|(_, field)| *field), space);
    let match_field = plan.match_on.as_deref().and_then(|name| field_of(entity, name));
    let mut existing: BTreeMap<String, Vec<Object>> = BTreeMap::new();
    if let Some(field) = match_field {
        for record in space.query(&entity.name, &Query::all()).unwrap_or_default() {
            if let Some(text) = key_text(&record.get(&field.name)) {
                existing.entry(text).or_default().push(record);
            }
        }
    }
    let said = &words.import;
    let mut result = ImportPreview::default();
    // The line of the file each value of the matching field was first seen on.
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for (index, cells) in table.rows.iter().enumerate() {
        let line = table.lines.get(index).copied().unwrap_or(index + 2);
        let (mut values, mut errors) = (Vec::new(), Vec::new());
        for (column, field) in &mapped {
            let text = cells.get(*column).map_or("", String::as_str);
            match value_of_text(field, text, words, &references) {
                Ok(value) => values.push((field.name.clone(), value)),
                Err(error) => errors.push(error),
            }
        }
        let put = |record: &mut Object| {
            for (name, value) in &values {
                record.set(name, value.clone());
            }
        };
        let fail = |result: &mut ImportPreview, reasons: Vec<String>| {
            result.rows.push(RowReport { line, outcome: RowOutcome::Error(reasons) });
            result.records.push(None);
        };
        if !errors.is_empty() {
            fail(&mut result, errors);
            continue;
        }
        let matched = match_field.and_then(|field| {
            let value = values.iter().find(|(name, _)| *name == field.name).map(|(_, value)| value)?;
            Some((key_text(value)?, display(field, value)))
        });
        let (mut record, outcome) = match &matched {
            Some((text, _)) if seen.contains_key(text) => {
                let first = seen[text];
                let first_line = result.rows[first].line;
                let repeated = format(&said.repeated, &[&first_line.to_string()]);
                match plan.duplicates {
                    Duplicates::Refuse => {
                        fail(&mut result, vec![repeated]);
                        continue;
                    }
                    Duplicates::Update => {
                        // The later line goes over what the earlier one will write.
                        let Some(mut merged) = result.records[first].clone() else {
                            fail(&mut result, vec![repeated]);
                            continue;
                        };
                        put(&mut merged);
                        let reasons = broken(entity, &merged, space, words);
                        if reasons.is_empty() {
                            result.records[first] = Some(merged);
                            result.rows.push(RowReport { line, outcome: RowOutcome::Update });
                            result.records.push(None);
                        } else {
                            fail(&mut result, reasons);
                        }
                        continue;
                    }
                    _ => {
                        result.rows.push(RowReport { line, outcome: RowOutcome::Skip(repeated) });
                        result.records.push(None);
                        continue;
                    }
                }
            }
            Some((text, shown)) if existing.contains_key(text) => {
                let found = &existing[text];
                let duplicate = format(&said.duplicate, &[shown]);
                match (plan.duplicates, found.as_slice()) {
                    (Duplicates::Update, [stored]) => (stored.clone(), RowOutcome::Update),
                    (Duplicates::Update | Duplicates::Refuse, _) => {
                        fail(&mut result, vec![duplicate]);
                        continue;
                    }
                    _ => {
                        result.rows.push(RowReport { line, outcome: RowOutcome::Skip(duplicate) });
                        result.records.push(None);
                        continue;
                    }
                }
            }
            _ => (entity.new_object(), RowOutcome::New),
        };
        put(&mut record);
        let reasons = broken(entity, &record, space, words);
        if !reasons.is_empty() {
            fail(&mut result, reasons);
            continue;
        }
        if let Some((text, _)) = matched {
            seen.insert(text, result.rows.len());
        }
        result.rows.push(RowReport { line, outcome });
        result.records.push(Some(record));
    }
    result
}

/// What writing a preview did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportReport {
    /// Records created and kept.
    pub created: usize,
    /// Records updated and kept.
    pub updated: usize,
    /// Lines left out: repeated, already there, or with errors when those were left out.
    pub skipped: usize,
    /// When it stopped: the line, and why. What its batch held was thrown away.
    pub failure: Option<(usize, String)>,
}

impl ImportReport {
    /// Whether everything asked for was kept.
    pub fn is_complete(&self) -> bool {
        self.failure.is_none()
    }
}

/// Writes what a preview found, through the space: a record at a time, as a form would, and a
/// commit for each batch of the plan. When the storage refuses a record or a commit, everything
/// since the last commit is thrown away and the report says which line it was. Lines with errors
/// stop it all before anything is written, unless the plan leaves them out.
pub fn write(
    entity: &EntityInfo,
    preview: &ImportPreview,
    plan: &ImportPlan,
    space: &mut dyn ObjectSpace,
) -> ImportReport {
    let mut report = ImportReport { skipped: preview.skip_count(), ..ImportReport::default() };
    if let Some(first) = preview.first_error() {
        if !plan.skip_invalid {
            report.failure = Some(first);
            return report;
        }
        report.skipped += preview.error_count();
    }
    let (mut created, mut updated, mut in_batch) = (0, 0, 0);
    let key_name = entity.key_field().map(|key| key.name.clone());
    for (row, record) in preview.rows.iter().zip(&preview.records) {
        let Some(record) = record else { continue };
        let kept = match row.outcome {
            RowOutcome::New => space.new_object(&entity.name).and_then(|mut fresh| {
                for (name, value) in record.values() {
                    // A key the file did not give is the one the space numbered.
                    let own_key = key_name.as_deref() == Some(name.as_str()) && value.is_null();
                    if !own_key {
                        fresh.set(name, value.clone());
                    }
                }
                space.insert(fresh)
            }),
            _ => space.save(record.clone()),
        };
        if let Err(error) = kept {
            space.rollback();
            report.failure = Some((row.line, error.to_string()));
            return report;
        }
        match row.outcome {
            RowOutcome::New => created += 1,
            _ => updated += 1,
        }
        in_batch += 1;
        if plan.batch > 0 && in_batch == plan.batch {
            if let Err(error) = space.commit() {
                space.rollback();
                report.failure = Some((row.line, error.to_string()));
                return report;
            }
            (report.created, report.updated, in_batch) = (created, updated, 0);
        }
    }
    if let Err(error) = space.commit() {
        space.rollback();
        let line = preview.rows.last().map_or(0, |row| row.line);
        report.failure = Some((line, error.to_string()));
        return report;
    }
    (report.created, report.updated) = (created, updated);
    report
}

/// The text of a line's outcome, for the preview.
pub(crate) fn outcome_text(outcome: &RowOutcome, words: &Words) -> String {
    match outcome {
        RowOutcome::New => words.import.new.clone(),
        RowOutcome::Update => words.import.updates.clone(),
        RowOutcome::Skip(why) => format!("{}: {why}", words.import.skipped),
        RowOutcome::Error(reasons) => reasons.join(" "),
    }
}

/// A table shared between the steps of the wizard.
pub(crate) type SharedTable = Rc<ImportTable>;
