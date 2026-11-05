//! The draft of a document, as the application hands it to the emitter.
//!
//! Business identifiers stay in Portuguese, because they are terms of the NF-e layout.

use rust_decimal::Decimal;

use crate::ambiente::{Modelo, Uf};
use crate::chave::{cnpj_valido, cpf_valido};
use crate::error::NfeError;

/// The text of the homologation recipient, required by rejection 598.
pub const TEXTO_HOMOLOGACAO_DESTINATARIO: &str = "NF-E EMITIDA EM AMBIENTE DE HOMOLOGACAO - SEM VALOR FISCAL";

/// The text of the first NFC-e item in homologation, required by rejection 373.
pub const TEXTO_HOMOLOGACAO_ITEM: &str = "NOTA FISCAL EMITIDA EM AMBIENTE DE HOMOLOGACAO - SEM VALOR FISCAL";

/// The sentence every document carries in `infCpl`, identifying the emitter as a beta.
pub const TEXTO_POC_BETA: &str =
    "EMITIDO POR RAVENRS-NFE: PROVA DE CONCEITO BETA, NAO HOMOLOGADA, SEM VALOR FISCAL.";

/// An address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endereco {
    /// Street, `xLgr`.
    pub logradouro: String,
    /// Number, `nro`.
    pub numero: String,
    /// District, `xBairro`.
    pub bairro: String,
    /// IBGE code of the city, `cMun`.
    pub codigo_municipio: String,
    /// City name, `xMun`.
    pub municipio: String,
    /// State.
    pub uf: Uf,
    /// Postal code, 8 digits.
    pub cep: String,
}

/// The issuer. Only Simples Nacional exists in this POC, so `CRT` is always 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Emitente {
    /// CNPJ as text, already accepting the alphanumeric form.
    pub cnpj: String,
    /// Legal name, `xNome`.
    pub razao_social: String,
    /// Trade name, `xFant`.
    pub nome_fantasia: Option<String>,
    /// State registration, `IE`.
    pub inscricao_estadual: String,
    /// Address.
    pub endereco: Endereco,
}

/// The tax id of a recipient.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentoDestinatario {
    /// A company, by CNPJ.
    Cnpj(String),
    /// A person, by CPF.
    Cpf(String),
}

/// The buyer. Required in model 55; optional in model 65.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Destinatario {
    /// CNPJ or CPF.
    pub documento: DocumentoDestinatario,
    /// Name, optional only in NFC-e.
    pub nome: Option<String>,
    /// Address, required in model 55.
    pub endereco: Option<Endereco>,
}

/// The tax situation of an item. Only two exist in this POC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Csosn {
    /// 102: Simples Nacional without credit.
    S102,
    /// 500: ICMS already charged by tax substitution.
    S500,
}

/// One line of the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Internal product code, `cProd`.
    pub codigo: String,
    /// GTIN, or `None` to write `SEM GTIN`.
    pub gtin: Option<String>,
    /// Description, `xProd`.
    pub descricao: String,
    /// NCM, 8 digits (or 2 for genre).
    pub ncm: String,
    /// CEST, 7 digits, for goods under tax substitution.
    pub cest: Option<String>,
    /// CFOP, 4 digits.
    pub cfop: String,
    /// Unit, `uCom`.
    pub unidade: String,
    /// Quantity, up to 4 decimal places.
    pub quantidade: Decimal,
    /// Unit price, up to 10 decimal places.
    pub valor_unitario: Decimal,
    /// Discount of this line, up to 2 decimal places.
    pub desconto: Decimal,
    /// CSOSN.
    pub csosn: Csosn,
    /// Origin of the goods, `orig`: 0 is national.
    pub origem: u8,
}

