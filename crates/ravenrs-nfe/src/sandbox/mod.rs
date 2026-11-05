//! A local SEFAZ in the style of MSW: the emitter does not know it was intercepted.
//!
//! The emitter builds the same bytes and calls the same addresses; only the [`Transport`] changes.
//! With no handler of yours, the sandbox checks each message for real and answers in the real
//! SOAP format, always with `verAplic=RAVENRS-SANDBOX` and with `xMotivo` starting `[SANDBOX] `.
//!
//! | MSW | Here |
//! |---|---|
//! | `setupServer(...handlers)` | [`SefazSandbox::com_handlers`] |
//! | `http.post(url, resolver)` | [`post`], or [`autorizacao`], [`status`], [`consulta`], [`evento`] |
//! | `server.use(handler)` | [`SefazSandbox::use_handler`] |
//! | `{ once: true }` | [`SefazSandbox::use_once`] |
//! | `server.resetHandlers()` | [`SefazSandbox::reset_handlers`] |
//! | `onUnhandledRequest` | [`SefazSandbox::on_unhandled`] |
//!
//! ```
//! use ravenrs_nfe::sandbox::{SefazSandbox, autorizacao, cenarios};
//!
//! let server = SefazSandbox::new();
//! // The next authorization finds the service down; the one after it is checked normally.
//! server.use_once(autorizacao(cenarios::paralisado_108));
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::assinatura::{RsaSigner, Signer};
use crate::error::NfeError;
use crate::retorno::descricao_c_stat;
use crate::soap::{Servico, acao_do_content_type};
use crate::transporte::{HttpRequest, HttpResponse, Transport, TransportError};
use crate::xml::{self, Element};

pub mod cenarios;
mod conferencia;
mod resposta;

pub(crate) use conferencia::Notas;

/// The value of `verAplic` in every answer of the sandbox.
pub const VER_APLIC: &str = "RAVENRS-SANDBOX";
/// The start of `xMotivo` in every answer of the sandbox.
pub const PREFIXO_MOTIVO: &str = "[SANDBOX] ";
/// The common name of the sandbox certificate.
pub const CN_CERTIFICADO: &str = "RAVENRS SANDBOX - SEM VALOR";
/// The sandbox certificate, in PEM. It has no fiscal value.
pub const CERTIFICADO_PEM: &str = include_str!("../../fixtures/sandbox-cert.pem");
/// The key of the sandbox certificate, in PEM. It is public on purpose and has no value.
pub const CHAVE_PEM: &str = include_str!("../../fixtures/sandbox-key.pem");

/// A signer with the public test certificate `RAVENRS SANDBOX - SEM VALOR`.
#[derive(Clone, Debug)]
pub struct SandboxSigner(RsaSigner);

impl SandboxSigner {
    /// Loads the bundled certificate and key.
    pub fn new() -> Result<Self, NfeError> {
        RsaSigner::from_pem(CERTIFICADO_PEM, CHAVE_PEM).map(Self)
    }
}

impl Signer for SandboxSigner {
    fn certificate_der(&self) -> &[u8] {
        self.0.certificate_der()
    }

    fn sign_rsa_sha1(&self, message: &[u8]) -> Result<Vec<u8>, NfeError> {
        self.0.sign_rsa_sha1(message)
    }
}

/// What a handler decides.
#[derive(Clone, Debug)]
pub enum Resposta {
    /// Answers this HTTP response.
    Http(HttpResponse),
    /// Behaves as if the time limit ran out.
    Timeout,
    /// Fails the request with this error.
    Erro(TransportError),
    /// Leaves it to the next handler, and in the end to the default checks.
    Proximo,
}

/// What a handler receives.
#[derive(Clone, Debug)]
pub struct SandboxRequest {
    /// The request as sent.
    pub http: HttpRequest,
    /// The service of the action in `Content-Type`, if it is one of the four.
    pub servico: Option<Servico>,
    /// The moment the sandbox gives as `dhRecbto`.
    pub dh_recbto: String,
    protocolos: Arc<AtomicU64>,
}

impl SandboxRequest {
    /// The body as text; empty if it is not UTF-8.
    #[must_use]
    pub fn body(&self) -> &str {
        std::str::from_utf8(&self.http.body).unwrap_or("")
    }

    /// The message inside `nfeDadosMsg`, if the body can be read.
    #[must_use]
    pub fn dados(&self) -> Option<Element> {
        let root = xml::parse(self.body()).ok()?;
        root.find("nfeDadosMsg")?.elements().next().cloned()
    }

    /// Answers with a `cStat` in the format of the service, with its usual text.
    #[must_use]
    pub fn responder(&self, c_stat: u32) -> Resposta {
        let motivo = descricao_c_stat(c_stat).unwrap_or("Código sem descrição conferida");
        self.responder_com(c_stat, motivo)
    }

    /// Answers with a `cStat` and a text of yours; the `[SANDBOX] ` prefix is added.
    #[must_use]
    pub fn responder_com(&self, c_stat: u32, motivo: &str) -> Resposta {
        resposta::responder(self, c_stat, motivo)
    }

