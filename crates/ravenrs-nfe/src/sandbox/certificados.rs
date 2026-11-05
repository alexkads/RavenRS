//! Laboratory certificates in the ICP-Brasil format, so production code runs unchanged here.
//!
//! Every certificate is issued by a test authority whose name says what it is:
//! `RAVENRS SANDBOX AC - SEM VALOR`. None of it has fiscal value, and it cannot leave the
//! laboratory: `Ambiente::Homologacao` and `HttpTransport` refuse a certificate from this
//! authority, and the sandbox refuses any other.
//!
//! # Format
//!
//! CN as `RAZAO:CNPJ`, and `otherName` entries 2.16.76.1.3.4 (data of the person responsible),
//! 2.16.76.1.3.2 (name of that person), 2.16.76.1.3.3 (CNPJ) and 2.16.76.1.3.7 (CEI). The MOC 7.0
//! confirms the CNPJ and CPF entries; the others are in `REGRAS.md` with their source and trust.
//!
//! # Keys
//!
//! The certificates are issued at run time, but [`AcTeste::padrao`] and the holders use RSA keys
//! bundled in `fixtures/laboratorio`, public on purpose, because generating RSA takes seconds per
//! key in a debug build. [`AcTeste::gerar`] and [`GeradorCertificado::com_chave_gerada`] really
//! generate one, from a seed, with no random source, so they work in WebAssembly.
//!
//! # A real token
//!
//! [`TokenSimulado`] stands behind [`Signer`], which is the same contract a real A3 will use.
//! A real token needs PKCS#11, which is foreign code, forbidden in this workspace. It will run
//! in a helper process, not implemented here, speaking JSON Lines on stdin and stdout:
//!
//! ```text
//! -> {"op":"slots"}                                  <- {"slots":[{"id":0,"rotulo":"..."}]}
//! -> {"op":"entrar","slot":0,"pin":"..."}            <- {"ok":true} | {"erro":"pin","restantes":2}
//! -> {"op":"certificado","slot":0}                   <- {"der_base64":"..."}
//! -> {"op":"assinar","hash":"sha1","dados_base64":"..."}  <- {"assinatura_base64":"..."}
//! -> {"op":"sair"}                                   <- {"ok":true}
//! ```
//!
//! The private key never crosses this pipe, and the PIN never reaches a log.

use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use rsa::pkcs8::{DecodePrivateKey, EncodePublicKey};
use rsa::rand_core::{CryptoRng, RngCore};
use rsa::{Pkcs1v15Sign, RsaPrivateKey};
use sha1::{Digest, Sha1};

use super::catalogo::{Regra, regra};
use super::certificado_x509::{
    self as x509, Eku, Especificacao, HashCertificado, InfoCertificado, OID_ICP_CEI, OID_ICP_CNPJ,
    OID_ICP_DADOS_RESPONSAVEL, OID_ICP_RESPONSAVEL,
};
use super::tempo::{Instante, RelogioControlado};
use crate::assinatura::{HashAssinatura, Signer, assinar_rsa};
use crate::error::NfeError;

/// The name of the test authority. It appears as issuer of everything issued here.
pub const NOME_AC: &str = "RAVENRS SANDBOX AC - SEM VALOR";
/// The organization of everything issued here.
pub const ORGANIZACAO: &str = "RAVENRS SANDBOX - SEM VALOR";
/// The CN of the certificate that fits any issuer in the sandbox, with no CNPJ.
pub const CN_CURINGA: &str = "RAVENRS SANDBOX - SEM VALOR";

const CHAVE_AC: &str = include_str!("../../fixtures/laboratorio/ac-key.pem");
const CHAVE_OUTRA_AC: &str = include_str!("../../fixtures/laboratorio/outra-ac-key.pem");
const CHAVE_TITULAR: &str = include_str!("../../fixtures/laboratorio/titular-key.pem");
const CHAVE_FRACA: &str = include_str!("../../fixtures/laboratorio/fraca-1024-key.pem");

/// The serial of the certificates in the fixed revocation list of the sandbox.
const SERIAL_REVOGADO: u64 = 0x0BAD_0000_0000_0001;

/// The chain in force: from 2020 to 2090, so the default certificate never expires in a test.
fn inicio_padrao() -> Instante {
    Instante::utc(2020, 1, 1, 0, 0, 0)
}

fn fim_padrao() -> Instante {
    Instante::utc(2090, 1, 1, 0, 0, 0)
}

fn erro_chave(motivo: impl fmt::Display) -> NfeError {
    NfeError::Assinatura(format!("chave de laboratório: {motivo}"))
}

