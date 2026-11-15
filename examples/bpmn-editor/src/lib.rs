//! A BPMN 2.0 editor made only of RavenRS components: `FlowLibrary`, `FlowToolbox`,
//! `FlowCanvas` and `FlowInspector` take BPMN from `ravenrs_flow_bpmn::notation()`, `Chat`
//! talks to a model running on this machine through `ravenrs-ai`, and `ModelManager` gets one.
//! Nothing on the screen is drawn here: the app only joins the parts.
//!
//! - The palette has the elements of BPMN; connections take the right kind as they are made.
//! - Check, in the canvas's controls, says what is wrong and outlines where.
//! - Processes are kept by name, and open and save as BPMN 2.0 XML with their drawing, which
//!   other BPMN tools read.
//! - The assistant drafts a process from a description, or reviews the one open.

use std::path::PathBuf;
use std::rc::Rc;

use ravenrs::core::{MemoryStorage, Storage};
use ravenrs::prelude::*;
use ravenrs_ai::{CATALOG, ModelFolder, ModelManager, ModelManagerLabels};
use ravenrs_flow_bpmn::plan;

/// What asks for a review instead of a draft.
pub const REVIEW: &str = "Revisar este processo";

/// The editor.
pub struct Editor {
    storage: Rc<dyn Storage>,
    models: Option<ModelManager>,
}

impl std::fmt::Debug for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor").finish_non_exhaustive()
    }
}

/// The editor with its processes kept only while it runs, and no model.
pub fn app() -> Localized<Editor> {
    Localized::new(Labels::pt_br(), Editor { storage: Rc::new(MemoryStorage::default()), models: None })
}

/// The editor keeping its processes in `storage`, and its local models in `models`.
pub fn app_kept_in(storage: Rc<dyn Storage>, models: Option<PathBuf>) -> Localized<Editor> {
    let models = models.map(|folder| {
        ModelManager::new(CATALOG, ModelFolder::new(folder)).labels(ModelManagerLabels::pt_br())
    });
    Localized::new(Labels::pt_br(), Editor { storage, models })
}

impl View for Editor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let bpmn = ravenrs_flow_bpmn::notation();
        let graph = cx.use_state(ravenrs_flow_bpmn::example_process);
        let selection = cx.use_state(FlowSelection::default);
        let history = cx.use_state(FlowHistory::new);
        let view = cx.use_state(FlowViewport::fit);
        let arrivals = cx.use_state(|| None::<FlowNode>);
        let messages = cx.use_state(Vec::<ChatMessage>::new);

        // The assistant: a description becomes a process (undone with Cmd/Ctrl+Z), and the
        // review reads the one open. Without a model, it says how to get one.
        let assistant = self.models.as_ref().and_then(ModelManager::assistant);
        let (said, g, h, v, waker) =
            (messages.clone(), graph.clone(), history.clone(), view.clone(), cx.ui_waker());
        let on_send = move |text: String| {
            let Some(model) = &assistant else {
                said.update(|m| m.push(ChatMessage::assistant("Escolha um modelo em Modelos de IA local. Ele roda neste computador e nada sai daqui.")));
                return;
            };
            let reply = if text == REVIEW {
                model.reply(plan::REVIEW_RULES.to_owned(), plan::summary(&g.get()), waker.clone(), |_| {})
            } else {
                let (told, g, h, v) = (said.clone(), g.clone(), h.clone(), v.clone());
                model.reply(plan::DRAFT_RULES.to_owned(), text, waker.clone(), move |answer| {
                    let words = match plan::plan(&answer) {
                        Ok(drafted) => {
                            let before = g.get();
                            h.update(|history| history.record(before));
                            let steps = drafted.nodes.len();
                            g.set(drafted);
                            v.set(FlowViewport::fit());
                            format!("Processo montado com {steps} elementos. Cmd/Ctrl+Z volta ao anterior.")
                        }
                        Err(why) => format!("Não consegui montar um processo com essa resposta: {why}"),
                    };
                    told.update(|m| m.push(ChatMessage::assistant(words)));
                })
            };
            said.update(|m| m.push(reply));
        };
        let chat = Chat::new(messages.binding())
            .intro("Descreva um processo, com quem faz o quê e quando decide, e eu monto o diagrama. A IA roda neste computador.")
            .placeholder("Descreva o processo")
            .examples([REVIEW])
            .on_send(on_send);
        let models: AnyView = match &self.models {
            Some(models) => Box::new(Disclosure::new("Modelos de IA local", models.clone())),
            None => Box::new(Alert::message("Sem pasta de modelos: a IA local fica desligada.")),
        };

        cx.child(
            &HStack::new((
                VStack::new((
                    FlowLibrary::new(graph.binding())
                        .title("Processos")
                        .key("processes")
                        .notation(&bpmn)
                        .history(history.binding())
                        .selection(selection.binding())
                        .view(view.binding())
                        .storage(self.storage.clone()),
                    FlowToolbox::new().title("Elementos BPMN").notation(&bpmn).arrivals(arrivals.binding()),
                ))
                .spacing(0.0)
                .align(Align::Stretch),
                FlowCanvas::new(graph.binding())
                    .label("Processo")
                    .notation(&bpmn)
                    .view(view.binding())
                    .selection(selection.binding())
                    .history(history.binding())
                    .arrivals(arrivals.binding())
                    .on_export(ravenrs_flow_export::saver(cx.file_picker(), "Processo")),
                VStack::new((
                    FlowInspector::new(graph.binding(), selection.binding())
                        .notation(&bpmn)
                        .history(history.binding()),
                    VStack::new((chat,)).grow(1.0).align(Align::Stretch),
                    models,
                ))
                .width(320.0)
                .spacing(8.0)
                .padding(8.0)
                .align(Align::Stretch),
            ))
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}
