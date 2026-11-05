//! XMLDSig with RSA-SHA1 and SHA1, the signature the NF-e layout requires, behind [`Signer`].
//!
//! The private key is only used to sign locally. Nothing here decrypts data, which is why the
//! timing advisory of the `rsa` crate does not reach this use; see `REGRAS.md`.

use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey};
use rsa::{Pkcs1v15Sign, RsaPrivateKey, RsaPublicKey};
use sha1::{Digest, Sha1};

use crate::c14n::{canonicalize_by_id, canonicalize_by_name};
use crate::error::NfeError;
use crate::xml::{self, Element, escape_attr};

const DSIG_NS: &str = "http://www.w3.org/2000/09/xmldsig#";
const C14N_ALG: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
const RSA_SHA1_ALG: &str = "http://www.w3.org/2000/09/xmldsig#rsa-sha1";
const ENVELOPED_ALG: &str = "http://www.w3.org/2000/09/xmldsig#enveloped-signature";
const SHA1_ALG: &str = "http://www.w3.org/2000/09/xmldsig#sha1";

/// Whatever holds a signing key. The default is [`RsaSigner`]; a token reader could be another.
pub trait Signer: Send + Sync {
    /// The DER of the certificate that goes in `X509Certificate`.
    fn certificate_der(&self) -> &[u8];

    /// Signs `message` with RSA PKCS#1 v1.5 over SHA-1.
    fn sign_rsa_sha1(&self, message: &[u8]) -> Result<Vec<u8>, NfeError>;

    /// Signs `message` with RSA PKCS#1 v1.5 over this hash. The NF-e only uses SHA-1; SHA-256 is
    /// here for the NFS-e, and a signer that does not offer it says so.
    fn sign_rsa(&self, hash: HashAssinatura, message: &[u8]) -> Result<Vec<u8>, NfeError> {
        match hash {
            HashAssinatura::Sha1 => self.sign_rsa_sha1(message),
            HashAssinatura::Sha256 => {
                Err(NfeError::Assinatura("este assinador não oferece SHA-256".to_string()))
            }
        }
    }
}

/// The hash under a signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashAssinatura {
    /// SHA-1, which is what the NF-e layout asks.
    Sha1,
    /// SHA-256, for documents outside the NF-e.
    Sha256,
}

/// Signs with RSA PKCS#1 v1.5 over this hash.
pub(crate) fn assinar_rsa(
    chave: &RsaPrivateKey,
    hash: HashAssinatura,
    mensagem: &[u8],
) -> Result<Vec<u8>, NfeError> {
    let assinatura = match hash {
        HashAssinatura::Sha1 => chave.sign(Pkcs1v15Sign::new::<Sha1>(), &Sha1::digest(mensagem)),
        HashAssinatura::Sha256 => {
            chave.sign(Pkcs1v15Sign::new::<rsa::sha2::Sha256>(), &rsa::sha2::Sha256::digest(mensagem))
        }
    };
    assinatura.map_err(|erro| NfeError::Assinatura(erro.to_string()))
}

/// A signer with an RSA key in memory.
#[derive(Clone)]
pub struct RsaSigner {
    key: RsaPrivateKey,
    certificate: Vec<u8>,
}

impl fmt::Debug for RsaSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RsaSigner").field("key", &"<oculta>").finish_non_exhaustive()
    }
}

impl RsaSigner {
    /// A signer from a certificate in DER and a PKCS#8 key in DER.
    pub fn from_der(certificate: Vec<u8>, key_pkcs8: &[u8]) -> Result<Self, NfeError> {
        let key = RsaPrivateKey::from_pkcs8_der(key_pkcs8)
            .map_err(|erro| NfeError::Assinatura(format!("chave privada: {erro}")))?;
        Ok(Self { key, certificate })
    }

    /// A signer from a certificate and a PKCS#8 key, both in PEM.
    pub fn from_pem(certificate: &str, key_pkcs8: &str) -> Result<Self, NfeError> {
        let key = RsaPrivateKey::from_pkcs8_pem(key_pkcs8)
            .map_err(|erro| NfeError::Assinatura(format!("chave privada: {erro}")))?;
        Ok(Self { key, certificate: pem_certificate(certificate)? })
    }
}