fn ler_chave(pem: &str) -> Result<RsaPrivateKey, NfeError> {
    RsaPrivateKey::from_pkcs8_pem(pem).map_err(erro_chave)
}

fn spki_de(chave: &RsaPrivateKey) -> Result<Vec<u8>, NfeError> {
    let publica = chave.to_public_key();
    publica.to_public_key_der().map(|der| der.as_bytes().to_vec()).map_err(erro_chave)
}

/// A random stream from a seed: SHA-1 of a counter. Only for test keys, never for real ones.
struct Sementeira {
    semente: Vec<u8>,
    contador: u64,
    bloco: Vec<u8>,
}

impl Sementeira {
    fn nova(semente: &[u8]) -> Self {
        Self { semente: semente.to_vec(), contador: 0, bloco: Vec::new() }
    }
}

impl RngCore for Sementeira {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0; 4];
        self.fill_bytes(&mut bytes);
        u32::from_be_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0; 8];
        self.fill_bytes(&mut bytes);
        u64::from_be_bytes(bytes)
    }

    fn fill_bytes(&mut self, destino: &mut [u8]) {
        for byte in destino {
            if self.bloco.is_empty() {
                let mut hash = Sha1::new();
                hash.update(&self.semente);
                hash.update(self.contador.to_be_bytes());
                self.bloco = hash.finalize().to_vec();
                self.contador += 1;
            }
            *byte = self.bloco.remove(0);
        }
    }

    fn try_fill_bytes(&mut self, destino: &mut [u8]) -> Result<(), rsa::rand_core::Error> {
        self.fill_bytes(destino);
        Ok(())
    }
}

// Only a test stream, as the type's documentation says: `rsa` asks for this marker.
impl CryptoRng for Sementeira {}

/// Generates an RSA key from a seed. Takes seconds in a debug build.
fn gerar_chave(semente: &[u8], bits: usize) -> Result<RsaPrivateKey, NfeError> {
    RsaPrivateKey::new(&mut Sementeira::nova(semente), bits).map_err(erro_chave)
}

fn assinar_certificado(
    chave: &RsaPrivateKey,
    hash: HashCertificado,
    tbs: &[u8],
) -> Result<Vec<u8>, NfeError> {
    let assinatura = match hash {
        HashCertificado::Sha1 => chave.sign(Pkcs1v15Sign::new::<Sha1>(), &Sha1::digest(tbs)),
        _ => chave.sign(Pkcs1v15Sign::new::<rsa::sha2::Sha256>(), &rsa::sha2::Sha256::digest(tbs)),
    };
    assinatura.map_err(erro_chave)
}

/// A certificate authority of the laboratory.
#[derive(Clone)]
pub struct AcTeste {
    chave: RsaPrivateKey,
    spki: Vec<u8>,
    certificado: Vec<u8>,
}

impl fmt::Debug for AcTeste {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AcTeste").field("nome", &NOME_AC).finish_non_exhaustive()
    }
}

impl AcTeste {
    fn com_chave(chave: RsaPrivateKey) -> Result<Self, NfeError> {
        let spki = spki_de(&chave)?;
        let corpo = x509::tbs(&Especificacao {
            serial: 1,
            emissor_org: ORGANIZACAO,
            emissor_cn: NOME_AC,
            titular_org: ORGANIZACAO,
            titular_cn: NOME_AC,
            de: inicio_padrao(),
            ate: fim_padrao(),
            spki: &spki,
            eh_ac: true,
            uso_assinatura: false,
            eku: None,
            outros_nomes: &[],
            hash: HashCertificado::Sha256,
        });
        let assinatura = assinar_certificado(&chave, HashCertificado::Sha256, &corpo)?;
        let certificado = x509::montar(&corpo, HashCertificado::Sha256, &assinatura);
        Ok(Self { chave, spki, certificado })
    }

    /// The authority the sandbox trusts by default, issued now over the bundled key.
    pub fn padrao() -> Result<Self, NfeError> {
        static PADRAO: OnceLock<Result<AcTeste, NfeError>> = OnceLock::new();
        PADRAO.get_or_init(|| ler_chave(CHAVE_AC).and_then(Self::com_chave)).clone()
    }

    /// A new authority, with a key really generated from this seed. A sandbox only trusts it
    /// after [`super::SefazSandbox::confiar_em`].
    pub fn gerar(semente: &[u8]) -> Result<Self, NfeError> {
        Self::com_chave(gerar_chave(semente, 2048)?)
    }

