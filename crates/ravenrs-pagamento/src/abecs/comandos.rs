//! The application layer: commands going out and responses coming back.
//!
//! Only commands that need no key from an acquirer exist here. There is no way to build a command
//! that asks for a PIN or captures a card.

use std::collections::BTreeMap;
use std::fmt;

use super::quadro::crc16;

const SPE_IDLIST: u16 = 0x0001;
const SPE_MSGIDX: u16 = 0x000B;
const SPE_TIMEOUT: u16 = 0x000C;
const SPE_MINDIG: u16 = 0x000D;
const SPE_MAXDIG: u16 = 0x000E;
const SPE_DATAIN: u16 = 0x000F;
const SPE_DSPMSG: u16 = 0x001B;
const SPE_MFNAME: u16 = 0x001E;
const SPE_MFINFO: u16 = 0x001F;
const SPE_MNUOPT: u16 = 0x0020;
const SPE_GCDOPT: u16 = 0x0026;

/// `PP_SERNUM`.
pub const PP_SERNUM: u16 = 0x8001;
/// `PP_MODEL`.
pub const PP_MODEL: u16 = 0x8003;
/// `PP_MNNAME`.
pub const PP_MNNAME: u16 = 0x8004;
/// `PP_SOVER`.
pub const PP_SOVER: u16 = 0x8006;
/// `PP_SPECVER`.
pub const PP_SPECVER: u16 = 0x8007;
/// `PP_DSPTXTSZ`.
pub const PP_DSPTXTSZ: u16 = 0x8020;
/// `PP_DSPGRSZ`.
pub const PP_DSPGRSZ: u16 = 0x8021;
/// `PP_MFSUP`.
pub const PP_MFSUP: u16 = 0x8022;
/// `PP_VALUE`.
pub const PP_VALUE: u16 = 0x804D;

/// The longest parameter of an "Abecs Command".
const TAMANHO_MAXIMO_PARAMETRO: usize = 995;
/// The most file data one `MLR` carries: two full blocks.
pub const TAMANHO_MAXIMO_MLR: usize = 2 * TAMANHO_MAXIMO_PARAMETRO;

/// `RSP_STAT`, the result of a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StatusAbecs(pub u16);

impl StatusAbecs {
    /// Success.
    pub const OK: Self = Self(0);
    /// Secure communication was asked for and does not exist.
    pub const NOSEC: Self = Self(3);
    /// The [CLEAR] key.
    pub const BACKSP: Self = Self(8);
    /// A call out of order, or a command it does not know.
    pub const INVCALL: Self = Self(10);
    /// A bad parameter.
    pub const INVPARM: Self = Self(11);
    /// The time given to the customer ran out.
    pub const TIMEOUT: Self = Self(12);
    /// The customer pressed [CANCEL].
    pub const CANCEL: Self = Self(13);
    /// A mandatory parameter is missing.
    pub const MANDAT: Self = Self(19);
    /// An internal error of the pinpad.
    pub const INTERR: Self = Self(40);
    /// The media file does not exist.
    pub const MFNFOUND: Self = Self(100);
    /// The media format or size is not accepted.
    pub const MFERRFMT: Self = Self(101);
    /// The loaded media does not match what `MLI` announced.
    pub const MFERR: Self = Self(102);

    /// True for [`Self::OK`].
    #[must_use]
    pub const fn ok(self) -> bool {
        self.0 == 0
    }
}

/// A message of the fixed table `GCD` shows, by its index. For PCI reasons, free text is not possible.
///
/// Only what a checkout uses has a name. Indexes for a card number, for the card security code and
/// for the last digits of a card are refused, also through [`Self::Outra`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MensagemGcd {
    /// Index 0003h: phone number.
    Telefone,
    /// Index 0005h: area code and phone number.
    DddTelefone,
    /// Index 0007h: CPF.
    Cpf,
    /// Index 000Dh: CNPJ.
    Cnpj,
    /// Index 001Eh: loyalty code.
    Fidelidade,
    /// Index 0030h: rate the service from 0 to 10.
    AvaliacaoAtendimento,
    /// Index 0031h: a token.
    Token,
    /// Any other index of the table.
    Outra(u16),
}

impl MensagemGcd {
    /// The index in the table.
    #[must_use]
    pub const fn indice(self) -> u16 {
        match self {
            Self::Telefone => 0x0003,
            Self::DddTelefone => 0x0005,
            Self::Cpf => 0x0007,
            Self::Cnpj => 0x000D,
            Self::Fidelidade => 0x001E,
            Self::AvaliacaoAtendimento => 0x0030,
            Self::Token => 0x0031,
            Self::Outra(indice) => indice,
        }
    }

