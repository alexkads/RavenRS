//! The few bytes a driver may send, and the check that they are nothing else.
//!
//! A manifest cannot carry an arbitrary command. Each slot accepts only whole commands from a short
//! list, matched to the last byte, so one command cannot hide a second one after it. The list leaves
//! out everything that writes to the printer's memory or changes its network.

use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The longest command a manifest can hold, in bytes.
pub(crate) const TAMANHO_MAXIMO: usize = 16;

const ESC: u8 = 0x1B;
const GS: u8 = 0x1D;

/// The bytes of one command. In JSON it is hexadecimal text, such as `"1D 56 42 18"`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Comando {
    bytes: [u8; TAMANHO_MAXIMO],
    tamanho: u8,
}

impl Comando {
    /// A command with these bytes, if they fit. Having one does not make it acceptable: that is
    /// checked with the manifest it is in.
    pub fn novo(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() || bytes.len() > TAMANHO_MAXIMO {
            return None;
        }
        let mut guardados = [0; TAMANHO_MAXIMO];
        guardados[..bytes.len()].copy_from_slice(bytes);
        Some(Self { bytes: guardados, tamanho: bytes.len() as u8 })
    }

    /// The bytes to send.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.tamanho)]
    }

    fn de_hex(texto: &str) -> Option<Self> {
        let digitos: Vec<u8> = texto.bytes().filter(|b| *b != b' ').collect();
        if !digitos.len().is_multiple_of(2) {
            return None;
        }
        let mut bytes = Vec::with_capacity(digitos.len() / 2);
        for par in digitos.as_chunks::<2>().0 {
            bytes.push(u8::from_str_radix(std::str::from_utf8(par).ok()?, 16).ok()?);
        }
        Self::novo(&bytes)
    }
}

impl fmt::Display for Comando {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (indice, byte) in self.bytes().iter().enumerate() {
            if indice > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{byte:02X}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Comando {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Comando({self})")
    }
}

impl Serialize for Comando {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Comando {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(deserializer)?;
        Self::de_hex(&texto).ok_or_else(|| {
            D::Error::custom(format!(
                "\"{texto}\" não é um comando em hexadecimal de até {TAMANHO_MAXIMO} bytes"
            ))
        })
    }
}

/// `GS V`: `m` alone, or `m n` for the forms that feed first.
pub(crate) fn corte_aceito(bytes: &[u8]) -> bool {
    match bytes {
        [GS, b'V', 0 | 1 | 48 | 49] | [GS, b'V', 65 | 66, _] => true,
        // ESC i, ESC m: the cuts of ESC/Bema.
        [ESC, b'i' | b'm'] => true,
        _ => false,
    }
}

/// `ESC ( A` as Epson and Bematech have it, whose length must be what it says, or `ESC B n t`.
pub(crate) fn bipe_aceito(bytes: &[u8]) -> bool {
    match bytes {
        [ESC, b'(', b'A', baixo, 0, funcao, resto @ ..] => {
            usize::from(*baixo) == resto.len() + 1 && matches!(funcao, 0x61 | 0x01 | 0x31)
        }
        [ESC, b'B', _, _] => true,
        _ => false,
    }
}

/// `ESC p m t1 t2`, or the drawer commands of ESC/Bema: `ESC v n` and `ESC 80h n`.
pub(crate) fn gaveta_aceita(bytes: &[u8]) -> bool {
    matches!(bytes, [ESC, b'p', 0 | 1 | 48 | 49, _, _] | [ESC, b'v' | 0x80, _])
}

/// Only the mode change that is not saved, `GS F9h SP n`. The one that saves (`GS F9h 5 n`) and the
/// rest of that family, which set the network, stay out.
pub(crate) fn modo_aceito(bytes: &[u8]) -> bool {
    matches!(bytes, [GS, 0xF9, 0x20, 0 | 1 | 48 | 49])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comando(texto: &str) -> Option<Comando> {
        Comando::de_hex(texto)
    }

    #[test]
    fn hex_goes_both_ways() {
        let lido = comando("1d 56 42 18");
        assert_eq!(lido.map(|c| c.to_string()), Some("1D 56 42 18".to_owned()));
        assert_eq!(comando("1D564218").map(|c| c.bytes().to_vec()), Some(vec![0x1D, 0x56, 0x42, 0x18]));
        for ruim in ["", "1", "ZZ", "é1", &"00".repeat(17)] {
            assert_eq!(comando(ruim), None, "{ruim}");
        }
    }

    #[test]
    fn only_whole_cut_commands_pass() {
        assert!(corte_aceito(&[GS, b'V', 66, 24]));
        assert!(corte_aceito(&[GS, b'V', 48]));
        assert!(!corte_aceito(&[GS, b'V', 66, 24, ESC, b'@']), "a second command after the first");
        assert!(!corte_aceito(&[GS, b'V', 66]));
        assert!(!corte_aceito(&[ESC, b'@']));
    }

    #[test]
    fn a_beep_must_have_the_length_it_declares() {
        assert!(bipe_aceito(&[ESC, b'(', b'A', 3, 0, 97, 1, 1]));
        assert!(bipe_aceito(&[ESC, b'(', b'A', 5, 0, 0x61, 0x64, 1, 2, 1]));
        assert!(!bipe_aceito(&[ESC, b'(', b'A', 3, 0, 97, 1, 1, ESC, b'@']));
        assert!(!bipe_aceito(&[ESC, b'(', b'A', 3, 0, 0x30, 1, 1]));
    }

    #[test]
    fn the_mode_change_that_saves_is_refused() {
        assert!(modo_aceito(&[GS, 0xF9, 0x20, 1]));
        assert!(!modo_aceito(&[GS, 0xF9, 0x35, 1]), "saves to the printer's memory");
        assert!(!modo_aceito(&[GS, 0xF9, b'W', 1]), "the same family sets the network");
    }

    #[test]
    fn the_drawer_takes_its_commands_only() {
        assert!(gaveta_aceita(&[ESC, b'p', 0, 25, 250]));
        assert!(gaveta_aceita(&[ESC, b'v', 100]));
        assert!(!gaveta_aceita(&[ESC, b'p', 7, 25, 250]));
    }
}
