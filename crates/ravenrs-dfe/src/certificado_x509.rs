//! Reads and writes the few parts of X.509 this crate needs, in plain DER.
//!
//! Written by hand because the failure cases need what a generator refuses to make: SHA-1 in the
//! certificate, a 1024-bit key, a chain that does not close. It also keeps WebAssembly working.

use rsa::pkcs8::DecodePublicKey;
use rsa::traits::PublicKeyParts;
use rsa::{Pkcs1v15Sign, RsaPublicKey};
use sha1::{Digest, Sha1};

use crate::tempo::Instante;

const OID_SHA256_RSA: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0b];
const OID_SHA1_RSA: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x05];
const OID_CN: &[u8] = &[0x55, 0x04, 0x03];
const OID_C: &[u8] = &[0x55, 0x04, 0x06];
const OID_O: &[u8] = &[0x55, 0x04, 0x0a];
const OID_KEY_USAGE: &[u8] = &[0x55, 0x1d, 0x0f];
const OID_SAN: &[u8] = &[0x55, 0x1d, 0x11];
const OID_BASIC: &[u8] = &[0x55, 0x1d, 0x13];
const OID_EKU: &[u8] = &[0x55, 0x1d, 0x25];
/// 1.3.6.1.5.5.7.3.2, Autenticação Cliente.
const OID_CLIENT_AUTH: &[u8] = &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x02];
/// 1.3.6.1.5.5.7.3.4, proteção de e-mail, que nada vale para a NF-e.
const OID_EMAIL: &[u8] = &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x04];
/// 2.16.76.1.3.3, the CNPJ of the holder of an e-CNPJ.
pub(crate) const OID_ICP_CNPJ: &[u8] = &[0x60, 0x4c, 0x01, 0x03, 0x03];
/// 2.16.76.1.3.1, the data of the person holding an e-CPF.
const OID_ICP_CPF: &[u8] = &[0x60, 0x4c, 0x01, 0x03, 0x01];
/// 2.16.76.1.3.2, the name of the person responsible for an e-CNPJ.
pub(crate) const OID_ICP_RESPONSAVEL: &[u8] = &[0x60, 0x4c, 0x01, 0x03, 0x02];
/// 2.16.76.1.3.4, the data of the person responsible for an e-CNPJ.
pub(crate) const OID_ICP_DADOS_RESPONSAVEL: &[u8] = &[0x60, 0x4c, 0x01, 0x03, 0x04];
/// 2.16.76.1.3.7, the CEI of the company.
pub(crate) const OID_ICP_CEI: &[u8] = &[0x60, 0x4c, 0x01, 0x03, 0x07];

/// The hash the issuer used to sign a certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashCertificado {
    /// SHA-256 with RSA.
    Sha256,
    /// SHA-1 with RSA.
    Sha1,
    /// Anything else.
    Outro,
}

/// One element of DER.
pub(crate) struct Der<'a> {
    pub tag: u8,
    pub whole: &'a [u8],
    pub content: &'a [u8],
    pub rest: &'a [u8],
}

/// Reads one DER element.
pub(crate) fn ler(der: &[u8]) -> Option<Der<'_>> {
    let tag = *der.first()?;
    let first = *der.get(1)?;
    let (length, header) = if first < 0x80 {
        (usize::from(first), 2)
    } else {
        let count = usize::from(first & 0x7f);
        if count == 0 || count > 4 {
            return None;
        }
        let bytes = der.get(2..2 + count)?;
        (bytes.iter().fold(0_usize, |acc, b| (acc << 8) | usize::from(*b)), 2 + count)
    };
    let end = header.checked_add(length)?;
    Some(Der { tag, whole: der.get(..end)?, content: der.get(header..end)?, rest: der.get(end..)? })
}

/// Writes one DER element.
pub(crate) fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let length = content.len();
    if length < 0x80 {
        out.push(length as u8);
    } else {
        let bytes: Vec<u8> = length.to_be_bytes().iter().copied().skip_while(|byte| *byte == 0).collect();
        out.push(0x80 | bytes.len() as u8);
        out.extend_from_slice(&bytes);
    }
    out.extend_from_slice(content);
    out
}

pub(crate) fn sequencia(partes: &[&[u8]]) -> Vec<u8> {
    tlv(0x30, &partes.concat())
}

fn oid(bytes: &[u8]) -> Vec<u8> {
    tlv(0x06, bytes)
}

fn utf8(texto: &str) -> Vec<u8> {
    tlv(0x0c, texto.as_bytes())
}