    /// Whether this crate refuses it: reading card data in clear is not something a POS software does.
    #[must_use]
    pub const fn proibida(self) -> bool {
        matches!(self.indice(), 0x000B | 0x000C | 0x0032)
    }
}

/// The image formats a pinpad may take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoMidia {
    /// PNG.
    Png,
    /// JPG.
    Jpg,
    /// GIF.
    Gif,
}

impl TipoMidia {
    const fn codigo(self) -> u8 {
        match self {
            Self::Png => 1,
            Self::Jpg => 2,
            Self::Gif => 3,
        }
    }
}

/// Why a command could not be built or a response could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroComando {
    /// A text or list does not fit where it goes.
    Tamanho(&'static str),
    /// A media name has a forbidden character or the wrong length.
    NomeMidia(String),
    /// This index would read card data in clear.
    MensagemProibida(u16),
    /// The bytes do not form a response.
    RespostaInvalida(&'static str),
}

impl fmt::Display for ErroComando {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tamanho(campo) => write!(f, "tamanho inválido em {campo}"),
            Self::NomeMidia(nome) => write!(f, "nome de mídia inválido: {nome:?}"),
            Self::MensagemProibida(indice) => {
                write!(f, "mensagem {indice:04X}h leria dados de cartão em aberto")
            }
            Self::RespostaInvalida(motivo) => write!(f, "resposta inválida: {motivo}"),
        }
    }
}

impl std::error::Error for ErroComando {}

/// A command to the pinpad.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Comando {
    /// `OPN` in its classic format. The secure format needs RSA and is not here.
    Abrir,
    /// `GIN`, general information. Index zero is the pinpad itself.
    InformacaoGeral {
        /// `GIN_ACQIDX`.
        rede: u8,
    },
    /// `GIX`. An empty list asks for what the pinpad returns by default.
    InformacaoEstendida {
        /// The `PP_*` identifiers wanted, at most 64.
        ids: Vec<u16>,
    },
    /// `DSP`: two rows of sixteen columns.
    Exibir {
        /// The first row.
        linha1: String,
        /// The second row.
        linha2: String,
    },
    /// `DEX`: free text, with line breaks, up to 160 characters.
    ExibirLivre {
        /// The text.
        mensagem: String,
    },
    /// `GKY`: waits for a key that is not a digit.
    LerTecla,
    /// `GCD`: asks the customer to type a value, in clear, under a message of the fixed table.
    Capturar {
        /// The message.
        mensagem: MensagemGcd,
        /// The least digits.
        minimo: Option<u8>,
        /// The most digits, up to 32.
        maximo: Option<u8>,
        /// How long to wait for the customer, in seconds.
        tempo_s: Option<u8>,
        /// Accepts letters, which only exists from the version 2.20.
        alfanumerico: bool,
    },
    /// `MNU`: a menu for the customer, up to 20 options of 24 characters.
    Menu {
        /// The title.
        titulo: Option<String>,
        /// The options.
        opcoes: Vec<String>,
        /// How long to wait for the customer, in seconds.
        tempo_s: Option<u8>,
    },
    /// `CLO`: releases the pinpad and leaves two rows on its display.
    Fechar {
        /// The first row.
        linha1: String,
        /// The second row.
        linha2: String,
    },
    /// `MLI`: starts loading a media file.
    CarregarMidiaInicio {
        /// Up to eight letters and digits.
        nome: String,
        /// The whole size.
        tamanho: u32,
        /// The CRC of the whole file.
        crc: u16,
        /// The format.
        tipo: TipoMidia,
    },
    /// `MLR`: a piece of the file, up to [`TAMANHO_MAXIMO_MLR`] bytes.
    CarregarMidiaDados {
        /// The bytes.
        dados: Vec<u8>,
    },
    /// `MLE`: ends the load and makes the pinpad check it.
    CarregarMidiaFim,
    /// `DSI`: shows a media file already loaded.
    ExibirMidia {
        /// Its name.
        nome: String,
    },
}

/// The command to announce a file: its size and CRC as the pinpad will check them.
pub fn comando_inicio_midia(nome: &str, tipo: TipoMidia, arquivo: &[u8]) -> Result<Comando, ErroComando> {
    let tamanho = u32::try_from(arquivo.len()).map_err(|_| ErroComando::Tamanho("arquivo de mídia"))?;
    Ok(Comando::CarregarMidiaInicio { nome: nome.to_owned(), tamanho, crc: crc16(arquivo), tipo })
}