    /// The certificate of the authority, in DER.
    #[must_use]
    pub fn certificado_der(&self) -> &[u8] {
        &self.certificado
    }

    /// The public key of the authority, as `SubjectPublicKeyInfo`.
    #[must_use]
    pub fn spki(&self) -> &[u8] {
        &self.spki
    }
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

/// Whether a certificate names ICP-Brasil as its issuer, as a real one does.
#[must_use]
pub fn cita_icp_brasil(der: &[u8]) -> bool {
    InfoCertificado::ler(der).is_some_and(|info| {
        let tem = |texto: &Option<String>| texto.as_deref().is_some_and(|nome| nome.contains("ICP-Brasil"));
        tem(&info.emissor_org) || tem(&info.emissor_cn)
    })
}

/// The fixed revocation list of the sandbox, as serial numbers in hexadecimal.
#[must_use]
pub fn serial_revogado_padrao() -> String {
    let mut bytes = vec![0x01];
    bytes.extend_from_slice(&SERIAL_REVOGADO.to_be_bytes());
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A failure a certificate can be built with, each matched with the rule it should trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CasoFalha {
    /// The validity already ended.
    Vencido,
    /// The validity has not begun.
    AindaNaoValido,
    /// The CNPJ is not the issuer's.
    CnpjDiferente,
    /// The authority named did not sign it.
    CadeiaQuebrada,
    /// It is in the revocation list of the sandbox.
    Revogado,
    /// It does not allow client authentication.
    EkuErrado,
    /// It does not allow signing.
    UsoDaChaveErrado,
    /// It has a 1024-bit key.
    ChaveFraca,
    /// The authority signed it with SHA-1.
    Sha1NoCertificado,
    /// It has no CNPJ or CPF.
    SemCnpj,
    /// The token was given a wrong PIN.
    PinErrado,
}

impl CasoFalha {
    /// All the cases.
    pub const TODOS: [Self; 11] = [
        Self::Vencido,
        Self::AindaNaoValido,
        Self::CnpjDiferente,
        Self::CadeiaQuebrada,
        Self::Revogado,
        Self::EkuErrado,
        Self::UsoDaChaveErrado,
        Self::ChaveFraca,
        Self::Sha1NoCertificado,
        Self::SemCnpj,
        Self::PinErrado,
    ];

    /// The id of the catalog rule this case should trigger.
    #[must_use]
    pub const fn id_regra(self) -> &'static str {
        match self {
            Self::Vencido | Self::AindaNaoValido => "E02",
            Self::CnpjDiferente => "F03",
            Self::CadeiaQuebrada => "E04",
            Self::Revogado => "E06",
            Self::EkuErrado => "A01",
            Self::UsoDaChaveErrado => "E01",
            Self::ChaveFraca => "CERT-CHAVE-FRACA",
            Self::Sha1NoCertificado => "CERT-SHA1",
            Self::SemCnpj => "E03",
            Self::PinErrado => "LOCAL-PIN",
        }
    }

    /// The catalog rule, with its code, source, trust and how to fix it.
    #[must_use]
    pub fn regra(self) -> Option<&'static Regra> {
        regra(self.id_regra())
    }
}

/// Issues test e-CNPJ certificates in the ICP-Brasil format.
#[derive(Clone, Debug)]
pub struct GeradorCertificado {
    razao: String,
    cnpj: String,
    chave_titular: Option<RsaPrivateKey>,
}

impl GeradorCertificado {
    /// A generator for this company.
    #[must_use]
    pub fn novo(razao_social: impl Into<String>, cnpj: impl Into<String>) -> Self {
        Self { razao: razao_social.into(), cnpj: cnpj.into(), chave_titular: None }
    }

    /// Really generates the holder's key from this seed, instead of the bundled one.
    pub fn com_chave_gerada(mut self, semente: &[u8]) -> Result<Self, NfeError> {
        self.chave_titular = Some(gerar_chave(semente, 2048)?);
        Ok(self)
    }

    /// A valid certificate.
    pub fn emitir(&self) -> Result<CertificadoGerado, NfeError> {
        self.construir(None)
    }

    /// A certificate with this failure built in. `PinErrado` gives a valid certificate, since the
    /// failure there comes from the token.
    pub fn com_falha(&self, caso: CasoFalha) -> Result<CertificadoGerado, NfeError> {
        self.construir(Some(caso))
    }

    /// Already expired.
    pub fn vencido(&self) -> Result<CertificadoGerado, NfeError> {
        self.com_falha(CasoFalha::Vencido)
    }

