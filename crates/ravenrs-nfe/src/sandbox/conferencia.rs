//! The default behavior of the sandbox: it really checks what arrives, in the order a real
//! server would: envelope, schema, environment, signature, key, required texts, totals, duplicates.
//!
//! The order of tags comes from `leiauteNFe_v4.00.xsd`; the sandbox only knows the tags, not all
//! of their value patterns, so passing here does not replace validating against the XSD.

use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::assinatura::{FalhaAssinatura, verify_enveloped};
use crate::chave::{ChaveAcesso, digito_mod11, forma_valida};
use crate::documento::TP_EVENTO_CANCELAMENTO;
use crate::modelo::{TEXTO_HOMOLOGACAO_DESTINATARIO, TEXTO_HOMOLOGACAO_ITEM};
use crate::retorno::descricao_c_stat;
use crate::soap::{NS_NFE, NS_SOAP12, Servico};
use crate::totais::arredondar;
use crate::xml::{self, Element, Node};

use super::resposta::{self, DadosEvento, DadosProtocolo};
use super::{Resposta, SandboxRequest, SefazSandbox};

/// One document the sandbox already authorized.
#[derive(Clone, Debug)]
struct Nota {
    n_prot: String,
    dig_val: String,
    dh: String,
    cancelada: bool,
}

/// What the sandbox remembers.
#[derive(Debug, Default)]
pub(crate) struct Notas {
    por_chave: HashMap<String, Nota>,
    /// Issuer, model, series and number to the key that took them.
    por_numero: HashMap<(String, String, String, String), String>,
}

/// The order of the tags under each parent.
const ORDEM: &[(&str, &[&str])] = &[
    ("enviNFe", &["idLote", "indSinc", "NFe"]),
    ("NFe", &["infNFe", "infNFeSupl", "Signature"]),
    (
        "infNFe",
        &[
            "ide",
            "emit",
            "avulsa",
            "dest",
            "retirada",
            "entrega",
            "autXML",
            "det",
            "total",
            "transp",
            "cobr",
            "pag",
            "infIntermed",
            "gPgtoVinc",
            "infAdic",
            "exporta",
            "compra",
            "cana",
            "infRespTec",
            "infSolicNFF",
            "agropecuario",
        ],
    ),
    (
        "ide",
        &[
            "cUF",
            "cNF",
            "natOp",
            "mod",
            "serie",
            "nNF",
            "dhEmi",
            "dhSaiEnt",
            "dPrevEntrega",
            "tpNF",
            "idDest",
            "cMunFG",
            "cMunFGIBS",
            "tpImp",
            "tpEmis",
            "cDV",
            "tpAmb",
            "finNFe",
            "tpNFDebito",
            "tpNFCredito",
            "indFinal",
            "indPres",
            "indIntermed",
            "cIndOp",
            "procEmi",
            "verProc",
            "dhCont",
            "xJust",
            "NFref",
            "gCompraGov",
            "gPagAntecipado",
        ],
    ),
    ("emit", &["CNPJ", "CPF", "xNome", "xFant", "enderEmit", "IE", "IEST", "IM", "CNAE", "CRT", "ISUFEmit"]),
    ("enderEmit", &["xLgr", "nro", "xCpl", "xBairro", "cMun", "xMun", "UF", "CEP", "cPais", "xPais", "fone"]),
    ("enderDest", &["xLgr", "nro", "xCpl", "xBairro", "cMun", "xMun", "UF", "CEP", "cPais", "xPais", "fone"]),
    (
        "dest",
        &["CNPJ", "CPF", "idEstrangeiro", "xNome", "enderDest", "indIEDest", "IE", "ISUF", "IM", "email"],
    ),
    ("det", &["prod", "imposto", "impostoDevol", "infAdProd", "obsItem", "vItem", "DFeReferenciado"]),
    (
        "prod",
        &[
            "cProd",
            "cEAN",
            "cBarra",
            "xProd",
            "NCM",
            "NVE",
            "CEST",
            "indEscala",
            "CNPJFab",
            "cBenef",
            "gCred",
            "tpCredPresIBSZFM",
            "EXTIPI",
            "CFOP",
            "uCom",
            "qCom",
            "vUnCom",
            "vProd",
            "vUnComLiq",
            "vProdLiq",
            "cEANTrib",
            "cBarraTrib",
            "uTrib",
            "qTrib",
            "vUnTrib",
            "vFrete",
            "vSeg",
            "vDesc",
            "vOutro",
            "indTot",
            "indBemMovelUsado",
        ],
    ),
    (
        "imposto",
        &[
            "vTotTrib",
            "ICMS",
            "IPI",
            "II",
            "ISSQN",
            "PIS",
            "PISST",
            "COFINS",
            "COFINSST",
            "ICMSUFDest",
            "IS",
            "IBSCBS",
        ],
    ),
    ("total", &["vProdLiqTot", "ICMSTot", "ISSQNtot", "retTrib", "ISTot", "IBSCBSTot", "vNFTot"]),
    (
        "ICMSTot",
        &[
            "vBC",
            "vICMS",
            "vICMSDeson",
            "vFCPUFDest",
            "vICMSUFDest",
            "vICMSUFRemet",
            "vFCP",
            "vBCST",
            "vST",
            "vFCPST",
            "vFCPSTRet",
            "qBCMono",
            "vICMSMono",
            "qBCMonoReten",
            "vICMSMonoReten",
            "qBCMonoRet",
            "vICMSMonoRet",
            "vProd",
            "vFrete",
            "vSeg",
            "vDesc",
            "vII",
            "vIPI",
            "vIPIDevol",
            "vPIS",
            "vCOFINS",
            "vOutro",
            "vNF",
            "vTotTrib",
        ],
    ),
    ("transp", &["modFrete", "transporta", "retTransp", "veicTransp", "reboque", "vagao", "balsa", "vol"]),
    ("pag", &["detPag", "vTroco"]),
    ("detPag", &["indPag", "tPag", "xPag", "vPag", "dPag", "CNPJPag", "UFPag", "card"]),
    ("infAdic", &["infAdFisco", "infCpl", "obsCont", "obsFisco", "procRef"]),
    ("infNFeSupl", &["qrCode", "urlChave"]),
    ("Signature", &["SignedInfo", "SignatureValue", "KeyInfo"]),
    ("envEvento", &["idLote", "evento"]),
    ("evento", &["infEvento", "Signature"]),
    (
        "infEvento",
        &[
            "cOrgao",
            "tpAmb",
            "CNPJ",
            "CPF",
            "chNFe",
            "dhEvento",
            "tpEvento",
            "nSeqEvento",
            "verEvento",
            "detEvento",
        ],
    ),
    ("detEvento", &["descEvento", "nProt", "xJust"]),
];