/// How a payment was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormaPagamento {
    /// 01: cash.
    Dinheiro,
    /// 03: credit card.
    CartaoCredito,
    /// 04: debit card.
    CartaoDebito,
    /// 17: Pix.
    Pix,
    /// 02: check.
    Cheque,
    /// 05: store credit.
    CreditoLoja,
    /// 10: food voucher.
    ValeAlimentacao,
    /// 11: meal voucher.
    ValeRefeicao,
    /// 12: gift voucher.
    ValePresente,
    /// 13: fuel voucher.
    ValeCombustivel,
    /// 15: bank slip.
    Boleto,
    /// 16: bank deposit.
    Deposito,
    /// 18: bank transfer or digital wallet.
    Transferencia,
    /// 19: loyalty program, cashback or virtual credit.
    Fidelidade,
    /// 90: no payment.
    SemPagamento,
    /// 99: others, with the description `xPag` requires.
    Outros,
}

impl FormaPagamento {
    /// The value of `tPag`.
    #[must_use]
    pub const fn codigo(self) -> &'static str {
        match self {
            Self::Dinheiro => "01",
            Self::CartaoCredito => "03",
            Self::CartaoDebito => "04",
            Self::Pix => "17",
            Self::Cheque => "02",
            Self::CreditoLoja => "05",
            Self::ValeAlimentacao => "10",
            Self::ValeRefeicao => "11",
            Self::ValePresente => "12",
            Self::ValeCombustivel => "13",
            Self::Boleto => "15",
            Self::Deposito => "16",
            Self::Transferencia => "18",
            Self::Fidelidade => "19",
            Self::SemPagamento => "90",
            Self::Outros => "99",
        }
    }

    /// Whether `card` may be informed: rule YA04-20 of NT 2023.004.
    #[must_use]
    pub const fn aceita_card(self) -> bool {
        matches!(
            self,
            Self::CartaoCredito
                | Self::CartaoDebito
                | Self::ValeAlimentacao
                | Self::ValeRefeicao
                | Self::ValePresente
                | Self::ValeCombustivel
                | Self::Boleto
                | Self::Pix
                | Self::Transferencia
        )
    }

    /// Whether `card` cannot be left out: rule YA04-10, which NT 2025.001 extended to Pix.
    #[must_use]
    pub const fn exige_card(self) -> bool {
        matches!(self, Self::CartaoCredito | Self::CartaoDebito | Self::Pix)
    }
}

/// How a card or Pix payment reached the system, `tpIntegra`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoIntegracao {
    /// 1: integrated, like TEF: `CNPJ` and `cAut` are expected (rule YA05-10).
    Integrado,
    /// 2: not integrated, like a stand-alone POS terminal.
    NaoIntegrado,
}

/// The `card` group of a payment. Despite the name, it also serves Pix, slips and vouchers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cartao {
    /// `tpIntegra`.
    pub integracao: TipoIntegracao,
    /// `CNPJ`: the payment institution.
    pub cnpj: Option<String>,
    /// `tBand`: the card brand, two digits.
    pub bandeira: Option<String>,
    /// `cAut`: the authorization number, exactly as it came from the terminal.
    pub autorizacao: Option<String>,
    /// `CNPJReceb`: who receives the payment.
    pub cnpj_recebedor: Option<String>,
    /// `idTermPag`: the terminal.
    pub terminal: Option<String>,
}

impl Cartao {
    /// A payment outside the system, with `tpIntegra=2`.
    #[must_use]
    pub fn nao_integrado() -> Self {
        Self {
            integracao: TipoIntegracao::NaoIntegrado,
            cnpj: None,
            bandeira: None,
            autorizacao: None,
            cnpj_recebedor: None,
            terminal: None,
        }
    }

    /// A payment through the system, with `tpIntegra=1` and what rule YA05-10 asks.
    #[must_use]
    pub fn integrado(cnpj: impl Into<String>, autorizacao: impl Into<String>) -> Self {
        Self {
            integracao: TipoIntegracao::Integrado,
            cnpj: Some(cnpj.into()),
            autorizacao: Some(autorizacao.into()),
            ..Self::nao_integrado()
        }
    }