fn latin1(texto: &str) -> Vec<u8> {
    texto.chars().map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')).collect()
}

fn linha16(texto: &str) -> Vec<u8> {
    let mut bytes = latin1(texto);
    bytes.truncate(16);
    bytes.resize(16, b' ');
    bytes
}

fn nome_midia(nome: &str) -> Result<Vec<u8>, ErroComando> {
    if nome.is_empty() || nome.len() > 8 || !nome.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(ErroComando::NomeMidia(nome.to_owned()));
    }
    Ok(nome.as_bytes().to_vec())
}

/// Lays parameters out as "Abecs Command" blocks of at most 999 bytes each.
fn blocos_abecs(id: &str, parametros: &[(u16, Vec<u8>)]) -> Result<Vec<u8>, ErroComando> {
    let mut blocos: Vec<Vec<u8>> = Vec::new();
    let mut atual: Vec<u8> = Vec::new();
    for (parid, valor) in parametros {
        if valor.len() > TAMANHO_MAXIMO_PARAMETRO {
            return Err(ErroComando::Tamanho("parâmetro"));
        }
        if atual.len() + 4 + valor.len() > 999 {
            blocos.push(std::mem::take(&mut atual));
        }
        atual.extend_from_slice(&parid.to_be_bytes());
        // The check above keeps this length inside two bytes.
        atual.extend_from_slice(&(valor.len() as u16).to_be_bytes());
        atual.extend_from_slice(valor);
    }
    if !atual.is_empty() {
        blocos.push(atual);
    }
    let mut saida = id.as_bytes().to_vec();
    for bloco in blocos {
        saida.extend_from_slice(format!("{:03}", bloco.len()).as_bytes());
        saida.extend_from_slice(&bloco);
    }
    Ok(saida)
}

fn bloco_fixo(id: &str, conteudo: &[u8]) -> Vec<u8> {
    let mut saida = id.as_bytes().to_vec();
    saida.extend_from_slice(format!("{:03}", conteudo.len()).as_bytes());
    saida.extend_from_slice(conteudo);
    saida
}

impl Comando {
    /// True for commands that wait for the customer, which have no time limit on the link.
    #[must_use]
    pub const fn bloqueante(&self) -> bool {
        matches!(self, Self::LerTecla | Self::Capturar { .. } | Self::Menu { .. })
    }

