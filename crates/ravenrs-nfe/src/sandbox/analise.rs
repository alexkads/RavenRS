//! The checks themselves, written once. The sandbox answers the first refusal, as the SEFAZ would;
//! the diagnosis lists them all. Rules that need what the server holds live in `conferencia`.

use rust_decimal::Decimal;

use super::autopsia::{self, AutopsiaAssinatura, Causa};
use super::catalogo::{Confianca, Efeito, Fonte, Regra, regra};
use super::certificado_x509::{HashCertificado, InfoCertificado};
use super::certificados::{CN_CURINGA, cita_icp_brasil, eh_do_laboratorio};
use super::mascara::mascarar_em_caminho;
use super::tempo::Instante;
use super::vigencia::em_vigor;
use crate::chave::{ChaveAcesso, cnpj_valido, digito_mod11, forma_valida};
use crate::documento::{TP_EVENTO_CANCELAMENTO, TipoManifestacao};
use crate::modelo::{TEXTO_HOMOLOGACAO_DESTINATARIO, TEXTO_HOMOLOGACAO_ITEM};
use crate::soap::NS_NFE;
use crate::totais::arredondar;
use crate::xml::{self, Element, Node};

/// The namespace of `Signature`.
const NS_DSIG: &str = "http://www.w3.org/2000/09/xmldsig#";
/// The tolerance of rules I11-10 and I11-20: one cent, by note (*4) of Anexo I.
const TOLERANCIA_ITEM: &str = "0.01";
/// The tolerance of rule W16-10: fifty cents, by note (*3) of Anexo I.
const TOLERANCIA_TOTAL: &str = "0.50";
/// The tolerance of date comparisons, in seconds, by rules B09-10, B09-40 and P13-10.
const TOLERANCIA_DATA: i64 = 300;

/// One finding about a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostico {
    /// The rule in the catalog.
    pub regra: &'static str,
    /// The code the server would answer; 0 when nothing reaches the server.
    pub c_stat: u32,
    /// The name of the code.
    pub titulo: String,
    /// Whether it stops the document.
    pub efeito: Efeito,
    /// The path of the field.
    pub caminho: String,
    /// What is there, masked when the field is personal.
    pub encontrado: Option<String>,
    /// What should be there.
    pub esperado: Option<String>,
    /// What goes wrong.
    pub explicacao: String,
    /// What to do.
    pub como_corrigir: String,
    /// Where the rule came from.
    pub fonte: Fonte,
    /// How sure that is.
    pub confianca: Confianca,
}

impl Diagnostico {
    /// Builds a finding from a rule of the catalog. Personal values come out masked.
    pub(crate) fn de(regra: &Regra, caminho: &str, encontrado: Option<&str>, esperado: Option<&str>) -> Self {
        Self {
            regra: regra.id,
            c_stat: regra.c_stat,
            titulo: regra.titulo().to_string(),
            efeito: regra.efeito,
            caminho: caminho.to_string(),
            encontrado: encontrado.map(|valor| mascarar_em_caminho(caminho, valor)),
            esperado: esperado.map(str::to_string),
            explicacao: regra.explicacao.to_string(),
            como_corrigir: regra.como_corrigir.to_string(),
            fonte: regra.fonte,
            confianca: regra.confianca,
        }
    }
}

/// How to judge a message.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Opcoes {
    /// The moment of the receipt. Without it, rules that need a clock are listed as not judged.
    pub agora: Option<Instante>,
    /// The `tpAmb` of the receiver. Without it, rule B24-10 is not judged.
    pub tp_amb_esperado: Option<String>,
    /// The trusted authorities, as `SubjectPublicKeyInfo`. Empty means the chain is not judged.
    pub ancoras: Vec<Vec<u8>>,
    /// The revoked serial numbers, in hexadecimal.
    pub revogados: Vec<String>,
    /// Also reads the signature certificate as the transmitter's, since the sandbox has no TLS.
    pub certificado_transmite: bool,
}

/// A message and everything found in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Analise {
    /// The findings, in the order a server would check.
    pub diagnosticos: Vec<Diagnostico>,
    /// What could not be judged from this message, and why.
    pub nao_avaliado: Vec<String>,
    /// The access key, if the message has one.
    pub chave: Option<String>,
    /// The tag that was taken as the document.
    pub tipo: String,
    /// The signature examined, if the message has one.
    pub autopsia: Option<AutopsiaAssinatura>,
}

impl Analise {
    /// The first refusal, which is what a server would answer.
    #[must_use]
    pub fn primeira_rejeicao(&self) -> Option<&Diagnostico> {
        self.diagnosticos.iter().find(|d| d.efeito == Efeito::Rejeicao)
    }

    fn nova(tipo: &str) -> Self {
        Self {
            diagnosticos: Vec::new(),
            nao_avaliado: Vec::new(),
            chave: None,
            tipo: tipo.to_string(),
            autopsia: None,
        }
    }

    fn achar(&mut self, id: &str, caminho: &str, encontrado: Option<&str>, esperado: Option<&str>) {
        if let Some(regra) = regra(id) {
            self.diagnosticos.push(Diagnostico::de(regra, caminho, encontrado, esperado));
        }
    }

    fn alertar(&mut self, id: &str, caminho: &str, encontrado: Option<&str>, esperado: Option<&str>) {
        if let Some(regra) = regra(id) {
            let mut achado = Diagnostico::de(regra, caminho, encontrado, esperado);
            achado.efeito = Efeito::Alerta;
            self.diagnosticos.push(achado);
        }
    }
}

/// The order of the tags under each parent, from `leiauteNFe_v4.00.xsd`.
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
    ("card", &["tpIntegra", "CNPJ", "tBand", "cAut", "CNPJReceb", "idTermPag"]),
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

/// The tags each group cannot go without, from the same schema.
const OBRIGATORIAS: &[(&str, &[&str])] = &[
    ("infNFe", &["ide", "emit", "det", "total", "transp", "pag"]),
    (
        "ide",
        &[
            "cUF", "cNF", "natOp", "mod", "serie", "nNF", "dhEmi", "tpNF", "idDest", "cMunFG", "tpImp",
            "tpEmis", "cDV", "tpAmb", "finNFe", "indFinal", "indPres", "procEmi", "verProc",
        ],
    ),
    ("emit", &["xNome", "enderEmit", "CRT"]),
    ("det", &["prod", "imposto"]),
    (
        "prod",
        &[
            "cProd", "cEAN", "xProd", "NCM", "CFOP", "uCom", "qCom", "vUnCom", "vProd", "cEANTrib", "uTrib",
            "qTrib", "vUnTrib", "indTot",
        ],
    ),
    ("total", &["ICMSTot"]),
    ("ICMSTot", &["vProd", "vDesc", "vNF"]),
    ("transp", &["modFrete"]),
    ("pag", &["detPag"]),
    ("detPag", &["tPag", "vPag"]),
    ("card", &["tpIntegra"]),
];