    fn validar(&self, numero: usize) -> Result<(), NfeError> {
        let campo = |nome: &str| format!("pag/detPag[{numero}]/card/{nome}");
        if self.integracao == TipoIntegracao::Integrado && (self.cnpj.is_none() || self.autorizacao.is_none())
        {
            return Err(NfeError::validacao(&campo("tpIntegra"), "pagamento integrado exige CNPJ e cAut"));
        }
        if let Some(cnpj) = &self.cnpj
            && !cnpj_valido(cnpj)
        {
            return Err(NfeError::validacao(&campo("CNPJ"), "CNPJ inválido"));
        }
        if let Some(cnpj) = &self.cnpj_recebedor
            && !cnpj_valido(cnpj)
        {
            return Err(NfeError::validacao(&campo("CNPJReceb"), "CNPJ inválido"));
        }
        if let Some(bandeira) = &self.bandeira {
            exigir_digitos(&campo("tBand"), bandeira, &[2])?;
        }
        if let Some(autorizacao) = &self.autorizacao {
            exigir_texto(&campo("cAut"), autorizacao, 1, 128)?;
        }
        if let Some(terminal) = &self.terminal {
            exigir_texto(&campo("idTermPag"), terminal, 1, 40)?;
        }
        Ok(())
    }
}

/// One payment of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pagamento {
    /// The form.
    pub forma: FormaPagamento,
    /// The amount, up to 2 decimal places.
    pub valor: Decimal,
    /// The `card` group. Left empty on a form that requires it, it goes with `tpIntegra=2`.
    pub card: Option<Cartao>,
}

/// The technical contact of the software, `infRespTec`, required by some states.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResponsavelTecnico {
    /// CNPJ of the software company.
    pub cnpj: String,
    /// Contact person.
    pub contato: String,
    /// Contact e-mail.
    pub email: String,
    /// Contact phone, digits only.
    pub fone: String,
}

/// A draft: everything the application decides about a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rascunho {
    /// The model.
    pub modelo: Modelo,
    /// The series, 0 to 999.
    pub serie: u16,
    /// The number, 1 to 999999999.
    pub numero: u32,
    /// The nature of the operation, `natOp`.
    pub natureza: String,
    /// Moment of issue, as `2026-11-05T14:30:00-03:00`. The crate reads no clock.
    pub emitido_em: String,
    /// The buyer.
    pub destinatario: Option<Destinatario>,
    /// The lines.
    pub itens: Vec<Item>,
    /// The payments.
    pub pagamentos: Vec<Pagamento>,
    /// Free text added to `infCpl`, after the sentence identifying the beta.
    pub observacao: Option<String>,
    /// The random code `cNF`; when absent it derives from the document.
    pub codigo_numerico: Option<String>,
    /// The technical contact, for states that require it.
    pub responsavel_tecnico: Option<ResponsavelTecnico>,
}

impl Rascunho {
    /// A draft with the required fields, ready to get its lines and payments.
    pub fn novo(
        modelo: Modelo,
        serie: u16,
        numero: u32,
        natureza: impl Into<String>,
        emitido_em: impl Into<String>,
    ) -> Self {
        Self {
            modelo,
            serie,
            numero,
            natureza: natureza.into(),
            emitido_em: emitido_em.into(),
            destinatario: None,
            itens: Vec::new(),
            pagamentos: Vec::new(),
            observacao: None,
            codigo_numerico: None,
            responsavel_tecnico: None,
        }
    }

    /// Sets the buyer.
    #[must_use]
    pub fn com_destinatario(mut self, destinatario: Destinatario) -> Self {
        self.destinatario = Some(destinatario);
        self
    }

    /// Adds a line.
    #[must_use]
    pub fn com_item(mut self, item: Item) -> Self {
        self.itens.push(item);
        self
    }

    /// Adds a payment.
    #[must_use]
    pub fn com_pagamento(mut self, forma: FormaPagamento, valor: Decimal) -> Self {
        self.pagamentos.push(Pagamento { forma, valor, card: None });
        self
    }

