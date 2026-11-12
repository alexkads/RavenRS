//! GZip + Base64, the way the national environments carry documents: `docZip` in the distribution
//! of NF-e, `dpsXmlGZipB64` and `nfseXmlGZipB64` in the NFS-e. Reading always has a limit, so a
//! gzip bomb stops before it fills the memory.

use std::io::{Read, Write};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::erro::DfeError;

/// The largest size a decompressed document may reach.
pub const LIMITE_DOCUMENTO: u64 = 20 * 1024 * 1024;

/// Compresses the text with GZip and encodes it in Base64.
pub fn gzip_base64(texto: &str) -> Result<String, DfeError> {
    let mut codificador = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    codificador.write_all(texto.as_bytes()).map_err(|erro| DfeError::Conteudo(format!("GZip: {erro}")))?;
    let bytes = codificador.finish().map_err(|erro| DfeError::Conteudo(format!("GZip: {erro}")))?;
    Ok(STANDARD.encode(bytes))
}

/// Decodes Base64 and decompresses GZip into UTF-8 text, refusing more than [`LIMITE_DOCUMENTO`].
pub fn de_gzip_base64(codificado: &str) -> Result<String, DfeError> {
    let limpo: String = codificado.split_whitespace().collect();
    let bytes =
        STANDARD.decode(limpo).map_err(|_| DfeError::Conteudo("o conteúdo não é Base64".to_owned()))?;
    de_gzip(&bytes)
}

/// Decompresses GZip into UTF-8 text, refusing more than [`LIMITE_DOCUMENTO`].
pub fn de_gzip(bytes: &[u8]) -> Result<String, DfeError> {
    let mut texto = String::new();
    flate2::read::GzDecoder::new(bytes)
        .take(LIMITE_DOCUMENTO + 1)
        .read_to_string(&mut texto)
        .map_err(|_| DfeError::Conteudo("o conteúdo não é GZip de texto UTF-8".to_owned()))?;
    if texto.len() as u64 > LIMITE_DOCUMENTO {
        return Err(DfeError::Conteudo("documento descompactado grande demais".to_owned()));
    }
    Ok(texto)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goes_and_comes_back() {
        let xml = "<DPS xmlns=\"http://www.sped.fazenda.gov.br/nfse\"><infDPS Id=\"DPS1\"/></DPS>";
        assert_eq!(de_gzip_base64(&gzip_base64(xml).expect("compressed")).expect("read"), xml);
    }

    #[test]
    fn refuses_what_is_not_base64_or_gzip() {
        assert!(matches!(de_gzip_base64("@@@"), Err(DfeError::Conteudo(_))));
        assert!(matches!(de_gzip_base64(&STANDARD.encode("texto puro")), Err(DfeError::Conteudo(_))));
    }

    #[test]
    fn a_bomb_stops_at_the_limit() {
        let grande = "a".repeat((LIMITE_DOCUMENTO + 10) as usize);
        let bomba = gzip_base64(&grande).expect("compressed");
        assert!(bomba.len() < 100_000, "it really is small");
        assert!(matches!(de_gzip_base64(&bomba), Err(DfeError::Conteudo(_))));
    }
}