/// Walks a tree looking for structure problems, with the path of each one.
fn conferir_estrutura(analise: &mut Analise, element: &Element, caminho: &str) {
    let nome = element.local_name();
    let filhos = element.elements().count();
    if filhos > 0 && element.children.iter().any(|node| matches!(node, Node::Text(_))) {
        analise.achar("D01e", caminho, Some("texto ou espaço entre as tags"), Some("nada entre as tags"));
    }
    if let Some((_, ordem)) = ORDEM.iter().find(|(pai, _)| *pai == nome) {
        let mut ultimo = 0;
        let mut contagem = std::collections::HashMap::new();
        for filho in element.elements() {
            let tag = filho.local_name();
            *contagem.entry(tag).or_insert(0_usize) += 1;
            let caminho_filho = format!("{caminho}/{tag}");
            match ordem.iter().position(|outro| *outro == tag) {
                None => analise.achar("XSD-DESCONHECIDA", &caminho_filho, Some(tag), None),
                Some(posicao) if posicao < ultimo => {
                    analise.achar(
                        "XSD-ORDEM",
                        &caminho_filho,
                        Some(tag),
                        Some(&format!("depois de {tag} não pode vir antes de {}", ordem[ultimo])),
                    );
                }
                Some(posicao) => ultimo = posicao,
            }
        }
    }
    if let Some((_, obrigatorias)) = OBRIGATORIAS.iter().find(|(pai, _)| *pai == nome) {
        for tag in *obrigatorias {
            if element.child(tag).is_none() {
                analise.achar("XSD-OBRIGATORIA", &format!("{caminho}/{tag}"), None, Some(tag));
            }
        }
    }
    if nome == "Signature" {
        return;
    }
    let mut repeticoes: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for filho in element.elements() {
        let tag = filho.local_name();
        let conta = repeticoes.entry(tag).or_insert(0);
        *conta += 1;
        let caminho_filho = if matches!(tag, "det" | "detPag") {
            format!("{caminho}/{tag}[{conta}]")
        } else {
            format!("{caminho}/{tag}")
        };
        conferir_estrutura(analise, filho, &caminho_filho);
    }
}

fn conferir_namespaces(analise: &mut Analise, element: &Element, caminho: &str) {
    for (chave, valor) in &element.attrs {
        if (chave == "xmlns" || chave.starts_with("xmlns:")) && valor != NS_NFE && valor != NS_DSIG {
            analise.achar("D01d", caminho, Some(&format!("{chave}={valor}")), Some(NS_NFE));
        }
    }
    if element.local_name() == "Signature" {
        return;
    }
    for filho in element.elements() {
        conferir_namespaces(analise, filho, &format!("{caminho}/{}", filho.local_name()));
    }
}

fn decimal(element: &Element, tag: &str) -> Option<Decimal> {
    element.child_text(tag)?.parse().ok()
}

fn decimal_ou_zero(element: &Element, tag: &str) -> Decimal {
    decimal(element, tag).unwrap_or(Decimal::ZERO)
}

/// Whether the certificate is the sandbox's one that fits any issuer.
fn eh_curinga(info: &InfoCertificado, der: &[u8]) -> bool {
    info.titular_cn.as_deref() == Some(CN_CURINGA) && eh_do_laboratorio(der)
}

/// Checks the certificate inside a signature: groups A and E of the MOC.
fn conferir_certificado(
    analise: &mut Analise,
    assinatura: &Element,
    caminho: &str,
    momento: Option<Instante>,
    opcoes: &Opcoes,
) {
    let caminho = format!("{caminho}/Signature/KeyInfo/X509Data/X509Certificate");
    let texto: Option<String> = assinatura
        .child("KeyInfo")
        .and_then(|k| k.child("X509Data"))
        .and_then(|x| x.child_text("X509Certificate"))
        .map(|t| t.split_whitespace().collect());
    let der = texto.and_then(|t| {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.decode(t).ok()
    });
    let Some(der) = der else {
        analise.achar("E01", &caminho, Some("ausente ou não é base64"), Some("certificado X.509"));
        return;
    };
    let Some(info) = InfoCertificado::ler(&der) else {
        analise.achar("E01", &caminho, Some("não foi possível ler o certificado"), Some("certificado X.509"));
        return;
    };

    // A real certificate in the laboratory is one refusal, ahead of what else it would fail.
    if !opcoes.ancoras.is_empty()
        && !eh_do_laboratorio(&der)
        && !opcoes.ancoras.iter().any(|spki| info.emitido_por(spki))
    {
        let motivo = if cita_icp_brasil(&der) { "certificado ICP-Brasil" } else { "certificado de fora" };
        analise.achar(
            "SBX-FORA-DO-LABORATORIO",
            &caminho,
            Some(motivo),
            Some("emitido pela AC de teste do sandbox"),
        );
        return;
    }
    if opcoes.certificado_transmite && info.autenticacao_cliente == Some(false) {
        analise.achar(
            "A01",
            &caminho,
            Some("sem Autenticação Cliente"),
            Some("Extended Key Usage com Autenticação Cliente"),
        );
    }
    if !opcoes.certificado_transmite {
        analise.nao_avaliado.push(
            "O certificado de transmissão (grupo A do MOC) não vem no XML e não foi avaliado.".to_string(),
        );
    }
    if info.versao != 3 {
        analise.achar("E01", &caminho, Some(&format!("versão {}", info.versao)), Some("versão 3"));
    }
    if info.eh_ac {
        analise.achar("E01", &caminho, Some("certificado de AC"), Some("certificado do titular"));
    }
    if info.uso_assinatura != Some(true) {
        analise.achar(
            "E01",
            &caminho,
            Some("uso da chave sem Assinatura Digital e Não Recusa"),
            Some("Assinatura Digital e Não Recusa"),
        );
    }
    if info.bits_chave.is_some_and(|bits| bits < 2048) {
        let bits = info.bits_chave.unwrap_or(0).to_string();
        analise.achar("CERT-CHAVE-FRACA", &caminho, Some(&format!("{bits} bits")), Some("2048 bits ou mais"));
    }
    if info.hash == HashCertificado::Sha1 {
        analise.achar("CERT-SHA1", &caminho, Some("sha1WithRSAEncryption"), Some("sha256WithRSAEncryption"));
    }
    match momento {
        Some(momento) => {
            let antes = info.valido_de.is_some_and(|de| momento < de);
            let depois = info.valido_ate.is_some_and(|ate| momento > ate);
            if antes || depois {
                let intervalo = format!(
                    "de {} a {}",
                    info.valido_de.map(Instante::para_xml).unwrap_or_default(),
                    info.valido_ate.map(Instante::para_xml).unwrap_or_default()
                );
                analise.achar(
                    "E02",
                    &caminho,
                    Some(&intervalo),
                    Some(&format!("valer em {}", momento.para_xml())),
                );
            }
        }
        None => analise
            .nao_avaliado
            .push("A validade do certificado não foi avaliada: não há data de referência.".to_string()),
    }
    if info.cnpj.is_none() && info.cpf.is_none() && !eh_curinga(&info, &der) {
        analise.achar(
            "E03",
            &caminho,
            Some("sem otherName de CNPJ ou CPF"),
            Some("OID 2.16.76.1.3.3 ou 2.16.76.1.3.1"),
        );
    }
    if opcoes.ancoras.is_empty() {
        analise
            .nao_avaliado
            .push("A cadeia, a raiz ICP-Brasil e a revogação do certificado não foram avaliadas: precisam da lista da SEFAZ.".to_string());
    } else {
        if !opcoes.ancoras.iter().any(|spki| info.emitido_por(spki)) {
            analise.achar("E04", &caminho, info.emissor_cn.as_deref(), Some("assinado pela AC que diz ser"));
        }
        if opcoes.revogados.iter().any(|serial| serial == info.serial_hex()) {
            analise.achar("E06", &caminho, Some(info.serial_hex()), Some("fora da lista de revogação"));
        }
    }
}

