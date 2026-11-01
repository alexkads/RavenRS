# RavenRS Reporting — feature parity with DevExpress Reporting

This is a fine-tooth-comb inventory of DevExpress Reporting v26.1, the reference product, with the RavenRS status of each feature. It drives the phases in [REPORTING.md](REPORTING.md).

Sources: the DevExpress end-user documentation repository ([dotnet-eud, branch 2026.1](https://github.com/DevExpress/dotnet-eud/tree/2026.1/eud-for-devexpress-reports)) and the developer documentation at docs.devexpress.com. Rows marked "(unverified)" could not be confirmed against a primary page.

Status: ✅ done · 🟡 partial · ⬜ planned (phase in brackets).

## 1. Bands

| Feature | DevExpress | RavenRS |
|---|---|---|
| Detail | Once per record; keep together, fill empty space, drill-down | ✅ Detail, with zebra rows. Fill empty space ⬜ [R3], drill-down ⬜ [R4] |
| Report Header / Footer | First band / last page; footer *print at bottom* | ✅. Print at bottom ⬜ [R3] |
| Page Header / Footer | Every page; *print on* not with report header/footer | ✅ every page, with `PageNumber()`/`PageCount()` and page-scoped aggregates. *Print on* ⬜ [R3] |
| Top / Bottom Margin bands | Content in the margins | ⬜ [R3] |
| Group Header / Footer | Several group fields, sort, keep together, repeat every page, page break before/after, print at bottom, sort by summary | ✅ nested groups, descending order, repeat header, page break before, keep-with-next header. Several fields per level, page break after, sort by summary ⬜ [R3] |
| Detail Report (master-detail) | Own data member and bands, nestable | ✅ one level, from a nested collection, with header/detail/footer, `[^.Field]` reaching the master, and filter/sort. Nesting ⬜ [R3] |
| Sub-band | Alternative content for a band | ⬜ [R3] |
| Vertical bands (header, detail, total) | Records laid out left to right | ⬜ [R5] |
| Band visibility, height 0 hides | Expressions | ✅ `visible` expression |

## 2. Report controls

| Control | DevExpress | RavenRS |
|---|---|---|
| Label | Expressions, format string, can grow/shrink, word wrap, angle, text fit, markup, duplicates/nulls handling, interactive sort, editing | ✅ text or expression, format, can grow, wrap, alignment, padding, borders, fill. Can shrink, angle, text fit, markup, suppress duplicates ⬜ [R3] |
| Table / row / cell | Merge, split, insert, bind cells, row height grows | ✅ rows of cells, cell styles, rows grow to their tallest cell, rules on cells. Merge/split ⬜ [R3] |
| Panel | Container, can shrink | ✅ container that grows with its content |
| Check Box | Checked/unchecked/indeterminate, glyphs, editable | ✅ checked/unchecked with caption. Indeterminate, glyphs ⬜ [R3] |
| Picture Box | PNG/JPEG/SVG/…; Normal/Stretch/Zoom/Squeeze/Tile; URL or binding | ⬜ [R2] |
| Line | Horizontal/vertical/slant/back slant, styles | ✅ horizontal/vertical/slant. Back slant, dash styles ⬜ [R3] |
| Shape | Rectangle, ellipse, arrow, star, polygon… | 🟡 rectangle (rounded) and ellipse. The rest ⬜ [R3] |
| Cross-band line / box | Spans bands | ⬜ [R3] |
| Page break | Forced break inside a band | 🟡 band page break before/after. Control ⬜ [R3] |
| Page info | Page n of N, date, user, per-group numbering | ✅ via `FormatString('Page {0} of {1}', PageNumber(), PageCount())`, `Now()`. Per-group numbering ⬜ [R3] |
| Subreport | Embeds another report, parameter bindings | ⬜ [R3] |
| Table of contents, bookmarks | From bookmarks; PDF outline | ⬜ [R2/R3] |
| Barcode | 40+ symbologies (Code 128, EAN, UPC, ITF, GS1, QR, Data Matrix, PDF417, Aztec, postal…) | ⬜ [R3]: Code 128, Code 39, EAN-13/8, UPC-A, ITF-14, QR, Data Matrix, PDF417 first |
| Chart | 2D/3D, many series types | ⬜ [R3]: bar, line, area, pie, doughnut, scatter |
| Sparkline | Line/area/bar/win-loss | ⬜ [R3] |
| Gauge | Linear and circular | ⬜ [R3] |
| Cross Tab | Pivot with totals, summaries, layouts | ⬜ [R5] |
| Rich text | RTF/HTML subset | ⬜ [R3]: HTML subset |
| Character comb | One character per cell (forms) | ⬜ [R3] |
| PDF content, PDF signature | Embed PDF pages; signature fields | ⬜ [R5] |
| Mail merge in text (`[Field]` inside text) | Embedded fields | ⬜ [R2] |

## 3. Expressions

| Feature | DevExpress | RavenRS |
|---|---|---|
| Literals, `[Field]`, `?param`, relations, comments | `'text'`, `#date#`, `True`, null, `{guid}`, `[A].[B]` | ✅ all of them, with numeric suffixes, `?` for null, `[A.B]` paths, `[^.Field]` parent row and `/* comments */` |
| Operators | Arithmetic, bitwise, comparison, `In`, `Between`, `Is Null`, logic | ✅ all, plus `Like` and `Not In` |
| Collection aggregates `[Coll][cond].Func()` | Count, Exists, Avg, Sum, Min, Max, Single, Join | ✅ with conditions: `[Coll][[Qty] > 0].Sum([Qty])`, Exists, Single, Join |
| Logical functions | Iif, IsNull (1 and 2 args), IsNullOrEmpty | ✅ |
| String functions | Ascii, Char, CharIndex, Concat, Contains, EndsWith, Insert, Len, Lower, PadLeft/Right, Remove, Replace, Reverse, StartsWith, Substring, ToStr, Trim, Upper | ✅ |
| Math functions | Abs, trig, Ceiling, Floor, Log, Power, Round, Sign, Sqr, conversions | ✅ |
| Date functions | Add*, DateDiff*, Get*, Now/Today/UtcNow, FromParts, relative predicates (IsThisMonth, InDateRange…) | 🟡 Add*, DateDiff*, Get*, Now/Today, FromParts, LocalDateTime*, Is* and range predicates. Tick precision, time-of-day predicates (IsLunchTime…), DateOnly/TimeOnly ⬜ |
| Reporting functions | FormatString, NewLine, Rgb, GetDisplayText, CurrentRowIndexInGroup, GroupIndex, Prev/NextRowColumnValue | 🟡 FormatString, NewLine, Rgb, Argb, PageNumber, PageCount, CurrentRowIndexInGroup, Prev/NextRowColumnValue. GetDisplayText, GroupIndex ⬜ [R2] |
| Custom functions | Registered by developers | ✅ `ReportEngine::register_function`, `Environment::register` |
| Criteria as a library (XPO) | Parse, print, tree operators, evaluate in memory, translate to SQL | ✅ `ravenrs-criteria`: public tree, printing that parses back, evaluation over any `Record`, schema validation, function catalog, parameterized SQL for 4 databases |

## 4. Summaries, calculated fields, data shaping

| Feature | DevExpress | RavenRS |
|---|---|---|
| Summary scope (report / group / page) | Summary Running | ✅ by band: group footer/header → group, report header/footer → report, page header/footer → page, detail → innermost group |
| Summary functions | Sum, Count, Avg, Min, Max, Median, StdDev, Var, Percentage, RecordNumber, RunningSum, distinct variants, weighted average, carry-over | ✅ all, with the `sumSum` spellings, including distinct variants, median, weighted average, percentage. Carry-over ⬜ [R3] |
| Calculated fields | Expression fields usable everywhere | ✅ |
| Grouping and sorting | Multi-level, sort groups by summary | ✅ multi-level, ascending/descending, report sort. Sort by summary ⬜ [R3] |
| Filtering | Report filter, query filter | ✅ report filter with parameters. Query-side filtering ⬜ [R5] |
| Formats | .NET format strings, Excel format | ✅ C/N/F/P/D/X, custom `#,##0.00`, `0.0%`, sections, date patterns, pt-BR and en-US. XLSX export translates them to Excel's own formats |

## 5. Parameters

| Feature | DevExpress | RavenRS |
|---|---|---|
| Types | String, dates, times, integers, decimals, Boolean, GUID | 🟡 Text, Number, Date, Boolean |
| Default (static or expression), visible/enabled expressions, allow null | | 🟡 static default. Expression defaults, visibility ⬜ [R2] |
| Multi-value, static and dynamic lookups, cascading, ranges | | ⬜ [R2] |
| Parameters panel in the viewer | Submit, reset, groups | 🟡 designer preview shows parameters. Full panel ⬜ [R2] |

## 6. Appearance

| Feature | DevExpress | RavenRS |
|---|---|---|
| Fonts, colors, borders per side, padding, alignment | | ✅ size, bold, color, background, borders per side, padding, horizontal/vertical alignment. Italic, underline, font families, dash styles ⬜ [R2] |
| Named styles, odd/even styles | | ✅ |
| Style sheets, report gallery | Reusable files | ⬜ [R4] |
| Conditional formatting | Expression-driven properties | ✅ formatting rules (condition + style), visibility expressions |
| Watermarks | Text/picture, page ranges | ⬜ [R3] |
| Units | Inches, mm, pixels | 🟡 points. Unit display in the designer ⬜ [R4] |
| Right-to-left | | ⬜ [R5] |

## 7. Page and layout

| Feature | DevExpress | RavenRS |
|---|---|---|
| Paper, orientation, margins | | ✅ A4, Letter, custom, landscape, margins |
| Multi-column | Across then down / down then across | ⬜ [R3] |
| Can grow, anchoring, push-down of controls below | | ✅ can grow, push-down. Anchoring, can shrink ⬜ [R3] |
| Keep together, print at bottom, repeat every page | | 🟡 keep-with-next, repeat group header. The rest ⬜ [R3] |
| Content splitting across pages | | ⬜ [R3]: bands taller than a page move whole |

## 8. Viewer and interactivity

| Feature | DevExpress | RavenRS |
|---|---|---|
| Page navigation, zoom, thumbnails, search, document map | | 🟡 designer preview: pages, zoom, navigation. Thumbnails, search, document map ⬜ [R2] |
| Drill-down, drill-through, interactive sorting, editing in preview | | ⬜ [R4] |
| Print, export from the viewer, email | | 🟡 print button and Cmd/Ctrl+P through the browser's dialog; PDF, Excel and CSV export. Email ⬜ [R3] |

## 9. Export

| Format | DevExpress options | RavenRS |
|---|---|---|
| PDF | PDF/A, PDF/UA, security, signatures, metadata, attachments, AcroForms | 🟡 `ravenrs-report-pdf`: PDF 1.7 vector pages, embedded font subsets, selectable text, title. Links and outline ⬜ [R2], PDF/A and signing ⬜ [R5] |
| XLSX / XLS | Single file, sheet per page, data-only mode, native formats | 🟡 `ravenrs-report-xlsx`: single sheet that follows the page, real numbers and dates, Excel-native formats, fills, borders, merged cells, page header once. Sheet per page, data-only mode ⬜ [R3], XLS not planned |
| DOCX, RTF | | ⬜ [R5] |
| HTML, MHT | | ⬜ [R3] HTML |
| CSV, TXT | | 🟡 CSV (`ravenrs_report_xlsx::to_csv`), with the culture's separator. TXT ⬜ [R3] |
| Images (PNG, SVG…) | | 🟡 SVG per page (`Document::page_svg`). PNG ⬜ [R3] |

## 10. Data

| Source | DevExpress | RavenRS |
|---|---|---|
| JSON (inline, file, web service with headers) | | ✅ inline JSON stored with the layout. URL source ⬜ [R2] |
| Objects | .NET objects | ✅ any `serde::Serialize` Rust value |
| SQL with query builder, stored procedures | MSSQL, Oracle, MySQL, PostgreSQL… | ⬜ [R5]: PostgreSQL, MySQL, SQL Server on the server, by connection name |
| Excel / CSV | | ⬜ [R5] |
| MongoDB, XML, federation (joins, unions) | | ⬜ [R5] |
| Layout saved with its data sources | REPX | ✅ JSON layout with `dataSources` |

## 11. Designer

| Feature | DevExpress | RavenRS |
|---|---|---|
| Field list (drag to create controls) | | ✅ in the web designer |
| Toolbox, property grid, report explorer, group & sort | | 🟡 toolbox on the Ribbon, property grid, band list. Group & sort panel ⬜ [R4] |
| Snap grid, resize, move across bands, undo/redo, zoom | | ✅ |
| Snap lines, align/size tools, rulers, multi-select | | ⬜ [R4] |
| Expression editor, format editor, validation (design analyzer) | | 🟡 format picker; render errors listed. Expression editor with function list ⬜ [R4] |
| Report wizard (table, labels, templates) | | 🟡 table wizard (fields, grouping, totals) and templates (invoice, sales, customer list). Labels wizard ⬜ [R4] |
| Localization editor, scripts | | ⬜ [R5]; scripts are intentionally not planned (expressions cover them safely) |
| AI: prompt to report, AI expressions | | ⬜ [R5] |

## 12. Server and automation

| Feature | DevExpress | RavenRS |
|---|---|---|
| Headless generation and export | `CreateDocument()` + `ExportTo*` | ✅ the engine has no UI dependency |
| Scheduled jobs with email (PDF/XLSX attachment, inline HTML, link), shared folder, retention, history | Report and Dashboard Server | 🟡 `ravenrs-report-server`: cron with time zones, parameters by expression, PDF/XLSX/CSV attachments, folder delivery, skip if empty, retention, history of who received what, retries. Inline HTML and links ⬜ [R3] |
| Report storage API | `ReportStorageWebExtension` | ✅ `ravenrs-report-server`: layouts stored with versions, over HTTP. Opening from the designer ⬜ [R3] |