impl Signer for RsaSigner {
    fn certificate_der(&self) -> &[u8] {
        &self.certificate
    }

    fn sign_rsa_sha1(&self, message: &[u8]) -> Result<Vec<u8>, NfeError> {
        assinar_rsa(&self.key, HashAssinatura::Sha1, message)
    }

    fn sign_rsa(&self, hash: HashAssinatura, message: &[u8]) -> Result<Vec<u8>, NfeError> {
        assinar_rsa(&self.key, hash, message)
    }
}

/// Reads the first certificate out of a PEM text.
pub fn pem_certificate(pem: &str) -> Result<Vec<u8>, NfeError> {
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";
    let erro = || NfeError::Assinatura("certificado PEM não encontrado".to_string());
    let start = pem.find(BEGIN).ok_or_else(erro)? + BEGIN.len();
    let end = pem[start..].find(END).ok_or_else(erro)? + start;
    let body: String = pem[start..end].split_whitespace().collect();
    STANDARD.decode(body).map_err(|e| NfeError::Assinatura(format!("certificado PEM: {e}")))
}

/// Signs the element with this `Id` and places `Signature` as the last child of `parent`.
pub fn sign_enveloped(
    xml_text: &str,
    parent: &str,
    id: &str,
    signer: &dyn Signer,
) -> Result<String, NfeError> {
    let root = xml::parse(xml_text)?;
    let canonical = canonicalize_by_id(&root, id, true)?;
    let digest = STANDARD.encode(Sha1::digest(canonical.as_bytes()));
    let certificate = STANDARD.encode(signer.certificate_der());

    let signed_info = format!(
        concat!(
            "<SignedInfo>",
            "<CanonicalizationMethod Algorithm=\"{c14n}\"/>",
            "<SignatureMethod Algorithm=\"{rsa}\"/>",
            "<Reference URI=\"#{id}\">",
            "<Transforms>",
            "<Transform Algorithm=\"{enveloped}\"/>",
            "<Transform Algorithm=\"{c14n}\"/>",
            "</Transforms>",
            "<DigestMethod Algorithm=\"{sha1}\"/>",
            "<DigestValue>{digest}</DigestValue>",
            "</Reference>",
            "</SignedInfo>",
        ),
        c14n = C14N_ALG,
        rsa = RSA_SHA1_ALG,
        id = escape_attr(id),
        enveloped = ENVELOPED_ALG,
        sha1 = SHA1_ALG,
        digest = digest,
    );

    let closing = format!("</{parent}>");
    let position = xml_text
        .rfind(&closing)
        .ok_or_else(|| NfeError::Assinatura(format!("elemento {parent} não encontrado")))?;
    let (head, tail) = xml_text.split_at(position);
    let compose = |value: &str| {
        format!(
            "{head}<Signature xmlns=\"{DSIG_NS}\">{signed_info}<SignatureValue>{value}</SignatureValue><KeyInfo><X509Data><X509Certificate>{certificate}</X509Certificate></X509Data></KeyInfo></Signature>{tail}"
        )
    };

    // SignedInfo is canonicalized in its final place, so it sees the namespaces it will really have.
    let placed = xml::parse(&compose(""))?;
    let canonical_info = canonicalize_by_name(&placed, "SignedInfo")?;
    let signature = signer.sign_rsa_sha1(canonical_info.as_bytes())?;
    Ok(compose(&STANDARD.encode(signature)))
}

/// What is wrong with a signature, split the way the SEFAZ splits its rejections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FalhaAssinatura {
    /// The signature does not follow the standard: rejection 298.
    ForaDoPadrao(String),
    /// The content changed after signing, or the signature does not match: rejection 297.
    Difere(String),
}