/// Checks a signature: group F of the MOC.
fn conferir_assinatura(
    analise: &mut Analise,
    pai: &Element,
    id: &str,
    caminho: &str,
    cru: Option<&str>,
    opcoes: &Opcoes,
) {
    const C14N: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
    const ENVELOPED: &str = "http://www.w3.org/2000/09/xmldsig#enveloped-signature";
    const RSA_SHA1: &str = "http://www.w3.org/2000/09/xmldsig#rsa-sha1";
    const SHA1: &str = "http://www.w3.org/2000/09/xmldsig#sha1";

    let exame = autopsia::examinar(pai, id, cru, opcoes.agora);
    let causa_texto = exame.causas.first().map(|c| c.explicacao.clone());
    let caminho_assinatura = format!("{caminho}/Signature");
    let mut padrao_ok = true;

    let Some(assinatura) = pai.child("Signature") else {
        analise.achar("F01", &caminho_assinatura, None, Some("Signature depois do documento"));
        analise.autopsia = Some(exame);
        return;
    };
    let sha256 = exame.causas.iter().any(|c| c.causa == Causa::Sha256NoLugarDeSha1);
    if sha256 {
        padrao_ok = false;
        analise.achar(
            "SIG-SHA256",
            &format!("{caminho_assinatura}/SignedInfo"),
            exame.algoritmo_assinatura.as_deref(),
            Some(RSA_SHA1),
        );
    }
    if assinatura.attr("xmlns") != Some(NS_DSIG) {
        padrao_ok = false;
        analise.achar("F01", &caminho_assinatura, assinatura.attr("xmlns"), Some(NS_DSIG));
    }
    let info = assinatura.child("SignedInfo");
    let algoritmo = |grupo: Option<&Element>, tag: &str| {
        grupo.and_then(|g| g.child(tag)).and_then(|t| t.attr("Algorithm")).map(str::to_string)
    };
    let c14n = algoritmo(info, "CanonicalizationMethod");
    if c14n.as_deref() != Some(C14N) {
        padrao_ok = false;
        analise.achar(
            "F01",
            &format!("{caminho_assinatura}/SignedInfo/CanonicalizationMethod"),
            c14n.as_deref(),
            Some(C14N),
        );
    }
    let metodo = algoritmo(info, "SignatureMethod");
    if metodo.as_deref() != Some(RSA_SHA1) && !sha256 {
        padrao_ok = false;
        analise.achar(
            "F01",
            &format!("{caminho_assinatura}/SignedInfo/SignatureMethod"),
            metodo.as_deref(),
            Some(RSA_SHA1),
        );
    }
    let referencia = info.and_then(|i| i.child("Reference"));
    let caminho_referencia = format!("{caminho_assinatura}/SignedInfo/Reference");
    let uri_certa = format!("#{id}");
    if referencia.and_then(|r| r.attr("URI")) != Some(uri_certa.as_str()) {
        padrao_ok = false;
        analise.achar(
            "F01",
            &format!("{caminho_referencia}/@URI"),
            referencia.and_then(|r| r.attr("URI")),
            Some(&uri_certa),
        );
    }
    let transforms: Vec<&str> = referencia
        .and_then(|r| r.child("Transforms"))
        .map(|t| t.elements().filter_map(|e| e.attr("Algorithm")).collect())
        .unwrap_or_default();
    if transforms != [ENVELOPED, C14N] {
        padrao_ok = false;
        analise.achar(
            "F01",
            &format!("{caminho_referencia}/Transforms"),
            Some(&transforms.join(", ")),
            Some(&format!("{ENVELOPED}, {C14N}")),
        );
    }
    let digest = algoritmo(referencia, "DigestMethod");
    if digest.as_deref() != Some(SHA1) && !sha256 {
        padrao_ok = false;
        analise.achar("F01", &format!("{caminho_referencia}/DigestMethod"), digest.as_deref(), Some(SHA1));
    }

    if padrao_ok {
        if !exame.digest_confere {
            let posicao = analise.diagnosticos.len();
            analise.achar(
                "F02-DIGEST",
                &format!("{caminho_referencia}/DigestValue"),
                exame.digest_informado.as_deref(),
                exame.digest_calculado.as_deref(),
            );
            if let (Some(achado), Some(causa)) = (analise.diagnosticos.get_mut(posicao), causa_texto) {
                achado.explicacao = format!("{} Causa provável: {causa}", achado.explicacao);
            }
        } else if exame.assinatura_confere == Some(false) {
            analise.achar(
                "F02",
                &format!("{caminho_assinatura}/SignatureValue"),
                None,
                Some("valor que confere com a chave do certificado"),
            );
        }
    }
    analise.autopsia = Some(exame);
}

/// The holder's certificate against the issuer: rules F03 and F03A, after the signature.
fn conferir_titular(analise: &mut Analise, emitente: Option<&Element>, caminho: &str) {
    let Some(info) = analise.autopsia.as_ref().and_then(|a| a.certificado.clone()) else {
        return;
    };
    let Some(emitente) = emitente else { return };
    if let (Some(cnpj), Some(certificado)) = (emitente.child_text("CNPJ"), &info.cnpj)
        && cnpj.get(..8) != certificado.get(..8)
    {
        analise.achar("F03", &format!("{caminho}/CNPJ"), certificado.get(..8), cnpj.get(..8));
    }
    if let (Some(cpf), Some(certificado)) = (emitente.child_text("CPF"), &info.cpf)
        && cpf != *certificado
    {
        analise.achar("F03A", &format!("{caminho}/CPF"), Some("***"), Some("o CPF do certificado"));
    }
}

