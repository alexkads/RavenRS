//! Report layouts: their format, building one from data, checking one and the ready
//! templates. Everything here returns JSON; nothing is written to disk.

use std::collections::{BTreeMap, HashMap};

use ravenrs_criteria::{Criteria, FieldType, Schema, validate};
use ravenrs_report::barcode::Symbology;
use ravenrs_report::chart::ChartKind;
use ravenrs_report::wizard::{self, ColumnWords, FieldInfo, FieldKind, WizardOptions};
use ravenrs_report::{
    Band, Control, DataSource, DataSourceKind, Group, PageSettings, ParameterKind, RenderOptions, Report,
    ReportEngine, Style, Table, TableRow, TextAlign, samples,
};
use rmcp::schemars;
use serde::Deserialize;
use serde_json::{Value, json};

/// Keys of the layout whose text is an expression.
const EXPRESSION_KEYS: [&str; 7] =
    ["expression", "filter", "visible", "argument", "value", "urlExpression", "condition"];

/// A field given by name when there is no sample data.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FieldInput {
    /// The path used in expressions, such as `Customer` or `Customer.Name`.
    pub name: String,
    /// text, number, date or boolean. Defaults to text.
    pub kind: Option<String>,
}

/// A column of a table layout.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ColumnInput {
    /// The field it shows.
    pub field: String,
    /// The header; a caption made from the field name when absent.
    pub caption: Option<String>,
    /// A .NET-style format, such as `C2`, `N0`, `P1` or `dd/MM/yyyy`.
    pub format: Option<String>,
}

/// What a new table layout is made of.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct CreateInput {
    /// The title printed at the top, also the layout's name.
    pub title: String,
    /// Culture of number and date formats, such as `pt-BR` or `en-US`. Defaults to `en-US`.
    pub culture: Option<String>,
    /// Rows of data as a JSON array of objects, embedded in the layout. Its fields are read from it.
    pub data: Option<Value>,
    /// Instead of embedded rows: an address the report server fetches when the report runs. Give `fields` with it.
    pub data_url: Option<String>,
    /// The fields, when there is no `data` to read them from.
    pub fields: Option<Vec<FieldInput>>,
    /// A path to the list of rows inside the data, such as `orders`.
    pub data_member: Option<String>,
    /// The columns, in order. Defaults to every field that is not a list.
    pub columns: Option<Vec<ColumnInput>>,
    /// Groups rows by this field, with a header and subtotals.
    pub group_by: Option<String>,
    /// Number fields that get subtotals and a grand total.
    pub totals: Option<Vec<String>>,
    /// Turns the page sideways. More than six columns turn it anyway.
    pub landscape: Option<bool>,
    /// The color of the column headers, as `#rrggbb`.
    pub accent: Option<String>,
    /// The name of the data source. Defaults to `Data`.
    pub source_name: Option<String>,
}

fn kind_of(name: Option<&str>) -> Result<FieldKind, String> {
    match name.unwrap_or("text").to_lowercase().as_str() {
        "text" | "string" => Ok(FieldKind::Text),
        "number" | "decimal" | "integer" => Ok(FieldKind::Number),
        "date" | "datetime" => Ok(FieldKind::Date),
        "boolean" | "bool" => Ok(FieldKind::Boolean),
        other => Err(format!("unknown field kind '{other}': use text, number, date or boolean")),
    }
}

/// One row with a plausible value for each field, so the layout has something to preview.
fn sample_row(fields: &[FieldInfo]) -> Value {
    let mut row = serde_json::Map::new();
    for f in fields {
        let value = match f.kind {
            FieldKind::Number => json!(0),
            FieldKind::Date => json!("2026-01-31"),
            FieldKind::Boolean => json!(true),
            _ => json!(wizard::caption(&f.path)),
        };
        row.insert(f.path.clone(), value);
    }
    Value::Array(vec![Value::Object(row)])
}