    /// Not valid yet.
    pub fn ainda_nao_valido(&self) -> Result<CertificadoGerado, NfeError> {
        self.com_falha(CasoFalha::AindaNaoValido)
    }

    /// With another company's CNPJ.
    pub fn cnpj_diferente(&self) -> Result<CertificadoGerado, NfeError> {
        self.com_falha(CasoFalha::CnpjDiferente)
    }

    /// Signed by a key other than the authority's it names.
    pub fn cadeia_quebrada(&self) -> Result<CertificadoGerado, NfeError> {
        self.com_falha(CasoFalha::CadeiaQuebrada)
    }

    /// In the revocation list of the sandbox.
    pub fn revogado(&self) -> Result<CertificadoGerado, NfeError> {
        self.com_falha(CasoFalha::Revogado)
    }

    /// Without client authentication.
    pub fn eku_errado(&self) -> Result<CertificadoGerado, NfeError> {
        self.com_falha(CasoFalha::EkuErrado)
    }

    /// With a 1024-bit key.
    pub fn chave_fraca(&self) -> Result<CertificadoGerado, NfeError> {
        self.com_falha(CasoFalha::ChaveFraca)
    }

    /// Signed by the authority with SHA-1.
    pub fn sha1_no_certificado(&self) -> Result<CertificadoGerado, NfeError> {
        self.com_falha(CasoFalha::Sha1NoCertificado)
    }

    /// A valid certificate in a token whose PIN is `pin`, for trying a wrong one.
    pub fn para_pin_errado(&self, pin: &str) -> Result<TokenSimulado, NfeError> {
        Ok(TokenSimulado::novo(&self.com_falha(CasoFalha::PinErrado)?, pin))
    }

    fn construir(&self, caso: Option<CasoFalha>) -> Result<CertificadoGerado, NfeError> {
        let ac = AcTeste::padrao()?;
        let titular = match (caso, &self.chave_titular) {
            (Some(CasoFalha::ChaveFraca), _) => ler_chave(CHAVE_FRACA)?,
            (_, Some(chave)) => chave.clone(),
            _ => ler_chave(CHAVE_TITULAR)?,
        };
        let spki = spki_de(&titular)?;
        let (de, ate) = match caso {
            Some(CasoFalha::Vencido) => {
                (Instante::utc(2010, 1, 1, 0, 0, 0), Instante::utc(2011, 1, 1, 0, 0, 0))
            }
            Some(CasoFalha::AindaNaoValido) => (Instante::utc(2080, 1, 1, 0, 0, 0), fim_padrao()),
            _ => (inicio_padrao(), fim_padrao()),
        };
        let cnpj = if caso == Some(CasoFalha::CnpjDiferente) {
            // Another company's CNPJ.
            "99888777000170".to_string()
        } else {
            self.cnpj.clone()
        };
        let comum = format!("{}:{}", self.razao, cnpj);
        let dados_responsavel = format!("01011980{:0>11}{:0>11}{:0>15}", "11144477735", "0", "0");
        let outros: Vec<(&[u8], String)> = if caso == Some(CasoFalha::SemCnpj) {
            Vec::new()
        } else {
            vec![
                (OID_ICP_DADOS_RESPONSAVEL, dados_responsavel),
                (OID_ICP_RESPONSAVEL, "RESPONSAVEL DE TESTE SEM VALOR".to_string()),
                (OID_ICP_CNPJ, cnpj),
                (OID_ICP_CEI, "000000000000".to_string()),
            ]
        };
        let hash = if caso == Some(CasoFalha::Sha1NoCertificado) {
            HashCertificado::Sha1
        } else {
            HashCertificado::Sha256
        };
        let corpo = x509::tbs(&Especificacao {
            serial: if caso == Some(CasoFalha::Revogado) { SERIAL_REVOGADO } else { 2 },
            emissor_org: ORGANIZACAO,
            emissor_cn: NOME_AC,
            titular_org: ORGANIZACAO,
            titular_cn: &comum,
            de,
            ate,
            spki: &spki,
            eh_ac: false,
            uso_assinatura: caso != Some(CasoFalha::UsoDaChaveErrado),
            eku: Some(if caso == Some(CasoFalha::EkuErrado) { Eku::SoEmail } else { Eku::ClientAuth }),
            outros_nomes: &outros,
            hash,
        });
        let assinante = if caso == Some(CasoFalha::CadeiaQuebrada) {
            ler_chave(CHAVE_OUTRA_AC)?
        } else {
            ac.chave.clone()
        };
        let assinatura = assinar_certificado(&assinante, hash, &corpo)?;
        let der = x509::montar(&corpo, hash, &assinatura);
        Ok(CertificadoGerado { der, cadeia_ac: ac.certificado.clone(), chave: titular, caso })
    }
}