fn eh_zero(texto: &str) -> bool {
    texto.parse::<Decimal>().is_ok_and(|valor| valor.is_zero())
}

fn conferir_ide(analise: &mut Analise, inf: &Element, chave: &str, opcoes: &Opcoes) {
    const IDE: &str = "/NFe/infNFe/ide";
    let Some(ide) = inf.child("ide") else { return };
    let texto = |tag: &str| ide.child_text(tag).unwrap_or_default();
    let modelo = texto("mod");
    let emit = inf.child("emit");
    let cnpj = emit.and_then(|e| e.child_text("CNPJ")).unwrap_or_default();

    // A03-10.
    let dh = texto("dhEmi");
    let serie: u32 = texto("serie").parse().unwrap_or(0);
    let numero: u32 = texto("nNF").parse().unwrap_or(0);
    let montada = format!(
        "{}{}{}{}{}{:03}{:09}{}{}{}",
        texto("cUF"),
        dh.get(2..4).unwrap_or(""),
        dh.get(5..7).unwrap_or(""),
        cnpj,
        modelo,
        serie,
        numero,
        texto("tpEmis"),
        texto("cNF"),
        texto("cDV"),
    );
    if montada != chave {
        analise.achar("A03-10", "/NFe/infNFe/@Id", Some(chave), Some(&montada));
    }
    // B03-10.
    let c_nf = texto("cNF");
    const PROIBIDOS: [&str; 20] = [
        "00000000", "11111111", "22222222", "33333333", "44444444", "55555555", "66666666", "77777777",
        "88888888", "99999999", "12345678", "23456789", "34567890", "45678901", "56789012", "67890123",
        "78901234", "89012345", "90123456", "01234567",
    ];
    if PROIBIDOS.contains(&c_nf.as_str()) || c_nf.parse::<u32>().ok() == Some(numero) {
        analise.achar(
            "B03-10",
            &format!("{IDE}/cNF"),
            Some(&c_nf),
            Some("fora da lista, e diferente de nNF"),
        );
    }
    // B09-10 and B09-40.
    let emissao = Instante::de_xml(&dh);
    if let (Some(agora), Some(emissao)) = (opcoes.agora, emissao) {
        if emissao.0 > agora.0 + TOLERANCIA_DATA {
            analise.achar(
                "B09-10",
                &format!("{IDE}/dhEmi"),
                Some(&dh),
                Some(&format!("até 5 minutos depois de {}", agora.para_xml())),
            );
        }
        if modelo == "65" && texto("tpEmis") == "1" && agora.0 > emissao.0 + TOLERANCIA_DATA {
            analise.achar(
                "B09-40",
                &format!("{IDE}/dhEmi"),
                Some(&dh),
                Some(&format!("até 5 minutos antes de {}", agora.para_xml())),
            );
        }
    } else if opcoes.agora.is_none() {
        analise.nao_avaliado.push(
            "O atraso da emissão (703 e 704) não foi avaliado: não há hora de recebimento.".to_string(),
        );
    }
    // Rules of the NFC-e.
    if modelo == "65" {
        if texto("idDest") != "1" {
            analise.achar("B11a-10", &format!("{IDE}/idDest"), Some(&texto("idDest")), Some("1"));
        }
        if !matches!(texto("tpImp").as_str(), "4" | "5") {
            analise.achar("B21-10", &format!("{IDE}/tpImp"), Some(&texto("tpImp")), Some("4 ou 5"));
        }
        if texto("finNFe") != "1" {
            analise.achar("B25-20", &format!("{IDE}/finNFe"), Some(&texto("finNFe")), Some("1"));
        }
        if texto("indFinal") == "0" {
            analise.achar("B25a-10", &format!("{IDE}/indFinal"), Some("0"), Some("1"));
        }
        if !matches!(texto("indPres").as_str(), "1" | "4") {
            analise.achar("B25b-20", &format!("{IDE}/indPres"), Some(&texto("indPres")), Some("1 ou 4"));
        }
    } else if matches!(texto("tpImp").as_str(), "4" | "5") {
        analise.achar("B21-20", &format!("{IDE}/tpImp"), Some(&texto("tpImp")), Some("0, 1, 2 ou 3"));
    }
    // B12-20.
    if texto("cMunFG").get(..2) != texto("cUF").get(..2) {
        analise.achar(
            "B12-20",
            &format!("{IDE}/cMunFG"),
            Some(&texto("cMunFG")),
            Some(&format!("começar com {}", texto("cUF"))),
        );
    }
    // B23-10.
    if forma_valida(chave) && chave.as_bytes()[43].wrapping_sub(b'0') != digito_mod11(&chave[..43]) {
        analise.achar(
            "B23-10",
            &format!("{IDE}/cDV"),
            chave.get(43..),
            Some(&digito_mod11(&chave[..43]).to_string()),
        );
    }
    // B24-10.
    if let Some(esperado) = &opcoes.tp_amb_esperado {
        if texto("tpAmb") != *esperado {
            analise.achar("B24-10", &format!("{IDE}/tpAmb"), Some(&texto("tpAmb")), Some(esperado));
        }
    } else {
        analise
            .nao_avaliado
            .push("O ambiente (252) não foi avaliado: não se sabe quem recebeu a mensagem.".to_string());
    }
    // C02-10.
    if !cnpj.is_empty() && !cnpj_valido(&cnpj) {
        analise.achar(
            "C02-10",
            "/NFe/infNFe/emit/CNPJ",
            Some(&cnpj),
            Some("CNPJ com dígito verificador válido"),
        );
    }
}