/// Checks the `Signature` child of `parent` against the certificate inside it.
///
/// `parent` must be the document as sent, with itself as the root of canonicalization.
/// `id_esperado` is the `Id` the reference has to point at. The trust chain is not looked at.
pub fn verify_enveloped(parent: &Element, id_esperado: &str) -> Result<(), FalhaAssinatura> {
    let fora = |motivo: &str| FalhaAssinatura::ForaDoPadrao(motivo.to_string());
    let signature = parent.child("Signature").ok_or_else(|| fora("Signature ausente"))?;
    if signature.attr("xmlns") != Some(DSIG_NS) {
        return Err(fora("namespace de Signature incorreto"));
    }
    let info = signature.child("SignedInfo").ok_or_else(|| fora("SignedInfo ausente"))?;
    let algorithm = |element: Option<&Element>| element.and_then(|e| e.attr("Algorithm")).map(str::to_string);
    if algorithm(info.child("CanonicalizationMethod")).as_deref() != Some(C14N_ALG) {
        return Err(fora("CanonicalizationMethod incorreto"));
    }
    if algorithm(info.child("SignatureMethod")).as_deref() != Some(RSA_SHA1_ALG) {
        return Err(fora("SignatureMethod incorreto"));
    }
    let references: Vec<&Element> = info.elements().filter(|e| e.local_name() == "Reference").collect();
    let [reference] = references.as_slice() else {
        return Err(fora("é preciso exatamente uma Reference"));
    };
    if reference.attr("URI") != Some(&format!("#{id_esperado}")) {
        return Err(fora("Reference não aponta para o Id do documento"));
    }
    let transforms: Vec<Option<&str>> = reference
        .child("Transforms")
        .ok_or_else(|| fora("Transforms ausente"))?
        .elements()
        .map(|t| t.attr("Algorithm"))
        .collect();
    if transforms != [Some(ENVELOPED_ALG), Some(C14N_ALG)] {
        return Err(fora("Transforms incorretos"));
    }
    if algorithm(reference.child("DigestMethod")).as_deref() != Some(SHA1_ALG) {
        return Err(fora("DigestMethod incorreto"));
    }
    let decode = |element: Option<&Element>, name: &str| -> Result<Vec<u8>, FalhaAssinatura> {
        let text: String = element
            .map(Element::text)
            .ok_or_else(|| fora(&format!("{name} ausente")))?
            .split_whitespace()
            .collect();
        STANDARD.decode(text).map_err(|_| fora(&format!("{name} não é base64")))
    };
    let digest = decode(reference.child("DigestValue"), "DigestValue")?;
    let signature_value = decode(signature.child("SignatureValue"), "SignatureValue")?;
    let certificate = decode(
        signature.child("KeyInfo").and_then(|k| k.child("X509Data")).and_then(|x| x.child("X509Certificate")),
        "X509Certificate",
    )?;

    let canonical = canonicalize_by_id(parent, id_esperado, true).map_err(|erro| fora(&erro.to_string()))?;
    if Sha1::digest(canonical.as_bytes()).as_slice() != digest.as_slice() {
        return Err(FalhaAssinatura::Difere("DigestValue não confere".to_string()));
    }

    let spki =
        subject_public_key_info(&certificate).ok_or_else(|| fora("certificado sem chave pública legível"))?;
    let key =
        RsaPublicKey::from_public_key_der(spki).map_err(|_| fora("a chave do certificado não é RSA"))?;
    let canonical_info =
        canonicalize_by_name(parent, "SignedInfo").map_err(|erro| fora(&erro.to_string()))?;
    let info_digest = Sha1::digest(canonical_info.as_bytes());
    key.verify(Pkcs1v15Sign::new::<Sha1>(), &info_digest, &signature_value)
        .map_err(|_| FalhaAssinatura::Difere("SignatureValue não confere".to_string()))
}

