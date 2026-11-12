//! Says why a signature does not match, as far as the message itself can prove.
//!
//! A digest cannot be undone, so the original is not recoverable. What can be done is to rebuild
//! the versions a known mistake would have signed and see whether one of them gives the digest
//! informed. When one does, the cause is proved and the first differing byte is known; when none
//! does, the answer says so and offers only signs.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rsa::pkcs8::DecodePublicKey;
use rsa::sha2::Sha256;
use rsa::{Pkcs1v15Sign, RsaPublicKey};
use sha1::{Digest, Sha1};

use super::catalogo::Fonte;
use super::certificado_x509::InfoCertificado;
use super::tempo::Instante;
use crate::c14n::{canonicalize_by_id, canonicalize_by_name};
use crate::error::NfeError;
use crate::xml::{self, Element, Node};

const RSA_SHA256: &str = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256";
const SHA256_URI: &str = "http://www.w3.org/2001/04/xmlenc#sha256";
const RSA_SHA1: &str = "http://www.w3.org/2000/09/xmldsig#rsa-sha1";

/// Namespaces that serializers and envelopes bring along, tried when looking for what was signed.
const NAMESPACES_COMUNS: [(&str, &str); 5] = [
    ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
    ("xmlns:xsd", "http://www.w3.org/2001/XMLSchema"),
    ("xmlns:soap", "http://www.w3.org/2003/05/soap-envelope"),
    ("xmlns:soap12", "http://www.w3.org/2003/05/soap-envelope"),
    ("xmlns:soapenv", "http://schemas.xmlsoap.org/soap/envelope/"),
];

/// A probable cause of a signature failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Causa {
    /// `infNFe` was signed without the namespace it inherits from `NFe`.
    XmlnsAusente,
    /// A namespace entered the canonical form on one side only.
    XmlnsAMais,
    /// Spaces or line breaks in values or between a few tags.
    EspacosEQuebras,
    /// A byte order mark.
    Bom,
    /// The XML was pretty-printed after signing.
    Reformatado,
    /// The text was signed without canonicalizing, and the attribute order gave it away.
    OrdemDeAtributos,
    /// The text was signed without canonicalizing, though the attributes are in order.
    NaoCanonizado,
    /// `Reference URI` does not point at the `Id` of the document.
    ReferenceUriDiferente,
    /// The certificate was out of its validity.
    CertificadoExpirado,
    /// The certificate holds another CNPJ.
    CertificadoDeOutroCnpj,
    /// SHA-256 where the layout asks for SHA-1.
    Sha256NoLugarDeSha1,
    /// The content really differs from what was signed, by an unknown change.
    ConteudoAlterado,
    /// The digest is right, but the signature does not match this certificate's key.
    ChaveDiferenteDoCertificado,
    /// The document has no `Signature`.
    SemAssinatura,
}

/// How strong the evidence of a cause is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Evidencia {
    /// Rebuilding this mistake gives the digest informed, or it is a plain fact of the message.
    Comprovada,
    /// It fits what is seen, but nothing proves it.
    Indicio,
}

/// A cause with its evidence, in pt-BR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CausaProvavel {
    /// The cause.
    pub causa: Causa,
    /// How strong it is.
    pub evidencia: Evidencia,
    /// What was seen.
    pub explicacao: String,
}

/// The first place two canonical forms differ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergencia {
    /// The offset of the first different byte.
    pub byte: usize,
    /// A short excerpt of the form received.
    pub recebido: String,
    /// A short excerpt of the form signed.
    pub assinado: String,
}

/// The result of examining a signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutopsiaAssinatura {
    /// The `Id` of the signed element.
    pub id_documento: String,
    /// The `Reference URI`.
    pub reference_uri: Option<String>,
    /// `DigestValue` as informed.
    pub digest_informado: Option<String>,
    /// The digest calculated over the content as received, with the algorithm informed.
    pub digest_calculado: Option<String>,
    /// Whether the two match.
    pub digest_confere: bool,
    /// Whether `SignatureValue` matches the certificate; `None` if it could not be tried.
    pub assinatura_confere: Option<bool>,
    /// The `DigestMethod` informed.
    pub algoritmo_digest: Option<String>,
    /// The `SignatureMethod` informed.
    pub algoritmo_assinatura: Option<String>,
    /// The size of the canonical form received.
    pub bytes_canonicos: usize,
    /// Where the canonical form differs from what was signed, when that could be rebuilt.
    pub divergencia: Option<Divergencia>,
    /// The causes, the proved ones first.
    pub causas: Vec<CausaProvavel>,
    /// What the certificate says.
    pub certificado: Option<InfoCertificado>,
    /// Where the method comes from.
    pub fonte: Fonte,
}