    /// Adds a payment with its `card` group: cards, Pix, vouchers, slips and transfers.
    #[must_use]
    pub fn com_pagamento_cartao(mut self, forma: FormaPagamento, valor: Decimal, card: Cartao) -> Self {
        self.pagamentos.push(Pagamento { forma, valor, card: Some(card) });
        self
    }

    /// Sets the free text of `infCpl`.
    #[must_use]
    pub fn com_observacao(mut self, observacao: impl Into<String>) -> Self {
        self.observacao = Some(observacao.into());
        self
    }
}

/// The values rule B03-10 of the MOC forbids in `cNF`.
const CNF_PROIBIDOS: [&str; 20] = [
    "00000000", "11111111", "22222222", "33333333", "44444444", "55555555", "66666666", "77777777",
    "88888888", "99999999", "12345678", "23456789", "34567890", "45678901", "56789012", "67890123",
    "78901234", "89012345", "90123456", "01234567",
];

/// Whether a `cNF` passes rule B03-10: out of the list, and different from `nNF`.
#[must_use]
pub fn codigo_numerico_aceito(codigo: &str, numero: u32) -> bool {
    !CNF_PROIBIDOS.contains(&codigo) && codigo.parse::<u32>().ok() != Some(numero)
}

/// Whether text fits the schema's `TString`: no line breaks and no outer spaces.
fn texto_valido(texto: &str, minimo: usize, maximo: usize) -> bool {
    let tamanho = texto.chars().count();
    tamanho >= minimo
        && tamanho <= maximo
        && texto.chars().all(|c| matches!(c, ' '..='\u{ff}') && c != '\u{7f}')
        && !texto.starts_with(' ')
        && !texto.ends_with(' ')
}

fn exigir_texto(campo: &str, texto: &str, minimo: usize, maximo: usize) -> Result<(), NfeError> {
    if texto_valido(texto, minimo, maximo) {
        Ok(())
    } else {
        Err(NfeError::validacao(
            campo,
            format!("texto de {minimo} a {maximo} caracteres, sem espaços nas pontas nem quebras"),
        ))
    }
}

fn exigir_digitos(campo: &str, texto: &str, tamanhos: &[usize]) -> Result<(), NfeError> {
    if tamanhos.contains(&texto.len()) && texto.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        let lista: Vec<String> = tamanhos.iter().map(ToString::to_string).collect();
        Err(NfeError::validacao(campo, format!("só dígitos, com {} posições", lista.join(" ou "))))
    }
}

fn validar_endereco(prefixo: &str, endereco: &Endereco) -> Result<(), NfeError> {
    exigir_texto(&format!("{prefixo}/xLgr"), &endereco.logradouro, 2, 60)?;
    exigir_texto(&format!("{prefixo}/nro"), &endereco.numero, 1, 60)?;
    exigir_texto(&format!("{prefixo}/xBairro"), &endereco.bairro, 2, 60)?;
    exigir_digitos(&format!("{prefixo}/cMun"), &endereco.codigo_municipio, &[7])?;
    exigir_texto(&format!("{prefixo}/xMun"), &endereco.municipio, 2, 60)?;
    exigir_digitos(&format!("{prefixo}/CEP"), &endereco.cep, &[8])
}

impl Emitente {
    /// Checks the issuer's fields against the layout.
    pub fn validar(&self) -> Result<(), NfeError> {
        if !cnpj_valido(&self.cnpj) {
            return Err(NfeError::validacao("emit/CNPJ", "CNPJ inválido"));
        }
        exigir_texto("emit/xNome", &self.razao_social, 2, 60)?;
        if let Some(fantasia) = &self.nome_fantasia {
            exigir_texto("emit/xFant", fantasia, 1, 60)?;
        }
        exigir_digitos("emit/IE", &self.inscricao_estadual, &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14])?;
        validar_endereco("enderEmit", &self.endereco)
    }
}

