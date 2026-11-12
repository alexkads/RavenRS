//! Signed lists of drivers. The signature covers the exact bytes of the contents, which travel as a
//! string inside the file, so nothing has to be put in a canonical form to check it.

use std::fmt;

use ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::data::data_valida;
use crate::manifesto::{ErroManifesto, Manifesto, TAMANHO_MAXIMO_MANIFESTO};
use crate::registro::Registro;
use crate::transporte::ErroTransporte;

/// The largest catalog accepted, in bytes.
const TAMANHO_MAXIMO_CATALOGO: usize = 4 * 1024 * 1024;
const MAXIMO_DRIVERS: usize = 2000;
const MAXIMO_NOTAS: usize = 2000;
const FORMATO: u32 = 1;

/// The key that checks catalogs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChavePublica(VerifyingKey);

impl ChavePublica {
    /// A key from its 32 bytes in hexadecimal.
    pub fn de_hex(texto: &str) -> Option<Self> {
        let bytes: [u8; 32] = de_hex(texto.trim())?.try_into().ok()?;
        VerifyingKey::from_bytes(&bytes).ok().map(Self)
    }

    /// The key in hexadecimal.
    pub fn para_hex(&self) -> String {
        para_hex(self.0.as_bytes())
    }
}

/// The key built into the program, from `RAVENRS_PERIFERICOS_CHAVE` when it was compiled. Without
/// one there is no way to accept a catalog until the application gives its own key.
pub fn chave_embutida() -> Option<ChavePublica> {
    ChavePublica::de_hex(option_env!("RAVENRS_PERIFERICOS_CHAVE")?)
}

/// The public key of a private one.
pub fn chave_publica_de(chave_privada: &[u8; 32]) -> ChavePublica {
    ChavePublica(SigningKey::from_bytes(chave_privada).verifying_key())
}

fn para_hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut texto, byte| {
        texto.push_str(&format!("{byte:02x}"));
        texto
    })
}

fn de_hex(texto: &str) -> Option<Vec<u8>> {
    if !texto.is_ascii() || !texto.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(texto.len() / 2);
    for indice in (0..texto.len()).step_by(2) {
        bytes.push(u8::from_str_radix(texto.get(indice..indice + 2)?, 16).ok()?);
    }
    Some(bytes)
}

/// Why a catalog was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroCatalogo {
    /// It could not be fetched.
    Transporte(ErroTransporte),
    /// It is larger than allowed.
    Grande {
        /// Its size in bytes.
        tamanho: usize,
    },
    /// It is not in the expected shape. When this comes after the signature, whoever signed it erred.
    Formato(String),
    /// The signature does not match the contents and the key.
    AssinaturaInvalida,
    /// Its validity has passed.
    Vencido {
        /// Until when it was valid.
        validade: String,
        /// The date it was checked on.
        hoje: String,
    },
    /// Its sequence is below one already seen: someone may be serving an old catalog on purpose.
    SequenciaAntiga {
        /// This catalog's sequence.
        recebida: u64,
        /// The highest seen.
        ultima: u64,
    },
    /// One of its drivers is not valid.
    ManifestoInvalido {
        /// Its position in the list.
        indice: usize,
        /// Why.
        erro: ErroManifesto,
    },
}

impl fmt::Display for ErroCatalogo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transporte(erro) => write!(f, "catálogo de drivers não veio: {erro}"),
            Self::Grande { tamanho } => {
                write!(f, "catálogo com {tamanho} bytes; o limite é {TAMANHO_MAXIMO_CATALOGO}")
            }
            Self::Formato(motivo) => write!(f, "catálogo mal formado: {motivo}"),
            Self::AssinaturaInvalida => {
                write!(f, "a assinatura do catálogo não confere; ele foi recusado")
            }
            Self::Vencido { validade, hoje } => {
                write!(f, "catálogo vencido em {validade}; hoje é {hoje}")
            }
            Self::SequenciaAntiga { recebida, ultima } => write!(
                f,
                "catálogo número {recebida} é mais antigo que o {ultima}, já visto; ele foi recusado"
            ),
            Self::ManifestoInvalido { indice, erro } => write!(f, "driver {indice} do catálogo: {erro}"),
        }
    }
}

impl std::error::Error for ErroCatalogo {}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    conteudo: String,
    assinatura: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Conteudo {
    formato: u32,
    sequencia: u64,
    validade: String,
    drivers: Vec<ItemBruto>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemBruto {
    manifesto: serde_json::Value,
    #[serde(default)]
    notas: String,
}

/// A driver a catalog offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemCatalogo {
    /// The driver, already checked.
    pub manifesto: Manifesto,
    /// What changed in this version.
    pub notas: String,
}

/// A catalog whose signature, validity, sequence and drivers were all checked. There is no other way
/// to get one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalogo {
    sequencia: u64,
    validade: String,
    itens: Vec<ItemCatalogo>,
}