/// The certificate of the sandbox that fits any issuer: no CNPJ, and known by name.
pub(crate) fn certificado_curinga() -> Result<CertificadoGerado, NfeError> {
    static CURINGA: OnceLock<Result<CertificadoGerado, NfeError>> = OnceLock::new();
    CURINGA
        .get_or_init(|| {
            let ac = AcTeste::padrao()?;
            let titular = ler_chave(CHAVE_TITULAR)?;
            let spki = spki_de(&titular)?;
            let corpo = x509::tbs(&Especificacao {
                serial: 3,
                emissor_org: ORGANIZACAO,
                emissor_cn: NOME_AC,
                titular_org: ORGANIZACAO,
                titular_cn: CN_CURINGA,
                de: inicio_padrao(),
                ate: fim_padrao(),
                spki: &spki,
                eh_ac: false,
                uso_assinatura: true,
                eku: Some(Eku::ClientAuth),
                outros_nomes: &[],
                hash: HashCertificado::Sha256,
            });
            let assinatura = assinar_certificado(&ac.chave, HashCertificado::Sha256, &corpo)?;
            let der = x509::montar(&corpo, HashCertificado::Sha256, &assinatura);
            Ok(CertificadoGerado { der, cadeia_ac: ac.certificado.clone(), chave: titular, caso: None })
        })
        .clone()
}

/// An issued certificate, with its key: the A1 of the laboratory, and a [`Signer`].
#[derive(Clone)]
pub struct CertificadoGerado {
    der: Vec<u8>,
    cadeia_ac: Vec<u8>,
    chave: RsaPrivateKey,
    caso: Option<CasoFalha>,
}

impl fmt::Debug for CertificadoGerado {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CertificadoGerado")
            .field("caso", &self.caso)
            .field("chave", &"<oculta>")
            .finish_non_exhaustive()
    }
}

impl CertificadoGerado {
    /// The certificate, in DER.
    #[must_use]
    pub fn der(&self) -> &[u8] {
        &self.der
    }

    /// The certificate of the authority, in DER.
    #[must_use]
    pub fn cadeia(&self) -> &[u8] {
        &self.cadeia_ac
    }

    /// The failure built in, if any.
    #[must_use]
    pub fn caso(&self) -> Option<CasoFalha> {
        self.caso
    }

    /// The certificate as PEM text.
    #[must_use]
    pub fn pem(&self) -> String {
        pem_certificado(&self.der)
    }

    /// A real PFX with this password, which `CertificadoA1::from_pfx` and `openssl pkcs12` open.
    #[cfg(all(feature = "http", not(target_arch = "wasm32")))]
    pub fn pfx(&self, senha: &crate::http::SenhaA1) -> Result<Vec<u8>, NfeError> {
        use p12_keystore::{Certificate, KeyStore, KeyStoreEntry, PrivateKey, PrivateKeyChain};
        use rsa::pkcs8::EncodePrivateKey;
        let erro = |motivo: String| NfeError::Assinatura(format!("não foi possível gerar o PFX: {motivo}"));
        let chave_der = self.chave.to_pkcs8_der().map_err(|e| erro(e.to_string()))?;
        let chave = PrivateKey::from_der(chave_der.as_bytes()).map_err(|e| erro(e.to_string()))?;
        let certificado = Certificate::from_der(&self.der).map_err(|e| erro(e.to_string()))?;
        let ac = Certificate::from_der(&self.cadeia_ac).map_err(|e| erro(e.to_string()))?;
        let identificador = Sha1::digest(&self.der).to_vec();
        let cadeia = PrivateKeyChain::new(identificador, chave, [certificado, ac]);
        let mut cofre = KeyStore::new();
        cofre.add_entry("sandbox", KeyStoreEntry::PrivateKeyChain(cadeia));
        cofre.writer(senha.expor()).write().map_err(|e| erro(e.to_string()))
    }

    /// Moves the certificate into a simulated token protected by this PIN.
    #[must_use]
    pub fn em_token(&self, pin: &str) -> TokenSimulado {
        TokenSimulado::novo(self, pin)
    }
}

impl Signer for CertificadoGerado {
    fn certificate_der(&self) -> &[u8] {
        &self.der
    }

    fn sign_rsa_sha1(&self, message: &[u8]) -> Result<Vec<u8>, NfeError> {
        assinar_rsa(&self.chave, HashAssinatura::Sha1, message)
    }

