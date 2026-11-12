//! Records what goes through any transport, and plays it back with no network.
//!
//! A recording is JSON Lines, one call per line, with personal data masked and with no certificate
//! or password in it. Bodies that were masked no longer verify: the recording serves to replay the
//! exchange and read what happened, not to prove a signature.

use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use super::mascara::mascarar_xml;
use super::tempo::RelogioControlado;
use crate::error::NfeError;
use crate::soap::acao_do_content_type;
use crate::transporte::{HttpRequest, HttpResponse, Transport, TransportError};

/// Headers that never go in a recording.
const CABECALHOS_SECRETOS: [&str; 4] = ["authorization", "cookie", "set-cookie", "proxy-authorization"];

fn cabecalhos(lista: &[(String, String)]) -> Value {
    let filtrados: Vec<Value> = lista
        .iter()
        .filter(|(nome, _)| !CABECALHOS_SECRETOS.contains(&nome.to_ascii_lowercase().as_str()))
        .map(|(nome, valor)| json!([nome, valor]))
        .collect();
    Value::Array(filtrados)
}

fn corpo_mascarado(bytes: &[u8]) -> String {
    mascarar_xml(&String::from_utf8_lossy(bytes))
}

fn erro_em_json(erro: &TransportError) -> Value {
    let (tipo, texto) = match erro {
        TransportError::Timeout => ("timeout", String::new()),
        TransportError::Conexao(texto) => ("conexao", texto.clone()),
        TransportError::Tls(texto) => ("tls", texto.clone()),
        TransportError::Redirecionamento(texto) => ("redirecionamento", texto.clone()),
        TransportError::NaoTratado { url, .. } => ("nao_tratado", url.clone()),
        other => ("outro", other.to_string()),
    };
    json!({ "tipo": tipo, "texto": texto })
}

fn erro_de_json(valor: &Value) -> TransportError {
    let texto = valor["texto"].as_str().unwrap_or_default().to_string();
    match valor["tipo"].as_str().unwrap_or_default() {
        "timeout" => TransportError::Timeout,
        "conexao" => TransportError::Conexao(texto),
        "tls" => TransportError::Tls(texto),
        "redirecionamento" => TransportError::Redirecionamento(texto),
        "nao_tratado" => TransportError::NaoTratado { url: texto, acao: None },
        _ => TransportError::Outro(texto),
    }
}

/// Wraps any transport, the real one included, and records each call.
pub struct Gravador<T: Transport> {
    interno: T,
    linhas: Mutex<Vec<String>>,
    destino: Mutex<Option<Box<dyn Write + Send>>>,
    sequencia: AtomicU64,
    relogio: Option<RelogioControlado>,
}

impl<T: Transport> std::fmt::Debug for Gravador<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gravador").finish_non_exhaustive()
    }
}

impl<T: Transport> Gravador<T> {
    /// Records in memory.
    #[must_use]
    pub fn novo(interno: T) -> Self {
        Self {
            interno,
            linhas: Mutex::new(Vec::new()),
            destino: Mutex::new(None),
            sequencia: AtomicU64::new(1),
            relogio: None,
        }
    }

    /// Also writes each line to this destination, such as a file.
    #[must_use]
    pub fn com_destino(self, destino: impl Write + Send + 'static) -> Self {
        *self.destino.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Box::new(destino));
        self
    }

    /// Takes time from this clock, so a recording in a test is the same on every run.
    #[must_use]
    pub fn com_relogio(mut self, relogio: RelogioControlado) -> Self {
        self.relogio = Some(relogio);
        self
    }

    /// What was recorded, as JSON Lines.
    #[must_use]
    pub fn jsonl(&self) -> String {
        let linhas = self.linhas.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut texto = linhas.join("\n");
        if !texto.is_empty() {
            texto.push('\n');
        }
        texto
    }

    fn medir<R>(&self, enviar: impl FnOnce() -> R) -> (R, Option<u64>, Option<String>) {
        if let Some(relogio) = &self.relogio {
            let inicio = relogio.agora();
            let resultado = enviar();
            let segundos = relogio.agora().0 - inicio.0;
            let duracao = u64::try_from(segundos.max(0)).unwrap_or(0).saturating_mul(1000);
            return (resultado, Some(duracao), Some(inicio.para_xml()));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let inicio = std::time::Instant::now();
            let resultado = enviar();
            let duracao = u64::try_from(inicio.elapsed().as_millis()).unwrap_or(u64::MAX);
            (resultado, Some(duracao), None)
        }
        #[cfg(target_arch = "wasm32")]
        {
            (enviar(), None, None)
        }
    }
}

