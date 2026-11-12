//! What the DANFSe has to have, from the NT 008 (v1.02, 14/07/2026): since 03/08/2026 the API of the
//! DANFSe is suspended and the issuing software draws it. One page, portrait, at least A4, a QR Code
//! of at least 1.52 cm, and "NFS-e SEM VALIDADE JURÍDICA" when `tpAmb=2`. The layout itself is a
//! report of the standard designer (`ravenrs-app-nfe`), which anyone can change.

use crate::chave::ChaveNfse;

/// The notice the NT 008 asks for when `tpAmb=2`. This crate only issues with `tpAmb=2`.
pub const AVISO_SEM_VALIDADE: &str = "NFS-e SEM VALIDADE JURÍDICA";

/// The smallest side of the QR Code, in centimeters.
pub const QR_MINIMO_CM: f32 = 1.52;

/// The smallest side of the QR Code, in points of 1/72 inch.
pub const QR_MINIMO_PONTOS: f32 = QR_MINIMO_CM / 2.54 * 72.0;

/// The address the QR Code opens: the public query of the NFS-e by its key.
///
/// The NT 008 was read for the rules of the page, not for this address, which comes from the
/// public query portal and is **not confirmed** for the QR Code. Check it before the first real use.
#[must_use]
pub fn conteudo_qr(chave: &ChaveNfse) -> String {
    format!("https://www.nfse.gov.br/ConsultaPublica/?tpc=1&chave={chave}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_smallest_qr_is_about_43_points() {
        assert!((QR_MINIMO_PONTOS - 43.09).abs() < 0.01);
        let chave = ChaveNfse::parse(&"3".repeat(50)).expect("key");
        assert!(conteudo_qr(&chave).ends_with(&"3".repeat(50)));
    }
}