impl Rascunho {
    /// Checks the draft against the layout, before any calculation.
    pub fn validar(&self) -> Result<(), NfeError> {
        if self.serie > 999 {
            return Err(NfeError::validacao("serie", "de 0 a 999"));
        }
        if self.numero == 0 || self.numero > 999_999_999 {
            return Err(NfeError::validacao("nNF", "de 1 a 999999999"));
        }
        exigir_texto("natOp", &self.natureza, 1, 60)?;
        validar_data_hora(&self.emitido_em)?;
        if self.itens.is_empty() || self.itens.len() > 990 {
            return Err(NfeError::validacao("det", "de 1 a 990 itens"));
        }
        for (posicao, item) in self.itens.iter().enumerate() {
            item.validar(posicao + 1)?;
        }
        if self.pagamentos.is_empty() || self.pagamentos.len() > 100 {
            return Err(NfeError::validacao("pag", "de 1 a 100 pagamentos"));
        }
        for (posicao, pagamento) in self.pagamentos.iter().enumerate() {
            let numero = posicao + 1;
            if let Some(card) = &pagamento.card {
                if !pagamento.forma.aceita_card() {
                    return Err(NfeError::validacao(
                        &format!("pag/detPag[{numero}]/card"),
                        "esta forma de pagamento não aceita o grupo de cartões",
                    ));
                }
                card.validar(numero)?;
            }
            if pagamento.forma == FormaPagamento::SemPagamento {
                if self.modelo == Modelo::Nfce65 {
                    return Err(NfeError::validacao(
                        &format!("pag/detPag[{numero}]/tPag"),
                        "a NFC-e não aceita \"sem pagamento\"",
                    ));
                }
                if !pagamento.valor.is_zero() {
                    return Err(NfeError::validacao(
                        &format!("pag/detPag[{numero}]/vPag"),
                        "\"sem pagamento\" tem valor zero",
                    ));
                }
            }
        }
        if let Some(codigo) = &self.codigo_numerico {
            exigir_digitos("cNF", codigo, &[8])?;
            if !codigo_numerico_aceito(codigo, self.numero) {
                return Err(NfeError::validacao(
                    "cNF",
                    "valor proibido pela regra B03-10, ou igual ao número da nota",
                ));
            }
        }
        match (&self.destinatario, self.modelo) {
            (None, Modelo::Nfe55) => {
                return Err(NfeError::validacao("dest", "a NF-e exige destinatário identificado"));
            }
            (Some(destinatario), modelo) => destinatario.validar(modelo)?,
            (None, Modelo::Nfce65) => {}
        }
        if let Some(observacao) = &self.observacao {
            exigir_texto("infCpl", observacao, 1, 4000)?;
        }
        if let Some(responsavel) = &self.responsavel_tecnico {
            if !cnpj_valido(&responsavel.cnpj) {
                return Err(NfeError::validacao("infRespTec/CNPJ", "CNPJ inválido"));
            }
            exigir_texto("infRespTec/xContato", &responsavel.contato, 2, 60)?;
            exigir_texto("infRespTec/email", &responsavel.email, 6, 60)?;
            exigir_digitos("infRespTec/fone", &responsavel.fone, &[6, 7, 8, 9, 10, 11, 12, 13, 14])?;
        }
        Ok(())
    }
}

impl Destinatario {
    fn validar(&self, modelo: Modelo) -> Result<(), NfeError> {
        match &self.documento {
            DocumentoDestinatario::Cnpj(cnpj) if !cnpj_valido(cnpj) => {
                return Err(NfeError::validacao("dest/CNPJ", "CNPJ inválido"));
            }
            DocumentoDestinatario::Cpf(cpf) if !cpf_valido(cpf) => {
                return Err(NfeError::validacao("dest/CPF", "CPF inválido"));
            }
            _ => {}
        }
        if let Some(nome) = &self.nome {
            exigir_texto("dest/xNome", nome, 2, 60)?;
        }
        if let Some(endereco) = &self.endereco {
            validar_endereco("enderDest", endereco)?;
        }
        if modelo == Modelo::Nfe55 && self.endereco.is_none() {
            return Err(NfeError::validacao("enderDest", "a NF-e exige o endereço do destinatário"));
        }
        Ok(())
    }
}

