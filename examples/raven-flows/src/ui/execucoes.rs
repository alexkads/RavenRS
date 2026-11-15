//! The runs: the list, and the one chosen drawn on its diagram, with Repetir and Aprovar e
//! continuar.

use std::collections::BTreeSet;

use ravenrs::prelude::*;

use super::editor::mark_key;
use super::{Cell, RavenFlows};
use crate::casa;
use crate::marcas::marked;

impl RavenFlows {
    /// The runs, and the one chosen with its diagram.
    pub(super) fn runs(&self) -> AnyView {
        let s = self.s.clone();
        let (casa, catalog) = (self.casa.clone(), self.catalog.clone());
        let now = casa::now_ms();
        let names: std::collections::BTreeMap<String, String> =
            casa.flows().into_iter().map(|f| (f.id, f.name)).collect();
        let runs = casa.runs();
        let rows: Vec<Vec<Cell>> = runs
            .iter()
            .map(|r| {
                vec![
                    r.id.clone().into(),
                    names.get(&r.flow_id).cloned().unwrap_or_else(|| r.flow_id.clone()).into(),
                    casa::run_words(r.status).into(),
                    casa::ago(r.created_ms, now).into(),
                    f64::from(r.revision).into(),
                    r.actor.clone().unwrap_or_default().into(),
                ]
            })
            .collect();
        let grid = DataGrid::new(
            GridData::new(rows),
            [
                GridColumn::text("Id").hidden(),
                GridColumn::text("Fluxo").fill(),
                GridColumn::text("Estado").width(180.0),
                GridColumn::text("Quando").width(140.0),
                GridColumn::number("Revisão").width(110.0),
                GridColumn::text("Pedida por").width(160.0),
            ],
        )
        .key_column(0)
        .selected_keys(s.run_keys.binding())
        .empty_text("Nenhuma execução ainda. Abra um fluxo e execute.")
        .height(280.0);

        let chosen = s.run_keys.get().into_iter().next().and_then(|id| casa.run_record(&id));
        let detail: AnyView = match chosen {
            None => Box::new(
                Text::new("Escolha uma execução para ver o que cada nó fez.").foreground(Semantic::InkMuted),
            ),
            Some(run) => {
                let key = mark_key(&casa, &run.id);
                if s.run_marks.get().as_deref() != Some(key.as_str()) {
                    let graph =
                        casa.flow(&run.flow_id).map(|f| crate::nos::drawn(&f, &catalog)).unwrap_or_default();
                    s.run_graph.set(marked(&graph, &casa.parts(&run.id)));
                    s.run_marks.set(Some(key));
                }
                let repeat = {
                    let (casa, s, id) = (casa.clone(), s.clone(), run.id.clone());
                    move || match casa.repeat(&id) {
                        Ok(new) => {
                            s.run_keys.set(BTreeSet::from([new]));
                            s.message.set("Execução repetida.".to_owned());
                        }
                        Err(why) => s.message.set(why),
                    }
                };
                let approve = {
                    let (casa, s, id) = (casa.clone(), s.clone(), run.id.clone());
                    move || match casa.approve(&id) {
                        Ok(()) => s.message.set("Aprovado. A execução continua.".to_owned()),
                        Err(why) => s.message.set(why),
                    }
                };
                let waiting = casa.waiting_for(&run.id).is_some();
                let mut actions = HStack::new((
                    Text::new(format!(
                        "Execução de {} · {}",
                        names.get(&run.flow_id).cloned().unwrap_or_default(),
                        casa::run_words(run.status)
                    ))
                    .style(TextStyle::Headline),
                    Spacer::new(),
                    Button::new("Repetir")
                        .icon(IconName::RotateCcw)
                        .style(ButtonStyle::Secondary)
                        .on_click(repeat),
                ))
                .spacing(8.0)
                .align(Align::Center);
                let approve_button: AnyView = if waiting {
                    Box::new(Button::new("Aprovar e continuar").icon(IconName::Check).on_click(approve))
                } else {
                    Box::new(Spacer::new().width(0.0))
                };
                actions = actions.spacing(8.0);
                Box::new(
                    VStack::new((
                        HStack::new((actions.grow(1.0), approve_button)).spacing(8.0).align(Align::Center),
                        crate::nos::kinds(
                            FlowCanvas::new(s.run_graph.binding())
                                .label("Caminho executado")
                                .locked(true)
                                .grow(1.0),
                            &catalog,
                        ),
                    ))
                    .spacing(8.0)
                    .align(Align::Stretch)
                    .grow(1.0),
                )
            }
        };
        Box::new(
            VStack::new((
                PageHeader::new("Execuções").description("Guardadas em ~/.raven-flows/execucoes.db."),
                grid,
                detail,
            ))
            .spacing(16.0)
            .padding(24.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}