/// A table layout built from `input`, with the check of what came out.
pub fn create(input: &CreateInput) -> Result<Value, String> {
    if input.title.trim().is_empty() {
        return Err("give the report a title".into());
    }
    let member = input.data_member.as_deref();
    let (fields, kind) = match (&input.data, &input.fields) {
        (Some(data), _) => {
            if !data.is_array() && member.is_none() {
                return Err("data must be an array of rows, or give data_member to point at one".into());
            }
            (wizard::fields(data, member), DataSourceKind::Json { data: data.clone() })
        }
        (None, Some(given)) => {
            let fields = given
                .iter()
                .map(|f| Ok(FieldInfo { path: f.name.clone(), kind: kind_of(f.kind.as_deref())? }))
                .collect::<Result<Vec<_>, String>>()?;
            let kind = match &input.data_url {
                Some(url) => DataSourceKind::Url { url: url.clone(), headers: BTreeMap::new() },
                None => DataSourceKind::Json { data: sample_row(&fields) },
            };
            (fields, kind)
        }
        (None, None) => return Err("give data (rows to embed) or fields (with or without data_url)".into()),
    };
    if fields.is_empty() {
        return Err("no fields found: the rows must be JSON objects".into());
    }
    let find = |name: &str| fields.iter().find(|f| f.path == name).cloned();
    let unknown = |name: &str| format!("unknown field '{name}'; the fields are: {}", names(&fields));
    let (columns, words): (Vec<FieldInfo>, Vec<ColumnWords>) = match &input.columns {
        Some(given) => given
            .iter()
            .map(|c| {
                let field = find(&c.field).ok_or_else(|| unknown(&c.field))?;
                Ok((field, ColumnWords { caption: c.caption.clone(), format: c.format.clone() }))
            })
            .collect::<Result<Vec<_>, String>>()?
            .into_iter()
            .unzip(),
        None => fields
            .iter()
            .filter(|f| f.kind != FieldKind::Collection)
            .map(|f| (f.clone(), ColumnWords::default()))
            .unzip(),
    };
    if let Some(group) = &input.group_by
        && find(group).is_none()
    {
        return Err(unknown(group));
    }
    let totals = input.totals.clone().unwrap_or_default();
    for t in &totals {
        match find(t) {
            Some(f) if f.kind == FieldKind::Number => {}
            Some(_) => return Err(format!("'{t}' is not a number field, so it cannot be totaled")),
            None => return Err(unknown(t)),
        }
    }
    let page =
        if input.landscape == Some(true) { PageSettings::a4().landscape() } else { PageSettings::a4() };
    let defaults = WizardOptions::default();
    let options = WizardOptions {
        title: input.title.clone(),
        culture: input.culture.clone().unwrap_or(defaults.culture),
        columns,
        group_by: input.group_by.clone(),
        totals,
        page,
        accent: input.accent.clone().unwrap_or(defaults.accent),
    };
    let source = DataSource { name: input.source_name.clone().unwrap_or_else(|| "Data".into()), kind };
    let mut report = wizard::table_report_with(source, &options, &words);
    if let Some(member) = member {
        report = report.data_member(member);
    }
    let layout: Value = serde_json::from_str(&report.to_json()).map_err(|e| e.to_string())?;
    Ok(json!({
        "file_name": format!("{}.rvreport.json", slug(&input.title)),
        "fields": fields.iter().map(|f| json!({ "name": f.path, "kind": format!("{:?}", f.kind) })).collect::<Vec<_>>(),
        "check": check_report(&report, None, &HashMap::new()),
        "layout": layout,
    }))
}

fn names(fields: &[FieldInfo]) -> String {
    fields.iter().map(|f| f.path.as_str()).collect::<Vec<_>>().join(", ")
}

/// A file name made of the title's letters and digits.
fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() { "report".into() } else { out }
}