    fn sign_rsa(&self, hash: HashAssinatura, message: &[u8]) -> Result<Vec<u8>, NfeError> {
        assinar_rsa(&self.chave, hash, message)
    }
}

/// Writes a certificate as PEM text.
#[must_use]
pub fn pem_certificado(der: &[u8]) -> String {
    use base64::Engine;
    let texto = base64::engine::general_purpose::STANDARD.encode(der);
    let mut saida = String::from("-----BEGIN CERTIFICATE-----\n");
    for pedaco in texto.as_bytes().chunks(64) {
        saida.push_str(&String::from_utf8_lossy(pedaco));
        saida.push('\n');
    }
    saida.push_str("-----END CERTIFICATE-----\n");
    saida
}

/// Why a token refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroToken {
    /// The PIN is wrong; this many attempts remain.
    PinErrado {
        /// Attempts left before it locks.
        restantes: u32,
    },
    /// Too many wrong attempts.
    Bloqueado,
    /// Nobody entered the PIN yet.
    SemSessao,
    /// No slot with this number.
    SlotInexistente,
}

impl fmt::Display for ErroToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PinErrado { restantes } => {
                write!(f, "PIN incorreto; restam {restantes} tentativas antes do bloqueio")
            }
            Self::Bloqueado => write!(f, "token bloqueado por tentativas erradas de PIN"),
            Self::SemSessao => write!(f, "informe o PIN do token antes de assinar"),
            Self::SlotInexistente => write!(f, "o token não tem este slot"),
        }
    }
}

impl std::error::Error for ErroToken {}

/// What a slot of a token shows, with no secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotInfo {
    /// The slot number.
    pub id: u32,
    /// The certificate's common name.
    pub rotulo: String,
}

struct EstadoToken {
    aberto: bool,
    bloqueado: bool,
}

struct InternoToken {
    slot: u32,
    pin: String,
    maximo: u32,
    erros: AtomicU32,
    estado: Mutex<EstadoToken>,
    der: Vec<u8>,
    chave: RsaPrivateKey,
    latencia: Duration,
    relogio: Option<RelogioControlado>,
    assinaturas: AtomicU32,
}

/// An A3 simulated in memory: slot, PIN, lock after wrong attempts, and a key that never leaves.
///
/// Cloning gives another handle to the same token. Nothing here returns the key, and `Debug` hides
/// the PIN and the key.
#[derive(Clone)]
pub struct TokenSimulado(Arc<InternoToken>);

impl fmt::Debug for TokenSimulado {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenSimulado")
            .field("slot", &self.0.slot)
            .field("pin", &"<oculto>")
            .field("chave", &"<fica no token>")
            .finish()
    }
}

impl TokenSimulado {
    /// A token in slot 0 holding this certificate, which locks after 3 wrong attempts.
    #[must_use]
    pub fn novo(certificado: &CertificadoGerado, pin: &str) -> Self {
        Self::construir(certificado, pin, 0, 3, Duration::ZERO, None)
    }

    fn construir(
        certificado: &CertificadoGerado,
        pin: &str,
        slot: u32,
        maximo: u32,
        latencia: Duration,
        relogio: Option<RelogioControlado>,
    ) -> Self {
        Self(Arc::new(InternoToken {
            slot,
            pin: pin.to_string(),
            maximo,
            erros: AtomicU32::new(0),
            estado: Mutex::new(EstadoToken { aberto: false, bloqueado: false }),
            der: certificado.der.clone(),
            chave: certificado.chave.clone(),
            latencia,
            relogio,
            assinaturas: AtomicU32::new(0),
        }))
    }

    /// The same certificate in a token with other slot, attempts and response time.
    ///
    /// With a controlled clock the response time moves it; without one the call really waits,
    /// and in WebAssembly, which cannot wait, the time is ignored.
    #[must_use]
    pub fn configurado(
        certificado: &CertificadoGerado,
        pin: &str,
        slot: u32,
        tentativas: u32,
        tempo_de_resposta: Duration,
        relogio: Option<RelogioControlado>,
    ) -> Self {
        Self::construir(certificado, pin, slot, tentativas, tempo_de_resposta, relogio)
    }

    /// The slots with a certificate.
    #[must_use]
    pub fn slots(&self) -> Vec<SlotInfo> {
        let rotulo = InfoCertificado::ler(&self.0.der).and_then(|info| info.titular_cn).unwrap_or_default();
        vec![SlotInfo { id: self.0.slot, rotulo }]
    }

