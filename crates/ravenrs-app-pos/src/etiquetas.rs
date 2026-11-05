//! The screen of shelf labels: which products, how many of each and which model, to a printer or a
//! file. The models are stored reports, opened in the standard designer.

use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs_app::{Message, MessageKind, Object, Query};
use ravenrs_app_report::{
    ReportWords, design_stored_report, label_bytes, render_stored_report, send_to_printer, stored_reports_for,
};
use ravenrs_app_ui::Session;
use ravenrs_layout::Align;
use ravenrs_pos::valor_em_texto;
use ravenrs_reactive::{Binding, State};
use ravenrs_report::ReportEngine;
use ravenrs_report_label::{Bitmap, LabelJob, Language, bitmap_to_png, output_for, presets, rasterize_page};
use ravenrs_style::{Semantic, TextStyle};
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, ButtonStyle, Card, FormField, HStack, Image, NodeId, ScrollView, Select,
    Spacer, Text, TextField, Tone, VStack, View, ViewExt,
};
use rust_decimal::Decimal;

use crate::caixa::{configuracao, texto};
use crate::modelo::PRODUTO;

/// The shelf labels.
pub(crate) struct TelaEtiquetas {
    pub(crate) session: Session,
    pub(crate) palavras: Rc<ReportWords>,
}

fn nome_de(record: &Object) -> String {
    texto(record, "name")
}

fn preco_de(produto: &Object) -> String {
    match produto.get("preco") {
        ravenrs_app::Value::Number(centavos) => valor_em_texto(Decimal::new(centavos.round() as i64, 2)),
        _ => String::new(),
    }
}

/// The first label as the printer's dots will make it.
fn previa(document: &ravenrs_report::Document, dpi: u32) -> Result<Option<Bitmap>, String> {
    let Some(page) = document.pages.first() else { return Ok(None) };
    let mut engine = ReportEngine::new();
    rasterize_page(page, dpi, engine.text_system()).map(Some).map_err(|error| error.to_string())
}

impl View for TelaEtiquetas {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let session = &self.session;
        let copias: State<BTreeMap<String, String>> = cx.use_state(BTreeMap::new);
        let modelo_escolhido: State<usize> = cx.use_state(|| 0);
        let impressora: State<usize> = cx.use_state(|| 0);
        let linguagem: State<usize> = cx.use_state(|| 0);
        let conexao: State<String> = cx.use_state(|| configuracao(session, "impressora_etiqueta"));
        let aviso: State<Option<(Tone, String)>> = cx.use_state(|| None);

        let space = session.frame().space();
        let mut produtos = space.borrow().query(PRODUTO, &Query::all()).unwrap_or_default();
        produtos.sort_by_key(|produto| texto(produto, "descricao"));
        let modelos = stored_reports_for(session, PRODUTO);

        let mut lista: Vec<AnyView> = Vec::new();
        for produto in &produtos {
            let codigo = texto(produto, "codigo");
            let (read, write, chave) = (copias.clone(), copias.clone(), codigo.clone());
            let chave_lida = chave.clone();
            let campo = TextField::new(Binding::new(
                move || read.with(|all| all.get(&chave_lida).cloned().unwrap_or_default()),
                move |value: String| {
                    let chave = chave.clone();
                    write.update(|all| {
                        all.insert(chave, value);
                    });
                },
            ))
            .placeholder("0")
            .width(72.0)
            .compact()
            .end_aligned();
            lista.push(Box::new(
                HStack::new(vec![
                    Box::new(Text::new(texto(produto, "descricao"))) as AnyView,
                    Box::new(Spacer::new()),
                    Box::new(Text::new(preco_de(produto)).foreground(Semantic::InkMuted)),
                    Box::new(campo),
                ])
                .spacing(12.0)
                .align(Align::Center),
            ));
        }

        let escolhidos: Vec<(Object, u32)> = produtos
            .iter()
            .filter_map(|produto| {
                let copias_dele = copias.with(|all| all.get(&texto(produto, "codigo")).cloned())?;
                let quantas = copias_dele.trim().parse::<u32>().ok().filter(|n| *n > 0)?;
                Some((produto.clone(), quantas))
            })
            .collect();
        let total: u32 = escolhidos.iter().map(|(_, n)| *n).sum();
        let modelo = modelos.get(modelo_escolhido.get().min(modelos.len().saturating_sub(1))).cloned();

        // The printer's model only fills in its language, which stays free to change.
        let (read, write, set_language) = (impressora.clone(), impressora.clone(), linguagem.clone());
        let impressoras = Select::new(
            Binding::new(
                move || read.get(),
                move |i: usize| {
                    write.set(i);
                    if let Some(first) = presets().get(i).and_then(|preset| preset.languages.first()) {
                        let at = Language::all().iter().position(|language| language == first).unwrap_or(0);
                        set_language.set(at);
                    }
                },
            ),
            presets().iter().map(|preset| preset.name.to_owned()).collect::<Vec<_>>(),
        )
        .label("Impressora");
        let (read, write) = (linguagem.clone(), linguagem.clone());
        let linguagens = Select::new(
            Binding::new(move || read.get(), move |i: usize| write.set(i)),
            Language::all().iter().map(|language| language.name().to_owned()).collect::<Vec<_>>(),
        )
        .label("Linguagem");
        let (read, write) = (modelo_escolhido.clone(), modelo_escolhido.clone());
        let seletor_modelo = Select::new(
            Binding::new(move || read.get(), move |i: usize| write.set(i)),
            modelos.iter().map(nome_de).collect::<Vec<_>>(),
        )
        .label("Modelo");
        let (read, write) = (conexao.clone(), conexao.clone());
        let campo_conexao = TextField::new(Binding::new(move || read.get(), move |value: String| write.set(value)))
            .placeholder("192.168.0.50:9100 ou cups:etiquetas");