    /// The bytes of `PKTDATA`.
    pub fn codificar(&self) -> Result<Vec<u8>, ErroComando> {
        match self {
            Self::Abrir => Ok(b"OPN".to_vec()),
            Self::InformacaoGeral { rede } => {
                if *rede > 99 {
                    return Err(ErroComando::Tamanho("GIN_ACQIDX"));
                }
                Ok(bloco_fixo("GIN", format!("{rede:02}").as_bytes()))
            }
            Self::InformacaoEstendida { ids } => {
                if ids.len() > 64 {
                    return Err(ErroComando::Tamanho("SPE_IDLIST"));
                }
                if ids.is_empty() {
                    return Ok(b"GIX".to_vec());
                }
                let lista = ids.iter().flat_map(|id| id.to_be_bytes()).collect();
                blocos_abecs("GIX", &[(SPE_IDLIST, lista)])
            }
            Self::Exibir { linha1, linha2 } => {
                let mut texto = linha16(linha1);
                texto.extend(linha16(linha2));
                Ok(bloco_fixo("DSP", &texto))
            }
            Self::ExibirLivre { mensagem } => {
                let texto: Vec<u8> =
                    latin1(mensagem).into_iter().map(|b| if b == b'\n' { 0x0D } else { b }).collect();
                if texto.len() > 160 {
                    return Err(ErroComando::Tamanho("DEX_MSG"));
                }
                let mut conteudo = format!("{:03}", texto.len()).into_bytes();
                conteudo.extend(texto);
                Ok(bloco_fixo("DEX", &conteudo))
            }
            Self::LerTecla => Ok(b"GKY".to_vec()),
            Self::Capturar { mensagem, minimo, maximo, tempo_s, alfanumerico } => {
                if mensagem.proibida() {
                    return Err(ErroComando::MensagemProibida(mensagem.indice()));
                }
                if maximo.is_some_and(|m| m > 32 || minimo.is_some_and(|n| n > m)) {
                    return Err(ErroComando::Tamanho("SPE_MAXDIG"));
                }
                let mut parametros = vec![(SPE_MSGIDX, mensagem.indice().to_be_bytes().to_vec())];
                if let Some(minimo) = minimo {
                    parametros.push((SPE_MINDIG, vec![*minimo]));
                }
                if let Some(maximo) = maximo {
                    parametros.push((SPE_MAXDIG, vec![*maximo]));
                }
                if let Some(tempo) = tempo_s {
                    parametros.push((SPE_TIMEOUT, vec![*tempo]));
                }
                if *alfanumerico {
                    parametros.push((SPE_GCDOPT, b"1000".to_vec()));
                }
                blocos_abecs("GCD", &parametros)
            }
            Self::Menu { titulo, opcoes, tempo_s } => {
                if opcoes.is_empty() || opcoes.len() > 20 {
                    return Err(ErroComando::Tamanho("SPE_MNUOPT"));
                }
                let mut parametros = Vec::new();
                if let Some(tempo) = tempo_s {
                    parametros.push((SPE_TIMEOUT, vec![*tempo]));
                }
                for opcao in opcoes {
                    let texto = latin1(opcao);
                    if texto.len() > 24 {
                        return Err(ErroComando::Tamanho("SPE_MNUOPT"));
                    }
                    parametros.push((SPE_MNUOPT, texto));
                }
                if let Some(titulo) = titulo {
                    let texto = latin1(titulo);
                    if texto.len() > 128 {
                        return Err(ErroComando::Tamanho("SPE_DSPMSG"));
                    }
                    parametros.push((SPE_DSPMSG, texto));
                }
                blocos_abecs("MNU", &parametros)
            }
            Self::Fechar { linha1, linha2 } => {
                let mut texto = linha16(linha1);
                texto.extend(linha16(linha2));
                Ok(bloco_fixo("CLO", &texto))
            }
            Self::CarregarMidiaInicio { nome, tamanho, crc, tipo } => {
                let mut info = tamanho.to_be_bytes().to_vec();
                info.extend_from_slice(&crc.to_be_bytes());
                info.push(tipo.codigo());
                info.extend_from_slice(&[0, 0, 0]);
                blocos_abecs("MLI", &[(SPE_MFNAME, nome_midia(nome)?), (SPE_MFINFO, info)])
            }
            Self::CarregarMidiaDados { dados } => {
                if dados.is_empty() || dados.len() > TAMANHO_MAXIMO_MLR {
                    return Err(ErroComando::Tamanho("SPE_DATAIN"));
                }
                let parametros: Vec<(u16, Vec<u8>)> = dados
                    .chunks(TAMANHO_MAXIMO_PARAMETRO)
                    .map(|parte| (SPE_DATAIN, parte.to_vec()))
                    .collect();
                blocos_abecs("MLR", &parametros)
            }
            Self::CarregarMidiaFim => Ok(b"MLE".to_vec()),
            Self::ExibirMidia { nome } => blocos_abecs("DSI", &[(SPE_MFNAME, nome_midia(nome)?)]),
        }
    }
}

/// A response of the pinpad.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RespostaAbecs {
    /// `RSP_ID`.
    pub id: [u8; 3],
    /// `RSP_STAT`.
    pub status: StatusAbecs,
    /// The data blocks, without their length prefixes.
    pub blocos: Vec<Vec<u8>>,
}

impl RespostaAbecs {
    /// Reads the content of a packet.
    pub fn ler(pacote: &[u8]) -> Result<Self, ErroComando> {
        if pacote.len() < 6 {
            return Err(ErroComando::RespostaInvalida("menos de seis bytes"));
        }
        let id = [pacote[0], pacote[1], pacote[2]];
        let status = numero(&pacote[3..6]).ok_or(ErroComando::RespostaInvalida("RSP_STAT"))?;
        let mut blocos = Vec::new();
        let mut resto = &pacote[6..];
        while !resto.is_empty() {
            if resto.len() < 3 {
                return Err(ErroComando::RespostaInvalida("RSP_LEN incompleto"));
            }
            let tamanho = usize::from(numero(&resto[..3]).ok_or(ErroComando::RespostaInvalida("RSP_LEN"))?);
            let fim = 3 + tamanho;
            if resto.len() < fim {
                return Err(ErroComando::RespostaInvalida("bloco incompleto"));
            }
            blocos.push(resto[3..fim].to_vec());
            resto = &resto[fim..];
        }
        Ok(Self { id, status: StatusAbecs(status), blocos })
    }