/// The first problem in the order of the tags, or in white space between them.
fn problema_de_estrutura(element: &Element) -> Option<u32> {
    let elementos = element.elements().count();
    if elementos > 0 && element.children.iter().any(|node| matches!(node, Node::Text(_))) {
        // Text between tags: the rejection for editing characters inside the message.
        return Some(588);
    }
    if let Some((_, ordem)) = ORDEM.iter().find(|(pai, _)| *pai == element.local_name()) {
        let mut ultimo = 0;
        for filho in element.elements() {
            let posicao = ordem.iter().position(|nome| *nome == filho.local_name())?;
            if posicao < ultimo {
                return Some(225);
            }
            ultimo = posicao;
        }
    }
    // Signature keeps its own inner structure, which the signature check looks at.
    if element.local_name() == "Signature" {
        return None;
    }
    element.elements().find_map(problema_de_estrutura)
}

/// Same as [`problema_de_estrutura`], also refusing tags the sandbox does not know.
fn problema_em_nivel(element: &Element) -> Option<u32> {
    if let Some((_, ordem)) = ORDEM.iter().find(|(pai, _)| *pai == element.local_name())
        && element.elements().any(|filho| !ordem.contains(&filho.local_name()))
    {
        return Some(225);
    }
    if element.local_name() == "Signature" {
        return None;
    }
    element.elements().find_map(problema_em_nivel)
}

fn estrutura(element: &Element) -> Option<u32> {
    problema_em_nivel(element).or_else(|| problema_de_estrutura(element))
}

/// Finds the namespace of an element's prefix among the attributes of the element itself.
fn namespace_proprio(element: &Element) -> Option<&str> {
    match element.name.split_once(':') {
        Some((prefixo, _)) => element.attr(&format!("xmlns:{prefixo}")),
        None => element.attr("xmlns"),
    }
}