impl AutopsiaAssinatura {
    /// Whether the signature is fine.
    #[must_use]
    pub fn integra(&self) -> bool {
        self.digest_confere && self.assinatura_confere == Some(true) && self.causas.is_empty()
    }

    /// The first cause, which is the strongest one.
    #[must_use]
    pub fn causa_principal(&self) -> Option<Causa> {
        self.causas.first().map(|causa| causa.causa)
    }
}

fn eh_branco(texto: &str) -> bool {
    texto.chars().all(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
}

fn tirar_brancos(element: &mut Element) -> usize {
    let tem_filhos = element.elements().next().is_some();
    let mut removidos = 0;
    if tem_filhos {
        let antes = element.children.len();
        element.children.retain(|node| !matches!(node, Node::Text(texto) if eh_branco(texto)));
        removidos += antes - element.children.len();
    }
    for node in &mut element.children {
        if let Node::Element(filho) = node {
            removidos += tirar_brancos(filho);
        }
    }
    removidos
}

fn aparar_valores(element: &mut Element) -> bool {
    let mut mudou = false;
    let folha = element.elements().next().is_none();
    for node in &mut element.children {
        match node {
            Node::Text(texto) if folha => {
                let aparado = texto.trim().to_string();
                if aparado != *texto {
                    *texto = aparado;
                    mudou = true;
                }
            }
            Node::Element(filho) => mudou |= aparar_valores(filho),
            _ => {}
        }
    }
    mudou
}

fn tirar_atributos_ns(element: &mut Element, manter_padrao: bool) -> bool {
    let antes = element.attrs.len();
    element.attrs.retain(|(chave, _)| !(chave.starts_with("xmlns:") || (!manter_padrao && chave == "xmlns")));
    let mut mudou = antes != element.attrs.len();
    for node in &mut element.children {
        if let Node::Element(filho) = node
            && filho.local_name() != "Signature"
        {
            mudou |= tirar_atributos_ns(filho, manter_padrao);
        }
    }
    mudou
}

fn com_id_mut<'a>(element: &'a mut Element, id: &str) -> Option<&'a mut Element> {
    if element.attr("Id") == Some(id) {
        return Some(element);
    }
    element.children.iter_mut().find_map(|node| match node {
        Node::Element(filho) => com_id_mut(filho, id),
        _ => None,
    })
}

fn hash_em_base64(dados: &[u8], tamanho_informado: usize) -> String {
    if tamanho_informado == 32 {
        STANDARD.encode(Sha256::digest(dados))
    } else {
        STANDARD.encode(Sha1::digest(dados))
    }
}

/// Cuts the element with this name out of the raw text, as it was written.
fn recorte_cru<'a>(cru: &'a str, nome: &str) -> Option<&'a str> {
    let inicio = cru.find(&format!("<{nome} "))?;
    let fecho = format!("</{nome}>");
    let fim = cru[inicio..].find(&fecho)? + inicio + fecho.len();
    cru.get(inicio..fim)
}

fn atributos_em_ordem(alvo: &Element) -> bool {
    let nomes: Vec<&str> = alvo
        .attrs
        .iter()
        .filter(|(chave, _)| !chave.starts_with("xmlns"))
        .map(|(chave, _)| chave.as_str())
        .collect();
    nomes.windows(2).all(|par| par[0] <= par[1])
}

/// Compares two canonical forms and shows the first place they differ.
#[must_use]
pub fn primeira_divergencia(recebido: &str, assinado: &str) -> Option<Divergencia> {
    let a = recebido.as_bytes();
    let b = assinado.as_bytes();
    let byte = a
        .iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .or_else(|| (a.len() != b.len()).then_some(a.len().min(b.len())))?;
    let trecho = |bytes: &[u8]| {
        let inicio = byte.saturating_sub(16);
        let fim = (byte + 24).min(bytes.len());
        String::from_utf8_lossy(bytes.get(inicio..fim).unwrap_or(&[])).into_owned()
    };
    Some(Divergencia { byte, recebido: trecho(a), assinado: trecho(b) })
}

/// A version of what may have been signed, and the cause it would prove.
struct Candidato {
    bytes: String,
    causa: Causa,
    explicacao: String,
}

