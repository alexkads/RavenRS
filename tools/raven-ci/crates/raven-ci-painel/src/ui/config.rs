//! The pipeline of a job (P-04, #888; page T5 and interaction I6, as #919 draws them): the text
//! in a `CodeEditor`, checked by the server's format at each change, each error marked on its line
//! and listed in an `Alert`, and Salvar for administrators; a new job from "Novo job" with its
//! name; and, when the plugin format-jenkinsfile is on, Converter Jenkinsfile: the `Jenkinsfile` and
//! the `.raven-ci.toml` it makes side by side, each part not converted marked on its line, with
//! "Ir para a linha".

use std::rc::Rc;

use ravenrs::prelude::*;
use serde_json::{Value as Json, json};

use super::S;

/// The format the converter of Jenkinsfiles brings.
const JENKINSFILE: &str = "jenkinsfile";

/// The pipeline a new job starts with.
const NEW_PIPELINE: &str = "[[stages]]\nname = \"Build\"\nsteps = [\"echo olá\"]\n";

/// The page of the pipeline.
pub(super) fn page(s: &Rc<S>) -> AnyView {
    let jobs: Vec<Json> = s.jobs_full.get().as_array().cloned().unwrap_or_default();
    let names: Vec<String> = jobs.iter().filter_map(|j| j["name"].as_str().map(str::to_owned)).collect();
    let new = s.config_new.get() || names.is_empty();
    // The job chosen in the grid of Jobs, or the first.
    let wanted = s.job_keys.get().into_iter().next();
    let pick = if new {
        0
    } else {
        let at = s.config_pick.get().min(names.len() - 1);
        match wanted.as_ref().and_then(|w| names.iter().position(|n| n == w)) {
            Some(chosen) if s.config_job.get().is_none() => {
                s.config_pick.set(chosen);
                chosen
            }
            _ => at,
        }
    };
    let (title, format) = if new {
        ("Novo job".to_owned(), "toml".to_owned())
    } else {
        let chosen = &jobs[pick];
        if s.config_job.get().as_deref() != Some(names[pick].as_str()) {
            s.config_job.set(Some(names[pick].clone()));
            s.config_text.set(chosen["pipeline"].as_str().unwrap_or_default().to_owned());
            s.config_checked.borrow_mut().clear();
        }
        (format!("{} · pipeline", names[pick]), chosen["format"].as_str().unwrap_or("toml").to_owned())
    };
    if new && s.config_job.get().is_some() {
        s.config_job.set(None);
    }
    if new && s.config_text.get().is_empty() {
        s.config_text.set(NEW_PIPELINE.to_owned());
    }
    check_when_changed(s, &format);

    let errors = s.config_errors.get();
    let diagnostics: Vec<CodeDiagnostic> = errors
        .iter()
        .filter(|(line, _)| *line > 0)
        .map(|(line, why)| CodeDiagnostic::error(*line, 0, 0, why.clone()))
        .collect();
    let editor = CodeEditor::new(s.config_text.binding())
        .language(if format == "toml" { CodeLanguage::Toml } else { CodeLanguage::Plain })
        .line_numbers(true)
        .diagnostics(diagnostics)
        .rows(20);
    let verdict: AnyView = if errors.is_empty() {
        Box::new(
            Alert::message(format!("O pipeline é lido sem erros pelo formato {format}."))
                .tone(Tone::Success)
                .compact(),
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
        let (s, job, format) = (s.clone(), names.get(pick).cloned().unwrap_or_default(), format.clone());
        Box::new(Button::new("Salvar").icon(IconName::Check).shortcut("⌘ S").on_click(move || {
            let name = if s.config_new.get() || job.is_empty() {
                s.config_name.get().trim().to_owned()
            } else {
                job.clone()
            };
            if name.is_empty() {
                s.message.set("Dê um nome ao job, como apps/site.".to_owned());
                return;
            }
            let body = json!({ "name": name, "format": format, "pipeline": s.config_text.get() });
            s.send("/api/ci/job", &body, move |s, status, body| {
                if status == 200 {
                    s.message.set(format!("Pipeline de {name} salvo."));
                    s.config_new.set(false);
                    s.config_job.set(None);
                    s.job_keys.set([name.clone()].into_iter().collect());
                    s.read("/api/ci/jobs", |s, body| s.jobs_full.set(body));
                } else {
                    s.message.set(body["message"].as_str().unwrap_or("não salvou").to_owned());
                }
            });
        }))
    } else {
        Box::new(Badge::new("Só leitura: salvar é dos administradores").tone(Tone::Neutral))
    };
    let chooser: AnyView = if new {
        Box::new(
            HStack::new((
                Text::new("Nome"),
                TextField::new(s.config_name.binding()).placeholder("apps/site").width(320.0),
            ))
            .spacing(12.0)
            .align(Align::Center),
        )
    } else {
        Box::new(
            HStack::new((
                Text::new("Job"),
                Select::new(s.config_pick.binding(), names.clone()).label("Job").width(320.0),
            ))
            .spacing(12.0)
            .align(Align::Center),
        )
    };
    let edit = VStack::new((chooser, editor, verdict)).spacing(16.0).align(Align::Stretch);
    let converter = s.server.get()["formats"].as_array().is_some_and(|f| f.iter().any(|x| x == JENKINSFILE));
    let body: AnyView = if converter {
        Box::new(
            Tabs::new(s.config_tab.binding())
                .variant(TabsVariant::Segmented)
                .tab("Editar", edit)
                .tab("Converter Jenkinsfile", convert_view(s)),
        )
    } else {
        Box::new(
            VStack::new((
                edit,
                Text::new("O conversor de Jenkinsfile chega com o plugin format-jenkinsfile.")
                    .foreground(Semantic::InkMuted),
            ))
            .spacing(12.0)
            .align(Align::Stretch),
        )
    };
    let description = if converter && s.config_tab.get() == 1 {
        "Cole um Jenkinsfile à esquerda. O que não puder ser convertido aparece com a linha, e nada é adivinhado.".to_owned()
    } else {
        format!("Formato {format}. Cada mudança é conferida pelo servidor; o erro aparece na linha.")
    };
    Box::new(
        VStack::new((PageHeader::new(title).description(description).action(save), body))
            .spacing(24.0)
            .padding(32.0)
            .align(Align::Stretch),
    )
}

/// Converter Jenkinsfile: the `Jenkinsfile` on the left, the TOML it makes on the right, the parts
/// not converted marked on their lines and listed, with "Ir para a linha" (I6).
fn convert_view(s: &Rc<S>) -> AnyView {
    convert_when_changed(s);
    let conversion = s.conversion.get();
    let notes: Vec<Json> = conversion["notes"].as_array().cloned().unwrap_or_default();
    let pending: Vec<&Json> = notes.iter().filter(|n| n["kind"] == "não convertido").collect();
    let diagnostics: Vec<CodeDiagnostic> = notes
        .iter()
        .map(|n| {
            let line = n["line"].as_u64().unwrap_or(1) as usize;
            let why = format!(
                "{}: {} ({})",
                n["kind"].as_str().unwrap_or_default(),
                n["what"].as_str().unwrap_or_default(),
                n["why"].as_str().unwrap_or_default()
            );
            if n["kind"] == "não convertido" {
                CodeDiagnostic::error(line, 0, 0, why)
            } else {
                CodeDiagnostic::warning(line, 0, 0, why)
            }
        })
        .collect();
    let source = CodeEditor::new(s.jenkinsfile.binding())
        .language(CodeLanguage::Plain)
        .line_numbers(true)
        .diagnostics(diagnostics)
        .caret(s.jenkinsfile_caret.clone())
        .focus_request(s.jenkinsfile_focus.clone())
        .rows(18);
    let toml = State::new(conversion["toml"].as_str().unwrap_or_default().to_owned());
    let made = CodeEditor::new(toml.binding())
        .language(CodeLanguage::Toml)
        .line_numbers(true)
        .read_only(true)
        .rows(18);
    let column = |caption: &str, editor: CodeEditor| -> AnyView {
        Box::new(
            VStack::new((
                Text::new(caption.to_owned()).style(TextStyle::Caption).foreground(Semantic::InkMuted),
                editor,
            ))
            .spacing(8.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
    };
    let sides: AnyView = if s.narrow() {
        Box::new(
            VStack::new((column("JENKINSFILE", source), column(".RAVEN-CI.TOML · GERADO", made)))
                .spacing(16.0)
                .align(Align::Stretch),
        )
    } else {
        Box::new(
            HStack::new((column("JENKINSFILE", source), column(".RAVEN-CI.TOML · GERADO", made)))
                .spacing(24.0)
                .align(Align::Start),
        )
    };
    let verdict: AnyView = match pending.first() {
        Some(first) => {
            let line = first["line"].as_u64().unwrap_or(1) as usize;
            let list: Vec<String> = pending
                .iter()
                .map(|n| {
                    format!(
                        "Linha {}: {} ({})",
                        n["line"],
                        n["what"].as_str().unwrap_or_default(),
                        n["why"].as_str().unwrap_or_default()
                    )
                })
                .collect();
            let title = if pending.len() == 1 {
                "1 trecho não convertido".to_owned()
            } else {
                format!("{} trechos não convertidos", pending.len())
            };
            let go = {
                let s = s.clone();
                move || {
                    let text = s.jenkinsfile.get();
                    let at: usize =
                        text.split_inclusive('\n').take(line.saturating_sub(1)).map(str::len).sum();
                    s.jenkinsfile_caret.set(Cursor::at(at));
                    s.jenkinsfile_focus.set(true);
                }
            };
            Box::new(
                Alert::new(title, list.join("\n"))
                    .tone(Tone::Danger)
                    .action(format!("Ir para a linha {line}"), go),
            )
        }
        None if conversion.is_null() || s.jenkinsfile.get().trim().is_empty() => Box::new(
            Alert::message("Cole o Jenkinsfile para ver o .raven-ci.toml gerado.")
                .tone(Tone::Neutral)
                .compact(),
        ),
        None => Box::new(
            Alert::message("Tudo convertido. Copie o TOML para o .raven-ci.toml do repositório.")
                .tone(Tone::Success)
                .compact(),
        ),
    };
    Box::new(
        VStack::new((
            sides,
            verdict,
            Alert::message("Os campos junit e lock só valem com os plugins junit e lock ligados. Sem eles, o erro diz qual plugin falta.")
                .tone(Tone::Neutral)
                .compact(),
        ))
        .spacing(16.0)
        .align(Align::Stretch),
    )
}

/// Asks the plugin to convert the `Jenkinsfile` again when it changed since the last time.
fn convert_when_changed(s: &Rc<S>) {
    let text = s.jenkinsfile.get();
    if *s.jenkinsfile_done.borrow() == text {
        return;
    }
    *s.jenkinsfile_done.borrow_mut() = text.clone();
    if text.trim().is_empty() {
        s.conversion.set(Json::Null);
        return;
    }
    s.send("/api/ci/jenkinsfile/convert", &json!({ "text": text }), move |s, status, body| {
        if *s.jenkinsfile_done.borrow() != text {
            return;
        }
        if status == 200 {
            s.conversion.set(body);
        } else {
            s.message.set(body["message"].as_str().unwrap_or("não converteu").to_owned());
        }
    });
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