    /// Enters the PIN in a slot.
    pub fn entrar(&self, slot: u32, pin: &str) -> Result<(), ErroToken> {
        if slot != self.0.slot {
            return Err(ErroToken::SlotInexistente);
        }
        let mut estado = self.0.estado.lock().unwrap_or_else(PoisonError::into_inner);
        if estado.bloqueado {
            return Err(ErroToken::Bloqueado);
        }
        if pin == self.0.pin {
            self.0.erros.store(0, Ordering::SeqCst);
            estado.aberto = true;
            return Ok(());
        }
        estado.aberto = false;
        let erros = self.0.erros.fetch_add(1, Ordering::SeqCst) + 1;
        if erros >= self.0.maximo {
            estado.bloqueado = true;
            return Err(ErroToken::Bloqueado);
        }
        Err(ErroToken::PinErrado { restantes: self.0.maximo - erros })
    }

    /// Closes the session; the next signature asks for the PIN again.
    pub fn sair(&self) {
        self.0.estado.lock().unwrap_or_else(PoisonError::into_inner).aberto = false;
    }

    /// Whether wrong attempts locked the token.
    #[must_use]
    pub fn bloqueado(&self) -> bool {
        self.0.estado.lock().unwrap_or_else(PoisonError::into_inner).bloqueado
    }

    /// How many signatures this token made.
    #[must_use]
    pub fn assinaturas_feitas(&self) -> u32 {
        self.0.assinaturas.load(Ordering::SeqCst)
    }

    fn esperar(&self) {
        if self.0.latencia.is_zero() {
            return;
        }
        if let Some(relogio) = &self.0.relogio {
            relogio.avancar(self.0.latencia);
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::sleep(self.0.latencia);
    }
}

impl Signer for TokenSimulado {
    fn certificate_der(&self) -> &[u8] {
        &self.0.der
    }

    fn sign_rsa_sha1(&self, message: &[u8]) -> Result<Vec<u8>, NfeError> {
        self.sign_rsa(HashAssinatura::Sha1, message)
    }

