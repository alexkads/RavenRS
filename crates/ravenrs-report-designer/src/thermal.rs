//! Printing straight to a label or receipt printer, in the printer's own language: what the dots will
//! be before anything goes out, and where it goes kept with the report.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ravenrs_reactive::{Binding, State};
use ravenrs_report::{PrinterSettings, ReportEngine};
use ravenrs_report_label::{LabelJob, Language, bitmap_to_png, compile, presets, rasterize_page};
use ravenrs_style::{SPACING, Semantic, TextStyle};
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, ButtonStyle, Dialog, FormField, HStack, Image, NodeId, ScrollView,
    Select, Text, TextField, Tone, VStack, View, ViewExt,
};

use crate::editor::Editor;
use crate::preview::{self, RenderCache};

/// The window for printing straight to a printer.
pub(crate) struct ThermalDialog {
    pub(crate) editor: Editor,
    pub(crate) cache: Rc<RefCell<RenderCache>>,
    pub(crate) params: State<HashMap<String, serde_json::Value>>,
}

/// Where the report goes, as it is kept or as it starts.
fn settings_of(editor: &Editor) -> PrinterSettings {
    editor.report.with(|report| report.printer.clone().unwrap_or_default())
}

fn change(editor: &Editor, what: impl FnOnce(&mut PrinterSettings)) {
    let mut settings = settings_of(editor);
    what(&mut settings);
    editor.edit_as("printer", |report| report.printer = Some(settings));
}

/// What it would print and in how many bytes, or why it cannot.
fn prepared(dialog: &ThermalDialog, settings: &PrinterSettings) -> Result<(Vec<u8>, Vec<Vec<u8>>), String> {
    let language = Language::from_name(&settings.language)
        .ok_or_else(|| format!("linguagem desconhecida: {}", settings.language))?;
    let document = preview::document(&dialog.editor, &dialog.cache, &dialog.params.get());
    if let Some(problem) = document.errors.first() {
        return Err(problem.clone());
    }
    let job = LabelJob::new(language, settings.dpi);
    let bytes = compile(&document, &job).map_err(|error| error.to_string())?;
    let mut engine = ReportEngine::new();
    let mut pictures = Vec::new();
    for page in document.pages.iter().take(3) {
        let bitmap =
            rasterize_page(page, settings.dpi, engine.text_system()).map_err(|error| error.to_string())?;
        pictures.push(bitmap_to_png(&bitmap));
    }
    Ok((bytes, pictures))
}

impl View for ThermalDialog {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.editor;
        if !e.thermal_open.get() {
            return cx.child(&VStack::new(Vec::<AnyView>::new()));
        }
        let settings = settings_of(e);
        let result: State<Option<Result<String, String>>> = cx.use_state(|| None);
        // The printer's model only fills in its language, which stays free to change.
        let (read, write) = (e.clone(), e.clone());
        let languages = Select::new(
            Binding::new(
                move || {
                    let name = settings_of(&read).language;
                    let found = Language::from_name(&name);
                    Language::all().iter().position(|language| Some(*language) == found).unwrap_or(0)
                },
                move |i: usize| {
                    let language = Language::all()[i.min(Language::all().len() - 1)];
                    change(&write, |s| s.language = language.name().to_owned());
                },
            ),
            Language::all().iter().map(|language| language.name().to_owned()).collect::<Vec<_>>(),
        )
        .width(220.0)
        .label("Linguagem");
        let write = e.clone();
        let chosen = State::new(0usize);
        let (chosen_read, chosen_write) = (chosen.clone(), chosen);
        let models = Select::new(
            Binding::new(
                move || chosen_read.get(),
                move |i: usize| {
                    chosen_write.set(i);
                    if let Some(language) =
                        presets().get(i).and_then(|preset| preset.languages.first().copied())
                    {
                        change(&write, |s| s.language = language.name().to_owned());
                    }
                },
            ),
            presets().iter().map(|preset| preset.name.to_owned()).collect::<Vec<_>>(),
        )
        .width(220.0)
        .label("Modelo");
        let (read, write) = (e.clone(), e.clone());
        let dpi = Select::new(
            Binding::new(
                move || usize::from(settings_of(&read).dpi == 300),
                move |i: usize| change(&write, |s| s.dpi = if i == 1 { 300 } else { 203 }),
            ),
            ["203 dpi", "300 dpi"],
        )
        .width(120.0)
        .label("Resolução");
        let (read, write) = (e.clone(), e.clone());
        let connection = TextField::new(Binding::new(
            move || settings_of(&read).connection,
            move |value: String| change(&write, |s| s.connection = value),
        ))
        .placeholder("192.168.0.50:9100, cups:termica ou /caminho/arquivo.prn");