impl<T: Transport> Transport for Gravador<T> {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let (resultado, duracao, quando) = self.medir(|| self.interno.send(request));
        let sequencia = self.sequencia.fetch_add(1, Ordering::Relaxed);
        let acao = request.header("Content-Type").and_then(acao_do_content_type);
        let mut linha = json!({
            "v": 1,
            "seq": sequencia,
            "quando": quando,
            "duracao_ms": duracao,
            "url": request.url.split('?').next().unwrap_or(&request.url),
            "acao": acao,
            "pedido": {
                "cabecalhos": cabecalhos(&request.headers),
                "corpo": corpo_mascarado(&request.body),
            },
        });
        match &resultado {
            Ok(resposta) => {
                linha["resposta"] = json!({
                    "status": resposta.status,
                    "cabecalhos": cabecalhos(&resposta.headers),
                    "corpo": corpo_mascarado(&resposta.body),
                });
            }
            Err(erro) => linha["erro"] = erro_em_json(erro),
        }
        let texto = linha.to_string();
        if let Some(destino) = self.destino.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_mut()
        {
            // A recording must never break the send it watches.
            let _ = writeln!(destino, "{texto}");
            let _ = destino.flush();
        }
        self.linhas.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(texto);
        resultado
    }

    fn intercepta(&self) -> bool {
        self.interno.intercepta()
    }
}

/// One recorded call.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Faixa {
    acao: Option<String>,
    resultado: Result<HttpResponse, TransportError>,
}

/// Plays a recording back as a transport, with no network.
///
/// Each request takes the next unused call with the same action, so the order between services
/// can differ. When none is left it fails with [`TransportError::NaoTratado`], rather than making
/// something up.
#[derive(Debug)]
pub struct Reprodutor {
    faixas: Mutex<Vec<Option<Faixa>>>,
}

impl Reprodutor {
    /// Reads a recording.
    pub fn de_jsonl(texto: &str) -> Result<Self, NfeError> {
        let mut faixas = Vec::new();
        for (numero, linha) in texto.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            let valor: Value = serde_json::from_str(linha).map_err(|erro| {
                NfeError::Xml(format!("linha {} da gravação inválida: {erro}", numero + 1))
            })?;
            let resultado = if let Some(resposta) = valor.get("resposta") {
                let headers = resposta["cabecalhos"]
                    .as_array()
                    .map(|lista| {
                        lista
                            .iter()
                            .map(|par| {
                                (
                                    par[0].as_str().unwrap_or_default().to_string(),
                                    par[1].as_str().unwrap_or_default().to_string(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(HttpResponse {
                    status: u16::try_from(resposta["status"].as_u64().unwrap_or(0)).unwrap_or(0),
                    headers,
                    body: resposta["corpo"].as_str().unwrap_or_default().as_bytes().to_vec(),
                })
            } else if let Some(erro) = valor.get("erro") {
                Err(erro_de_json(erro))
            } else {
                return Err(NfeError::Xml(format!("linha {} da gravação sem resposta nem erro", numero + 1)));
            };
            faixas.push(Some(Faixa { acao: valor["acao"].as_str().map(str::to_string), resultado }));
        }
        Ok(Self { faixas: Mutex::new(faixas) })
    }

    /// How many recorded calls were not played yet.
    #[must_use]
    pub fn restantes(&self) -> usize {
        self.faixas
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|faixa| faixa.is_some())
            .count()
    }
}

impl Transport for Reprodutor {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let acao = request.header("Content-Type").and_then(acao_do_content_type);
        let mut faixas = self.faixas.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let achada =
            faixas.iter_mut().find(|faixa| faixa.as_ref().is_some_and(|f| f.acao.as_deref() == acao));
        match achada.and_then(Option::take) {
            Some(faixa) => faixa.resultado,
            None => {
                Err(TransportError::NaoTratado { url: request.url.clone(), acao: acao.map(str::to_string) })
            }
        }
    }

    fn intercepta(&self) -> bool {
        true
    }
}