    /// True for a notification, which comes in the middle of a blocking command.
    #[must_use]
    pub fn notificacao(&self) -> bool {
        &self.id == b"NTM"
    }

    /// The fields of an "Abecs Command" response, by `PP_*` identifier.
    pub fn campos(&self) -> Result<BTreeMap<u16, Vec<u8>>, ErroComando> {
        let mut campos = BTreeMap::new();
        for bloco in &self.blocos {
            let mut resto = bloco.as_slice();
            while !resto.is_empty() {
                if resto.len() < 4 {
                    return Err(ErroComando::RespostaInvalida("RSP_DATID incompleto"));
                }
                let id = u16::from_be_bytes([resto[0], resto[1]]);
                let tamanho = usize::from(u16::from_be_bytes([resto[2], resto[3]]));
                if resto.len() < 4 + tamanho {
                    return Err(ErroComando::RespostaInvalida("RSP_DAT incompleto"));
                }
                campos.insert(id, resto[4..4 + tamanho].to_vec());
                resto = &resto[4 + tamanho..];
            }
        }
        Ok(campos)
    }

    /// The value of a field as text.
    pub fn texto(&self, id: u16) -> Result<Option<String>, ErroComando> {
        Ok(self.campos()?.get(&id).map(|valor| de_latin1(valor)))
    }
}

/// Turns bytes the pinpad sent into text.
#[must_use]
pub fn de_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|b| char::from(*b)).collect()
}

fn numero(bytes: &[u8]) -> Option<u16> {
    let mut valor = 0u16;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        valor = valor * 10 + u16::from(byte - b'0');
    }
    Some(valor)
}

/// What `GIN` with index zero says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InformacaoGeral {
    /// The maker.
    pub fabricante: String,
    /// The model.
    pub modelo: String,
    /// Whether it reads contactless cards.
    pub aproximacao: bool,
    /// The system version.
    pub sistema: String,
    /// The version of the specification it follows, such as `2.20`.
    pub especificacao: String,
    /// The version of its manager application.
    pub gerenciador: String,
    /// Its serial number.
    pub serie: String,
}

