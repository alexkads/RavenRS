//! The DANFSe on a sheet of A4: an ordinary stored report over one row, so anyone can change it in
//! the standard designer, as the cupom and the DANFE. What the NT 008 asks stays: the key, the QR
//! Code of at least 1.52 cm, the number, and the notice of no legal validity, which comes back by
//! itself if the layout takes it out.

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Object, Value};
use ravenrs_app_report::{ReportWords, render_stored_report_with};
use ravenrs_app_ui::Session;
use ravenrs_nfse::danfse::{AVISO_SEM_VALIDADE, QR_MINIMO_PONTOS, conteudo_qr};
use ravenrs_nfse::{Dps, Emitida, Inscricao};
use ravenrs_report::barcode::Symbology;
use ravenrs_report::{Band, Control, Document, PageSettings, Report, Style, TextAlign};

/// The entity of the row of a DANFSe, which storage has to know.
pub const NFSE_LINHA: &str = "NfseLinha";

/// What the layout of the DANFSe that comes with the module is called.
pub const MODELO_DANFSE: &str = "DANFSe A4";

/// The fields a layout of the DANFSe cannot do without.
pub const CAMPOS_OBRIGATORIOS_DANFSE: [&str; 4] = ["chave", "qr_code", "numero", "aviso"];

const CAMPOS: [(&str, &str); 16] = [
    ("aviso", "Aviso do ambiente"),
    ("numero", "Número da NFS-e"),
    ("chave", "Chave de acesso"),
    ("qr_code", "QR Code"),
    ("emitida_em", "Emissão da DPS"),
    ("competencia", "Competência"),
    ("dps", "DPS"),
    ("prestador", "Prestador"),
    ("prestador_documento", "CNPJ ou CPF do prestador"),
    ("prestador_im", "Inscrição municipal do prestador"),
    ("municipio", "Município de emissão"),
    ("tomador", "Tomador"),
    ("tomador_documento", "CNPJ ou CPF do tomador"),
    ("servico", "Serviço"),
    ("codigo_servico", "Código de tributação nacional"),
    ("valor", "Valor do serviço"),
];

/// The entity of the row of a DANFSe.
pub fn danfse_entity() -> EntityInfo {
    CAMPOS.iter().fold(
        EntityInfo::new(NFSE_LINHA).caption("DANFSe").field(FieldInfo::new("id", FieldKind::Guid).key()),
        |entity, (name, caption)| entity.field(FieldInfo::new(*name, FieldKind::Text).caption(*caption)),
    )
}

fn documento(inscricao: &Inscricao) -> String {
    let n = inscricao.numero();
    match inscricao {
        Inscricao::Cnpj(_) if n.len() == 14 => {
            format!("{}.{}.{}/{}-{}", &n[..2], &n[2..5], &n[5..8], &n[8..12], &n[12..])
        }
        Inscricao::Cpf(_) if n.len() == 11 => format!("{}.{}.{}-{}", &n[..3], &n[3..6], &n[6..9], &n[9..]),
        _ => n.to_owned(),
    }
}

/// The row of the DANFSe of an NFS-e and the DPS it came from, all as text ready to print.
pub fn linha_da_nfse(emitida: &Emitida, dps: &Dps) -> Object {
    let mut linha = danfse_entity().new_object();
    let chave = emitida.chave.as_str();
    let agrupada: Vec<&str> =
        (0..chave.len()).step_by(5).filter_map(|i| chave.get(i..(i + 5).min(chave.len()))).collect();
    let valor = format!("{:.2}", dps.servico.valor).replace('.', ",");
    let tomador = dps.tomador.as_ref();
    let campos: [(&str, String); 16] = [
        ("aviso", AVISO_SEM_VALIDADE.to_owned()),
        ("numero", emitida.chave.numero().to_string()),
        ("chave", agrupada.join(" ")),
        ("qr_code", conteudo_qr(&emitida.chave)),
        ("emitida_em", dps.emitida_em.clone()),
        ("competencia", dps.competencia.clone()),
        ("dps", format!("série {} número {}", dps.serie, dps.numero)),
        ("prestador", dps.prestador.razao_social.clone()),
        ("prestador_documento", documento(&dps.prestador.inscricao)),
        ("prestador_im", dps.prestador.inscricao_municipal.clone().unwrap_or_default()),
        ("municipio", dps.prestador.municipio.clone()),
        ("tomador", tomador.map_or_else(|| "Não identificado".to_owned(), |t| t.nome.clone())),
        ("tomador_documento", tomador.map(|t| documento(&t.inscricao)).unwrap_or_default()),
        ("servico", dps.servico.descricao.clone()),
        ("codigo_servico", dps.servico.codigo_tributacao_nacional.clone()),
        ("valor", valor),
    ];
    for (campo, texto) in campos {
        linha.set(campo, texto);
    }
    linha
}

