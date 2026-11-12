//! Access keys and tax ids, always as text, already accepting the alphanumeric CNPJ.

use std::fmt;

use crate::ambiente::{Modelo, Uf};
use crate::error::NfeError;

/// The value of one character in the mod-11 check: its ASCII code minus 48.
///
/// Digits keep their own value and `A` is worth 17, which is what the alphanumeric CNPJ uses.
fn valor(caractere: u8) -> u32 {
    u32::from(caractere).saturating_sub(48)
}

/// The mod-11 check digit with weights 2 to 9 from the right, the rule of `cDV` and of the CNPJ.
#[must_use]
pub fn digito_mod11(texto: &str) -> u8 {
    let soma: u32 = texto
        .bytes()
        .rev()
        .enumerate()
        .map(|(posicao, caractere)| valor(caractere) * ((posicao as u32 % 8) + 2))
        .sum();
    let resto = soma % 11;
    if resto < 2 { 0 } else { (11 - resto) as u8 }
}

/// Whether the text is a CNPJ: 12 capitals or digits followed by two valid check digits.
#[must_use]
pub fn cnpj_valido(cnpj: &str) -> bool {
    let bytes = cnpj.as_bytes();
    if bytes.len() != 14 {
        return false;
    }
    let (base, dv) = bytes.split_at(12);
    if !base.iter().all(|b| b.is_ascii_digit() || b.is_ascii_uppercase()) {
        return false;
    }
    if !dv.iter().all(u8::is_ascii_digit) || bytes.iter().all(|b| *b == bytes[0]) {
        return false;
    }
    let primeiro = digito_mod11(&cnpj[..12]);
    let segundo = digito_mod11(&cnpj[..13]);
    dv[0] - b'0' == primeiro && dv[1] - b'0' == segundo
}

/// Whether the text is a CPF: 11 digits with two valid check digits.
#[must_use]
pub fn cpf_valido(cpf: &str) -> bool {
    let bytes = cpf.as_bytes();
    if bytes.len() != 11 || !bytes.iter().all(u8::is_ascii_digit) {
        return false;
    }
    if bytes.iter().all(|b| *b == bytes[0]) {
        return false;
    }
    let digito = |tamanho: usize| -> u8 {
        let soma: u32 = bytes[..tamanho]
            .iter()
            .enumerate()
            .map(|(posicao, b)| u32::from(b - b'0') * (tamanho as u32 + 1 - posicao as u32))
            .sum();
        let resto = soma % 11;
        if resto < 2 { 0 } else { (11 - resto) as u8 }
    };
    bytes[9] - b'0' == digito(9) && bytes[10] - b'0' == digito(10)
}

/// The fields concatenated into an access key.
#[derive(Clone, Debug)]
pub struct CamposChave<'a> {
    /// The issuer's state.
    pub uf: Uf,
    /// Year and month of issue, `AAMM`.
    pub aamm: &'a str,
    /// The issuer's CNPJ.
    pub cnpj: &'a str,
    /// The document model.
    pub modelo: Modelo,
    /// The series, up to 999.
    pub serie: u16,
    /// The number, up to 999999999.
    pub numero: u32,
    /// The kind of issue, `tpEmis`.
    pub tp_emis: u8,
    /// The 8-digit random code, `cNF`.
    pub c_nf: &'a str,
}

/// An access key of 44 characters, with a valid check digit.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChaveAcesso(String);

impl ChaveAcesso {
    /// Builds the key from its fields and computes the check digit.
    pub fn montar(campos: &CamposChave<'_>) -> Result<Self, NfeError> {
        let semente = format!(
            "{}{}{}{}{:03}{:09}{}{}",
            campos.uf.codigo_ibge(),
            campos.aamm,
            campos.cnpj,
            campos.modelo.codigo(),
            campos.serie,
            campos.numero,
            campos.tp_emis,
            campos.c_nf,
        );
        if semente.len() != 43 {
            return Err(NfeError::validacao("chNFe", "os campos da chave não somam 43 posições"));
        }
        let digito = digito_mod11(&semente);
        Self::parse(&format!("{semente}{digito}"))
    }

    /// Reads a key from text, checking its shape and its check digit.
    pub fn parse(texto: &str) -> Result<Self, NfeError> {
        if !forma_valida(texto) {
            return Err(NfeError::validacao("chNFe", "a chave de acesso não tem a forma esperada"));
        }
        let (semente, dv) = texto.split_at(43);
        if dv.as_bytes()[0] - b'0' != digito_mod11(semente) {
            return Err(NfeError::validacao("chNFe", "dígito verificador da chave incorreto"));
        }
        Ok(Self(texto.to_string()))
    }

    /// The key as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The check digit, `cDV`.
    #[must_use]
    pub fn dv(&self) -> &str {
        &self.0[43..]
    }

    /// The `cUF` part.
    #[must_use]
    pub fn c_uf(&self) -> &str {
        &self.0[..2]
    }
}

impl fmt::Display for ChaveAcesso {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether the text has the shape the schema allows: `[0-9]{6}[0-9A-Z]{12}[0-9]{26}`.
pub(crate) fn forma_valida(texto: &str) -> bool {
    let bytes = texto.as_bytes();
    bytes.len() == 44
        && bytes[..6].iter().all(u8::is_ascii_digit)
        && bytes[6..18].iter().all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
        && bytes[18..].iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_cnpj_is_checked() {
        assert!(cnpj_valido("11222333000181"));
        assert!(!cnpj_valido("11222333000182"));
        assert!(!cnpj_valido("11111111111111"));
    }

    #[test]
    fn alphanumeric_cnpj_uses_ascii_minus_48() {
        // The example the Receita Federal published for the alphanumeric CNPJ.
        assert!(cnpj_valido("12ABC34501DE35"));
        assert!(!cnpj_valido("12ABC34501DE36"));
        assert!(!cnpj_valido("12abc34501DE35"));
    }

    #[test]
    fn cpf_is_checked() {
        assert!(cpf_valido("11144477735"));
        assert!(!cpf_valido("11144477736"));
        assert!(!cpf_valido("00000000000"));
    }

    #[test]
    fn key_gets_its_check_digit_and_reads_back() {
        let chave = ChaveAcesso::montar(&CamposChave {
            uf: Uf::SP,
            aamm: "2611",
            cnpj: "12ABC34501DE35",
            modelo: Modelo::Nfce65,
            serie: 1,
            numero: 42,
            tp_emis: 1,
            c_nf: "12345678",
        })
        .expect("key");
        assert_eq!(chave.as_str().len(), 44);
        assert_eq!(ChaveAcesso::parse(chave.as_str()), Ok(chave.clone()));
        let digito: u8 = chave.dv().parse().expect("digit");
        let errada = format!("{}{}", &chave.as_str()[..43], (digito + 1) % 10);
        assert!(ChaveAcesso::parse(&errada).is_err());
    }

    #[test]
    fn key_with_lowercase_is_refused() {
        assert!(!forma_valida("35261112abc34501DE35650010000000421123456780"));
    }

    #[test]
    fn mod11_matches_a_hand_computed_value() {
        // 2*4 + 3*3 + 4*2 = 25, 25 % 11 = 3, 11 - 3 = 8.
        assert_eq!(digito_mod11("234"), 8);
        // A remainder below 2 gives zero: 2*0 = 0.
        assert_eq!(digito_mod11("0"), 0);
    }
}