/// Finds `subjectPublicKeyInfo` in an X.509 certificate.
fn subject_public_key_info(certificate: &[u8]) -> Option<&[u8]> {
    let cert = crate::sandbox::certificado_x509::ler(certificate)?;
    let tbs = crate::sandbox::certificado_x509::ler(cert.content)?;
    let first = crate::sandbox::certificado_x509::ler(tbs.content)?;
    let mut rest = first.rest;
    // The optional version comes as [0]; without it the first field was already the serial.
    if first.tag == 0xa0 {
        rest = crate::sandbox::certificado_x509::ler(rest)?.rest;
    }
    // Signature algorithm, issuer, validity and subject come before the key.
    for _ in 0..4 {
        rest = crate::sandbox::certificado_x509::ler(rest)?.rest;
    }
    Some(crate::sandbox::certificado_x509::ler(rest)?.whole)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = include_str!("../fixtures/laboratorio/externo-key.pem");
    const CERT: &str = include_str!("../fixtures/laboratorio/externo-cert.pem");
    const DOC: &str = "<NFe xmlns=\"http://www.portalfiscal.inf.br/nfe\"><infNFe Id=\"NFe1\" versao=\"4.00\"><a>1</a></infNFe></NFe>";

    fn signer() -> RsaSigner {
        RsaSigner::from_pem(CERT, KEY).expect("signer")
    }

    #[test]
    fn signature_goes_last_and_verifies() {
        let signed = sign_enveloped(DOC, "NFe", "NFe1", &signer()).expect("signed");
        assert!(signed.ends_with("</Signature></NFe>"));
        let root = xml::parse(&signed).expect("xml");
        assert_eq!(verify_enveloped(&root, "NFe1"), Ok(()));
    }

    #[test]
    fn signing_twice_gives_the_same_bytes() {
        let a = sign_enveloped(DOC, "NFe", "NFe1", &signer()).expect("signed");
        let b = sign_enveloped(DOC, "NFe", "NFe1", &signer()).expect("signed");
        assert_eq!(a, b);
    }

    #[test]
    fn changed_content_is_caught_by_the_digest() {
        let signed = sign_enveloped(DOC, "NFe", "NFe1", &signer()).expect("signed");
        let root = xml::parse(&signed.replace("<a>1</a>", "<a>2</a>")).expect("xml");
        assert!(matches!(verify_enveloped(&root, "NFe1"), Err(FalhaAssinatura::Difere(_))));
    }

    #[test]
    fn changed_signed_info_is_caught_by_the_signature() {
        let signed = sign_enveloped(DOC, "NFe", "NFe1", &signer()).expect("signed");
        // A digest with a valid form for other content: the digest check would pass if SignedInfo
        // were not signed, so the content changes along with it.
        let other = DOC.replace("<a>1</a>", "<a>2</a>");
        let other_root = xml::parse(&other).expect("xml");
        let canonical = canonicalize_by_id(&other_root, "NFe1", true).expect("c14n");
        let digest = STANDARD.encode(Sha1::digest(canonical.as_bytes()));
        let start = signed.find("<DigestValue>").expect("digest") + "<DigestValue>".len();
        let end = signed.find("</DigestValue>").expect("digest");
        let forged =
            format!("{}{}{}", &signed[..start], digest, &signed[end..]).replace("<a>1</a>", "<a>2</a>");
        let root = xml::parse(&forged).expect("xml");
        assert_eq!(
            verify_enveloped(&root, "NFe1"),
            Err(FalhaAssinatura::Difere("SignatureValue não confere".to_string()))
        );
    }

    #[test]
    fn wrong_algorithm_is_out_of_standard() {
        let signed = sign_enveloped(DOC, "NFe", "NFe1", &signer()).expect("signed");
        let root = xml::parse(&signed.replace(RSA_SHA1_ALG, "urn:outro")).expect("xml");
        assert!(matches!(verify_enveloped(&root, "NFe1"), Err(FalhaAssinatura::ForaDoPadrao(_))));
    }

    #[test]
    fn missing_signature_is_out_of_standard() {
        let root = xml::parse(DOC).expect("xml");
        assert!(matches!(verify_enveloped(&root, "NFe1"), Err(FalhaAssinatura::ForaDoPadrao(_))));
    }

    #[test]
    fn reference_must_point_at_the_expected_id() {
        let signed = sign_enveloped(DOC, "NFe", "NFe1", &signer()).expect("signed");
        let root = xml::parse(&signed).expect("xml");
        assert!(matches!(verify_enveloped(&root, "NFe2"), Err(FalhaAssinatura::ForaDoPadrao(_))));
    }

    #[test]
    fn spki_is_found_in_the_sandbox_certificate() {
        let der = pem_certificate(CERT).expect("pem");
        let spki = subject_public_key_info(&der).expect("spki");
        assert!(RsaPublicKey::from_public_key_der(spki).is_ok());
    }

    #[test]
    fn truncated_der_is_refused() {
        assert!(subject_public_key_info(&[0x30, 0x82]).is_none());
        assert!(subject_public_key_info(&[]).is_none());
    }

    #[test]
    fn debug_hides_the_key() {
        assert!(!format!("{:?}", signer()).contains("BEGIN"));
    }
}