/// What expressions in this report may refer to.
fn schema_of(report: &Report, data: Option<&Value>, rows_of: Option<&str>) -> Schema {
    let kind = |k: FieldKind| match k {
        FieldKind::Number => FieldType::Number,
        FieldKind::Date => FieldType::Date,
        FieldKind::Boolean => FieldType::Boolean,
        FieldKind::Text => FieldType::Text,
        _ => FieldType::Any,
    };
    let mut schema = Schema::new();
    if let Some(data) = data {
        let member = report.data_member.as_deref();
        let first = wizard::rows_of(data, member).first().copied().cloned();
        for f in wizard::fields(data, member) {
            if f.kind == FieldKind::Collection {
                let items = first.as_ref().and_then(|row| row.get(&f.path)).cloned().unwrap_or_default();
                let rows = wizard::fields(&items, None)
                    .into_iter()
                    .fold(Schema::new(), |s, sub| s.field(&sub.path, kind(sub.kind)));
                schema = schema.collection(&f.path, rows);
                // Inside a detail report the rows are the collection's.
                if rows_of == Some(f.path.as_str()) {
                    for sub in wizard::fields(&items, None) {
                        schema = schema.field(&sub.path, kind(sub.kind));
                    }
                }
            } else {
                schema = schema.field(&f.path, kind(f.kind));
            }
        }
    }
    for c in &report.calculated_fields {
        schema = schema.field(&c.name, FieldType::Any);
    }
    for p in &report.parameters {
        let k = match p.kind {
            ParameterKind::Number => FieldType::Number,
            ParameterKind::Date => FieldType::Date,
            ParameterKind::Boolean => FieldType::Boolean,
            _ => FieldType::Text,
        };
        schema = schema.parameter(&p.name, k);
    }
    schema
}

/// Every expression in the layout, with where it is.
fn expressions(value: &Value, path: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, v) in map {
                let here = if path.is_empty() { key.clone() } else { format!("{path}.{key}") };
                match v {
                    Value::String(text)
                        if EXPRESSION_KEYS.contains(&key.as_str()) && !text.trim().is_empty() =>
                    {
                        // Calculated fields and groups keep their expression as text; data sources do not.
                        if !here.starts_with("dataSources") {
                            out.push((here, text.clone()));
                        }
                    }
                    _ => expressions(v, &here, out),
                }
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                expressions(v, &format!("{path}[{i}]"), out);
            }
        }
        _ => {}
    }
}

/// The check of a parsed layout: its expressions, then a render over its data.
fn check_report(report: &Report, data: Option<&Value>, parameters: &HashMap<String, Value>) -> Value {
    let embedded = report.embedded_data();
    let data = data.or(embedded);
    let layout: Value = serde_json::from_str(&report.to_json()).unwrap_or(Value::Null);
    let mut found = Vec::new();
    expressions(&layout, "", &mut found);
    let master = schema_of(report, data, None);
    let mut problems = Vec::new();
    let mut warnings = Vec::new();
    for (path, text) in &found {
        // A detail report's expressions read its collection's rows.
        let schema = match path.strip_prefix("detailReports[").and_then(|rest| rest.split(']').next()) {
            Some(i) => {
                let member = i
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| report.detail_reports.get(i))
                    .map(|d| d.data_member.as_str());
                schema_of(report, data, member)
            }
            None => master.clone(),
        };
        match Criteria::parse(text) {
            Err(e) => problems
                .push(json!({ "at": path, "expression": text, "error": e.message, "offset": e.offset })),
            Ok(parsed) => {
                let unknown: Vec<String> =
                    validate(&parsed, &schema).iter().map(ToString::to_string).collect();
                // The sample data may lack a field the real data has, so these only warn.
                if !unknown.is_empty() && data.is_some() {
                    warnings.push(json!({ "at": path, "expression": text, "unknown": unknown }));
                }
            }
        }
    }
    let render = match data {
        Some(data) => {
            let options = RenderOptions { parameters: parameters.clone(), ..RenderOptions::default() };
            let doc = ReportEngine::new().render_with(report, data, &options);
            json!({ "pages": doc.pages.len(), "errors": doc.errors })
        }
        None => json!({ "skipped": "the data comes from outside the layout; pass data to render it" }),
    };
    json!({
        "valid": problems.is_empty() && render["errors"].as_array().is_none_or(Vec::is_empty),
        "expressions_checked": found.len(),
        "problems": problems,
        "warnings": warnings,
        "render": render,
    })
}

