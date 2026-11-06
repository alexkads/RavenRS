//! The access key of the NFS-e: 50 digits, given by the Sefin in its answer.
//!
//! Its parts (XSD v1.01): city (7) + generating environment (1) + kind of registration (1) +
//! registration (14) + NFS-e number (13) + year and month (4) + numeric code (9) + check digit (1).
//! The number is given by the Sefin, so the key always comes from the answer, never computed here.
//! The algorithm of the check digit is not confirmed; only the sandbox computes one, by modulo 11.

use std::fmt;

use crate::erro::NfseError;

/// The access key, as text.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChaveNfse(String);

impl ChaveNfse {
    /// Reads a key: 50 digits, or `NFS` followed by them, as in the Id of the NFS-e.
    pub fn parse(texto: &str) -> Result<Self, NfseError> {
        let texto = texto.trim();
        let digitos = texto.strip_prefix("NFS").unwrap_or(texto);
        if digitos.len() != 50 || !digitos.bytes().all(|b| b.is_ascii_digit()) {
            return Err(NfseError::validacao("chNFSe", "a chave da NFS-e tem 50 dígitos"));
        }
        Ok(Self(digitos.to_owned()))
    }

    /// The digits.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The city where it was issued.
    #[must_use]
    pub fn municipio(&self) -> &str {
        &self.0[..7]
    }

    /// The number of the NFS-e.
    #[must_use]
    pub fn numero(&self) -> u64 {
        self.0[23..36].parse().unwrap_or(0)
    }
}

impl fmt::Display for ChaveNfse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Modulo 11 with weights 2 to 9 from the right, as in the NF-e. Only the sandbox uses it: the
/// real check digit of the NFS-e is not confirmed.
#[must_use]
pub fn dv_modulo_11(digitos: &str) -> u8 {
    let soma: u32 = digitos
        .bytes()
        .rev()
        .zip((2..=9).cycle())
        .map(|(digito, peso)| u32::from(digito - b'0') * peso)
        .sum();
    match 11 - soma % 11 {
        10 | 11 => 0,
        dv => dv as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_with_or_without_the_prefix() {
        let digitos = "3550308".to_owned()
            + "2"
            + "2"
            + "11222333000181"
            + "0000000000042"
            + "2611"
            + "123456789"
            + "0";
        let chave = ChaveNfse::parse(&format!("NFS{digitos}")).expect("key");
        assert_eq!(chave.as_str(), digitos);
        assert_eq!(chave.municipio(), "3550308");
        assert_eq!(chave.numero(), 42);
        assert!(ChaveNfse::parse("123").is_err());
    }
}
