//! What a driver says about a device, and the check that it says nothing else.

use std::fmt;

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::comando::{Comando, bipe_aceito, corte_aceito, gaveta_aceita, modo_aceito};
use crate::data::data_valida;

/// The largest manifest accepted, in bytes.
pub(crate) const TAMANHO_MAXIMO_MANIFESTO: usize = 16 * 1024;
const TEXTO_CURTO: usize = 120;
const TEXTO_LONGO: usize = 1000;
const MAXIMO_MODELOS: usize = 32;
const MAXIMO_VARIACOES: usize = 32;
/// Epson's reference stops at 4607 rows in a `GS v 0`.
const MAIOR_BLOCO: u32 = 4607;
/// The most dots across a raster can have.
const MAIOR_LARGURA: u32 = 4096;
const DPI_ACEITOS: [u32; 3] = [180, 203, 300];
const BAUDS_ACEITOS: [u32; 8] = [1200, 2400, 4800, 9600, 19_200, 38_400, 57_600, 115_200];
/// Names that say nothing about which device it is.
const NOMES_VAGOS: [&str; 7] = ["generico", "genérico", "qualquer", "todos", "todas", "diversos", "padrao"];

/// The kind of peripheral.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoPeriferico {
    /// A thermal receipt printer.
    ImpressoraTermica,
    /// A label printer.
    Etiquetadora,
    /// A pinpad.
    Pinpad,
    /// A barcode scanner.
    LeitorCodigo,
    /// A scale.
    Balanca,
    /// A cash drawer.
    Gaveta,
}

/// How a device can be connected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoConexao {
    /// USB.
    Usb,
    /// A serial port.
    Serial,
    /// The network.
    Rede,
    /// Bluetooth.
    Bluetooth,
    /// Through another device, as a drawer through a printer.
    PeloAparelho,
}

/// A protocol's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NomeProtocolo {
    /// ESC/POS.
    EscPos,
    /// ZPL.
    Zpl,
    /// PPLA.
    Ppla,
    /// TSPL.
    Tspl,
    /// EPL, also called PPLB.
    Epl,
    /// The Brazilian pinpad protocol.
    Abecs,
    /// USB HID, seen as a keyboard.
    Hid,
    /// Plain serial.
    Serial,
}

/// A protocol and, when it has one, its version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protocolo {
    /// Which protocol.
    pub nome: NomeProtocolo,
    /// Which version of it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versao: Option<String>,
}

/// One exact device model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modelo {
    /// Its full name, as on the device.
    pub nome: String,
    /// Other names and versions of the same model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variacoes: Vec<String>,
    /// The firmware this is for, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
}

/// Values a device starts with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Padroes {
    /// Resolution in dots per inch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dpi: Option<u32>,
    /// Printable width in dots.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub largura_pontos: Option<u32>,
    /// Serial speed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baud: Option<u32>,
}

/// What a device can do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Capacidades {
    /// Cuts the paper.
    pub corta: bool,
    /// Cuts leaving a bit attached.
    pub corte_parcial: bool,
    /// Opens a cash drawer.
    pub gaveta: bool,
    /// Beeps.
    pub bipe: bool,
    /// Prints QR codes by itself.
    pub qr_nativo: bool,
    /// Shows a Pix code on the pinpad's screen.
    pub pix_no_pinpad: bool,
}

/// What differs on this model, in values only. What is not set takes the language's usual form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Ajustes {
    /// The full cut.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corte_total: Option<Comando>,
    /// The partial cut.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corte_parcial: Option<Comando>,
    /// The beep, since printers do not share one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bipe: Option<Comando>,
    /// The drawer's pulse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gaveta: Option<Comando>,
    /// What to send first, on printers with more than one mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comando_modo: Option<Comando>,
    /// The character table's number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codepage: Option<u8>,
    /// The most rows in one block of raster.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tamanho_bloco: Option<u32>,
    /// Whether this model reads raster bits the opposite way of what its language says.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub polaridade_invertida: bool,
}

/// How far a driver was verified. Screens show this as it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "estado", rename_all = "snake_case", deny_unknown_fields)]
pub enum Situacao {
    /// Someone printed or operated on the real device.
    TestadoNoAparelho {
        /// When, as `YYYY-MM-DD`.
        data: String,
        /// Who.
        por: String,
        /// On which firmware.
        firmware: String,
    },
    /// It follows the maker's documents and never met the device.
    SoPelaDocumentacao {
        /// Which document.
        fonte: String,
    },
    /// Taken from sellers, memory or a similar model.
    Presumido,
}