    fn proximo_protocolo(&self) -> String {
        let sequencia = self.protocolos.fetch_add(1, Ordering::Relaxed);
        format!("1{sequencia:014}")
    }
}

#[derive(Clone)]
enum Alvo {
    Servico(Servico),
    Url(String),
}

/// A rule that answers a kind of request.
#[derive(Clone)]
pub struct Handler {
    alvo: Alvo,
    responder: Arc<dyn Fn(&SandboxRequest) -> Resposta + Send + Sync>,
}

impl std::fmt::Debug for Handler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let alvo = match &self.alvo {
            Alvo::Servico(servico) => servico.nome().to_string(),
            Alvo::Url(url) => url.clone(),
        };
        f.debug_struct("Handler").field("alvo", &alvo).finish_non_exhaustive()
    }
}

impl Handler {
    fn atende(&self, pedido: &SandboxRequest) -> bool {
        match &self.alvo {
            Alvo::Servico(servico) => pedido.servico == Some(*servico),
            Alvo::Url(trecho) => pedido.http.url.contains(trecho.as_str()),
        }
    }
}

fn handler(alvo: Alvo, responder: impl Fn(&SandboxRequest) -> Resposta + Send + Sync + 'static) -> Handler {
    Handler { alvo, responder: Arc::new(responder) }
}

/// Handles the authorization service.
pub fn autorizacao(f: impl Fn(&SandboxRequest) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Alvo::Servico(Servico::Autorizacao), f)
}

/// Handles the service status.
pub fn status(f: impl Fn(&SandboxRequest) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Alvo::Servico(Servico::Status), f)
}

/// Handles the protocol query.
pub fn consulta(f: impl Fn(&SandboxRequest) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Alvo::Servico(Servico::Consulta), f)
}

/// Handles the event service.
pub fn evento(f: impl Fn(&SandboxRequest) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Alvo::Servico(Servico::Evento), f)
}

/// Handles any request whose address contains this text, like `http.post` in MSW.
pub fn post(
    trecho_url: impl Into<String>,
    f: impl Fn(&SandboxRequest) -> Resposta + Send + Sync + 'static,
) -> Handler {
    handler(Alvo::Url(trecho_url.into()), f)
}

/// What to do with a request nothing answers, like `onUnhandledRequest` in MSW.
#[derive(Clone)]
pub enum OnUnhandled {
    /// Fails it with [`TransportError::NaoTratado`]. The default.
    Error,
    /// Records it in [`SefazSandbox::warnings`] and answers a SOAP fault with HTTP 404.
    ///
    /// Unlike MSW, a warning never goes to the network: only `Passthrough` does that.
    Warn,
    /// Sends it through this transport. This is the only way a request can leave the sandbox.
    Passthrough(Arc<dyn Transport>),
}

impl std::fmt::Debug for OnUnhandled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Error => "Error",
            Self::Warn => "Warn",
            Self::Passthrough(_) => "Passthrough",
        })
    }
}

/// A request the sandbox received.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistroPedido {
    /// The request as sent.
    pub http: HttpRequest,
    /// The service of its action, if any.
    pub servico: Option<Servico>,
}

impl RegistroPedido {
    /// The body as text; empty if it is not UTF-8.
    #[must_use]
    pub fn body(&self) -> &str {
        std::str::from_utf8(&self.http.body).unwrap_or("")
    }
}

struct EntradaHandler {
    handler: Handler,
    once: bool,
    id: u64,
}

struct Estado {
    iniciais: Vec<Handler>,
    handlers: Vec<EntradaHandler>,
    on_unhandled: OnUnhandled,
    pedidos: Vec<RegistroPedido>,
    avisos: Vec<String>,
    notas: Notas,
    conferir: bool,
    relogio: Arc<dyn Fn() -> String + Send + Sync>,
    proximo_id: u64,
}

impl Estado {
    fn adicionar(&mut self, handler: Handler, once: bool) {
        let id = self.proximo_id;
        self.proximo_id += 1;
        self.handlers.push(EntradaHandler { handler, once, id });
    }

    fn restaurar(&mut self) {
        self.handlers.clear();
        let iniciais = self.iniciais.clone();
        for handler in iniciais {
            self.adicionar(handler, false);
        }
    }
}

/// The local SEFAZ. Cloning shares the same state, and it is safe across threads.
#[derive(Clone)]
pub struct SefazSandbox {
    estado: Arc<Mutex<Estado>>,
    protocolos: Arc<AtomicU64>,
}

impl Default for SefazSandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SefazSandbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SefazSandbox").finish_non_exhaustive()
    }
}

impl SefazSandbox {
    /// A sandbox that checks every message of the four services.
    #[must_use]
    pub fn new() -> Self {
        Self::construir(true, Vec::new())
    }

    /// A sandbox with no default answers: whatever no handler takes is unhandled.
    #[must_use]
    pub fn vazio() -> Self {
        Self::construir(false, Vec::new())
    }

    /// A checking sandbox with handlers that [`Self::reset_handlers`] brings back.
    #[must_use]
    pub fn com_handlers(handlers: Vec<Handler>) -> Self {
        Self::construir(true, handlers)
    }