fn candidatos(pai: &Element, id: &str, cru: Option<&str>, alvo_nome: &str) -> Vec<Candidato> {
    let mut lista = Vec::new();
    let canon = |raiz: &Element| canonicalize_by_id(raiz, id, true).ok();

    let mut copia = pai.clone();
    let removidos = tirar_brancos(&mut copia);
    if removidos > 0
        && let Some(bytes) = canon(&copia)
    {
        let (causa, explicacao) = if removidos >= 3 {
            (
                Causa::Reformatado,
                format!(
                    "O XML foi reformatado depois de assinado: tirando {removidos} trechos de indentação entre as tags, o digest confere."
                ),
            )
        } else {
            (
                Causa::EspacosEQuebras,
                format!(
                    "Há {removidos} trecho(s) de espaço ou quebra de linha entre as tags que não existiam quando a nota foi assinada: sem eles, o digest confere."
                ),
            )
        };
        lista.push(Candidato { bytes, causa, explicacao });
    }

    let mut copia = pai.clone();
    if aparar_valores(&mut copia)
        && let Some(bytes) = canon(&copia)
    {
        lista.push(Candidato {
            bytes,
            causa: Causa::EspacosEQuebras,
            explicacao: "Há espaço ou quebra de linha nas pontas de um valor, acrescentado depois de assinar: sem eles, o digest confere.".to_string(),
        });
    }

    let mut copia = pai.clone();
    if tirar_atributos_ns(&mut copia, false)
        && let Some(bytes) = canon(&copia)
    {
        lista.push(Candidato {
            bytes,
            causa: Causa::XmlnsAusente,
            explicacao: "A nota foi assinada sem o xmlns que herda de NFe: canonizada sem namespace, o digest confere. Assine o elemento já dentro de NFe.".to_string(),
        });
    }

    let mut copia = pai.clone();
    if tirar_atributos_ns(&mut copia, true)
        && let Some(bytes) = canon(&copia)
    {
        lista.push(Candidato {
            bytes,
            causa: Causa::XmlnsAMais,
            explicacao: "Um namespace a mais (xsi, xsd ou outro) entrou no XML depois de assinar: sem ele, o digest confere.".to_string(),
        });
    }

    for (chave, uri) in NAMESPACES_COMUNS {
        let mut copia = pai.clone();
        if let Some(alvo) = com_id_mut(&mut copia, id) {
            alvo.attrs.push((chave.to_string(), uri.to_string()));
        }
        if let Some(bytes) = canon(&copia) {
            lista.push(Candidato {
                bytes,
                causa: Causa::XmlnsAMais,
                explicacao: format!(
                    "A nota foi assinada com {chave} em escopo, herdado de um envelope ou do serializador, e a C14N inclusiva o levou para o digest. Assine fora do envelope."
                ),
            });
        }
    }

    if let Some(cru) = cru
        && let Some(recorte) = recorte_cru(cru, alvo_nome)
    {
        let ordem = pai.find(alvo_nome).is_none_or(atributos_em_ordem);
        let (causa, explicacao) = if ordem {
            (Causa::NaoCanonizado, "O texto foi assinado como estava, sem passar pela C14N.".to_string())
        } else {
            (
                Causa::OrdemDeAtributos,
                "O texto foi assinado sem passar pela C14N, com os atributos fora da ordem canônica."
                    .to_string(),
            )
        };
        lista.push(Candidato { bytes: recorte.to_string(), causa, explicacao });
    }

    if let Some(bytes) = canon(pai) {
        lista.push(Candidato {
            bytes: format!("\u{feff}{bytes}"),
            causa: Causa::Bom,
            explicacao: "O digest foi calculado com um BOM na frente do conteúdo.".to_string(),
        });
    }
    lista
}

fn ordenar(causas: &mut [CausaProvavel]) {
    causas.sort_by_key(|causa| causa.evidencia != Evidencia::Comprovada);
}

/// Examines the signature inside a document, as text.
pub fn autopsia_assinatura(xml_texto: &str) -> Result<AutopsiaAssinatura, NfeError> {
    let raiz = xml::parse(xml_texto)?;
    let (pai, id) =
        localizar(&raiz).ok_or_else(|| NfeError::Xml("nenhum documento assinável encontrado".to_string()))?;
    Ok(examinar(pai, &id, Some(xml_texto), None))
}

/// Compares a document with the original that was signed, which pins the first differing byte.
pub fn autopsia_contra_original(xml_texto: &str, original: &str) -> Result<AutopsiaAssinatura, NfeError> {
    let raiz = xml::parse(xml_texto)?;
    let raiz_original = xml::parse(original)?;
    let (pai, id) =
        localizar(&raiz).ok_or_else(|| NfeError::Xml("nenhum documento assinável encontrado".to_string()))?;
    let (pai_original, _) = localizar(&raiz_original)
        .ok_or_else(|| NfeError::Xml("original sem documento assinável".to_string()))?;
    let mut autopsia = examinar(pai, &id, Some(xml_texto), None);
    if let (Ok(recebido), Ok(assinado)) =
        (canonicalize_by_id(pai, &id, true), canonicalize_by_id(pai_original, &id, true))
    {
        autopsia.divergencia = primeira_divergencia(&recebido, &assinado);
    }
    Ok(autopsia)
}