fn conferir_dest_e_itens(analise: &mut Analise, inf: &Element, homologacao: bool, modelo: &str) {
    let dest = inf.child("dest");
    match dest {
        None if modelo == "55" => analise.achar("E01-10", "/NFe/infNFe/dest", None, Some("destinatário")),
        None => {}
        Some(dest) => {
            let nome = dest.child_text("xNome");
            match &nome {
                None if modelo == "55" => {
                    analise.achar("E04-10", "/NFe/infNFe/dest/xNome", None, Some("nome"));
                }
                Some(nome) if homologacao && nome != TEXTO_HOMOLOGACAO_DESTINATARIO => {
                    analise.achar(
                        "E04-20",
                        "/NFe/infNFe/dest/xNome",
                        Some(nome),
                        Some(TEXTO_HOMOLOGACAO_DESTINATARIO),
                    );
                }
                _ => {}
            }
            if modelo == "55" && dest.child("enderDest").is_none() {
                analise.achar("E05-10", "/NFe/infNFe/dest/enderDest", None, Some("endereço"));
            }
            if modelo == "65" && dest.child_text("indIEDest").as_deref() != Some("9") {
                analise.achar(
                    "E16a-10",
                    "/NFe/infNFe/dest/indIEDest",
                    dest.child_text("indIEDest").as_deref(),
                    Some("9"),
                );
            }
        }
    }

    const CFOP_NFCE: [&str; 9] = ["5101", "5102", "5103", "5104", "5115", "5405", "5656", "5667", "5933"];
    let tolerancia: Decimal = TOLERANCIA_ITEM.parse().unwrap_or(Decimal::ZERO);
    let itens = inf.elements().filter(|e| e.local_name() == "det");
    for (posicao, det) in itens.enumerate() {
        let caminho = format!("/NFe/infNFe/det[{}]/prod", posicao + 1);
        let Some(prod) = det.child("prod") else { continue };
        if posicao == 0 && modelo == "65" && homologacao {
            let descricao = prod.child_text("xProd");
            if descricao.as_deref() != Some(TEXTO_HOMOLOGACAO_ITEM) {
                analise.achar(
                    "I04-10",
                    &format!("{caminho}/xProd"),
                    descricao.as_deref(),
                    Some(TEXTO_HOMOLOGACAO_ITEM),
                );
            }
        }
        if modelo == "65" {
            let cfop = prod.child_text("CFOP").unwrap_or_default();
            if !CFOP_NFCE.contains(&cfop.as_str()) {
                analise.achar(
                    "I08-150",
                    &format!("{caminho}/CFOP"),
                    Some(&cfop),
                    Some(&CFOP_NFCE.join(", ")),
                );
            }
        }
        for (id, quantidade, unitario) in [("I11-10", "qCom", "vUnCom"), ("I11-20", "qTrib", "vUnTrib")] {
            let (Some(q), Some(u), Some(valor)) =
                (decimal(prod, quantidade), decimal(prod, unitario), decimal(prod, "vProd"))
            else {
                continue;
            };
            let Some(calculado) = q.checked_mul(u) else { continue };
            let calculado = arredondar(calculado);
            if (calculado - valor).abs() > tolerancia {
                analise.achar(
                    id,
                    &format!("{caminho}/vProd"),
                    Some(&valor.to_string()),
                    Some(&calculado.to_string()),
                );
            }
        }
        let tem_500 = det.find("ICMSSN500").is_some();
        if modelo == "55" && tem_500 && prod.child("CEST").is_none() {
            analise.achar("N23-10", &format!("{caminho}/CEST"), None, Some("CEST do produto"));
        }
    }
}

fn conferir_ibscbs(analise: &mut Analise, inf: &Element, homologacao: bool) {
    let crt = inf.child("emit").and_then(|emit| emit.child_text("CRT"));
    if crt.as_deref() != Some("3") {
        return;
    }
    let emissao = inf.child("ide").and_then(|i| i.child_text("dhEmi")).and_then(|d| Instante::de_xml(&d));
    if !emissao.is_some_and(|momento| em_vigor("IBSCBS-1115", homologacao, momento)) {
        return;
    }
    let itens = inf.elements().filter(|e| e.local_name() == "det");
    for (posicao, det) in itens.enumerate() {
        if det.child("imposto").and_then(|i| i.child("IBSCBS")).is_none() {
            analise.achar(
                "UB12-10",
                &format!("/NFe/infNFe/det[{}]/imposto/IBSCBS", posicao + 1),
                None,
                Some("grupo IBSCBS"),
            );
        }
    }
}

fn conferir_totais(analise: &mut Analise, inf: &Element) {
    const TOTAL: &str = "/NFe/infNFe/total/ICMSTot";
    let Some(totais) = inf.child("total").and_then(|t| t.child("ICMSTot")) else { return };
    let mut soma_prod = Decimal::ZERO;
    let mut soma_desc = Decimal::ZERO;
    for det in inf.elements().filter(|e| e.local_name() == "det") {
        let Some(prod) = det.child("prod") else { continue };
        if prod.child_text("indTot").as_deref() != Some("0") {
            soma_prod += decimal_ou_zero(prod, "vProd");
        }
        soma_desc += decimal_ou_zero(prod, "vDesc");
    }
    let v_prod = decimal_ou_zero(totais, "vProd");
    if soma_prod != v_prod {
        analise.achar(
            "W07-10",
            &format!("{TOTAL}/vProd"),
            Some(&v_prod.to_string()),
            Some(&soma_prod.to_string()),
        );
    }
    let v_desc = decimal_ou_zero(totais, "vDesc");
    if soma_desc != v_desc {
        analise.achar(
            "W10-10",
            &format!("{TOTAL}/vDesc"),
            Some(&v_desc.to_string()),
            Some(&soma_desc.to_string()),
        );
    }
    let campo = |tag: &str| decimal_ou_zero(totais, tag);
    let v_serv = inf
        .child("total")
        .and_then(|t| t.child("ISSQNtot"))
        .map_or(Decimal::ZERO, |grupo| decimal_ou_zero(grupo, "vServ"));
    let calculado = v_prod - v_desc - campo("vICMSDeson")
        + campo("vST")
        + campo("vFCPST")
        + campo("vFrete")
        + campo("vSeg")
        + campo("vOutro")
        + campo("vII")
        + campo("vIPI")
        + campo("vIPIDevol")
        + v_serv;
    let v_nf = campo("vNF");
    let tolerancia: Decimal = TOLERANCIA_TOTAL.parse().unwrap_or(Decimal::ZERO);
    if (calculado - v_nf).abs() > tolerancia {
        analise.achar(
            "W16-10",
            &format!("{TOTAL}/vNF"),
            Some(&v_nf.to_string()),
            Some(&calculado.to_string()),
        );
    }
}

