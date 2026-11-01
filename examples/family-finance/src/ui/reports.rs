//! The reports on screen, with the viewer's pages, zoom and search, and the exports.

use std::rc::Rc;

use ravenrs::native::open_with_system;
use ravenrs::prelude::*;
use ravenrs_report::Document;
use ravenrs_report_export::{ExportFormat, export};
use ravenrs_report_viewer::{DocumentViewer, ViewerLabels, ViewerState};

use super::store::{Memo, Store, heading, month_nav, screen};
use crate::reports::{ReportKind, render};

/// What this screen keeps while another one is open.
#[derive(Clone)]
pub(crate) struct ReportsState {
    kind: State<usize>,
    month: State<Date>,
    viewer: ViewerState,
    document: Memo<(u64, usize, Date), Result<Rc<Document>, String>>,
}

impl ReportsState {
    pub fn new(today: Date) -> Self {
        Self {
            kind: State::new(0),
            month: State::new(today.first_of_month()),
            viewer: ViewerState::new(),
            document: Memo::new(),
        }
    }
}

fn save(store: &Store, document: &Document, format: ExportFormat) {
    match export(document, format, Some(&store.today.to_string())) {
        Ok(file) => {
            let extension = format.extension();
            let saved = store.save_file(&file.name, (extension, &[extension]), &file.bytes);
            if let Some(path) = saved
                && let Err(e) = open_with_system(&path)
            {
                tracing::warn!(error = %e, "could not open the exported file");
            }
        }
        Err(e) => store.toasts.show_for("Não foi possível exportar", e, Tone::Danger, 8.0),
    }
}

pub(crate) fn reports(store: &Store, state: &ReportsState) -> impl View + use<> {
    let (kind_index, month) = (state.kind.get(), state.month.get());
    let kind = ReportKind::ALL[kind_index.min(ReportKind::ALL.len() - 1)];
    let document = state.document.get((store.version(), kind_index, month), || {
        let mut engine = store.engine.borrow_mut();
        render(&mut engine, &store.db, kind, month, store.today).map(Rc::new).map_err(|e| e.to_string())
    });

    let (read, write, page) = (state.kind.clone(), state.kind.clone(), state.viewer.page().clone());
    let kind_binding = Binding::new(
        move || read.get(),
        move |value| {
            // Another report starts on its first page.
            page.set(0);
            write.set(value);
        },
    );
    let uses_month = kind != ReportKind::Bills;
    let controls = HStack::new((
        SegmentedControl::new(kind_binding, ReportKind::ALL.map(ReportKind::label)).label("Relatório"),
        uses_month.then(|| month_nav(&state.month, store.today)),
    ))
    .spacing(16.0)
    .align(Align::Center);

    let body: AnyView = match document {
        Ok(document) => {
            let (export_store, export_document) = (store.clone(), document.clone());
            Box::new(
                DocumentViewer::new(document, &state.viewer).labels(ViewerLabels::pt_br()).toolbar_trailing(
                    move || {
                        let mut menu =
                            DropdownMenu::new("Exportar").icon(IconName::Download).size(ButtonSize::Small);
                        for (label, icon, format) in [
                            ("PDF", IconName::FileText, ExportFormat::Pdf),
                            ("Excel", IconName::Table, ExportFormat::Xlsx),
                            ("CSV", IconName::File, ExportFormat::Csv),
                        ] {
                            let (store, document) = (export_store.clone(), export_document.clone());
                            menu = menu.item(
                                MenuItem::new(label)
                                    .icon(icon)
                                    .on_select(move || save(&store, &document, format)),
                            );
                        }
                        vec![Box::new(menu) as AnyView]
                    },
                ),
            )
        }
        Err(message) => {
            Box::new(Alert::new("Não foi possível montar o relatório", message).tone(Tone::Danger))
        }
    };
    screen((heading("Relatórios", "O mesmo documento na tela, no PDF e na planilha."), controls, body))
}