/// Checks the envelope and gives back the message inside it.
fn abrir_envelope(pedido: &SandboxRequest, servico: Servico) -> Result<Element, Resposta> {
    let tipo = pedido.http.header("Content-Type").unwrap_or("");
    if !tipo.to_ascii_lowercase().starts_with("application/soap+xml") {
        return Err(Resposta::Http(resposta::fault(415, "Content-Type não é SOAP 1.2")));
    }
    let envelope = xml::parse(pedido.body()).map_err(|_| pedido.responder(243))?;
    if envelope.local_name() != "Envelope" || namespace_proprio(&envelope) != Some(NS_SOAP12) {
        return Err(Resposta::Http(resposta::fault(400, "Envelope não é SOAP 1.2")));
    }
    let mensagem = envelope
        .child("Body")
        .and_then(|body| body.child("nfeDadosMsg"))
        .ok_or_else(|| Resposta::Http(resposta::fault(400, "Body sem nfeDadosMsg")))?;
    if mensagem.attr("xmlns") != Some(servico.namespace().as_str()) {
        return Err(Resposta::Http(resposta::fault(400, "nfeDadosMsg com namespace diferente da ação")));
    }
    let mut filhos = mensagem.elements();
    let dados = filhos.next().ok_or_else(|| pedido.responder(225))?;
    if filhos.next().is_some() {
        return Err(pedido.responder(225));
    }
    Ok(dados.clone())
}

/// Runs the checks of the service of this request.
pub(super) fn conferir(pedido: &SandboxRequest, sandbox: &SefazSandbox) -> Resposta {
    let Some(servico) = pedido.servico else {
        return Resposta::Proximo;
    };
    let dados = match abrir_envelope(pedido, servico) {
        Ok(dados) => dados,
        Err(resposta) => return resposta,
    };
    if dados.attr("xmlns") != Some(NS_NFE) {
        return pedido.responder(225);
    }
    if let Some(tp_amb) = tp_amb_de(&dados)
        && tp_amb != "2"
    {
        return pedido.responder_com(252, "O sandbox só aceita tpAmb=2");
    }
    match servico {
        Servico::Status => conferir_status(pedido, &dados),
        Servico::Autorizacao => conferir_autorizacao(pedido, sandbox, &dados),
        Servico::Consulta => conferir_consulta(pedido, sandbox, &dados),
        Servico::Evento => conferir_evento(pedido, sandbox, &dados),
    }
}

fn tp_amb_de(dados: &Element) -> Option<String> {
    dados.find_text("tpAmb")
}

fn conferir_status(pedido: &SandboxRequest, dados: &Element) -> Resposta {
    if dados.local_name() != "consStatServ"
        || dados.child_text("xServ").as_deref() != Some("STATUS")
        || dados.child("cUF").is_none()
        || dados.child("tpAmb").is_none()
    {
        return pedido.responder(225);
    }
    pedido.responder(107)
}

fn decimal(element: &Element, tag: &str) -> Option<Decimal> {
    element.child_text(tag)?.parse().ok()
}

/// The totals of a document, or the rejection for the first that does not add up.
fn conferir_totais(inf_nfe: &Element) -> Result<(), u32> {
    let totais = inf_nfe.child("total").and_then(|t| t.child("ICMSTot")).ok_or(225_u32)?;
    let campo = |tag: &str| decimal(totais, tag).ok_or(225_u32);
    let mut soma_prod = Decimal::ZERO;
    let mut soma_desc = Decimal::ZERO;
    for det in inf_nfe.elements().filter(|e| e.local_name() == "det") {
        let prod = det.child("prod").ok_or(225_u32)?;
        let quantidade = decimal(prod, "qCom").ok_or(225_u32)?;
        let unitario = decimal(prod, "vUnCom").ok_or(225_u32)?;
        let valor = decimal(prod, "vProd").ok_or(225_u32)?;
        let calculado = quantidade.checked_mul(unitario).ok_or(225_u32)?;
        if arredondar(calculado) != valor {
            return Err(629);
        }
        soma_prod += valor;
        soma_desc += decimal(prod, "vDesc").unwrap_or(Decimal::ZERO);
    }
    if soma_prod != campo("vProd")? {
        return Err(564);
    }
    if soma_desc != campo("vDesc")? {
        return Err(537);
    }
    let calculado = campo("vProd")? - campo("vDesc")? - campo("vICMSDeson")?
        + campo("vST")?
        + campo("vFCPST")?
        + campo("vFrete")?
        + campo("vSeg")?
        + campo("vOutro")?
        + campo("vII")?
        + campo("vIPI")?
        + campo("vIPIDevol")?;
    let v_nf = campo("vNF")?;
    if calculado != v_nf {
        return Err(610);
    }
    let pag = inf_nfe.child("pag").ok_or(225_u32)?;
    let mut pago = Decimal::ZERO;
    for det_pag in pag.elements().filter(|e| e.local_name() == "detPag") {
        pago += decimal(det_pag, "vPag").ok_or(225_u32)?;
    }
    if pago < v_nf {
        return Err(865);
    }
    if pago - v_nf != decimal(pag, "vTroco").unwrap_or(Decimal::ZERO) {
        return Err(869);
    }
    Ok(())
}

