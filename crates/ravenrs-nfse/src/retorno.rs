//! Reading the answers of the Sefin: JSON around documents in GZip + Base64.
//!
//! The names of the fields (`chaveAcesso`, `nfseXmlGZipB64`, `eventoXmlGZipB64`, `erros` or `erro`,
//! with `codigo` or `Codigo`) come from the code of two projects, not from an official source: the
//! Swagger did not open. They are read with tolerance until a real answer is captured.

use ravenrs_dfe::HttpResponse;
use serde_json::Value;

use crate::erro::{Mensagem, NfseError};

/// The body of an answer as JSON.
pub fn json(resposta: &HttpResponse) -> Result<Value, NfseError> {
    serde_json::from_slice(&resposta.body).map_err(|_| NfseError::RetornoInvalido {
        status: resposta.status,
        motivo: "a resposta não é JSON".to_owned(),
    })
}

/// A text field, by any of these names.
#[must_use]
pub fn texto(valor: &Value, nomes: &[&str]) -> Option<String> {
    nomes.iter().find_map(|nome| valor.get(*nome)).and_then(Value::as_str).map(str::to_owned)
}

fn mensagem(valor: &Value) -> Mensagem {
    Mensagem {
        codigo: texto(valor, &["codigo", "Codigo"]).unwrap_or_else(|| "?".to_owned()),
        descricao: texto(valor, &["descricao", "Descricao", "mensagem", "Mensagem"]).unwrap_or_default(),
        complemento: texto(valor, &["complemento", "Complemento"]),
    }
}

/// The messages under `erros`, `erro` or `alertas`.
#[must_use]
pub fn mensagens(valor: &Value, nomes: &[&str]) -> Vec<Mensagem> {
    nomes
        .iter()
        .filter_map(|nome| valor.get(*nome))
        .flat_map(|item| match item {
            Value::Array(lista) => lista.iter().map(mensagem).collect(),
            Value::Object(_) => vec![mensagem(item)],
            _ => Vec::new(),
        })
        .collect()
}

/// The refusal of an answer, if it is one: status 4xx with messages.
#[must_use]
pub fn recusa(resposta: &HttpResponse) -> Option<NfseError> {
    if !(400..500).contains(&resposta.status) {
        return None;
    }
    let corpo = json(resposta).ok()?;
    let erros = mensagens(&corpo, &["erros", "erro"]);
    (!erros.is_empty()).then_some(NfseError::Rejeitada(erros))
}

/// A document in GZip + Base64 under this field.
pub fn documento(resposta: &HttpResponse, campo: &str) -> Result<String, NfseError> {
    let corpo = json(resposta)?;
    let codificado = texto(&corpo, &[campo]).ok_or_else(|| NfseError::RetornoInvalido {
        status: resposta.status,
        motivo: format!("{campo} ausente"),
    })?;
    Ok(ravenrs_dfe::compactacao::de_gzip_base64(&codificado)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resposta(status: u16, corpo: &str) -> HttpResponse {
        HttpResponse { status, headers: Vec::new(), body: corpo.as_bytes().to_vec() }
    }

    #[test]
    fn reads_errors_in_either_spelling() {
        let a = recusa(&resposta(400, r#"{"erros":[{"Codigo":"E0014","Descricao":"DPS já existe"}]}"#));
        let b = recusa(&resposta(400, r#"{"erro":{"codigo":"E0014","descricao":"DPS já existe"}}"#));
        assert_eq!(a.as_ref().map(NfseError::codigos), Some(vec!["E0014"]));
        assert_eq!(a, b);
        assert_eq!(recusa(&resposta(201, "{}")), None);
    }
}