/// A driver: a version of what is known about one device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifesto {
    /// Its stable name, as `elgin.l42-pro.etiqueta`.
    pub id: String,
    /// Its version.
    pub versao: Version,
    /// The kind of device.
    pub tipo: TipoPeriferico,
    /// Who makes the device.
    pub fabricante: String,
    /// The exact models it is for.
    pub modelos: Vec<Modelo>,
    /// How they can be connected.
    pub conexoes: Vec<TipoConexao>,
    /// Starting values.
    #[serde(default)]
    pub padroes: Padroes,
    /// The protocol to use.
    pub protocolo: Protocolo,
    /// Others the same device also takes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocolos_alternativos: Vec<Protocolo>,
    /// What it can do.
    #[serde(default)]
    pub capacidades: Capacidades,
    /// What differs on this model.
    #[serde(default)]
    pub ajustes: Ajustes,
    /// How far it was verified.
    pub situacao: Situacao,
    /// The oldest program this version works with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_versao_programa: Option<Version>,
    /// What to set on the device before using it, for whoever installs it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instalacao: Option<String>,
    /// What else should be known, such as which part is not confirmed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observacoes: Option<String>,
}

/// Why a manifest is not accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroManifesto {
    /// It is larger than allowed.
    Grande {
        /// Its size in bytes.
        tamanho: usize,
    },
    /// It is not JSON in the expected shape. Unknown fields fall here.
    Formato(String),
    /// A field has a value that is not allowed.
    Campo {
        /// Which field.
        campo: &'static str,
        /// Why.
        motivo: String,
    },
    /// A command is not one this kind of device may send.
    ComandoProibido {
        /// Which field.
        campo: &'static str,
        /// The bytes.
        comando: String,
    },
}

impl fmt::Display for ErroManifesto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Grande { tamanho } => {
                write!(f, "manifesto com {tamanho} bytes; o limite é {TAMANHO_MAXIMO_MANIFESTO}")
            }
            Self::Formato(motivo) => write!(f, "manifesto mal formado: {motivo}"),
            Self::Campo { campo, motivo } => write!(f, "campo {campo}: {motivo}"),
            Self::ComandoProibido { campo, comando } => {
                write!(f, "campo {campo}: o comando {comando} não é permitido para este tipo de aparelho")
            }
        }
    }
}

impl std::error::Error for ErroManifesto {}

fn campo(campo: &'static str, motivo: impl Into<String>) -> ErroManifesto {
    ErroManifesto::Campo { campo, motivo: motivo.into() }
}

/// A short text with no way of being read as a path or a control sequence.
fn texto(nome: &'static str, valor: &str, limite: usize) -> Result<(), ErroManifesto> {
    if valor.trim().is_empty() {
        return Err(campo(nome, "está vazio"));
    }
    if valor.chars().count() > limite {
        return Err(campo(nome, format!("passa de {limite} caracteres")));
    }
    if valor.chars().any(char::is_control) {
        return Err(campo(nome, "tem caractere de controle"));
    }
    if valor.contains('/') || valor.contains('\\') || valor.contains("..") {
        return Err(campo(
            nome,
            format!("\"{valor}\" parece um caminho, e manifesto não aponta para arquivos"),
        ));
    }
    Ok(())
}

/// A sentence for people, where `ESC/POS` is fine and a path or an address is not.
fn frase(nome: &'static str, valor: &str) -> Result<(), ErroManifesto> {
    if valor.trim().is_empty() {
        return Err(campo(nome, "está vazio"));
    }
    if valor.chars().count() > TEXTO_LONGO {
        return Err(campo(nome, format!("passa de {TEXTO_LONGO} caracteres")));
    }
    if valor.chars().any(char::is_control) {
        return Err(campo(nome, "tem caractere de controle"));
    }
    let aponta = valor.contains('\\')
        || valor.contains("..")
        || valor.contains("://")
        || valor.split_whitespace().any(|palavra| palavra.starts_with('/'));
    if aponta {
        return Err(campo(nome, "tem o que parece um caminho ou endereço, e manifesto não aponta para nada"));
    }
    Ok(())
}