fn text(expression: &str, x: f32, y: f32, width: f32, height: f32, style: Style) -> Control {
    Control::field(expression).at(x, y, width, height).style(style)
}

/// The DANFSe as it comes, on one page of A4. Everything in it can change in the designer.
pub fn danfse_report(name: impl Into<String>) -> Report {
    let page = PageSettings::a4();
    let width = page.content_width();
    let plain = || Style::new().font_size(9.0);
    let strong = || Style::new().font_size(10.0).bold();
    let center = || Style::new().font_size(12.0).bold().align(TextAlign::Center);
    let mut report = Report::new(name).page(page);
    report.culture = "pt-BR".to_owned();
    let line = 15.0;
    // Comfortably above the 1.52 cm the NT 008 asks.
    let qr = (QR_MINIMO_PONTOS * 2.0).round();
    let left = width - qr - 12.0;
    report.report_header = Some(
        Band::new(line * 22.0)
            .control(text("[aviso]", 0.0, 0.0, width, line, center()))
            .control(text(
                "'DANFSe - Documento Auxiliar da NFS-e'",
                0.0,
                line * 1.2,
                width,
                line * 1.5,
                center(),
            ))
            .control(text("'NFS-e nº ' + [numero] + '   ' + [dps]", 0.0, line * 3.0, left, line, strong()))
            .control(text("'Chave de acesso: ' + [chave]", 0.0, line * 4.0, left, line * 2.0, plain()))
            .control(text(
                "'Emissão: ' + [emitida_em] + '   Competência: ' + [competencia]",
                0.0,
                line * 6.0,
                left,
                line,
                plain(),
            ))
            .control(Control::barcode(Symbology::Qr, "[qr_code]").at(width - qr, line * 3.0, qr, qr))
            .control(text("'PRESTADOR: ' + [prestador]", 0.0, line * 8.5, width, line, strong()))
            .control(text(
                "[prestador_documento] + '   IM ' + [prestador_im] + '   Município ' + [municipio]",
                0.0,
                line * 9.5,
                width,
                line,
                plain(),
            ))
            .control(text("'TOMADOR: ' + [tomador]", 0.0, line * 11.0, width, line, strong()))
            .control(text("[tomador_documento]", 0.0, line * 12.0, width, line, plain()))
            .control(text("'SERVIÇO ' + [codigo_servico]", 0.0, line * 13.5, width, line, strong()))
            .control(text("[servico]", 0.0, line * 14.5, width, line * 4.0, plain()))
            .control(text(
                "'VALOR DO SERVIÇO R$ ' + [valor]",
                0.0,
                line * 19.0,
                width,
                line * 1.3,
                Style::new().font_size(11.0).bold().align(TextAlign::Right),
            ))
            .control(text(
                "'ISS tributável, sem retenção. Prestador optante pelo Simples Nacional (ME/EPP).'",
                0.0,
                line * 20.5,
                width,
                line,
                plain(),
            )),
    );
    report.detail = Band::new(0.0);
    report
}

/// The layouts of the DANFSe, with what they are called.
pub fn modelos_de_danfse() -> Vec<(String, Report)> {
    vec![(MODELO_DANFSE.to_owned(), danfse_report(MODELO_DANFSE))]
}

/// What a DANFSe cannot do without and this layout left out. The notice comes back by itself.
pub fn campos_faltando_danfse(report: &Report) -> Vec<&'static str> {
    let written = report.to_json();
    CAMPOS_OBRIGATORIOS_DANFSE
        .iter()
        .copied()
        .filter(|field| *field != "aviso" && !written.contains(&format!("[{field}]")))
        .collect()
}

fn com_aviso(report: &mut Report) {
    if report.to_json().contains("[aviso]") {
        return;
    }
    let width = report.page.content_width();
    let notice = Control::field("[aviso]")
        .at(0.0, 0.0, width, 14.0)
        .style(Style::new().font_size(10.0).bold().align(TextAlign::Center));
    let header = report.report_header.get_or_insert_with(|| Band::new(0.0));
    for control in &mut header.controls {
        control.y += 14.0;
    }
    header.height += 14.0;
    header.controls.insert(0, notice);
}