impl InformacaoGeral {
    /// Reads the fixed format of the first block.
    pub fn ler(resposta: &RespostaAbecs) -> Result<Self, ErroComando> {
        let bloco = resposta.blocos.first().ok_or(ErroComando::RespostaInvalida("GIN sem dados"))?;
        if bloco.len() != 100 {
            return Err(ErroComando::RespostaInvalida("GIN com tamanho diferente de 100"));
        }
        let campo =
            |inicio: usize, tamanho: usize| de_latin1(&bloco[inicio..inicio + tamanho]).trim().to_owned();
        Ok(Self {
            fabricante: campo(0, 20),
            modelo: campo(20, 19),
            aproximacao: bloco[39] == b'C',
            sistema: campo(40, 20),
            especificacao: campo(60, 4),
            gerenciador: campo(64, 16),
            serie: campo(80, 20),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exemplos_da_especificacao_saem_iguais() {
        assert_eq!(
            Comando::ExibirMidia { nome: "QRCODE01".into() }.codificar().expect("dsi"),
            b"DSI012\x00\x1E\x00\x08QRCODE01"
        );
        assert_eq!(
            Comando::CarregarMidiaInicio {
                nome: "QRCODE01".into(),
                tamanho: 3334,
                crc: 0xF211,
                tipo: TipoMidia::Png
            }
            .codificar()
            .expect("mli"),
            b"MLI026\x00\x1E\x00\x08QRCODE01\x00\x1F\x00\x0A\x00\x00\x0D\x06\xF2\x11\x01\x00\x00\x00"
        );
        assert_eq!(
            Comando::ExibirLivre { mensagem: "NOT AUTHORIZED.".into() }.codificar().expect("dex")[..9],
            *b"DEX018015"
        );
        assert_eq!(Comando::CarregarMidiaFim.codificar().expect("mle"), b"MLE");
        assert_eq!(Comando::LerTecla.codificar().expect("gky"), b"GKY");
    }

    #[test]
    fn dsp_e_clo_sempre_tem_32_colunas() {
        let dsp = Comando::Exibir { linha1: "OLA".into(), linha2: "X".repeat(40) }.codificar().expect("dsp");
        assert_eq!(&dsp[..6], b"DSP032");
        assert_eq!(dsp.len(), 38);
        assert_eq!(&dsp[6..22], b"OLA             ");
        let clo = Comando::Fechar { linha1: String::new(), linha2: String::new() }.codificar().expect("clo");
        assert_eq!(&clo[..6], b"CLO032");
    }

    #[test]
    fn gcd_tem_os_identificadores_do_exemplo_oficial() {
        let gcd = Comando::Capturar {
            mensagem: MensagemGcd::Outra(0x0009),
            minimo: None,
            maximo: Some(10),
            tempo_s: Some(60),
            alfanumerico: false,
        }
        .codificar()
        .expect("gcd");
        assert_eq!(gcd, b"GCD016\x00\x0B\x00\x02\x00\x09\x00\x0E\x00\x01\x0A\x00\x0C\x00\x01\x3C");
    }

    #[test]
    fn indices_de_dados_de_cartao_sao_recusados() {
        for indice in [0x000B, 0x000C, 0x0032] {
            let comando = Comando::Capturar {
                mensagem: MensagemGcd::Outra(indice),
                minimo: None,
                maximo: None,
                tempo_s: None,
                alfanumerico: false,
            };
            assert_eq!(comando.codificar(), Err(ErroComando::MensagemProibida(indice)));
        }
    }

    #[test]
    fn mnu_leva_opcoes_na_ordem_e_em_latin1() {
        let mnu = Comando::Menu {
            titulo: Some("Sim?".into()),
            opcoes: vec!["1.Técnico".into(), "2".into()],
            tempo_s: Some(30),
        }
        .codificar()
        .expect("mnu");
        assert_eq!(&mnu[..3], b"MNU");
        assert!(mnu.windows(9).any(|j| j == b"1.T\xE9cnico"));
        let sem = Comando::Menu { titulo: None, opcoes: Vec::new(), tempo_s: None };
        assert!(sem.codificar().is_err());
        let longa = Comando::Menu { titulo: None, opcoes: vec!["x".repeat(25)], tempo_s: None };
        assert!(longa.codificar().is_err());
    }

    #[test]
    fn mlr_divide_em_blocos_que_cabem_no_limite_do_comando_abecs() {
        let mlr =
            Comando::CarregarMidiaDados { dados: vec![0x41; TAMANHO_MAXIMO_MLR] }.codificar().expect("mlr");
        assert_eq!(&mlr[..6], b"MLR999");
        assert!(mlr.len() <= 2044);
        assert!(Comando::CarregarMidiaDados { dados: vec![0; TAMANHO_MAXIMO_MLR + 1] }.codificar().is_err());
    }

    #[test]
    fn nome_de_midia_so_aceita_o_que_a_especificacao_pede() {
        assert!(Comando::ExibirMidia { nome: "COM ESPACO".into() }.codificar().is_err());
        assert!(Comando::ExibirMidia { nome: "NOVENOVE9".into() }.codificar().is_err());
        assert!(Comando::ExibirMidia { nome: String::new() }.codificar().is_err());
    }

    #[test]
    fn resposta_com_erro_e_com_campos() {
        let erro = RespostaAbecs::ler(b"GKY013").expect("gky");
        assert_eq!(erro.status, StatusAbecs::CANCEL);
        let valor = RespostaAbecs::ler(b"GCD000013\x80\x4D\x00\x09169870234").expect("gcd");
        assert!(valor.status.ok());
        assert_eq!(valor.texto(PP_VALUE).expect("campos").as_deref(), Some("169870234"));
        assert!(RespostaAbecs::ler(b"GC").is_err());
        assert!(RespostaAbecs::ler(b"GCD000099xx").is_err());
        assert!(RespostaAbecs::ler(b"GCD000002\x80\x4D").expect("bloco").campos().is_err());
    }

    #[test]
    fn gin_le_o_formato_fixo() {
        let mut bloco = String::new();
        bloco.push_str(&format!("{:<20}", "GERTEC"));
        bloco.push_str(&format!("{:<19}", "PPC930;2MB"));
        bloco.push('C');
        bloco.push_str(&format!("{:<20}", "SO 1"));
        bloco.push_str("2.20");
        bloco.push_str(&format!("{:<16}", "001.00 250101"));
        bloco.push_str(&format!("{:<20}", "ABC123"));
        let resposta =
            RespostaAbecs { id: *b"GIN", status: StatusAbecs::OK, blocos: vec![bloco.into_bytes()] };
        let informacao = InformacaoGeral::ler(&resposta).expect("gin");
        assert_eq!(informacao.fabricante, "GERTEC");
        assert_eq!(informacao.especificacao, "2.20");
        assert!(informacao.aproximacao);
        assert_eq!(informacao.serie, "ABC123");
    }
}