fn validar_id(id: &str) -> Result<(), ErroManifesto> {
    let partes: Vec<&str> = id.split('.').collect();
    let parte_boa = |parte: &&str| {
        !parte.is_empty()
            && !parte.starts_with('-')
            && !parte.ends_with('-')
            && parte.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    if id.len() > 80 || partes.len() != 3 || !partes.iter().all(parte_boa) {
        return Err(campo(
            "id",
            format!("\"{id}\" não é fabricante.modelo.função, como elgin.l42-pro.etiqueta"),
        ));
    }
    Ok(())
}

fn validar_modelo(modelo: &Modelo, fabricante: &str) -> Result<(), ErroManifesto> {
    texto("modelos.nome", &modelo.nome, TEXTO_CURTO)?;
    let nome = modelo.nome.trim().to_lowercase();
    if nome == fabricante.trim().to_lowercase()
        || NOMES_VAGOS.contains(&nome.as_str())
        || nome.chars().count() < 3
    {
        return Err(campo(
            "modelos.nome",
            format!("\"{}\" não diz qual é o aparelho; use o nome exato do modelo", modelo.nome),
        ));
    }
    if modelo.variacoes.len() > MAXIMO_VARIACOES {
        return Err(campo("modelos.variacoes", format!("mais de {MAXIMO_VARIACOES}")));
    }
    for variacao in &modelo.variacoes {
        texto("modelos.variacoes", variacao, TEXTO_CURTO)?;
    }
    if let Some(firmware) = &modelo.firmware {
        texto("modelos.firmware", firmware, TEXTO_CURTO)?;
    }
    Ok(())
}

fn protocolo_serve(tipo: TipoPeriferico, nome: NomeProtocolo) -> bool {
    use NomeProtocolo as P;
    match tipo {
        TipoPeriferico::ImpressoraTermica => nome == P::EscPos,
        TipoPeriferico::Etiquetadora => matches!(nome, P::Zpl | P::Epl | P::Ppla | P::Tspl),
        TipoPeriferico::Pinpad => nome == P::Abecs,
        TipoPeriferico::LeitorCodigo => matches!(nome, P::Hid | P::Serial),
        TipoPeriferico::Balanca => nome == P::Serial,
        TipoPeriferico::Gaveta => matches!(nome, P::EscPos | P::Serial),
    }
}

fn validar_protocolo(tipo: TipoPeriferico, protocolo: &Protocolo) -> Result<(), ErroManifesto> {
    if !protocolo_serve(tipo, protocolo.nome) {
        return Err(campo("protocolo", format!("{:?} não serve para {tipo:?}", protocolo.nome)));
    }
    if let Some(versao) = &protocolo.versao {
        texto("protocolo.versao", versao, TEXTO_CURTO)?;
    } else if protocolo.nome == NomeProtocolo::Abecs {
        return Err(campo("protocolo.versao", "ABECS precisa dizer a versão"));
    }
    Ok(())
}

fn validar_capacidades(tipo: TipoPeriferico, capacidades: &Capacidades) -> Result<(), ErroManifesto> {
    if capacidades.pix_no_pinpad && tipo != TipoPeriferico::Pinpad {
        return Err(campo("capacidades.pix_no_pinpad", "só pinpad mostra Pix"));
    }
    let imprime = matches!(tipo, TipoPeriferico::ImpressoraTermica | TipoPeriferico::Etiquetadora);
    if !imprime && (capacidades.corta || capacidades.corte_parcial || capacidades.qr_nativo) {
        return Err(campo("capacidades", "corte e QR são de impressora"));
    }
    if capacidades.gaveta && !matches!(tipo, TipoPeriferico::ImpressoraTermica | TipoPeriferico::Gaveta) {
        return Err(campo("capacidades.gaveta", "só impressora de cupom e gaveta abrem gaveta"));
    }
    Ok(())
}

fn validar_comando(
    nome: &'static str,
    comando: Option<Comando>,
    aceito: impl Fn(&[u8]) -> bool,
) -> Result<(), ErroManifesto> {
    match comando {
        Some(comando) if !aceito(comando.bytes()) => {
            Err(ErroManifesto::ComandoProibido { campo: nome, comando: comando.to_string() })
        }
        _ => Ok(()),
    }
}

fn validar_ajustes(tipo: TipoPeriferico, ajustes: &Ajustes) -> Result<(), ErroManifesto> {
    let recibo = tipo == TipoPeriferico::ImpressoraTermica;
    let gaveta = recibo || tipo == TipoPeriferico::Gaveta;
    validar_comando("ajustes.corte_total", ajustes.corte_total, |b| recibo && corte_aceito(b))?;
    validar_comando("ajustes.corte_parcial", ajustes.corte_parcial, |b| recibo && corte_aceito(b))?;
    validar_comando("ajustes.bipe", ajustes.bipe, |b| recibo && bipe_aceito(b))?;
    validar_comando("ajustes.gaveta", ajustes.gaveta, |b| gaveta && gaveta_aceita(b))?;
    validar_comando("ajustes.comando_modo", ajustes.comando_modo, |b| recibo && modo_aceito(b))?;
    let imprime = recibo || tipo == TipoPeriferico::Etiquetadora;
    if !imprime
        && (ajustes.tamanho_bloco.is_some() || ajustes.polaridade_invertida || ajustes.codepage.is_some())
    {
        return Err(campo("ajustes", "bloco, polaridade e codepage são de impressora"));
    }
    if let Some(bloco) = ajustes.tamanho_bloco
        && !(1..=MAIOR_BLOCO).contains(&bloco)
    {
        return Err(campo("ajustes.tamanho_bloco", format!("{bloco}; vai de 1 a {MAIOR_BLOCO}")));
    }
    Ok(())
}

fn validar_padroes(padroes: &Padroes) -> Result<(), ErroManifesto> {
    if let Some(dpi) = padroes.dpi
        && !DPI_ACEITOS.contains(&dpi)
    {
        return Err(campo("padroes.dpi", format!("{dpi}; aceita {DPI_ACEITOS:?}")));
    }
    if let Some(largura) = padroes.largura_pontos
        && !(8..=MAIOR_LARGURA).contains(&largura)
    {
        return Err(campo("padroes.largura_pontos", format!("{largura}; vai de 8 a {MAIOR_LARGURA}")));
    }
    if let Some(baud) = padroes.baud
        && !BAUDS_ACEITOS.contains(&baud)
    {
        return Err(campo("padroes.baud", format!("{baud}; aceita {BAUDS_ACEITOS:?}")));
    }
    Ok(())
}

fn validar_situacao(situacao: &Situacao) -> Result<(), ErroManifesto> {
    match situacao {
        Situacao::TestadoNoAparelho { data, por, firmware } => {
            if !data_valida(data) {
                return Err(campo("situacao.data", format!("\"{data}\" não é uma data AAAA-MM-DD")));
            }
            texto("situacao.por", por, TEXTO_CURTO)?;
            texto("situacao.firmware", firmware, TEXTO_CURTO)
        }
        // A source can be an address, the only text that may have slashes. Nothing ever opens it.
        Situacao::SoPelaDocumentacao { fonte } => {
            let limpa = !fonte.trim().is_empty()
                && fonte.chars().count() <= TEXTO_LONGO
                && !fonte.chars().any(char::is_control)
                && !fonte.contains('\\')
                && !fonte.contains("..")
                && !fonte.starts_with('/');
            if limpa {
                Ok(())
            } else {
                Err(campo("situacao.fonte", "precisa dizer de onde veio, sem caminho local"))
            }
        }
        Situacao::Presumido => Ok(()),
    }
}

impl Manifesto {
    /// Reads and checks a manifest.
    pub fn de_json(bytes: &[u8]) -> Result<Self, ErroManifesto> {
        if bytes.len() > TAMANHO_MAXIMO_MANIFESTO {
            return Err(ErroManifesto::Grande { tamanho: bytes.len() });
        }
        let manifesto: Self =
            serde_json::from_slice(bytes).map_err(|erro| ErroManifesto::Formato(erro.to_string()))?;
        manifesto.validar()?;
        Ok(manifesto)
    }

    /// The manifest as JSON.
    pub fn para_json(&self) -> Result<String, ErroManifesto> {
        serde_json::to_string_pretty(self).map_err(|erro| ErroManifesto::Formato(erro.to_string()))
    }

    /// Checks every limit. A manifest from anywhere goes through this before it is used.
    pub fn validar(&self) -> Result<(), ErroManifesto> {
        validar_id(&self.id)?;
        texto("fabricante", &self.fabricante, TEXTO_CURTO)?;
        if self.modelos.is_empty() {
            return Err(campo("modelos", "é obrigatório dizer o modelo exato do aparelho"));
        }
        if self.modelos.len() > MAXIMO_MODELOS {
            return Err(campo("modelos", format!("mais de {MAXIMO_MODELOS}")));
        }
        for modelo in &self.modelos {
            validar_modelo(modelo, &self.fabricante)?;
        }
        if self.conexoes.is_empty() {
            return Err(campo("conexoes", "é obrigatório dizer como o aparelho se conecta"));
        }
        validar_padroes(&self.padroes)?;
        validar_protocolo(self.tipo, &self.protocolo)?;
        if self.protocolos_alternativos.len() > 8 {
            return Err(campo("protocolos_alternativos", "mais de 8"));
        }
        for protocolo in &self.protocolos_alternativos {
            validar_protocolo(self.tipo, protocolo)?;
        }
        validar_capacidades(self.tipo, &self.capacidades)?;
        validar_ajustes(self.tipo, &self.ajustes)?;
        validar_situacao(&self.situacao)?;
        if let Some(instalacao) = &self.instalacao {
            frase("instalacao", instalacao)?;
        }
        if let Some(observacoes) = &self.observacoes {
            frase("observacoes", observacoes)?;
        }
        Ok(())
    }

    /// Whether it works with this version of the program.
    pub fn compativel_com(&self, versao_do_programa: &Version) -> bool {
        self.min_versao_programa.as_ref().is_none_or(|minima| versao_do_programa >= minima)
    }

    /// The model's name to show first.
    pub fn nome_do_modelo(&self) -> &str {
        self.modelos.first().map_or(self.id.as_str(), |modelo| modelo.nome.as_str())
    }
}