/// Writes a time: UTCTime up to 2049 and GeneralizedTime after, as RFC 5280 asks.
fn tempo(instante: Instante) -> Vec<u8> {
    let (ano, mes, dia, hora, minuto, segundo) = instante.partes_utc();
    if (1950..2050).contains(&ano) {
        let texto = format!("{:02}{mes:02}{dia:02}{hora:02}{minuto:02}{segundo:02}Z", ano % 100);
        tlv(0x17, texto.as_bytes())
    } else {
        let texto = format!("{ano:04}{mes:02}{dia:02}{hora:02}{minuto:02}{segundo:02}Z");
        tlv(0x18, texto.as_bytes())
    }
}

fn ler_tempo(der: &Der<'_>) -> Option<Instante> {
    let texto = std::str::from_utf8(der.content).ok()?;
    let parte = |inicio: usize, fim: usize| texto.get(inicio..fim)?.parse::<i64>().ok();
    let (ano, resto) = match der.tag {
        0x17 => {
            let curto = parte(0, 2)?;
            (if curto >= 50 { 1900 + curto } else { 2000 + curto }, 2)
        }
        0x18 => (parte(0, 4)?, 4),
        _ => return None,
    };
    Some(Instante::utc(
        ano,
        parte(resto, resto + 2)?,
        parte(resto + 2, resto + 4)?,
        parte(resto + 4, resto + 6)?,
        parte(resto + 6, resto + 8)?,
        parte(resto + 8, resto + 10)?,
    ))
}

/// A name with country, organization and common name.
fn nome(organizacao: &str, comum: &str) -> Vec<u8> {
    let rdn = |tipo: &[u8], valor: Vec<u8>| tlv(0x31, &sequencia(&[&oid(tipo), &valor]));
    sequencia(&[&rdn(OID_C, tlv(0x13, b"BR")), &rdn(OID_O, utf8(organizacao)), &rdn(OID_CN, utf8(comum))])
}

fn extensao(tipo: &[u8], critica: bool, valor: &[u8]) -> Vec<u8> {
    let tipo = oid(tipo);
    let octetos = tlv(0x04, valor);
    if critica { sequencia(&[&tipo, &tlv(0x01, &[0xff]), &octetos]) } else { sequencia(&[&tipo, &octetos]) }
}

/// Which extended key usage a certificate declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Eku {
    ClientAuth,
    SoEmail,
}

/// Everything that goes in a certificate.
pub(crate) struct Especificacao<'a> {
    pub serial: u64,
    pub emissor_org: &'a str,
    pub emissor_cn: &'a str,
    pub titular_org: &'a str,
    pub titular_cn: &'a str,
    pub de: Instante,
    pub ate: Instante,
    pub spki: &'a [u8],
    pub eh_ac: bool,
    pub uso_assinatura: bool,
    pub eku: Option<Eku>,
    pub outros_nomes: &'a [(&'a [u8], String)],
    pub hash: HashCertificado,
}

fn algoritmo(hash: HashCertificado) -> Vec<u8> {
    let tipo = if hash == HashCertificado::Sha1 { OID_SHA1_RSA } else { OID_SHA256_RSA };
    sequencia(&[&oid(tipo), &tlv(0x05, &[])])
}

/// Writes the part of a certificate that gets signed.
pub(crate) fn tbs(especificacao: &Especificacao<'_>) -> Vec<u8> {
    let versao = tlv(0xa0, &tlv(0x02, &[2]));
    // A leading 0x01 keeps the serial positive with no extra handling.
    let mut serial = vec![0x01];
    serial.extend_from_slice(&especificacao.serial.to_be_bytes());
    let serial = tlv(0x02, &serial);
    let algoritmo = algoritmo(especificacao.hash);
    let emissor = nome(especificacao.emissor_org, especificacao.emissor_cn);
    let validade = sequencia(&[&tempo(especificacao.de), &tempo(especificacao.ate)]);
    let titular = nome(especificacao.titular_org, especificacao.titular_cn);

    let mut extensoes = Vec::new();
    let basico = if especificacao.eh_ac { sequencia(&[&tlv(0x01, &[0xff])]) } else { sequencia(&[]) };
    extensoes.push(extensao(OID_BASIC, true, &basico));
    // Bits: 0 digitalSignature, 1 nonRepudiation, 2 keyEncipherment, 5 keyCertSign, 6 cRLSign.
    let uso: Vec<u8> = if especificacao.eh_ac {
        tlv(0x03, &[1, 0x06])
    } else if especificacao.uso_assinatura {
        tlv(0x03, &[5, 0xe0])
    } else {
        tlv(0x03, &[5, 0x20])
    };
    extensoes.push(extensao(OID_KEY_USAGE, true, &uso));
    if let Some(eku) = especificacao.eku {
        let tipos = match eku {
            Eku::ClientAuth => sequencia(&[&oid(OID_CLIENT_AUTH), &oid(OID_EMAIL)]),
            Eku::SoEmail => sequencia(&[&oid(OID_EMAIL)]),
        };
        extensoes.push(extensao(OID_EKU, false, &tipos));
    }
    if !especificacao.outros_nomes.is_empty() {
        let mut nomes = Vec::new();
        for (tipo, valor) in especificacao.outros_nomes {
            // otherName [0] { type-id, value [0] EXPLICIT PrintableString }
            let valor = tlv(0xa0, &tlv(0x13, valor.as_bytes()));
            let conteudo = [oid(tipo), valor].concat();
            nomes.extend_from_slice(&tlv(0xa0, &conteudo));
        }
        extensoes.push(extensao(OID_SAN, false, &tlv(0x30, &nomes)));
    }
    let lista: Vec<&[u8]> = extensoes.iter().map(Vec::as_slice).collect();
    let extensoes = tlv(0xa3, &sequencia(&lista));
    sequencia(&[&versao, &serial, &algoritmo, &emissor, &validade, &titular, especificacao.spki, &extensoes])
}