/// The payments, with the `card` group.
fn conferir_pagamentos(analise: &mut Analise, inf: &Element, modelo: &str) {
    const PAG: &str = "/NFe/infNFe/pag";
    const ACEITAM_CARD: [&str; 9] = ["03", "04", "10", "11", "12", "13", "15", "17", "18"];
    let Some(pag) = inf.child("pag") else { return };
    let v_nf = inf
        .child("total")
        .and_then(|t| t.child("ICMSTot"))
        .map_or(Decimal::ZERO, |grupo| decimal_ou_zero(grupo, "vNF"));
    let c_uf = inf.child("ide").and_then(|i| i.child_text("cUF")).unwrap_or_default();
    let mut pago = Decimal::ZERO;
    let mut sem_pagamento = false;
    for (posicao, det) in pag.elements().filter(|e| e.local_name() == "detPag").enumerate() {
        let caminho = format!("{PAG}/detPag[{}]", posicao + 1);
        let t_pag = det.child_text("tPag").unwrap_or_default();
        let valor = det.child_text("vPag").unwrap_or_default();
        pago += valor.parse().unwrap_or(Decimal::ZERO);
        if t_pag == "90" {
            sem_pagamento = true;
            if modelo == "65" {
                analise.achar(
                    "YA02-40",
                    &format!("{caminho}/tPag"),
                    Some("90"),
                    Some("a forma de pagamento real"),
                );
            }
            if !eh_zero(&valor) {
                analise.achar("YA03-30", &format!("{caminho}/vPag"), Some(&valor), Some("0.00"));
            }
        }
        match det.child("card") {
            None if matches!(t_pag.as_str(), "03" | "04" | "17") => {
                if modelo == "65" {
                    analise.achar("YA04-10", &format!("{caminho}/card"), None, Some("grupo card"));
                } else {
                    // Announced for the future in model 55 by NT 2025.001.
                    analise.alertar(
                        "YA04-10",
                        &format!("{caminho}/card"),
                        None,
                        Some("grupo card, em implementação futura no modelo 55"),
                    );
                }
            }
            None => {}
            Some(card) => {
                let caminho_card = format!("{caminho}/card");
                if !ACEITAM_CARD.contains(&t_pag.as_str()) {
                    analise.achar("YA04-20", &caminho_card, Some(&t_pag), Some(&ACEITAM_CARD.join(", ")));
                }
                let cnpj = card.child_text("CNPJ");
                let autorizacao = card.child_text("cAut");
                let integrado = card.child_text("tpIntegra").as_deref() == Some("1");
                if integrado && (cnpj.is_none() || autorizacao.is_none()) {
                    analise.achar("YA05-10", &caminho_card, None, Some("CNPJ e cAut"));
                }
                if let Some(cnpj) = &cnpj
                    && !cnpj_valido(cnpj)
                {
                    analise.achar(
                        "YA05-20",
                        &format!("{caminho_card}/CNPJ"),
                        Some(cnpj),
                        Some("CNPJ com dígito verificador válido"),
                    );
                }
                if let Some(cnpj) = card.child_text("CNPJReceb")
                    && !cnpj_valido(&cnpj)
                {
                    analise.achar(
                        "YA07a",
                        &format!("{caminho_card}/CNPJReceb"),
                        Some(&cnpj),
                        Some("CNPJ com dígito verificador válido"),
                    );
                }
                if c_uf == "43" && integrado && autorizacao.is_some() {
                    analise.achar(
                        "SBX-RS-CAUT",
                        &format!("{caminho_card}/cAut"),
                        autorizacao.as_deref(),
                        Some("igual ao comprovante do TEF"),
                    );
                }
            }
        }
    }
    let troco = pag.child_text("vTroco");
    if modelo == "65" && pago < v_nf && !sem_pagamento {
        analise.achar(
            "YA03-10",
            &format!("{PAG}/detPag/vPag"),
            Some(&pago.to_string()),
            Some(&v_nf.to_string()),
        );
    }
    match &troco {
        None if pago > v_nf => {
            analise.achar("YA03-20", &format!("{PAG}/vTroco"), None, Some(&(pago - v_nf).to_string()));
        }
        Some(texto) => {
            let informado: Decimal = texto.parse().unwrap_or(Decimal::ZERO);
            if informado != pago - v_nf {
                analise.achar(
                    "YA09-10",
                    &format!("{PAG}/vTroco"),
                    Some(texto),
                    Some(&(pago - v_nf).to_string()),
                );
            }
        }
        None => {}
    }
}

fn conferir_qr(analise: &mut Analise, nfe: &Element, chave: &str, modelo: &str, tp_amb: &str) {
    if modelo != "65" {
        return;
    }
    const QR: &str = "/NFe/infNFeSupl/qrCode";
    let Some(texto) = nfe.child("infNFeSupl").and_then(|s| s.child_text("qrCode")) else {
        analise.achar("ZX02-10", QR, None, Some("infNFeSupl/qrCode"));
        return;
    };
    let Some(parametros) = texto.split("?p=").nth(1) else {
        analise.nao_avaliado.push("O QR Code não usa o parâmetro p=, então não foi conferido.".to_string());
        return;
    };
    let partes: Vec<&str> = parametros.split('|').collect();
    if partes.first() != Some(&chave) {
        analise.achar("ZX02-228", QR, partes.first().copied(), Some(chave));
    }
    if !matches!(partes.get(1).copied(), Some("2" | "3")) {
        analise.achar("ZX02-236", QR, partes.get(1).copied(), Some("2 ou 3"));
    }
    if partes.get(2).copied() != Some(tp_amb) {
        analise.achar("ZX02-244", QR, partes.get(2).copied(), Some(tp_amb));
    }
}

/// Judges one `NFe`. `cru` is its text as received, when known.
pub(crate) fn analisar_nfe(nfe: &Element, cru: Option<&str>, opcoes: &Opcoes) -> Analise {
    let mut analise = Analise::nova("NFe");
    conferir_estrutura(&mut analise, nfe, "/NFe");
    conferir_namespaces(&mut analise, nfe, "/NFe");
    let Some(inf) = nfe.child("infNFe") else {
        return analise;
    };
    if inf.attr("versao") != Some("4.00") {
        analise.achar("B06", "/NFe/infNFe/@versao", inf.attr("versao"), Some("4.00"));
    }
    let id = inf.attr("Id").unwrap_or_default().to_string();
    let chave = id.strip_prefix("NFe").unwrap_or_default().to_string();
    if !forma_valida(&chave) {
        analise.achar(
            "XSD-VALOR",
            "/NFe/infNFe/@Id",
            Some(&id),
            Some("NFe seguido dos 44 caracteres da chave"),
        );
    } else {
        analise.chave = Some(chave.clone());
    }
    let dh = inf.child("ide").and_then(|i| i.child_text("dhEmi"));
    let momento = opcoes.agora.or_else(|| dh.as_deref().and_then(Instante::de_xml));
    if let Some(assinatura) = nfe.child("Signature") {
        conferir_certificado(&mut analise, assinatura, "/NFe", momento, opcoes);
    }
    conferir_assinatura(&mut analise, nfe, &id, "/NFe", cru, opcoes);
    conferir_titular(&mut analise, inf.child("emit"), "/NFe/infNFe/emit");

    if forma_valida(&chave) {
        conferir_ide(&mut analise, inf, &chave, opcoes);
    }
    let ide = inf.child("ide");
    let modelo = ide.and_then(|i| i.child_text("mod")).unwrap_or_default();
    let tp_amb = ide.and_then(|i| i.child_text("tpAmb")).unwrap_or_default();
    let homologacao = tp_amb == "2";
    conferir_dest_e_itens(&mut analise, inf, homologacao, &modelo);
    conferir_ibscbs(&mut analise, inf, homologacao);
    conferir_totais(&mut analise, inf);
    conferir_pagamentos(&mut analise, inf, &modelo);
    conferir_qr(&mut analise, nfe, &chave, &modelo, &tp_amb);
    analise.nao_avaliado.push(
        "Duplicidade de chave e de número (204 e 539) e cadastro do emitente (203, 209, 245) dependem do que a SEFAZ guarda."
            .to_string(),
    );
    analise
}