/// Finds the signed parent (`NFe` or `evento`) and the `Id` of what it signs.
pub(crate) fn localizar(raiz: &Element) -> Option<(&Element, String)> {
    if let Some(nfe) = raiz.find("NFe") {
        let id = nfe.child("infNFe")?.attr("Id")?.to_string();
        return Some((nfe, id));
    }
    let evento = raiz.find("evento")?;
    let id = evento.child("infEvento")?.attr("Id")?.to_string();
    Some((evento, id))
}

/// Examines a signature, given its parent and the `Id` of the document.
pub(crate) fn examinar(
    pai: &Element,
    id: &str,
    cru: Option<&str>,
    agora: Option<Instante>,
) -> AutopsiaAssinatura {
    let fonte = Fonte::Sandbox("método desta autópsia; o padrão vem do MOC 7.0, Visão Geral, 4.2.4");
    let alvo_nome = if pai.local_name() == "NFe" { "infNFe" } else { "infEvento" };
    let canonica = canonicalize_by_id(pai, id, true).unwrap_or_default();
    let mut autopsia = AutopsiaAssinatura {
        id_documento: id.to_string(),
        reference_uri: None,
        digest_informado: None,
        digest_calculado: None,
        digest_confere: false,
        assinatura_confere: None,
        algoritmo_digest: None,
        algoritmo_assinatura: None,
        bytes_canonicos: canonica.len(),
        divergencia: None,
        causas: Vec::new(),
        certificado: None,
        fonte,
    };
    let mut causas = Vec::new();

    if let Some(cru) = cru
        && cru.starts_with('\u{feff}')
    {
        causas.push(CausaProvavel {
            causa: Causa::Bom,
            evidencia: Evidencia::Comprovada,
            explicacao: "O texto começa com um BOM, que o MOC não aceita nas pontas da mensagem.".to_string(),
        });
    }

    let Some(assinatura) = pai.child("Signature") else {
        causas.push(CausaProvavel {
            causa: Causa::SemAssinatura,
            evidencia: Evidencia::Comprovada,
            explicacao: "O documento não tem Signature.".to_string(),
        });
        autopsia.causas = causas;
        return autopsia;
    };

    let info = assinatura.child("SignedInfo");
    let referencia = info.and_then(|i| i.child("Reference"));
    autopsia.reference_uri = referencia.and_then(|r| r.attr("URI")).map(str::to_string);
    autopsia.algoritmo_digest = referencia
        .and_then(|r| r.child("DigestMethod"))
        .and_then(|m| m.attr("Algorithm"))
        .map(str::to_string);
    autopsia.algoritmo_assinatura =
        info.and_then(|i| i.child("SignatureMethod")).and_then(|m| m.attr("Algorithm")).map(str::to_string);
    autopsia.digest_informado = referencia.and_then(|r| r.child_text("DigestValue"));

    let esperado_uri = format!("#{id}");
    if autopsia.reference_uri.as_deref() != Some(esperado_uri.as_str()) {
        causas.push(CausaProvavel {
            causa: Causa::ReferenceUriDiferente,
            evidencia: Evidencia::Comprovada,
            explicacao: format!(
                "Reference URI é {} e o Id do documento pede {esperado_uri}.",
                autopsia.reference_uri.as_deref().unwrap_or("(ausente)")
            ),
        });
    }

    let informado = autopsia
        .digest_informado
        .as_deref()
        .map(|texto| texto.split_whitespace().collect::<String>())
        .and_then(|texto| STANDARD.decode(texto).ok());
    let tamanho = informado.as_ref().map_or(20, Vec::len);
    let declarou_sha256 = autopsia.algoritmo_digest.as_deref() == Some(SHA256_URI)
        || autopsia.algoritmo_assinatura.as_deref() == Some(RSA_SHA256);
    if declarou_sha256 || tamanho == 32 {
        causas.push(CausaProvavel {
            causa: Causa::Sha256NoLugarDeSha1,
            evidencia: Evidencia::Comprovada,
            explicacao: "A assinatura usa SHA-256; o padrão da NF-e é RSA-SHA1 com digest SHA-1, mesmo sendo o algoritmo mais antigo.".to_string(),
        });
    }

    let calculado = hash_em_base64(canonica.as_bytes(), tamanho);
    autopsia.digest_confere = informado.as_deref().is_some_and(|bytes| STANDARD.encode(bytes) == calculado);
    autopsia.digest_calculado = Some(calculado);

    if !autopsia.digest_confere {
        let alvo = informado.as_deref().map(|bytes| STANDARD.encode(bytes)).unwrap_or_default();
        let achado = candidatos(pai, id, cru, alvo_nome)
            .into_iter()
            .find(|candidato| hash_em_base64(candidato.bytes.as_bytes(), tamanho) == alvo);
        match achado {
            Some(candidato) => {
                autopsia.divergencia = primeira_divergencia(&canonica, &candidato.bytes);
                causas.push(CausaProvavel {
                    causa: candidato.causa,
                    evidencia: Evidencia::Comprovada,
                    explicacao: candidato.explicacao,
                });
            }
            None => causas.push(CausaProvavel {
                causa: Causa::ConteudoAlterado,
                evidencia: Evidencia::Indicio,
                explicacao: "Nenhuma das formas conhecidas de erro dá o digest informado. O conteúdo foi alterado depois de assinado, ou assinado por um processo que este exame não conhece; sem o original, não dá para apontar o byte.".to_string(),
            }),
        }
    }

    let texto_certificado: Option<String> = assinatura
        .child("KeyInfo")
        .and_then(|k| k.child("X509Data"))
        .and_then(|x| x.child_text("X509Certificate"))
        .map(|texto| texto.split_whitespace().collect());
    let der = texto_certificado.and_then(|texto| STANDARD.decode(texto).ok());
    let leitura = der.as_deref().and_then(InfoCertificado::ler);
    if let Some(leitura) = &leitura {
        autopsia.assinatura_confere = verificar_assinatura(pai, assinatura, leitura);
        if autopsia.digest_confere && autopsia.assinatura_confere == Some(false) {
            causas.push(CausaProvavel {
                causa: Causa::ChaveDiferenteDoCertificado,
                evidencia: Evidencia::Indicio,
                explicacao: "O digest confere, mas o SignatureValue não bate com a chave do certificado no XML: ou o valor foi alterado, ou a nota foi assinada com outra chave.".to_string(),
            });
        }
        let referencia_tempo = agora
            .or_else(|| pai.find_text("dhEmi").and_then(|d| Instante::de_xml(&d)))
            .or_else(|| pai.find_text("dhEvento").and_then(|d| Instante::de_xml(&d)));
        if let (Some(momento), Some(fim)) = (referencia_tempo, leitura.valido_ate)
            && momento > fim
        {
            causas.push(CausaProvavel {
                causa: Causa::CertificadoExpirado,
                evidencia: Evidencia::Comprovada,
                explicacao: format!("O certificado venceu em {}.", fim.para_xml()),
            });
        }
        let emitente = pai
            .find("emit")
            .and_then(|emit| emit.child_text("CNPJ"))
            .or_else(|| pai.find("infEvento").and_then(|inf| inf.child_text("CNPJ")));
        if let (Some(emitente), Some(certificado)) = (emitente, &leitura.cnpj)
            && emitente.get(..8) != certificado.get(..8)
        {
            causas.push(CausaProvavel {
                causa: Causa::CertificadoDeOutroCnpj,
                evidencia: Evidencia::Comprovada,
                explicacao: format!(
                    "O certificado é do CNPJ-base {} e o emitente é do CNPJ-base {}.",
                    certificado.get(..8).unwrap_or(""),
                    emitente.get(..8).unwrap_or("")
                ),
            });
        }
    }
    autopsia.certificado = leitura;
    ordenar(&mut causas);
    autopsia.causas = causas;
    autopsia
}

fn verificar_assinatura(pai: &Element, assinatura: &Element, certificado: &InfoCertificado) -> Option<bool> {
    let valor: String = assinatura.child_text("SignatureValue")?.split_whitespace().collect();
    let valor = STANDARD.decode(valor).ok()?;
    let chave = RsaPublicKey::from_public_key_der(certificado.spki()).ok()?;
    let canonico = canonicalize_by_name(pai, "SignedInfo").ok()?;
    let algoritmo = assinatura.child("SignedInfo")?.child("SignatureMethod")?.attr("Algorithm")?;
    let confere = match algoritmo {
        RSA_SHA1 => {
            chave.verify(Pkcs1v15Sign::new::<Sha1>(), &Sha1::digest(canonico.as_bytes()), &valor).is_ok()
        }
        RSA_SHA256 => {
            chave.verify(Pkcs1v15Sign::new::<Sha256>(), &Sha256::digest(canonico.as_bytes()), &valor).is_ok()
        }
        _ => return None,
    };
    Some(confere)
}