/// The key inside the message must be the concatenation of the fields it repeats.
fn conferir_chave(inf_nfe: &Element, chave: &str) -> Result<(), u32> {
    let semente = &chave[..43];
    if chave.as_bytes()[43] - b'0' != digito_mod11(semente) {
        return Err(253);
    }
    let ide = inf_nfe.child("ide").ok_or(225_u32)?;
    let emit = inf_nfe.child("emit").ok_or(225_u32)?;
    let texto = |e: &Element, tag: &str| e.child_text(tag).ok_or(225_u32);
    let dh = texto(ide, "dhEmi")?;
    let serie: u32 = texto(ide, "serie")?.parse().map_err(|_| 225_u32)?;
    let numero: u32 = texto(ide, "nNF")?.parse().map_err(|_| 225_u32)?;
    let esperada = format!(
        "{}{}{}{}{}{:03}{:09}{}{}{}",
        texto(ide, "cUF")?,
        dh.get(2..4).ok_or(225_u32)?,
        dh.get(5..7).ok_or(225_u32)?,
        texto(emit, "CNPJ")?,
        texto(ide, "mod")?,
        serie,
        numero,
        texto(ide, "tpEmis")?,
        texto(ide, "cNF")?,
        texto(ide, "cDV")?,
    );
    if esperada != chave {
        return Err(502);
    }
    Ok(())
}

/// The texts homologation requires.
fn conferir_textos(inf_nfe: &Element, modelo: &str) -> Result<(), u32> {
    if modelo == "55" {
        let nome = inf_nfe.child("dest").and_then(|dest| dest.child_text("xNome"));
        if nome.as_deref() != Some(TEXTO_HOMOLOGACAO_DESTINATARIO) {
            return Err(598);
        }
    } else {
        let item = inf_nfe.child("det").and_then(|det| det.child("prod")).and_then(|p| p.child_text("xProd"));
        if item.as_deref() != Some(TEXTO_HOMOLOGACAO_ITEM) {
            return Err(373);
        }
    }
    Ok(())
}