        let idioma = Language::all()[linguagem.get().min(Language::all().len() - 1)];
        let job = LabelJob::new(idioma, 203);
        let amostra: Vec<(Object, u32)> = if escolhidos.is_empty() {
            produtos.first().map(|produto| (produto.clone(), 1)).into_iter().collect()
        } else {
            escolhidos.clone()
        };
        let documento = modelo
            .as_ref()
            .ok_or_else(|| "Não há modelo de etiqueta.".to_owned())
            .and_then(|modelo| render_stored_report(session, modelo, &amostra, &self.palavras));

        let mut lado: Vec<AnyView> = vec![
            Box::new(FormField::new("Modelo", seletor_modelo)),
            Box::new(FormField::new("Impressora", impressoras)),
            Box::new(FormField::new("Linguagem", linguagens)),
            Box::new(FormField::new("Conexão", campo_conexao)),
        ];
        match &documento {
            Ok(documento) => match previa(documento, job.dpi) {
                Ok(Some(bitmap)) => {
                    if let Ok(imagem) = Image::from_bytes(&bitmap_to_png(&bitmap)) {
                        lado.push(Box::new(Text::new("Prévia em pontos, 203 dpi").style(TextStyle::Caption)));
                        lado.push(Box::new(imagem.width(240.0)));
                    }
                }
                Ok(None) => {}
                Err(why) => lado.push(Box::new(Alert::new("Não cabe na etiqueta", why).tone(Tone::Danger))),
            },
            Err(why) => lado.push(Box::new(Alert::new("Modelo com problema", why.clone()).tone(Tone::Danger))),
        }
        if let Some((tone, text)) = aviso.get() {
            lado.push(Box::new(Alert::new("Impressão", text).tone(tone)));
        }

        let editar = {
            let (session, modelo, palavras) = (session.clone(), modelo.clone(), self.palavras.clone());
            Button::new("Editar modelo no editor de relatórios").style(ButtonStyle::Secondary).full_width().disabled(modelo.is_none()).on_click(
                move || {
                    if let Some(modelo) = &modelo
                        && let Err(why) = design_stored_report(&session, modelo, &palavras)
                    {
                        session.show_message(Message::new(why, MessageKind::Error));
                    }
                },
            )
        };
        let bytes = documento.as_ref().ok().filter(|_| total > 0).map(|document| label_bytes(document, &job));
        let imprimir = {
            let (session, aviso, conexao) = (session.clone(), aviso.clone(), conexao.clone());
            let bytes = bytes.clone();
            Button::new(format!("Imprimir {total} etiquetas"))
                .style(ButtonStyle::Primary)
                .full_width()
                .disabled(bytes.is_none())
                .on_click(move || match bytes.clone() {
                    Some(Ok(bytes)) => match output_for(&conexao.peek()) {
                        Ok(output) => {
                            let aviso = aviso.clone();
                            send_to_printer(&session, "Imprimindo etiquetas", bytes, output, move |_, sent| {
                                aviso.set(Some(match sent {
                                    Ok(()) => (Tone::Success, "Enviado para a impressora.".to_owned()),
                                    Err(why) => (Tone::Danger, why),
                                }));
                            });
                        }
                        Err(why) => aviso.set(Some((Tone::Danger, why.to_string()))),
                    },
                    Some(Err(why)) => aviso.set(Some((Tone::Danger, why))),
                    None => {}
                })
        };
        let salvar = {
            let picker = cx.file_picker();
            let session = session.clone();
            let bytes = bytes.clone();
            Button::new("Salvar arquivo .prn").style(ButtonStyle::Secondary).full_width().disabled(bytes.is_none()).on_click(move || {
                if let Some(Ok(bytes)) = &bytes {
                    session.hand_file("etiquetas.prn".to_owned(), bytes.clone(), &picker);
                }
            })
        };
        let botoes: Vec<AnyView> = vec![Box::new(editar), Box::new(salvar), Box::new(imprimir)];

        cx.child(
            &HStack::new(vec![
                Box::new(
                    Card::new(ScrollView::new(vec![
                        Box::new(VStack::new(lista).spacing(8.0).align(Align::Stretch)) as AnyView,
                    ]))
                    .grow(1.0)
                    .stretch(),
                ) as AnyView,
                Box::new(
                    VStack::new(vec![
                        Box::new(
                            ScrollView::new(vec![
                                Box::new(VStack::new(lado).spacing(12.0).align(Align::Stretch)) as AnyView,
                            ])
                            .grow(1.0),
                        ) as AnyView,
                        Box::new(VStack::new(botoes).spacing(8.0).align(Align::Stretch)),
                    ])
                    .spacing(12.0)
                    .align(Align::Stretch)
                    .width(380.0),
                ),
            ])
            .spacing(16.0)
            .align(Align::Stretch)
            .padding(16.0)
            .grow(1.0),
        )
    }
}
