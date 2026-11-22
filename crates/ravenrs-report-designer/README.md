# ravenrs-report-designer

The RavenRS report designer is one view, `Designer`, that an application places in a window or a page. With it, a person lays a report out on bands, drags fields from the data onto them, edits properties and previews the paginated result. They can then print it, export it to PDF, Excel or CSV, or send it straight to a label or receipt printer. The same code runs on the desktop and in the browser. `tools/raven-report-designer` runs it on its own, and `ravenrs-app-report` opens it inside an application.

Part of [RavenRS](https://github.com/alexkads/RavenRS).

## Role

This crate is the editing half of the reporting stack. It does not define reports, and it does not render them itself:

- `ravenrs-report` holds the model and the engine. The model is `Report` with its bands, controls, data sources and parameters. The engine turns a report and its rows into pages. The designer edits a `Report` value and asks that engine for the preview.
- `ravenrs-report-viewer` shows finished pages, with a parameter panel. The designer's Preview tab is that viewer. An application that only shows reports uses the viewer, not the designer.
- `ravenrs-report-export` writes PDF, XLSX and CSV. `ravenrs-report-label` compiles pages into the languages of label and receipt printers. The designer calls both from its menus.

Reach for this crate when people need to *change* a layout. A `Report` comes in. When the person presses Save, the edited `Report` goes out to a closure that the application supplies. Storing it in a file, a database or a server is up to the application.

There are two ways to use it:

- **On its own**, with `Designer::new()`. It opens on a gallery of sample templates, and it offers a wizard that builds a report from a sample data set. This is the playground. `tools/raven-report-designer` runs it in a desktop window (`cargo run -p raven-report-designer`) and on the web page. On the web page, `?report=invoice` opens one sample through `Designer::sample`.
- **Embedded**, with `Designer::embedded(report, save_label, close_label, on_save, on_close)`. It opens on the application's report, with no gallery or wizard, and adds two buttons above the ribbon. Save calls `on_save` with the report as it stands, and Close calls `on_close`. `ravenrs-app-report` opens the stored reports of a business application this way.

## Use from start to finish

This example opens a desktop window that edits one report over two sample rows, and Save writes the layout to a file. The designer is created once, outside any build, and kept in an `Rc`. The window shows it while it is open.

```rust,no_run
use std::rc::Rc;

use ravenrs_platform_native::{PlatformError, WindowOptions, run};
use ravenrs_reactive::State;
use ravenrs_report::{Band, Control, DataSource, DataSourceKind, Report};
use ravenrs_report_designer::Designer;
use ravenrs_widgets::{BuildCx, NodeId, Text, View};

/// The window: the designer while it is open, one line once it is closed.
struct Screen {
    designer: Rc<Designer>,
    open: State<bool>,
}

impl View for Screen {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        if self.open.get() {
            self.designer.build(cx)
        } else {
            cx.child(&Text::new("The designer is closed."))
        }
    }
}

fn main() -> Result<(), PlatformError> {
    // Sample rows, kept inside the report: the field list and the preview read them.
    let rows = serde_json::json!([
        { "Customer": "Ana", "Total": 120.5 },
        { "Customer": "Bruno", "Total": 80.0 },
    ]);
    let report = Report::new("Customers")
        .culture("pt-BR")
        .data_source(DataSource { name: "Customers".into(), kind: DataSourceKind::Json { data: rows } })
        .page_header(Band::new(24.0).control(Control::label("Customers").at(0.0, 0.0, 200.0, 20.0)))
        .detail(
            Band::new(20.0)
                .control(Control::field("[Customer]").at(0.0, 0.0, 200.0, 18.0))
                .control(Control::field("[Total]").format("C2").at(220.0, 0.0, 100.0, 18.0)),
        );

    let open = State::new(true);
    let closing = open.clone();
    let designer = Designer::embedded(
        report,
        "Save",
        "Close",
        |mut report: Report| {
            // The report comes back with the sample rows in it: keep only the layout.
            for source in &mut report.data_sources {
                source.kind = DataSourceKind::Json { data: serde_json::Value::Null };
            }
            if let Err(error) = std::fs::write("customers.rvreport.json", report.to_json()) {
                eprintln!("could not save the report: {error}");
            }
        },
        move || closing.set(false),
    )
    .theme_menu(false);

    let screen = Screen { designer: Rc::new(designer), open };
    run(screen, WindowOptions::new("Customers report").size(1440.0, 900.0))
}
```

An application built on the `ravenrs` facade writes the same code with `ravenrs::native::run` and `ravenrs::native::WindowOptions`. On the web, it hands the same view to `ravenrs::web::start`, as `tools/raven-report-designer/src/lib.rs` does.

### The input

- **The report.** Any `Report` works. You can build it in code as above, read it from a `.rvreport.json` file with `Report::from_json`, or make it with `ravenrs_report::wizard`. Read the JSON before you create the designer, and tell the person there if the file is broken.
- **The preview data.** The designer only reads data stored in the report itself. It uses the `DataSourceKind::Json` source named by `report.data_source`, or the first source when none is named, narrowed by `report.data_member`. Those rows fill the field list, the row count in the status bar and the preview. A source of kind `Url` or `Sql` is kept and saved, but the designer does not fetch it. With only such a source, the field list is empty and the preview prints no rows. To design against live data, copy a sample of it into a `Json` source before you open the designer, and remove it again when you save. `ravenrs-app-report` does this with the first 200 records of the entity.
- **The parameters.** Parameters are part of the report (`Report::parameter`). When a report has any, the Preview tab shows a panel that asks for them. The values the person applies there feed the preview, print and export. Parameters not yet applied take their defaults. The values live in the designer for the session and are not written into the report. You do not pass parameter values in, and there is no way to.

### The output

- **Save.** The Save button exists only in embedded mode. It calls `on_save` with a clone of the report as it stands at that moment, sample data included. The person can press it any number of times. Nothing in the designer changes when they do, and the undo history stays. The closure shows its own storage errors, because the designer has no message for them.
- **Close.** The Close button calls `on_close` and does nothing else. The designer does not remove itself, and it does not ask about unsaved changes. The application takes it off the screen, as `Screen` does above by setting `open` to false.
- On its own (`Designer::new`), there are no Save and Close buttons. The report leaves through the JSON tab, where Copy JSON puts the whole definition on the clipboard, or through print and export.

### Choosing an entry point

- Use `Designer::new()` for a playground or a demo. It opens the sales sample with the template gallery open.
- Use `Designer::new().sample(key)` to open one of the samples in `SAMPLE_KEYS` (`invoice`, `sales`, `sales-10k`, `dashboard`, `customers`) on the Preview tab, with the gallery closed. An unknown key leaves the gallery open.
- Use `Designer::embedded(..)` whenever the report belongs to the application. Add `.theme_menu(false)` when the application keeps a look of its own. The theme menu in the status bar changes the theme of the whole application, not only the designer's.

## How it works

`Designer` is a `Component`. The constructor creates all of its state: the report being edited, the selection, the open tab, the zoom, the undo history, the parameter values and the last render. That state is shared through `State` handles and `Rc`s, and every build only reads it. Two rules follow from this:

- **Create the designer once and keep it.** If you call `Designer::embedded(..)` inside a `body` or `build`, every rebuild makes a new designer and every edit is lost at once. Keep the designer in a field. `Designer` is not `Clone`, so when a closure has to hand it out more than once, keep it in an `Rc`.
- **It lives on the UI thread.** It holds `Rc`s, so it is neither `Send` nor `Sync`. `on_save` and `on_close` run on the UI thread, inside the click.

Every change to the report is one undo step. Typing in one property field counts as a single step, however many characters it takes. The history keeps the last 200 steps.

When the Preview tab is built, the preview renders the whole report on the UI thread, through the `ReportEngine` of `ravenrs-report`, and the window waits for it. The designer keeps the rendered document and reuses it until the report or the applied parameters change. Switching tabs or turning pages does not render again. The status bar shows how long the last render took ("Gerado em … ms"), and with large data sets that is how long the window waits. Print, export and the direct printer dialog use the same render. Only sending to a printer on the desktop leaves the UI thread: the bytes go out on a thread of their own, and the dialog shows the result when it comes back.

## What comes for free

The person gets all of this without any code from the application:

- **The window.**
  - A ribbon: the File menu; the Home, Insert and Output pages; and undo, redo and print in the quick access bar.
  - A toolbox down the left side.
  - Docked on the right: the field list, the report explorer and the group-and-sort panel, above the properties, with a bar to drag between them.
  - Design, Preview and JSON tabs under the page. The ribbon and the side panels show only on the Design tab.
- **The design surface.** Bands are stacked as on paper, with rulers in centimeters and a dotted grid. The page stays light in every theme, because it is what prints.
  - A click selects a control. Dragging moves it, also into another band, and the corner handle resizes it.
  - Dragging a band's bottom edge resizes the band, never shorter than its controls.
  - Shift- or Cmd-click adds controls to the selection. The Arrange group lines them up, matches their sizes and spreads them evenly.
  - Positions snap to a 2-point grid.
  - A click on the empty canvas selects the report itself.
- **Fields and controls.** A field dragged from the field list drops where the pointer lets go. A click on a field, or Enter, adds it to the selected band. Fields of a nested collection are listed for its detail band. The toolbox and the Insert page add these controls: labels, fields, tables, pictures, barcodes, charts, check boxes, lines, rectangles, totals, "Page N of M", links and bookmarks. They also add three printer commands: cut, open drawer and beep.
- **Keyboard.** Cmd/Ctrl+Z undoes, Cmd/Ctrl+Y redoes and Cmd/Ctrl+P prints. Delete and Backspace delete the selection.
- **Expressions.** An expression editor opens for a field's expression, the report filter and each group.
- **Output.**
  - Print, PDF, Excel and CSV, from the ribbon and from the preview toolbar.
  - A direct printer dialog. It shows the dots the printer will get before anything is sent, and the report keeps the printer's model, language, resolution and connection.
- **On its own only.** The template gallery and the report wizard, from the File menu and the Home page.

The designer's text is in Brazilian Portuguese. In embedded mode, only the Save and Close labels come from the application.

## Per platform

The crate has no Cargo features. It builds for the desktop through `ravenrs-platform-native` and for `wasm32` through `web-sys`. What differs between them is how things leave the designer:

| | Desktop | Browser |
|---|---|---|
| Print | Writes the pages as HTML to the temporary folder and opens them in the default browser, with its print dialog up | Prints from a hidden frame through the browser's print dialog |
| PDF, Excel, CSV | Writes the file to the temporary folder and opens it with the program the system has for that format | Downloads the file |
| Picture from a file | Not available yet. A warning goes to the log, and the person types the picture's source into its field | A file dialog for PNG and JPEG. The report keeps the picture as a `data:` address |
| Direct printer | Print sends to the connection (`host:port`, `cups:<queue>` or a file path) on a background thread. "Salvar arquivo .prn" saves the bytes to a file | Print is disabled, because a browser cannot reach a printer. "Salvar arquivo .prn" downloads the bytes, to send some other way |

Everything else is the same on both: editing, preview, parameters, undo and the JSON tab.

## When something goes wrong

- **Edits disappear at once.** The designer is being created again in a build. Create it once, as in the example.
- **The field list is empty and the preview has no rows.** Either the report has no `Json` source with rows, or `data_member` points at nothing. The designer does not fetch a `Url` or `Sql` source, so copy sample rows into a `Json` source.
- **A red alert over the preview page.** The render found a problem, such as an expression that does not parse or a field the data does not have. The preview still renders and shows the first three messages. Fix the expression in the properties or in the expression editor.
- **"Não dá para imprimir assim" in the direct printer dialog.** The alert says which of these happened: the language is unknown, the report has a render error, or the pages could not be compiled for that printer. "Não saiu" after Print means the connection failed, and it gives the system's reason.
- **Export, print or a picture does nothing.** The failure is logged through `tracing` at warn level, from `ravenrs_report_designer` or `ravenrs_report_viewer`. The messages are:
  - "could not export the report"
  - "could not write the exported file"
  - "could not open the file"
  - "could not write the print page"
  - "choosing an image from a file is only available in the browser"

  Install a subscriber to see them. `tools/raven-report-designer` reads `RUST_LOG`, for example `RUST_LOG=info,ravenrs_report_designer=debug`.

For a bug report, send three things: the report's JSON (Copy JSON on the JSON tab, with the data if it can be shared), the platform, and the log lines above.