impl Item {
    fn validar(&self, numero: usize) -> Result<(), NfeError> {
        let campo = |nome: &str| format!("det[{numero}]/{nome}");
        exigir_texto(&campo("cProd"), &self.codigo, 1, 60)?;
        exigir_texto(&campo("xProd"), &self.descricao, 1, 120)?;
        exigir_digitos(&campo("NCM"), &self.ncm, &[2, 8])?;
        if let Some(cest) = &self.cest {
            exigir_digitos(&campo("CEST"), cest, &[7])?;
        }
        exigir_digitos(&campo("CFOP"), &self.cfop, &[4])?;
        if !matches!(self.cfop.as_bytes().first(), Some(b'1' | b'2' | b'3' | b'5' | b'6' | b'7')) {
            return Err(NfeError::validacao(&campo("CFOP"), "começa em 1, 2, 3, 5, 6 ou 7"));
        }
        exigir_texto(&campo("uCom"), &self.unidade, 1, 6)?;
        if let Some(gtin) = &self.gtin
            && !ravenrs_barcode::is_valid_gtin(gtin)
        {
            return Err(NfeError::validacao(&campo("cEAN"), "GTIN com dígito verificador incorreto"));
        }
        if self.origem > 8 {
            return Err(NfeError::validacao(&campo("orig"), "de 0 a 8"));
        }
        Ok(())
    }
}

fn dois_digitos(texto: &str, inicio: usize) -> Option<u32> {
    let parte = texto.get(inicio..inicio + 2)?;
    if parte.bytes().all(|b| b.is_ascii_digit()) { parte.parse().ok() } else { None }
}

/// Checks a moment against the shape `AAAA-MM-DDThh:mm:ss-03:00` of `TDateTimeUTC`.
pub(crate) fn validar_data_hora(texto: &str) -> Result<(), NfeError> {
    let erro = || NfeError::validacao("dhEmi", "use a forma 2026-11-05T14:30:00-03:00");
    let bytes = texto.as_bytes();
    if bytes.len() != 25
        || !texto.starts_with("20")
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || !matches!(bytes[19], b'-' | b'+')
        || bytes[22] != b':'
        || &bytes[23..] != b"00"
    {
        return Err(erro());
    }
    let ano = dois_digitos(texto, 2).ok_or_else(erro)?;
    let mes = dois_digitos(texto, 5).ok_or_else(erro)?;
    let dia = dois_digitos(texto, 8).ok_or_else(erro)?;
    let hora = dois_digitos(texto, 11).ok_or_else(erro)?;
    let minuto = dois_digitos(texto, 14).ok_or_else(erro)?;
    let segundo = dois_digitos(texto, 17).ok_or_else(erro)?;
    let fuso = dois_digitos(texto, 20).ok_or_else(erro)?;
    let dias_no_mes = match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if ano % 4 == 0 => 29,
        2 => 28,
        _ => return Err(erro()),
    };
    if dia == 0 || dia > dias_no_mes || hora > 23 || minuto > 59 || segundo > 59 || fuso > 11 {
        return Err(erro());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moment_shape_is_checked() {
        assert!(validar_data_hora("2026-11-05T14:30:00-03:00").is_ok());
        assert!(validar_data_hora("2028-02-29T14:30:00+00:00").is_ok());
        assert!(validar_data_hora("2026-02-29T14:30:00-03:00").is_err());
        assert!(validar_data_hora("2026-11-05 14:30:00").is_err());
        assert!(validar_data_hora("2026-13-05T14:30:00-03:00").is_err());
        assert!(validar_data_hora("2026-11-05T14:30:00-03:30").is_err());
    }

    #[test]
    fn text_rules_follow_the_schema() {
        assert!(texto_valido("ARROZ TIPO 1", 1, 60));
        assert!(!texto_valido(" ARROZ", 1, 60));
        assert!(!texto_valido("ARROZ\nTIPO", 1, 60));
        assert!(texto_valido("Açúcar", 1, 60));
        assert!(!texto_valido("emoji \u{1f600}", 1, 60));
    }
}
