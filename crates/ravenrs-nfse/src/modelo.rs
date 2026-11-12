//! The DPS of the proof of concept: one service, ISS taxable and not withheld, provider ME/EPP.

use ravenrs_dfe::Instante;
use rust_decimal::{Decimal, RoundingStrategy};

use crate::erro::NfseError;

/// A CNPJ or a CPF, as text.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Inscricao {
    /// Fourteen digits.
    Cnpj(String),
    /// Eleven digits.
    Cpf(String),
}

impl Inscricao {
    /// The digits.
    #[must_use]
    pub fn numero(&self) -> &str {
        match self {
            Self::Cnpj(numero) | Self::Cpf(numero) => numero,
        }
    }

    /// The tag of the layout: `CNPJ` or `CPF`.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Cnpj(_) => "CNPJ",
            Self::Cpf(_) => "CPF",
        }
    }

    /// The kind in the Id of the DPS: 1 for CPF, 2 for CNPJ.
    #[must_use]
    pub const fn tipo(&self) -> u8 {
        match self {
            Self::Cpf(_) => 1,
            Self::Cnpj(_) => 2,
        }
    }

    fn validar(&self, campo: &str) -> Result<(), NfseError> {
        let (tamanho, nome) = match self {
            Self::Cnpj(_) => (14, "CNPJ"),
            Self::Cpf(_) => (11, "CPF"),
        };
        if self.numero().len() != tamanho || !self.numero().bytes().all(|b| b.is_ascii_digit()) {
            return Err(NfseError::validacao(campo, format!("{nome} precisa ter {tamanho} dígitos")));
        }
        Ok(())
    }
}

/// Who provides the service and issues the DPS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prestador {
    /// CNPJ or CPF.
    pub inscricao: Inscricao,
    /// The municipal registration, when the city has one.
    pub inscricao_municipal: Option<String>,
    /// The name, for the DANFSe. The DPS does not carry it: the Sefin takes it from the register.
    pub razao_social: String,
    /// The IBGE code of the city that issues, `cLocEmi`.
    pub municipio: String,
}

/// Who takes the service. Optional for a consumer who does not identify.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tomador {
    /// CNPJ or CPF.
    pub inscricao: Inscricao,
    /// The name.
    pub nome: String,
    /// Where the NFS-e is sent, if anywhere.
    pub email: Option<String>,
}

/// The service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Servico {
    /// `cTribNac`: item, subitem and national breakdown of the list of services, six digits.
    pub codigo_tributacao_nacional: String,
    /// What was done, up to 2000 characters.
    pub descricao: String,
    /// The IBGE code of the city where it was done.
    pub municipio_prestacao: String,
    /// The value, in reais.
    pub valor: Decimal,
}

/// A declaration of service provided, before it becomes an NFS-e.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dps {
    /// The series, 1 to 99999.
    pub serie: u32,
    /// The number, never repeated in the series. After a lost answer, the same one goes again.
    pub numero: u64,
    /// When it was issued: `2026-11-06T10:00:00-03:00`.
    pub emitida_em: String,
    /// The month of competence, as its first day or any day of it: `2026-11-06`.
    pub competencia: String,
    /// Who provides.
    pub prestador: Prestador,
    /// Who takes, if identified.
    pub tomador: Option<Tomador>,
    /// The service.
    pub servico: Servico,
}

fn digitos(valor: &str, tamanho: usize) -> bool {
    valor.len() == tamanho && valor.bytes().all(|b| b.is_ascii_digit())
}

impl Dps {
    /// Checks what can be checked before sending.
    pub fn validar(&self) -> Result<(), NfseError> {
        if !(1..=99_999).contains(&self.serie) {
            return Err(NfseError::validacao("serie", "a série vai de 1 a 99999"));
        }
        if !(1..=999_999_999_999_999).contains(&self.numero) {
            return Err(NfseError::validacao("nDPS", "o número vai de 1 a 999999999999999"));
        }
        if Instante::de_xml(&self.emitida_em).is_none() {
            return Err(NfseError::validacao("dhEmi", "use AAAA-MM-DDThh:mm:ss-03:00"));
        }
        let competencia = self.competencia.as_bytes();
        if competencia.len() != 10 || competencia.get(4) != Some(&b'-') || competencia.get(7) != Some(&b'-') {
            return Err(NfseError::validacao("dCompet", "use AAAA-MM-DD"));
        }
        self.prestador.inscricao.validar("prest")?;
        if !digitos(&self.prestador.municipio, 7) {
            return Err(NfseError::validacao("cLocEmi", "o código IBGE do município tem 7 dígitos"));
        }
        if let Some(im) = &self.prestador.inscricao_municipal
            && (im.is_empty() || im.len() > 15)
        {
            return Err(NfseError::validacao("IM", "a inscrição municipal tem de 1 a 15 caracteres"));
        }
        if let Some(tomador) = &self.tomador {
            tomador.inscricao.validar("toma")?;
            if tomador.nome.trim().is_empty() || tomador.nome.chars().count() > 300 {
                return Err(NfseError::validacao("toma/xNome", "o nome tem de 1 a 300 caracteres"));
            }
        }
        let servico = &self.servico;
        if !digitos(&servico.codigo_tributacao_nacional, 6) {
            return Err(NfseError::validacao("cTribNac", "o código de tributação nacional tem 6 dígitos"));
        }
        if !digitos(&servico.municipio_prestacao, 7) {
            return Err(NfseError::validacao("cLocPrestacao", "o código IBGE do município tem 7 dígitos"));
        }
        let descricao = servico.descricao.trim();
        if descricao.is_empty() || descricao.chars().count() > 2000 {
            return Err(NfseError::validacao("xDescServ", "a descrição tem de 1 a 2000 caracteres"));
        }
        if servico.valor <= Decimal::ZERO {
            return Err(NfseError::validacao("vServ", "o valor do serviço precisa ser positivo"));
        }
        if servico.valor.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero) != servico.valor {
            return Err(NfseError::validacao("vServ", "o valor tem no máximo 2 casas decimais"));
        }
        Ok(())
    }
}