/// Puts a certificate together from the signed part and its signature.
pub(crate) fn montar(tbs: &[u8], hash: HashCertificado, assinatura: &[u8]) -> Vec<u8> {
    let mut bits = vec![0_u8];
    bits.extend_from_slice(assinatura);
    sequencia(&[tbs, &algoritmo(hash), &tlv(0x03, &bits)])
}

/// What can be read from a certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoCertificado {
    /// The version, as written: 3 is the only accepted one.
    pub versao: u8,
    /// The serial number, in hexadecimal.
    pub serial: String,
    /// The issuer's common name.
    pub emissor_cn: Option<String>,
    /// The issuer's organization.
    pub emissor_org: Option<String>,
    /// The holder's common name.
    pub titular_cn: Option<String>,
    /// Start of validity.
    pub valido_de: Option<Instante>,
    /// End of validity.
    pub valido_ate: Option<Instante>,
    /// Whether it is a certificate authority.
    pub eh_ac: bool,
    /// Whether the key may sign and not repudiate; `None` if the extension is absent.
    pub uso_assinatura: Option<bool>,
    /// Whether it declares client authentication; `None` if the extension is absent.
    pub autenticacao_cliente: Option<bool>,
    /// The CNPJ in `otherName` 2.16.76.1.3.3.
    pub cnpj: Option<String>,
    /// The CPF in `otherName` 2.16.76.1.3.1, from positions 9 to 19.
    pub cpf: Option<String>,
    /// The size of the RSA key, in bits.
    pub bits_chave: Option<usize>,
    /// The hash of the issuer's signature.
    pub hash: HashCertificado,
    tbs: Vec<u8>,
    assinatura: Vec<u8>,
    spki: Vec<u8>,
}

impl InfoCertificado {
    /// Reads a certificate in DER.
    #[must_use]
    pub fn ler(der: &[u8]) -> Option<Self> {
        let certificado = ler(der)?;
        let tbs_der = ler(certificado.content)?;
        let algoritmo_der = ler(tbs_der.rest)?;
        let assinatura_der = ler(algoritmo_der.rest)?;
        let hash = match ler(algoritmo_der.content).map(|o| o.content) {
            Some(tipo) if tipo == OID_SHA256_RSA => HashCertificado::Sha256,
            Some(tipo) if tipo == OID_SHA1_RSA => HashCertificado::Sha1,
            _ => HashCertificado::Outro,
        };
        let primeiro = ler(tbs_der.content)?;
        let (versao, serial_der) = if primeiro.tag == 0xa0 {
            let numero = ler(primeiro.content)?.content.first().copied()?;
            (numero + 1, ler(primeiro.rest)?)
        } else {
            (1, primeiro)
        };
        let serial: String = serial_der.content.iter().map(|b| format!("{b:02x}")).collect();
        let algoritmo_tbs = ler(serial_der.rest)?;
        let emissor = ler(algoritmo_tbs.rest)?;
        let validade = ler(emissor.rest)?;
        let titular = ler(validade.rest)?;
        let spki = ler(titular.rest)?;
        let de = ler(validade.content)?;
        let ate = ler(de.rest)?;
        let bits_chave = RsaPublicKey::from_public_key_der(spki.whole).ok().map(|chave| chave.n().bits());

        let mut info = Self {
            versao,
            serial,
            emissor_cn: atributo(emissor.content, OID_CN),
            emissor_org: atributo(emissor.content, OID_O),
            titular_cn: atributo(titular.content, OID_CN),
            valido_de: ler_tempo(&de),
            valido_ate: ler_tempo(&ate),
            eh_ac: false,
            uso_assinatura: None,
            autenticacao_cliente: None,
            cnpj: None,
            cpf: None,
            bits_chave,
            hash,
            tbs: tbs_der.whole.to_vec(),
            assinatura: assinatura_der.content.get(1..)?.to_vec(),
            spki: spki.whole.to_vec(),
        };
        let mut resto = spki.rest;
        while let Some(campo) = ler(resto) {
            if campo.tag == 0xa3 {
                info.ler_extensoes(campo.content);
            }
            resto = campo.rest;
        }
        Some(info)
    }