        let prepared = prepared(self, &settings);
        let mut parts: Vec<AnyView> = vec![Box::new(
            HStack::new((
                FormField::new("Modelo", models),
                FormField::new("Linguagem", languages),
                FormField::new("Resolução", dpi),
            ))
            .spacing(SPACING.s3),
        )];
        parts.push(Box::new(FormField::new("Conexão", connection)));
        let mut bytes_to_send = None;
        match prepared {
            Ok((bytes, pictures)) => {
                parts.push(Box::new(
                    Text::new(format!("Prévia em pontos, {} dpi. {} bytes.", settings.dpi, bytes.len()))
                        .style(TextStyle::Callout)
                        .foreground(Semantic::InkMuted),
                ));
                let shown: Vec<AnyView> = pictures
                    .iter()
                    .filter_map(|png| Image::from_bytes(png).ok())
                    .map(|image| Box::new(image) as AnyView)
                    .collect();
                parts.push(Box::new(ScrollView::new(shown).frame(None, Some(360.0))));
                bytes_to_send = Some(bytes);
            }
            Err(problem) => {
                parts.push(Box::new(Alert::new("Não dá para imprimir assim", problem).tone(Tone::Danger)))
            }
        }
        if let Some(outcome) = result.get() {
            parts.push(Box::new(match outcome {
                Ok(what) => Alert::new("Pronto", what).tone(Tone::Success),
                Err(why) => Alert::new("Não saiu", why).tone(Tone::Danger),
            }));
        }
        let picker = cx.file_picker();
        let name = e.report.with(|report| report.name.clone());
        let to_file = bytes_to_send.clone();
        let save = Button::new("Salvar arquivo .prn").disabled(to_file.is_none()).on_click(move || {
            if let Some(bytes) = &to_file {
                picker.save(&format!("{name}.prn"), bytes);
            }
        });
        let send = send_button(bytes_to_send, settings.connection.clone(), result, cx);
        cx.child(
            &Dialog::new(e.thermal_open.binding(), "Impressora direta")
                .description(
                    "Etiquetas e cupons na linguagem da impressora, com o mesmo layout deste relatório.",
                )
                .width(720.0)
                .close_label("Fechar")
                .content(VStack::new(parts).spacing(SPACING.s4).align(ravenrs_layout::Align::Stretch))
                .action(HStack::new((save, send)).spacing(SPACING.s2)),
        )
    }
}

/// How a send that is under way comes back.
#[cfg(not(target_arch = "wasm32"))]
type Answer = std::sync::mpsc::Receiver<Result<(), String>>;

/// Sends what the printer takes, out of the way of the screen.
#[cfg(not(target_arch = "wasm32"))]
fn send_button(
    bytes: Option<Vec<u8>>,
    connection: String,
    result: State<Option<Result<String, String>>>,
    cx: &mut BuildCx<'_>,
) -> Button {
    let inbox: State<Option<Answer>> = cx.use_state(|| None);
    let answered = inbox.peek().as_ref().and_then(|receiver| receiver.try_recv().ok());
    if let Some(answer) = answered {
        inbox.set(None);
        result.set(Some(answer.map(|()| "Enviado para a impressora.".to_owned())));
    }
    let busy = inbox.peek().is_some();
    let waker = cx.ui_waker();
    Button::new("Imprimir")
        .style(ButtonStyle::Primary)
        .loading(busy)
        .disabled(bytes.is_none() || busy)
        .on_click(move || {
            let Some(bytes) = bytes.clone() else { return };
            let (connection, waker) = (connection.clone(), waker.clone());
            let (answer, receiver) = std::sync::mpsc::channel();
            inbox.set(Some(receiver));
            std::thread::spawn(move || {
                let sent = ravenrs_report_label::output_for(&connection)
                    .and_then(|output| output.send(&bytes))
                    .map_err(|error| error.to_string());
                let _ = answer.send(sent);
                waker.wake();
            });
        })
}

/// A browser has no way to a printer of its own: the file is the way there.
#[cfg(target_arch = "wasm32")]
fn send_button(
    _bytes: Option<Vec<u8>>,
    _connection: String,
    _result: State<Option<Result<String, String>>>,
    _cx: &mut BuildCx<'_>,
) -> Button {
    Button::new("Imprimir").style(ButtonStyle::Primary).disabled(true)
}