fn conferir_autorizacao(pedido: &SandboxRequest, sandbox: &SefazSandbox, dados: &Element) -> Resposta {
    if dados.local_name() != "enviNFe" || dados.attr("versao") != Some("4.00") {
        return pedido.responder(225);
    }
    if !dados
        .child_text("idLote")
        .is_some_and(|l| !l.is_empty() && l.len() <= 15 && l.bytes().all(|b| b.is_ascii_digit()))
    {
        return pedido.responder(225);
    }
    if let Some(codigo) = estrutura(dados) {
        return pedido.responder(codigo);
    }
    match dados.child_text("indSinc").as_deref() {
        Some("1") => {}
        Some("0") => return pedido.responder(452),
        _ => return pedido.responder(225),
    }
    let notas: Vec<&Element> = dados.elements().filter(|e| e.local_name() == "NFe").collect();
    let [nfe] = notas.as_slice() else {
        return pedido.responder_com(225, "O sandbox só aceita lote com uma nota");
    };
    let Some(inf_nfe) = nfe.child("infNFe") else {
        return pedido.responder(225);
    };
    let Some(id) = inf_nfe.attr("Id").filter(|_| inf_nfe.attr("versao") == Some("4.00")) else {
        return pedido.responder(225);
    };
    let Some(chave) = id.strip_prefix("NFe").filter(|chave| forma_valida(chave)) else {
        return pedido.responder(225);
    };
    let Some(ide) = inf_nfe.child("ide") else {
        return pedido.responder(225);
    };
    for grupo in ["emit", "det", "total", "transp", "pag"] {
        if inf_nfe.child(grupo).is_none() {
            return pedido.responder(225);
        }
    }
    let modelo = ide.child_text("mod").unwrap_or_default();
    if modelo != "55" && modelo != "65" {
        return pedido.responder(225);
    }

    // The document is checked by itself, as it travels: the SOAP around it signs nothing.
    match verify_enveloped(nfe, id) {
        Ok(()) => {}
        Err(FalhaAssinatura::ForaDoPadrao(motivo)) => {
            return pedido.responder_com(298, &format!("{}: {motivo}", texto_de(298)));
        }
        Err(FalhaAssinatura::Difere(motivo)) => {
            return pedido.responder_com(297, &format!("{}: {motivo}", texto_de(297)));
        }
    }

    if let Err(codigo) = conferir_chave(inf_nfe, chave) {
        return pedido.responder(codigo);
    }
    if let Err(codigo) = conferir_textos(inf_nfe, &modelo) {
        return pedido.responder(codigo);
    }
    if modelo == "65" && nfe.child("infNFeSupl").is_none() {
        return pedido.responder(394);
    }
    let crt = inf_nfe.child("emit").and_then(|emit| emit.child_text("CRT"));
    if crt.as_deref() == Some("3")
        && inf_nfe
            .elements()
            .filter(|e| e.local_name() == "det")
            .any(|det| det.child("imposto").and_then(|i| i.child("IBSCBS")).is_none())
    {
        return pedido.responder(1115);
    }
    if let Err(codigo) = conferir_totais(inf_nfe) {
        return pedido.responder(codigo);
    }
    registrar_nota(pedido, sandbox, nfe, inf_nfe, ide, chave, &modelo)
}

fn registrar_nota(
    pedido: &SandboxRequest,
    sandbox: &SefazSandbox,
    nfe: &Element,
    inf_nfe: &Element,
    ide: &Element,
    chave: &str,
    modelo: &str,
) -> Resposta {
    let cnpj = inf_nfe.child("emit").and_then(|e| e.child_text("CNPJ")).unwrap_or_default();
    let numero_chave = (
        cnpj,
        modelo.to_string(),
        ide.child_text("serie").unwrap_or_default(),
        ide.child_text("nNF").unwrap_or_default(),
    );
    let dig_val = nfe.find_text("DigestValue").unwrap_or_default();
    let c_uf = ide.child_text("cUF").unwrap_or_default();
    let dh = pedido.dh_recbto.as_str();

    let mut estado = sandbox.lock();
    if let Some(nota) = estado.notas.por_chave.get(chave) {
        let motivo = format!("{} [nProt:{}]", texto_de(204), nota.n_prot);
        drop(estado);
        return pedido.responder_com(204, &motivo);
    }
    if let Some(existente) = estado.notas.por_numero.get(&numero_chave) {
        let motivo = format!("{} [chNFe:{existente}]", texto_de(539));
        drop(estado);
        return pedido.responder_com(539, &motivo);
    }
    let n_prot = pedido.proximo_protocolo();
    estado.notas.por_numero.insert(numero_chave, chave.to_string());
    estado.notas.por_chave.insert(
        chave.to_string(),
        Nota { n_prot: n_prot.clone(), dig_val: dig_val.clone(), dh: dh.to_string(), cancelada: false },
    );
    drop(estado);

    let prot = resposta::prot_nfe(&DadosProtocolo {
        chave,
        dh,
        n_prot: Some(&n_prot),
        dig_val: Some(&dig_val),
        c_stat: 100,
        motivo: texto_de(100),
    });
    let retorno = resposta::ret_lote(104, texto_de(104), &c_uf, dh, Some(&prot));
    Resposta::Http(resposta::soap(Servico::Autorizacao, &retorno))
}

fn texto_de(c_stat: u32) -> &'static str {
    descricao_c_stat(c_stat).unwrap_or("")
}