/// Judges a batch and the document inside it.
pub(crate) fn analisar_lote(lote: &Element, cru: Option<&str>, opcoes: &Opcoes) -> Analise {
    let mut prefixo = Analise::nova("enviNFe");
    if lote.local_name() != "enviNFe" {
        prefixo.achar("D01a-LOTE", "/", Some(lote.local_name()), Some("enviNFe"));
        return prefixo;
    }
    match lote.attr("versao") {
        None => prefixo.achar("D01b", "/enviNFe/@versao", None, Some("4.00")),
        Some("4.00") => {}
        Some(outra) => prefixo.achar("B06", "/enviNFe/@versao", Some(outra), Some("4.00")),
    }
    if lote.attr("xmlns") != Some(NS_NFE) {
        prefixo.achar("D01-LOTE", "/enviNFe/@xmlns", lote.attr("xmlns"), Some(NS_NFE));
    }
    conferir_cabecalho_lote(&mut prefixo, lote);
    let id_lote = lote.child_text("idLote").unwrap_or_default();
    if id_lote.is_empty() || id_lote.len() > 15 || !id_lote.bytes().all(|b| b.is_ascii_digit()) {
        prefixo.achar("XSD-VALOR", "/enviNFe/idLote", Some(&id_lote), Some("de 1 a 15 dígitos"));
    }
    match lote.child_text("indSinc").as_deref() {
        Some("1") => {}
        Some("0") => {
            let extinto = opcoes.agora.is_some_and(|agora| {
                em_vigor("ASSINCRONO-EXTINTO", opcoes.tp_amb_esperado.as_deref() != Some("1"), agora)
            });
            let id = if extinto { "GAP03a-6" } else { "GAP03a-3" };
            prefixo.achar(id, "/enviNFe/indSinc", Some("0"), Some("1"));
        }
        outro => prefixo.achar("XSD-VALOR", "/enviNFe/indSinc", outro, Some("1")),
    }
    let notas: Vec<&Element> = lote.elements().filter(|e| e.local_name() == "NFe").collect();
    let [nfe] = notas.as_slice() else {
        let modelo_65 = notas.iter().any(|n| n.find_text("mod").as_deref() == Some("65"));
        if notas.len() > 1 && modelo_65 {
            prefixo.achar("GAP03a-4", "/enviNFe/NFe", Some(&notas.len().to_string()), Some("1"));
        } else {
            prefixo.achar(
                "D01-LOTE",
                "/enviNFe/NFe",
                Some(&notas.len().to_string()),
                Some("uma nota por lote neste sandbox"),
            );
        }
        return prefixo;
    };
    let mut analise = analisar_nfe(nfe, cru, opcoes);
    prefixo.diagnosticos.append(&mut analise.diagnosticos);
    analise.diagnosticos = prefixo.diagnosticos;
    analise.tipo = "enviNFe".to_string();
    analise
}

fn conferir_cabecalho_lote(analise: &mut Analise, lote: &Element) {
    for filho in lote.children.iter() {
        if let Node::Text(_) = filho {
            analise.achar(
                "D01e",
                "/enviNFe",
                Some("texto ou espaço entre as tags"),
                Some("nada entre as tags"),
            );
            break;
        }
    }
    let ordem = ["idLote", "indSinc", "NFe"];
    let mut ultimo = 0;
    for filho in lote.elements() {
        match ordem.iter().position(|tag| *tag == filho.local_name()) {
            None => {
                analise.achar("XSD-DESCONHECIDA", &format!("/enviNFe/{}", filho.local_name()), None, None)
            }
            Some(posicao) if posicao < ultimo => {
                analise.achar("XSD-ORDEM", &format!("/enviNFe/{}", filho.local_name()), None, None);
            }
            Some(posicao) => ultimo = posicao,
        }
    }
}