/// Checks a layout given as JSON text or as an object.
pub fn check(layout: &Value, data: Option<&Value>, parameters: Option<&Value>) -> Result<Value, String> {
    let text = match layout {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let report = match Report::from_json(&text) {
        Ok(report) => report,
        Err(e) => {
            return Ok(json!({
                "valid": false,
                "json_error": { "message": e.to_string(), "line": e.line(), "column": e.column() },
            }));
        }
    };
    let parameters: HashMap<String, Value> = match parameters {
        Some(Value::Object(map)) => map.clone().into_iter().collect(),
        Some(_) => return Err("parameters must be an object of name and value".into()),
        None => HashMap::new(),
    };
    let mut answer = check_report(&report, data, &parameters);
    answer["name"] = json!(report.name);
    Ok(answer)
}

/// A ready layout: its name, what it shows and how it is made.
type Template = (&'static str, &'static str, fn() -> Report);

/// The ready layouts, or the layout of one.
pub fn templates(name: Option<&str>) -> Result<Value, String> {
    let list: [Template; 4] = [
        ("invoice", "Issuer, customer, items as master-detail, totals and notes.", samples::invoice),
        (
            "sales_by_region",
            "Nested groups with subtotals, zebra rows, conditional highlight and a minimum value parameter.",
            || samples::sales_by_region(20),
        ),
        ("sales_dashboard", "Area, bar and doughnut charts over the same orders.", samples::sales_dashboard),
        ("customer_list", "Grouped by initial, with a check box.", samples::customer_list),
    ];
    match name {
        None => Ok(json!({
            "templates": list.iter().map(|(n, d, _)| json!({ "name": n, "description": d })).collect::<Vec<_>>(),
        })),
        Some(wanted) => {
            let (n, d, make) = list
                .iter()
                .find(|(n, _, _)| n.eq_ignore_ascii_case(wanted))
                .ok_or_else(|| format!("unknown template '{wanted}': use invoice, sales_by_region, sales_dashboard or customer_list"))?;
            let layout: Value = serde_json::from_str(&make().to_json()).map_err(|e| e.to_string())?;
            Ok(
                json!({ "name": n, "description": d, "file_name": format!("{n}.rvreport.json"), "layout": layout }),
            )
        }
    }
}

/// The layout format, with examples made by the report library itself.
pub fn format() -> Value {
    let to = |c: Control| serde_json::to_value(c).unwrap_or(Value::Null);
    let controls = json!({
        "label": to(Control::label("Total").at(0.0, 0.0, 120.0, 18.0).style(Style::new().bold().align(TextAlign::Right))),
        "field": to(Control::field("[Quantity] * [UnitPrice]").format("C2").at(0.0, 0.0, 120.0, 18.0)),
        "checkBox": to(Control::check_box("[Paid]", "Paid").at(0.0, 0.0, 100.0, 18.0)),
        "table": to(Control::table(Table::new().row(TableRow::new(18.0).text(120.0, "Customer").field(80.0, "[Total]", Some("N2"))))),
        "picture": to(Control::picture("").at(0.0, 0.0, 100.0, 60.0)),
        "barcode": to(Control::barcode(Symbology::Code128, "[Sku]").at(0.0, 0.0, 160.0, 50.0)),
        "chart": to(Control::chart(ChartKind::Bar, "[Region]", "[Quantity] * [UnitPrice]").at(0.0, 0.0, 320.0, 180.0)),
        "line": to(Control::line().at(0.0, 0.0, 200.0, 4.0)),
        "rectangle": to(Control::rect(4.0).at(0.0, 0.0, 120.0, 40.0)),
        "link": to(Control::link("Open", "https://example.com").at(0.0, 0.0, 120.0, 18.0)),
        "bookmark": to(Control::bookmark("[Region]", 0).at(0.0, 0.0, 120.0, 12.0)),
    });
    let example = Report::new("Orders")
        .culture("en-US")
        .data_source(DataSource {
            name: "Orders".into(),
            kind: DataSourceKind::Json {
                data: json!([
                    { "Region": "North", "Customer": "Ana", "Quantity": 2, "UnitPrice": 10.5, "Paid": true },
                    { "Region": "South", "Customer": "Bruno", "Quantity": 1, "UnitPrice": 99.0, "Paid": false }
                ]),
            },
        })
        .calculated("Total", "[Quantity] * [UnitPrice]")
        .parameter("MinTotal", ParameterKind::Number, json!(0))
        .filter("[Total] >= ?MinTotal")
        .report_header(Band::new(30.0).control(Control::label("Orders").at(0.0, 0.0, 300.0, 24.0).style(Style::new().font_size(16.0).bold())))
        .group(
            Group::new("[Region]")
                .header(Band::new(20.0).control(Control::field("[Region]").at(0.0, 0.0, 200.0, 18.0).style(Style::new().bold())))
                .footer(Band::new(20.0).control(Control::field("Sum([Total])").format("C2").at(400.0, 0.0, 120.0, 18.0))),
        )
        .detail(
            Band::new(18.0)
                .control(Control::field("[Customer]").at(0.0, 0.0, 200.0, 18.0))
                .control(Control::field("[Total]").format("C2").at(400.0, 0.0, 120.0, 18.0)),
        )
        .page_footer(Band::new(16.0).control(
            Control::field("FormatString('Page {0} of {1}', PageNumber(), PageCount())").at(0.0, 0.0, 200.0, 14.0),
        ));
    json!({
        "file": "One JSON file, *.rvreport.json, holds the layout and its data sources. The report server and the report designer read the same file.",
        "units": "Positions and sizes are in points (1/72 inch), relative to the band. A4 portrait leaves about 523 points of width between the margins.",
        "bands": {
            "reportHeader": "Once, at the start.",
            "pageHeader": "At the top of every page.",
            "groups": "A list; each has an expression to group by, an optional header and footer band, and options such as repeatHeader and pageBreakBefore.",
            "detail": "Once per row.",
            "detailReports": "Master-detail: a list with a dataMember naming a list inside each row, and its own header, detail and footer bands.",
            "reportFooter": "Once, at the end; aggregates here cover every row.",
            "pageFooter": "At the bottom of every page.",
        },
        "controls": controls,
        "style": serde_json::to_value(Style::new().font_size(10.0).bold().color("#12151c").background("#f4f5f8").border("bottom", 0.75, "#dadde5").padding(2.0).align(TextAlign::Left)).unwrap_or(Value::Null),
        "data_sources": {
            "json": "Rows embedded in the layout: { \"name\": \"Orders\", \"type\": \"json\", \"data\": [ ... ] }.",
            "url": "JSON the report server fetches when the report runs: { \"name\": \"Orders\", \"type\": \"url\", \"url\": \"https://...{?Year}\", \"headers\": {} }. Secrets belong in the server's configuration.",
            "sql": "A query the report server runs on a named connection: { \"name\": \"Orders\", \"type\": \"sql\", \"connection\": \"erp\", \"query\": \"select ... where year = @Year\" }.",
        },
        "expressions": "Fields are [Name], parameters ?Name, aggregates Sum([Total]), Count() and others. See expression_functions for the functions and validate_expression to check one. Formats are .NET style: C2, N0, P1, dd/MM/yyyy.",
        "example": serde_json::from_str::<Value>(&example.to_json()).unwrap_or(Value::Null),
    })
}