    fn construir(conferir: bool, iniciais: Vec<Handler>) -> Self {
        let mut estado = Estado {
            iniciais,
            handlers: Vec::new(),
            on_unhandled: OnUnhandled::Error,
            pedidos: Vec::new(),
            avisos: Vec::new(),
            notas: Notas::default(),
            conferir,
            relogio: Arc::new(|| "2026-01-01T00:00:00-03:00".to_string()),
            proximo_id: 0,
        };
        estado.restaurar();
        Self { estado: Arc::new(Mutex::new(estado)), protocolos: Arc::new(AtomicU64::new(1)) }
    }

    fn lock(&self) -> MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Replaces the fixed moment of `dhRecbto` with a clock of yours.
    pub fn relogio(&self, relogio: impl Fn() -> String + Send + Sync + 'static) {
        self.lock().relogio = Arc::new(relogio);
    }

    /// Adds a handler that wins over the older ones, like `server.use`.
    pub fn use_handler(&self, handler: Handler) {
        self.lock().adicionar(handler, false);
    }

    /// Adds a handler that answers once and goes away.
    pub fn use_once(&self, handler: Handler) {
        self.lock().adicionar(handler, true);
    }

    /// Goes back to the initial handlers. What was already issued stays.
    pub fn reset_handlers(&self) {
        self.lock().restaurar();
    }

    /// Forgets issued documents, requests and warnings. Handlers stay.
    pub fn limpar_dados(&self) {
        let mut estado = self.lock();
        estado.pedidos.clear();
        estado.avisos.clear();
        estado.notas = Notas::default();
    }

    /// Sets what happens to a request nothing answers.
    pub fn on_unhandled(&self, regra: OnUnhandled) {
        self.lock().on_unhandled = regra;
    }

    /// The requests received so far, in order.
    #[must_use]
    pub fn requests(&self) -> Vec<RegistroPedido> {
        self.lock().pedidos.clone()
    }

    /// The warnings of `OnUnhandled::Warn`.
    #[must_use]
    pub fn warnings(&self) -> Vec<String> {
        self.lock().avisos.clone()
    }

    /// Runs the handlers, newest first, removing a `use_once` when it answers.
    ///
    /// Handlers run with the lock released, so one can call back into the sandbox.
    fn rodar_handlers(&self, pedido: &SandboxRequest) -> Option<Resposta> {
        let candidatos: Vec<(u64, bool, Handler)> = {
            let estado = self.lock();
            estado
                .handlers
                .iter()
                .rev()
                .filter(|entrada| entrada.handler.atende(pedido))
                .map(|entrada| (entrada.id, entrada.once, entrada.handler.clone()))
                .collect()
        };
        for (id, once, handler) in candidatos {
            if once {
                let mut estado = self.lock();
                let Some(posicao) = estado.handlers.iter().position(|e| e.id == id) else {
                    // Another thread used it first.
                    continue;
                };
                estado.handlers.remove(posicao);
            }
            let resposta = (handler.responder)(pedido);
            if !matches!(resposta, Resposta::Proximo) {
                return Some(resposta);
            }
        }
        None
    }

    fn nao_tratado(
        &self,
        pedido: &SandboxRequest,
        acao: Option<String>,
    ) -> Result<HttpResponse, TransportError> {
        let regra = self.lock().on_unhandled.clone();
        match regra {
            OnUnhandled::Error => Err(TransportError::NaoTratado { url: pedido.http.url.clone(), acao }),
            OnUnhandled::Warn => {
                let aviso = format!(
                    "pedido não tratado: {} (ação: {})",
                    pedido.http.url,
                    acao.as_deref().unwrap_or("-")
                );
                self.lock().avisos.push(aviso);
                Ok(resposta::fault(404, "Sem handler para este pedido"))
            }
            OnUnhandled::Passthrough(transport) => transport.send(&pedido.http),
        }
    }
}

impl Transport for SefazSandbox {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let acao = request.header("Content-Type").and_then(acao_do_content_type).map(str::to_string);
        let servico = acao.as_deref().and_then(Servico::from_acao);
        let (dh_recbto, conferir) = {
            let mut estado = self.lock();
            estado.pedidos.push(RegistroPedido { http: request.clone(), servico });
            ((estado.relogio)(), estado.conferir)
        };
        let pedido = SandboxRequest {
            http: request.clone(),
            servico,
            dh_recbto,
            protocolos: Arc::clone(&self.protocolos),
        };
        let resposta = match self.rodar_handlers(&pedido) {
            Some(resposta) => resposta,
            None => match servico.filter(|_| conferir) {
                Some(_) => conferencia::conferir(&pedido, self),
                None => return self.nao_tratado(&pedido, acao),
            },
        };
        match resposta {
            Resposta::Http(http) => Ok(http),
            Resposta::Timeout => Err(TransportError::Timeout),
            Resposta::Erro(erro) => Err(erro),
            Resposta::Proximo => self.nao_tratado(&pedido, acao),
        }
    }

    fn intercepta(&self) -> bool {
        true
    }
}