/// Judges a cancellation event.
pub(crate) fn analisar_evento(evento: &Element, cru: Option<&str>, opcoes: &Opcoes) -> Analise {
    const INF: &str = "/evento/infEvento";
    let mut analise = Analise::nova("evento");
    conferir_estrutura(&mut analise, evento, "/evento");
    let Some(inf) = evento.child("infEvento") else {
        analise.achar("D01", INF, None, Some("infEvento"));
        return analise;
    };
    let campo = |tag: &str| inf.child_text(tag).unwrap_or_default();
    let chave = campo("chNFe");
    let tp_evento = campo("tpEvento");
    let cancelamento = tp_evento == TP_EVENTO_CANCELAMENTO;
    let manifestacao = TipoManifestacao::de_codigo(&tp_evento);
    if !cancelamento && manifestacao.is_none() {
        analise.achar(
            "D04",
            &format!("{INF}/tpEvento"),
            Some(&tp_evento),
            Some("110111, 210200, 210210, 210220 ou 210240"),
        );
    }
    if campo("verEvento") != "1.00" {
        analise.achar("D05", &format!("{INF}/verEvento"), Some(&campo("verEvento")), Some("1.00"));
    }
    let detalhe = inf.child("detEvento");
    let justificativa = detalhe.and_then(|d| d.child_text("xJust")).unwrap_or_default().chars().count();
    let descricao = detalhe.and_then(|d| d.child_text("descEvento"));
    if cancelamento {
        if descricao.as_deref() != Some("Cancelamento") || !(15..=255).contains(&justificativa) {
            analise.achar(
                "D06",
                &format!("{INF}/detEvento"),
                Some(&format!("xJust com {justificativa} caracteres")),
                Some("descEvento=Cancelamento e xJust de 15 a 255 caracteres"),
            );
        }
    } else if let Some(tipo) = manifestacao {
        if descricao.as_deref() != Some(tipo.descricao()) {
            analise.achar(
                "D06",
                &format!("{INF}/detEvento/descEvento"),
                descricao.as_deref(),
                Some(tipo.descricao()),
            );
        }
        if tipo == TipoManifestacao::NaoRealizada && !(15..=255).contains(&justificativa) {
            analise.achar(
                "MANIF-595",
                &format!("{INF}/detEvento/xJust"),
                None,
                Some("de 15 a 255 caracteres"),
            );
        }
    }
    if !forma_valida(&chave) {
        analise.achar("D01", &format!("{INF}/chNFe"), Some(&chave), Some("44 caracteres"));
    } else {
        analise.chave = Some(chave.clone());
        if ChaveAcesso::parse(&chave).is_err() {
            analise.achar("P-DV", &format!("{INF}/chNFe"), Some(&chave), None);
        }
    }
    let sequencia = campo("nSeqEvento");
    let numero: u8 = sequencia.parse().unwrap_or(0);
    let id = format!("ID{tp_evento}{chave}{numero:02}");
    if inf.attr("Id") != Some(id.as_str()) {
        analise.achar("D01", &format!("{INF}/@Id"), inf.attr("Id"), Some(&id));
    }
    let dh = campo("dhEvento");
    let momento = opcoes.agora.or_else(|| Instante::de_xml(&dh));
    if let Some(assinatura) = evento.child("Signature") {
        conferir_certificado(&mut analise, assinatura, "/evento", momento, opcoes);
    }
    let id_real = inf.attr("Id").unwrap_or_default().to_string();
    conferir_assinatura(&mut analise, evento, &id_real, "/evento", cru, opcoes);
    conferir_titular(&mut analise, Some(inf), INF);
    if let Some(esperado) = &opcoes.tp_amb_esperado
        && campo("tpAmb") != *esperado
    {
        analise.achar("B24-10", &format!("{INF}/tpAmb"), Some(&campo("tpAmb")), Some(esperado));
    }
    if cancelamento && chave.get(6..20) != Some(campo("CNPJ").as_str()) {
        analise.achar("P12-44", &format!("{INF}/CNPJ"), Some(&campo("CNPJ")), chave.get(6..20));
    }
    if let (Some(agora), Some(evento_em)) = (opcoes.agora, Instante::de_xml(&dh))
        && evento_em.0 > agora.0 + TOLERANCIA_DATA
    {
        analise.achar(
            "P13-10",
            &format!("{INF}/dhEvento"),
            Some(&dh),
            Some(&format!("até 5 minutos depois de {}", agora.para_xml())),
        );
    }
    if numero != 1 {
        analise.achar("P15-10", &format!("{INF}/nSeqEvento"), Some(&sequencia), Some("1"));
    }
    analise.nao_avaliado.push(
        "Existência, situação e prazo da nota (494, 501, 580, 222, 573) dependem do que a SEFAZ guarda."
            .to_string(),
    );
    analise
}

/// Judges a protocol query.
pub(crate) fn analisar_consulta(mensagem: &Element, opcoes: &Opcoes) -> Analise {
    let mut analise = Analise::nova("consSitNFe");
    if mensagem.local_name() != "consSitNFe" {
        analise.achar("D01a", "/", Some(mensagem.local_name()), Some("consSitNFe"));
        return analise;
    }
    if mensagem.child_text("xServ").as_deref() != Some("CONSULTAR") {
        analise.achar("D01", "/consSitNFe/xServ", mensagem.child_text("xServ").as_deref(), Some("CONSULTAR"));
    }
    if let Some(esperado) = &opcoes.tp_amb_esperado
        && mensagem.child_text("tpAmb").as_deref() != Some(esperado)
    {
        analise.achar("J01", "/consSitNFe/tpAmb", mensagem.child_text("tpAmb").as_deref(), Some(esperado));
    }
    let chave = mensagem.child_text("chNFe").unwrap_or_default();
    if !forma_valida(&chave) {
        analise.achar("D01", "/consSitNFe/chNFe", Some(&chave), Some("44 caracteres"));
    } else {
        analise.chave = Some(chave.clone());
        if ChaveAcesso::parse(&chave).is_err() {
            analise.achar("J02a", "/consSitNFe/chNFe", Some(&chave), None);
        }
    }
    analise
}

/// Judges a status query.
pub(crate) fn analisar_status(mensagem: &Element, opcoes: &Opcoes) -> Analise {
    let mut analise = Analise::nova("consStatServ");
    if mensagem.local_name() != "consStatServ" {
        analise.achar("D01a", "/", Some(mensagem.local_name()), Some("consStatServ"));
        return analise;
    }
    if mensagem.child_text("xServ").as_deref() != Some("STATUS") || mensagem.child("cUF").is_none() {
        analise.achar("D01", "/consStatServ", None, Some("cUF e xServ=STATUS"));
    }
    if let Some(esperado) = &opcoes.tp_amb_esperado
        && mensagem.child_text("tpAmb").as_deref() != Some(esperado)
    {
        analise.achar(
            "B24-10",
            "/consStatServ/tpAmb",
            mensagem.child_text("tpAmb").as_deref(),
            Some(esperado),
        );
    }
    analise
}

/// Judges any message: `NFe`, `nfeProc`, `enviNFe`, `evento`, the queries, or any of them in SOAP.
pub fn analisar(xml_texto: &str, opcoes: &Opcoes) -> Analise {
    let raiz = match xml::parse(xml_texto) {
        Ok(raiz) => raiz,
        Err(erro) => {
            let mut analise = Analise::nova("ilegível");
            analise.achar("B02", "/", Some(&erro.to_string()), Some("XML bem formado"));
            return analise;
        }
    };
    let mut analise = if let Some(lote) = raiz.find("enviNFe") {
        // Comes before NFe, which is inside it.
        analisar_lote(lote, Some(xml_texto), opcoes)
    } else if let Some(nfe) = raiz.find("NFe") {
        analisar_nfe(nfe, Some(xml_texto), opcoes)
    } else if let Some(evento) = raiz.find("evento") {
        analisar_evento(evento, Some(xml_texto), opcoes)
    } else if let Some(consulta) = raiz.find("consSitNFe") {
        analisar_consulta(consulta, opcoes)
    } else if let Some(status) = raiz.find("consStatServ") {
        analisar_status(status, opcoes)
    } else {
        let mut analise = Analise::nova(raiz.local_name());
        analise.achar("D01a", "/", Some(raiz.local_name()), Some("NFe, enviNFe, evento ou uma consulta"));
        analise
    };
    if xml_texto.starts_with('\u{feff}')
        && let Some(regra) = regra("D01e")
    {
        let achado =
            Diagnostico::de(regra, "/", Some("BOM no início do texto"), Some("texto começando em <"));
        analise.diagnosticos.insert(0, achado);
    }
    analise
}

/// Lists everything wrong with a message, with no network and without stopping at the first.
#[must_use]
pub fn diagnosticar(xml_texto: &str) -> Vec<Diagnostico> {
    analisar(xml_texto, &Opcoes::default()).diagnosticos
}