    fn ler_extensoes(&mut self, conteudo: &[u8]) {
        let Some(lista) = ler(conteudo) else { return };
        let mut resto = lista.content;
        while let Some(extensao) = ler(resto) {
            resto = extensao.rest;
            let Some(tipo) = ler(extensao.content) else { continue };
            let mut valor = ler(tipo.rest);
            if valor.as_ref().is_some_and(|v| v.tag == 0x01) {
                valor = valor.and_then(|v| ler(v.rest));
            }
            let Some(valor) = valor else { continue };
            if tipo.content == OID_BASIC {
                self.eh_ac = ler(valor.content)
                    .and_then(|s| ler(s.content))
                    .is_some_and(|b| b.tag == 0x01 && b.content.first() == Some(&0xff));
            } else if tipo.content == OID_KEY_USAGE {
                let bits = ler(valor.content).and_then(|b| b.content.get(1).copied()).unwrap_or(0);
                self.uso_assinatura = Some(bits & 0xc0 == 0xc0);
            } else if tipo.content == OID_EKU {
                self.autenticacao_cliente = Some(contem(valor.content, OID_CLIENT_AUTH));
            } else if tipo.content == OID_SAN {
                self.cnpj = outro_nome(valor.content, OID_ICP_CNPJ).filter(|texto| texto.len() == 14);
                self.cpf = outro_nome(valor.content, OID_ICP_CPF)
                    .and_then(|texto| texto.get(8..19).map(str::to_string));
            }
        }
    }

    /// Whether this certificate was signed by the holder of this public key.
    #[must_use]
    pub fn emitido_por(&self, spki_emissor: &[u8]) -> bool {
        let Ok(chave) = RsaPublicKey::from_public_key_der(spki_emissor) else {
            return false;
        };
        match self.hash {
            HashCertificado::Sha256 => {
                let digest = rsa::sha2::Sha256::digest(&self.tbs);
                chave.verify(Pkcs1v15Sign::new::<rsa::sha2::Sha256>(), &digest, &self.assinatura).is_ok()
            }
            HashCertificado::Sha1 => {
                let digest = Sha1::digest(&self.tbs);
                chave.verify(Pkcs1v15Sign::new::<Sha1>(), &digest, &self.assinatura).is_ok()
            }
            HashCertificado::Outro => false,
        }
    }

    /// The holder's public key, as `SubjectPublicKeyInfo`.
    #[must_use]
    pub fn spki(&self) -> &[u8] {
        &self.spki
    }

    /// The hexadecimal text of the serial number.
    #[must_use]
    pub fn serial_hex(&self) -> &str {
        &self.serial
    }
}

fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    palheiro.windows(agulha.len()).any(|janela| janela == agulha)
}

/// The value of an attribute in a name.
fn atributo(nome: &[u8], tipo: &[u8]) -> Option<String> {
    let mut resto = nome;
    while let Some(rdn) = ler(resto) {
        resto = rdn.rest;
        let par = ler(rdn.content)?;
        let chave = ler(par.content)?;
        if chave.content == tipo {
            return std::str::from_utf8(ler(chave.rest)?.content).ok().map(str::to_string);
        }
    }
    None
}

/// The value of an `otherName` with this type, in a `GeneralNames`.
fn outro_nome(san: &[u8], tipo: &[u8]) -> Option<String> {
    let lista = ler(san)?;
    let mut resto = lista.content;
    while let Some(nome) = ler(resto) {
        resto = nome.rest;
        if nome.tag != 0xa0 {
            continue;
        }
        let Some(chave) = ler(nome.content) else { continue };
        if chave.content != tipo {
            continue;
        }
        let envelope = ler(chave.rest)?;
        let valor = ler(envelope.content)?;
        return std::str::from_utf8(valor.content).ok().map(str::to_string);
    }
    None
}

/// Whether a certificate came from the laboratory, whichever test authority signed it.
#[must_use]
pub fn eh_do_laboratorio(der: &[u8]) -> bool {
    InfoCertificado::ler(der).is_some_and(|info| {
        let tem =
            |texto: &Option<String>| texto.as_deref().is_some_and(|nome| nome.contains("RAVENRS SANDBOX"));
        tem(&info.emissor_cn) || tem(&info.emissor_org) || tem(&info.titular_cn)
    })
}