/// Prepares a layout for printing: refuses one that lost what the DANFSe needs, and puts the notice
/// back. Kept apart so the rule is the same in the designer, in a test and on the counter.
pub fn preparar_danfse(report: &mut Report) -> Result<(), String> {
    let missing = campos_faltando_danfse(report);
    if !missing.is_empty() {
        return Err(format!(
            "Este layout não traz o que a DANFSe precisa: {}. Abra-o no editor e inclua.",
            missing.join(", ")
        ));
    }
    com_aviso(report);
    Ok(())
}

/// A stored DANFSe over the row of an NFS-e, or what keeps it from being printed.
pub fn renderizar_danfse(
    session: &Session,
    modelo: &Object,
    linha: &Object,
    words: &ReportWords,
) -> Result<Document, String> {
    render_stored_report_with(session, modelo, &[(linha.clone(), 1)], words, preparar_danfse)
}

/// The text of a field of the row.
pub fn campo_da_danfse(linha: &Object, nome: &str) -> String {
    match linha.get(nome) {
        Value::Text(texto) => texto,
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use ravenrs_nfe::sandbox::certificados::certificado_curinga;
    use ravenrs_nfse::sandbox::SefinSandbox;
    use ravenrs_nfse::{BetaAck, Emissor, exemplo};
    use ravenrs_report::{Item, ReportEngine};

    use super::*;

    fn emitida() -> (Emitida, Dps) {
        let emissor = Emissor::builder(BetaAck::i_understand_this_is_experimental())
            .signer(certificado_curinga().expect("certificate"))
            .transport(SefinSandbox::new())
            .build()
            .expect("emitter");
        let dps = exemplo::dps(1);
        (emissor.emitir(&dps).expect("issued"), dps)
    }

    fn dados(linha: &Object) -> serde_json::Value {
        let mut mapa = serde_json::Map::new();
        for (nome, _) in CAMPOS {
            mapa.insert(nome.to_owned(), serde_json::Value::String(campo_da_danfse(linha, nome)));
        }
        serde_json::Value::Array(vec![serde_json::Value::Object(mapa)])
    }

    #[test]
    fn one_page_of_a4_with_the_notice_the_key_and_a_large_enough_qr() {
        let (emitida, dps) = emitida();
        let linha = linha_da_nfse(&emitida, &dps);
        let mut report = danfse_report("x");
        preparar_danfse(&mut report).expect("complete");
        let doc = ReportEngine::new().render(&report, &dados(&linha));
        assert_eq!(doc.pages.len(), 1);
        assert_eq!(report.page.width, PageSettings::a4().width);
        assert!(doc.page_contains(0, AVISO_SEM_VALIDADE));
        assert!(doc.page_text(0).iter().any(|texto| texto.contains(&campo_da_danfse(&linha, "chave"))));
        assert!(doc.page_text(0).iter().any(|texto| texto.contains("150,00")));
        let qr = doc.pages[0].items.iter().find_map(|item| match item {
            Item::BarcodeMark { rect, symbology: Symbology::Qr, content } => {
                Some((rect.width(), content.clone()))
            }
            _ => None,
        });
        let (lado, conteudo) = qr.expect("a QR Code");
        assert!(lado >= QR_MINIMO_PONTOS, "{lado}");
        assert!(conteudo.ends_with(emitida.chave.as_str()));
    }

    #[test]
    fn a_layout_without_the_key_is_refused_and_the_notice_comes_back() {
        let mut sem_aviso = danfse_report("x");
        if let Some(header) = sem_aviso.report_header.as_mut() {
            header.controls.retain(|control| !format!("{control:?}").contains("[aviso]"));
        }
        assert!(!sem_aviso.to_json().contains("[aviso]"));
        preparar_danfse(&mut sem_aviso).expect("the notice is not a reason to refuse");
        assert!(sem_aviso.to_json().contains("[aviso]"));
        let mut sem_chave = danfse_report("x");
        if let Some(header) = sem_chave.report_header.as_mut() {
            header.controls.retain(|control| !format!("{control:?}").contains("[chave]"));
        }
        assert_eq!(campos_faltando_danfse(&sem_chave), ["chave"]);
        assert!(preparar_danfse(&mut sem_chave).is_err());
    }
}
