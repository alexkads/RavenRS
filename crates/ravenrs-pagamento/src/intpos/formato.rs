//! The text of an `IntPos` file: one `NNN-SSS = value` per line. It touches no disk.

use std::collections::BTreeMap;

/// A request or a response of the standard manager.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArquivoIntPos {
    campos: BTreeMap<(u16, u16), String>,
}

impl ArquivoIntPos {
    /// A new request with this command and identification, and the closing record.
    #[must_use]
    pub fn novo(comando: &str, identificacao: u64) -> Self {
        let mut arquivo = Self::default();
        arquivo.definir(0, 0, comando);
        arquivo.definir(1, 0, identificacao.to_string());
        arquivo.definir(999, 999, "0");
        arquivo
    }

    /// Sets a field.
    pub fn definir(&mut self, campo: u16, sequencia: u16, valor: impl Into<String>) {
        self.campos.insert((campo, sequencia), valor.into());
    }

    /// Reads a field.
    #[must_use]
    pub fn campo(&self, campo: u16, sequencia: u16) -> Option<&str> {
        self.campos.get(&(campo, sequencia)).map(String::as_str)
    }

    /// Reads a field with sequence zero, as a value that is not empty.
    #[must_use]
    pub fn valor(&self, campo: u16) -> Option<&str> {
        self.campo(campo, 0).filter(|valor| !valor.is_empty())
    }

    /// `000-000`.
    #[must_use]
    pub fn comando(&self) -> Option<&str> {
        self.valor(0)
    }

    /// `001-000`.
    #[must_use]
    pub fn identificacao(&self) -> Option<&str> {
        self.valor(1)
    }

    /// True when the closing record `999-999` is here: the file was written to its end.
    #[must_use]
    pub fn completo(&self) -> bool {
        self.campos.contains_key(&(999, 999))
    }

    /// The lines from `sequencia` 1 up to the count held in field `tamanho`, without their quotes.
    #[must_use]
    pub fn linhas(&self, tamanho: u16, linha: u16) -> Vec<String> {
        let total = self.valor(tamanho).and_then(|valor| valor.trim().parse::<u16>().ok()).unwrap_or(0);
        (1..=total).map(|sequencia| self.campo(linha, sequencia).unwrap_or_default().to_owned()).collect()
    }

    /// Writes the bytes, ordered by field. Text goes as Latin-1, which the manager is taken to use;
    /// that encoding is not stated in the documents read.
    #[must_use]
    pub fn codificar(&self) -> Vec<u8> {
        let mut saida = Vec::new();
        for ((campo, sequencia), valor) in &self.campos {
            let texto = format!("{campo:03}-{sequencia:03} = {valor}\r\n");
            saida.extend(texto.chars().map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')));
        }
        saida
    }

    /// Reads the bytes. A line that is no field is skipped, and quotes around a value are removed.
    ///
    /// The documentation of PayGo shows `029-001: "text"` in one place and the code of others uses
    /// ` = `, so both are read.
    #[must_use]
    pub fn ler(bytes: &[u8]) -> Self {
        let texto: String = bytes.iter().map(|b| char::from(*b)).collect();
        let mut arquivo = Self::default();
        for linha in texto.lines() {
            if let Some((chave, valor)) = dividir(linha) {
                arquivo.campos.insert(chave, sem_aspas(valor.trim()).to_owned());
            }
        }
        arquivo
    }
}

fn dividir(linha: &str) -> Option<((u16, u16), &str)> {
    let cabeca = linha.get(..7)?;
    let (campo, sequencia) = cabeca.split_once('-')?;
    let chave = (campo.parse().ok()?, sequencia.parse().ok()?);
    let resto = linha.get(7..)?.trim_start();
    Some((chave, resto.strip_prefix('=').or_else(|| resto.strip_prefix(':'))?))
}

fn sem_aspas(valor: &str) -> &str {
    valor.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(valor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vai_e_volta_em_ordem_e_com_fim() {
        let mut pedido = ArquivoIntPos::novo("CRT", 1234);
        pedido.definir(4, 0, "0");
        pedido.definir(3, 0, "1990");
        let bytes = pedido.codificar();
        assert_eq!(
            String::from_utf8(bytes.clone()).expect("ascii"),
            "000-000 = CRT\r\n001-000 = 1234\r\n003-000 = 1990\r\n004-000 = 0\r\n999-999 = 0\r\n"
        );
        assert_eq!(ArquivoIntPos::ler(&bytes), pedido);
    }

    #[test]
    fn aspas_e_os_dois_separadores() {
        let arquivo = ArquivoIntPos::ler(b"028-000 = 2\r\n029-001 = \"LINHA 1\"\r\n029-002: \"LINHA 2\"\r\n");
        assert_eq!(arquivo.linhas(28, 29), ["LINHA 1", "LINHA 2"]);
    }

    #[test]
    fn latin1_vai_e_volta() {
        let mut arquivo = ArquivoIntPos::default();
        arquivo.definir(30, 0, "APROVAÇÃO");
        let bytes = arquivo.codificar();
        assert!(bytes.windows(2).any(|j| j == [0xC7, 0xC3]));
        assert_eq!(ArquivoIntPos::ler(&bytes).valor(30), Some("APROVAÇÃO"));
    }

    #[test]
    fn lixo_e_arquivo_pela_metade() {
        let arquivo = ArquivoIntPos::ler(b"nada\r\n000-000 = CRT\r\n001-");
        assert_eq!(arquivo.comando(), Some("CRT"));
        assert!(!arquivo.completo());
    }
}