impl Catalogo {
    /// Checks a catalog file: size, signature, shape, validity, sequence and each driver, in this order.
    /// `hoje` is today as `YYYY-MM-DD`, and the catalog is good through its validity date.
    pub fn abrir(
        bytes: &[u8],
        chave: &ChavePublica,
        hoje: &str,
        ultima_sequencia: u64,
    ) -> Result<Self, ErroCatalogo> {
        if bytes.len() > TAMANHO_MAXIMO_CATALOGO {
            return Err(ErroCatalogo::Grande { tamanho: bytes.len() });
        }
        let formato = |erro: serde_json::Error| ErroCatalogo::Formato(erro.to_string());
        let envelope: Envelope = serde_json::from_slice(bytes).map_err(formato)?;
        let assinatura: [u8; 64] = de_hex(&envelope.assinatura)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(ErroCatalogo::AssinaturaInvalida)?;
        chave
            .0
            .verify_strict(envelope.conteudo.as_bytes(), &Signature::from_bytes(&assinatura))
            .map_err(|_| ErroCatalogo::AssinaturaInvalida)?;

        let conteudo: Conteudo = serde_json::from_str(&envelope.conteudo).map_err(formato)?;
        if conteudo.formato != FORMATO {
            return Err(ErroCatalogo::Formato(format!("formato {} desconhecido", conteudo.formato)));
        }
        if !data_valida(&conteudo.validade) || !data_valida(hoje) {
            return Err(ErroCatalogo::Formato(format!(
                "validade \"{}\" ou data de hoje \"{hoje}\" não é AAAA-MM-DD",
                conteudo.validade
            )));
        }
        if hoje > conteudo.validade.as_str() {
            return Err(ErroCatalogo::Vencido { validade: conteudo.validade, hoje: hoje.to_owned() });
        }
        if conteudo.sequencia < ultima_sequencia {
            return Err(ErroCatalogo::SequenciaAntiga {
                recebida: conteudo.sequencia,
                ultima: ultima_sequencia,
            });
        }
        if conteudo.drivers.len() > MAXIMO_DRIVERS {
            return Err(ErroCatalogo::Formato(format!("mais de {MAXIMO_DRIVERS} drivers")));
        }
        let mut itens = Vec::with_capacity(conteudo.drivers.len());
        for (indice, item) in conteudo.drivers.into_iter().enumerate() {
            let invalido = |erro| ErroCatalogo::ManifestoInvalido { indice, erro };
            let manifesto = abrir_manifesto(item.manifesto).map_err(invalido)?;
            if item.notas.chars().count() > MAXIMO_NOTAS {
                return Err(invalido(ErroManifesto::Campo {
                    campo: "notas",
                    motivo: format!("passa de {MAXIMO_NOTAS} caracteres"),
                }));
            }
            itens.push(ItemCatalogo { manifesto, notas: item.notas });
        }
        Ok(Self { sequencia: conteudo.sequencia, validade: conteudo.validade, itens })
    }

    /// Its sequence number.
    pub fn sequencia(&self) -> u64 {
        self.sequencia
    }

    /// The last day it is good for.
    pub fn validade(&self) -> &str {
        &self.validade
    }

    /// What it offers.
    pub fn itens(&self) -> &[ItemCatalogo] {
        &self.itens
    }
}

fn abrir_manifesto(valor: serde_json::Value) -> Result<Manifesto, ErroManifesto> {
    let tamanho = valor.to_string().len();
    if tamanho > TAMANHO_MAXIMO_MANIFESTO {
        return Err(ErroManifesto::Grande { tamanho });
    }
    let manifesto: Manifesto =
        serde_json::from_value(valor).map_err(|erro| ErroManifesto::Formato(erro.to_string()))?;
    manifesto.validar()?;
    Ok(manifesto)
}

/// Wraps the contents of a catalog with its signature, as the file to publish.
pub fn assinar(conteudo: &str, chave_privada: &[u8; 32]) -> Result<String, ErroCatalogo> {
    let assinatura = SigningKey::from_bytes(chave_privada).sign(conteudo.as_bytes());
    let envelope = Envelope { conteudo: conteudo.to_owned(), assinatura: para_hex(&assinatura.to_bytes()) };
    serde_json::to_string_pretty(&envelope).map_err(|erro| ErroCatalogo::Formato(erro.to_string()))
}

/// A newer version to offer. Nothing is installed until a person accepts it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Atualizacao {
    /// The driver.
    pub id: String,
    /// The version in use. `None` when this driver is not here yet.
    pub de: Option<Version>,
    /// The version offered.
    pub para: Version,
    /// What changed.
    pub notas: String,
}

/// What a catalog has that is newer than what is in use, one offer per driver: the newest this
/// program can run. A driver that asks for a newer program is not offered.
pub fn verificar_atualizacoes(catalogo: &Catalogo, registro: &Registro) -> Vec<Atualizacao> {
    let mut melhores: std::collections::BTreeMap<&str, &ItemCatalogo> = std::collections::BTreeMap::new();
    for item in catalogo.itens() {
        if !item.manifesto.compativel_com(registro.programa()) {
            continue;
        }
        let melhor = melhores.entry(&item.manifesto.id).or_insert(item);
        if item.manifesto.versao > melhor.manifesto.versao {
            *melhor = item;
        }
    }
    melhores
        .into_iter()
        .filter_map(|(id, item)| {
            let de = registro.ativo(id).map(|ativo| ativo.versao.clone());
            let mais_nova = de.as_ref().is_none_or(|de| item.manifesto.versao > *de);
            mais_nova.then(|| Atualizacao {
                id: id.to_owned(),
                de,
                para: item.manifesto.versao.clone(),
                notas: item.notas.clone(),
            })
        })
        .collect()
}
