//! The pipeline of a job (P-04, #888; page T5 and interaction I6): the text in a `CodeEditor`,
//! checked by the server's format at each change, each error marked on its line and listed in an
//! `Alert`, and Salvar (⌘ S) for administrators. The tab Converter Jenkinsfile shows only when a
//! plugin on reads that format (PL-02).

use std::rc::Rc;

use ravenrs::prelude::*;
use serde_json::{Value as Json, json};

use super::{S, nada};

/// The format the converter of Jenkinsfiles brings.
const JENKINSFILE: &str = "jenkinsfile";

/// The page of the pipeline.
pub(super) fn page(s: &Rc<S>) -> AnyView {
    let jobs: Vec<Json> = s.jobs_full.get().as_array().cloned().unwrap_or_default();
    let names: Vec<String> = jobs.iter().filter_map(|j| j["name"].as_str().map(str::to_owned)).collect();
    if names.is_empty() {
        return Box::new(
            VStack::new((
                PageHeader::new("Configuração")
                    .description("O pipeline de cada job, no formato do plugin que o lê."),
                EmptyState::new("Nenhum job ainda")
                    .description("Crie um com raven-ci, ou pela API, e edite o pipeline aqui.")
                    .icon(IconName::Code),
            ))
            .spacing(16.0)
            .padding(24.0)
            .align(Align::Stretch),
        );
    }
    let pick = s.config_pick.get().min(names.len() - 1);
    let chosen = &jobs[names.iter().position(|n| *n == names[pick]).unwrap_or(0)];
    if s.config_job.get().as_deref() != Some(names[pick].as_str()) {
        s.config_job.set(Some(names[pick].clone()));
        s.config_text.set(chosen["pipeline"].as_str().unwrap_or_default().to_owned());
        s.config_checked.borrow_mut().clear();
    }
    let format = chosen["format"].as_str().unwrap_or("toml").to_owned();
    check_when_changed(s, &format);

    let errors = s.config_errors.get();
    let diagnostics: Vec<CodeDiagnostic> = errors
        .iter()
        .filter(|(line, _)| *line > 0)
        .map(|(line, why)| CodeDiagnostic::error(*line, 0, 0, why.clone()))
        .collect();
    let editor = CodeEditor::new(s.config_text.binding())
        .language(CodeLanguage::Plain)
        .line_numbers(true)
        .diagnostics(diagnostics)
        .rows(20);
    let verdict: AnyView = if errors.is_empty() {
        Box::new(
            Alert::message(format!("O pipeline é lido sem erros pelo formato {format}.")).tone(Tone::Success),
        )
    } else {
        let list: Vec<String> = errors
            .iter()
            .map(|(line, why)| if *line > 0 { format!("Linha {line}: {why}") } else { why.clone() })
            .collect();
        let title = if errors.len() == 1 {
            "1 erro no pipeline".to_owned()
        } else {
            format!("{} erros no pipeline", errors.len())
        };
        Box::new(Alert::new(title, list.join("\n")).tone(Tone::Danger))
    };
    let save: AnyView = if s.admin() {
        let (s, job, format) = (s.clone(), names[pick].clone(), format.clone());
        Box::new(Button::new("Salvar").icon(IconName::Save).shortcut("⌘ S").on_click(move || {
            let body = json!({ "name": job, "format": format, "pipeline": s.config_text.get() });
            let job = job.clone();
            s.send("/api/ci/job", &body, move |s, status, body| {
                if status == 200 {
                    s.message.set(format!("Pipeline de {job} salvo."));
                    s.read("/api/ci/jobs", |s, body| s.jobs_full.set(body));
                } else {
                    s.message.set(body["message"].as_str().unwrap_or("não salvou").to_owned());
                }
            });
        }))
    } else {
        Box::new(Badge::new("Só leitura: salvar é dos administradores").tone(Tone::Neutral))
    };
    let converter = s.server.get()["formats"].as_array().is_some_and(|f| f.iter().any(|x| x == JENKINSFILE));
    let edit = VStack::new((editor, verdict)).spacing(12.0).align(Align::Stretch);
    let body: AnyView = if converter {
        Box::new(Tabs::new(s.config_tab.binding()).tab("Editar", edit).tab(
            "Converter Jenkinsfile",
            Alert::message("Cole o Jenkinsfile para ver o .raven-ci.toml gerado.").tone(Tone::Neutral),
        ))
    } else {
        Box::new(edit)
    };
    Box::new(
        VStack::new((
            PageHeader::new(format!("{} · pipeline", names[pick]))
                .description(format!(
                    "Formato {format}. Cada mudança é conferida pelo servidor; o erro aparece na linha."
                ))
                .action(save),
            HStack::new((
                Text::new("Job"),
                Select::new(s.config_pick.binding(), names.clone()).label("Job").width(320.0),
                Spacer::new(),
                if converter {
                    nada()
                } else {
                    Box::new(
                        Text::new("O conversor de Jenkinsfile chega com o plugin format-jenkinsfile.")
                            .foreground(Semantic::InkMuted),
                    ) as AnyView
                },
            ))
            .spacing(12.0)
            .align(Align::Center),
            body,
        ))
        .spacing(16.0)
        .padding(24.0)
        .align(Align::Stretch),
    )
}

/// Asks the server to read the text again when it changed since the last check.
fn check_when_changed(s: &Rc<S>, format: &str) {
    let text = s.config_text.get();
    if *s.config_checked.borrow() == text {
        return;
    }
    *s.config_checked.borrow_mut() = text.clone();
    s.send("/api/ci/pipeline/check", &json!({ "format": format, "text": text }), move |s, status, body| {
        if *s.config_checked.borrow() != text {
            return;
        }
        if status != 200 {
            s.config_errors.set(vec![(0, body["message"].as_str().unwrap_or("não conferiu").to_owned())]);
            return;
        }
        let errors = body["errors"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| {
                (
                    e["line"].as_u64().unwrap_or(0) as usize,
                    e["message"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect();
        s.config_errors.set(errors);
    });
}