    fn sign_rsa(&self, hash: HashAssinatura, message: &[u8]) -> Result<Vec<u8>, NfeError> {
        {
            let estado = self.0.estado.lock().unwrap_or_else(PoisonError::into_inner);
            if estado.bloqueado {
                return Err(NfeError::Assinatura(ErroToken::Bloqueado.to_string()));
            }
            if !estado.aberto {
                return Err(NfeError::Assinatura(ErroToken::SemSessao.to_string()));
            }
        }
        self.esperar();
        self.0.assinaturas.fetch_add(1, Ordering::SeqCst);
        assinar_rsa(&self.0.chave, hash, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gerador() -> GeradorCertificado {
        GeradorCertificado::novo("MERCADO DE TESTE LTDA", "11222333000181")
    }

    #[test]
    fn valid_certificate_has_the_icp_brasil_format_and_a_test_issuer() {
        let certificado = gerador().emitir().expect("issued");
        let info = InfoCertificado::ler(certificado.der()).expect("readable");
        assert_eq!(info.versao, 3);
        assert_eq!(info.emissor_cn.as_deref(), Some(NOME_AC));
        assert_eq!(info.titular_cn.as_deref(), Some("MERCADO DE TESTE LTDA:11222333000181"));
        assert_eq!(info.cnpj.as_deref(), Some("11222333000181"));
        assert_eq!(info.uso_assinatura, Some(true));
        assert_eq!(info.autenticacao_cliente, Some(true));
        assert!(!info.eh_ac);
        assert_eq!(info.bits_chave, Some(2048));
        assert_eq!(info.hash, HashCertificado::Sha256);
        assert!(info.emitido_por(AcTeste::padrao().expect("ac").spki()));
        assert!(eh_do_laboratorio(certificado.der()));
        assert!(!cita_icp_brasil(certificado.der()));
    }

    #[test]
    fn authority_is_a_ca_signed_by_itself() {
        let ac = AcTeste::padrao().expect("ac");
        let info = InfoCertificado::ler(ac.certificado_der()).expect("readable");
        assert!(info.eh_ac);
        assert!(info.emitido_por(ac.spki()));
    }

    #[test]
    fn each_failure_is_really_in_the_certificate() {
        let g = gerador();
        let ler = |c: CertificadoGerado| InfoCertificado::ler(c.der()).expect("readable");
        let ac = AcTeste::padrao().expect("ac");
        assert!(ler(g.vencido().expect("c")).valido_ate < Some(Instante::utc(2012, 1, 1, 0, 0, 0)));
        assert!(ler(g.ainda_nao_valido().expect("c")).valido_de > Some(Instante::utc(2079, 1, 1, 0, 0, 0)));
        assert_eq!(ler(g.cnpj_diferente().expect("c")).cnpj.as_deref(), Some("99888777000170"));
        assert!(!ler(g.cadeia_quebrada().expect("c")).emitido_por(ac.spki()));
        assert_eq!(ler(g.revogado().expect("c")).serial_hex(), serial_revogado_padrao());
        assert_eq!(ler(g.eku_errado().expect("c")).autenticacao_cliente, Some(false));
        assert_eq!(ler(g.chave_fraca().expect("c")).bits_chave, Some(1024));
        let sha1 = ler(g.sha1_no_certificado().expect("c"));
        assert_eq!(sha1.hash, HashCertificado::Sha1);
        assert!(sha1.emitido_por(ac.spki()));
    }

    #[test]
    fn every_case_points_at_a_rule_of_the_catalog() {
        for caso in CasoFalha::TODOS {
            let regra = caso.regra().expect("rule");
            assert!(!regra.como_corrigir.is_empty(), "{caso:?}");
        }
        assert_eq!(CasoFalha::CnpjDiferente.regra().expect("rule").c_stat, 213);
        assert_eq!(CasoFalha::Vencido.regra().expect("rule").c_stat, 291);
        assert_eq!(CasoFalha::Revogado.regra().expect("rule").c_stat, 294);
        assert_eq!(CasoFalha::PinErrado.regra().expect("rule").c_stat, 0);
    }

    #[test]
    fn token_locks_and_never_shows_its_secrets() {
        let token = gerador().para_pin_errado("1234").expect("token");
        assert!(matches!(
            token.sign_rsa_sha1(b"x"),
            Err(NfeError::Assinatura(texto)) if texto.contains("informe o PIN")
        ));
        assert_eq!(token.entrar(0, "0000"), Err(ErroToken::PinErrado { restantes: 2 }));
        assert_eq!(token.entrar(7, "1234"), Err(ErroToken::SlotInexistente));
        assert_eq!(token.entrar(0, "0000"), Err(ErroToken::PinErrado { restantes: 1 }));
        assert_eq!(token.entrar(0, "1234"), Ok(()));
        assert_eq!(token.sign_rsa_sha1(b"x").expect("signed").len(), 256);
        token.sair();
        for _ in 0..2 {
            let _ = token.entrar(0, "0000");
        }
        assert_eq!(token.entrar(0, "0000"), Err(ErroToken::Bloqueado));
        assert!(token.bloqueado());
        assert_eq!(token.entrar(0, "1234"), Err(ErroToken::Bloqueado));
        assert!(token.sign_rsa_sha1(b"x").is_err());
        let texto = format!("{token:?}");
        assert!(!texto.contains("1234") && texto.contains("<oculto>"));
    }

    #[test]
    fn token_response_time_moves_a_controlled_clock() {
        let relogio = RelogioControlado::em(Instante(0));
        let certificado = gerador().emitir().expect("issued");
        let token = TokenSimulado::configurado(
            &certificado,
            "1234",
            2,
            3,
            Duration::from_secs(2),
            Some(relogio.clone()),
        );
        assert_eq!(token.slots()[0].id, 2);
        token.entrar(2, "1234").expect("pin");
        token.sign_rsa(HashAssinatura::Sha256, b"x").expect("signed");
        assert_eq!(relogio.agora(), Instante(2));
        assert_eq!(token.assinaturas_feitas(), 1);
    }

    #[test]
    fn signer_can_do_sha1_and_sha256() {
        let certificado = gerador().emitir().expect("issued");
        let sha1 = certificado.sign_rsa(HashAssinatura::Sha1, b"x").expect("sha1");
        let sha256 = certificado.sign_rsa(HashAssinatura::Sha256, b"x").expect("sha256");
        assert_ne!(sha1, sha256);
        assert_eq!(certificado.sign_rsa_sha1(b"x").expect("sha1"), sha1);
    }

    #[test]
    #[ignore = "gera chaves RSA de verdade: segundos em build de depuração"]
    fn really_generated_keys_are_deterministic_and_valid() {
        let ac = AcTeste::gerar(b"semente").expect("ac");
        assert_eq!(ac.spki(), AcTeste::gerar(b"semente").expect("ac").spki());
        let gerado = gerador().com_chave_gerada(b"outra").expect("key").emitir().expect("issued");
        assert_eq!(InfoCertificado::ler(gerado.der()).expect("read").bits_chave, Some(2048));
    }
}