fn conferir_consulta(pedido: &SandboxRequest, sandbox: &SefazSandbox, dados: &Element) -> Resposta {
    if dados.local_name() != "consSitNFe" || dados.child_text("xServ").as_deref() != Some("CONSULTAR") {
        return pedido.responder(225);
    }
    let Some(chave) = dados.child_text("chNFe").filter(|chave| forma_valida(chave)) else {
        return pedido.responder(225);
    };
    if ChaveAcesso::parse(&chave).is_err() {
        return pedido.responder(236);
    }
    let nota = sandbox.lock().notas.por_chave.get(&chave).cloned();
    let Some(nota) = nota else {
        return pedido.responder(217);
    };
    let c_uf = chave.get(..2).unwrap_or("");
    let (c_stat, texto) = if nota.cancelada { (101, texto_de(101)) } else { (100, texto_de(100)) };
    let prot = resposta::prot_nfe(&DadosProtocolo {
        chave: &chave,
        dh: &nota.dh,
        n_prot: Some(&nota.n_prot),
        dig_val: Some(&nota.dig_val),
        c_stat: 100,
        motivo: texto_de(100),
    });
    let retorno = resposta::ret_consulta(c_stat, texto, c_uf, &pedido.dh_recbto, &chave, Some(&prot));
    Resposta::Http(resposta::soap(Servico::Consulta, &retorno))
}

fn conferir_evento(pedido: &SandboxRequest, sandbox: &SefazSandbox, dados: &Element) -> Resposta {
    if dados.local_name() != "envEvento" || dados.attr("versao") != Some("1.00") {
        return pedido.responder(225);
    }
    if let Some(codigo) = estrutura(dados) {
        return pedido.responder(codigo);
    }
    let eventos: Vec<&Element> = dados.elements().filter(|e| e.local_name() == "evento").collect();
    let [evento] = eventos.as_slice() else {
        return pedido.responder_com(225, "O sandbox só aceita lote com um evento");
    };
    let Some(inf) = evento.child("infEvento") else {
        return pedido.responder(225);
    };
    let campo = |tag: &str| inf.child_text(tag).unwrap_or_default();
    let chave = campo("chNFe");
    let sequencia = campo("nSeqEvento");
    let Some(detalhe) = inf.child("detEvento") else {
        return pedido.responder(225);
    };
    let justificativa = detalhe.child_text("xJust").unwrap_or_default().chars().count();
    if !forma_valida(&chave)
        || campo("tpEvento") != TP_EVENTO_CANCELAMENTO
        || detalhe.child_text("descEvento").as_deref() != Some("Cancelamento")
        || !(15..=255).contains(&justificativa)
    {
        return pedido.responder(225);
    }
    let numero_sequencia: u8 = sequencia.parse().unwrap_or(0);
    let id = format!("ID{TP_EVENTO_CANCELAMENTO}{chave}{numero_sequencia:02}");
    if inf.attr("Id") != Some(id.as_str()) {
        return pedido.responder(225);
    }
    match verify_enveloped(evento, &id) {
        Ok(()) => {}
        Err(FalhaAssinatura::ForaDoPadrao(motivo)) => {
            return pedido.responder_com(298, &format!("{}: {motivo}", texto_de(298)));
        }
        Err(FalhaAssinatura::Difere(motivo)) => {
            return pedido.responder_com(297, &format!("{}: {motivo}", texto_de(297)));
        }
    }

    let protocolo = detalhe.child_text("nProt").unwrap_or_default();
    let mut estado = sandbox.lock();
    let codigo = match estado.notas.por_chave.get_mut(&chave) {
        None => Some(494),
        Some(nota) if nota.n_prot != protocolo => Some(222),
        Some(nota) if nota.cancelada => Some(573),
        Some(nota) => {
            nota.cancelada = true;
            None
        }
    };
    drop(estado);
    if let Some(codigo) = codigo {
        return pedido.responder(codigo);
    }

    let id_lote = dados.child_text("idLote").unwrap_or_default();
    let c_orgao = campo("cOrgao");
    let dh = pedido.dh_recbto.as_str();
    let n_prot = pedido.proximo_protocolo();
    let retorno_evento = resposta::ret_evento(&DadosEvento {
        c_orgao: &c_orgao,
        c_stat: 135,
        motivo: texto_de(135),
        chave: &chave,
        sequencia: &sequencia,
        dh,
        n_prot: Some(&n_prot),
    });
    let retorno = resposta::ret_lote_evento(&id_lote, 128, texto_de(128), &c_orgao, Some(&retorno_evento));
    Resposta::Http(resposta::soap(Servico::Evento, &retorno))
}
