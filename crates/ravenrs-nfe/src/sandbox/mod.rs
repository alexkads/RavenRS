//! A local SEFAZ in the style of MSW, which also explains what went wrong.
//!
//! The emitter builds the same bytes and calls the same addresses; only the [`Transport`] changes.
//! With no handler of yours, the sandbox checks each message for real and answers in the real
//! SOAP format, always with `verAplic=RAVENRS-SANDBOX` and with `xMotivo` starting `[SANDBOX] `.
//!
//! | MSW | Here |
//! |---|---|
//! | `setupServer(...handlers)` | [`SefazSandbox::com_handlers`] |
//! | `http.post(url, resolver)` | [`post`], or [`autorizacao`], [`status`], [`consulta`], [`evento`], [`distribuicao`] |
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
//!
//! # A diagnosis tool
//!
//! - [`diagnosticar`] lists everything wrong in a message, from the one [`catalogo`] the sandbox
//!   answers from, each finding with its source and how far it can be trusted.
//! - [`autopsia::autopsia_assinatura`] says why a signature does not match.
//! - [`Incidente::de_producao`] compares the answer the SEFAZ gave with what the catalog predicts,
//!   and says when it does not reproduce.
//! - [`Gravador`] and [`Reprodutor`] take a field problem to the laboratory, masked.
//! - [`SefazSandbox::injetar`] and [`Falha`] make failures happen on demand, repeatably.
//! - [`SefazSandbox::linha_do_tempo`] tells the story of each key.
//! - [`certificados`] issues A1 and simulated A3 test certificates that cannot leave the laboratory.
//! - [`SefazSandbox::colocar_nota_recebida`] feeds the national distribution with test purchases.

use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::assinatura::{HashAssinatura, Signer};
use crate::error::NfeError;
use crate::retorno::descricao_c_stat;
use crate::soap::{Servico, acao_do_content_type};
use crate::transporte::{HttpRequest, HttpResponse, Transport, TransportError};
use crate::xml::{self, Element};

pub mod analise;
pub mod autopsia;
pub mod catalogo;
pub mod cenarios;
pub use ravenrs_dfe::certificado_x509;
use ravenrs_dfe::{Cadeia, DfeError};
pub mod certificados;
pub mod compras;
mod conferencia;
pub mod falhas;
pub mod gravacao;
pub mod incidente;
pub mod mascara;
mod resposta;
pub use ravenrs_dfe::tempo;
pub mod vigencia;

pub use analise::{Analise, Diagnostico, Opcoes, analisar, diagnosticar};
pub use autopsia::{AutopsiaAssinatura, Causa, CausaProvavel, Divergencia, Evidencia, autopsia_assinatura};
pub use catalogo::{CATALOGO, Cobertura, Confianca, Efeito, Fonte, Regra, cobertura, regra};
pub use certificado_x509::{HashCertificado, InfoCertificado};
pub use certificados::{
    AcTeste, CasoFalha, CertificadoGerado, ErroToken, GeradorCertificado, SlotInfo, TokenSimulado,
};
pub use compras::{Fornecedor, gerar_nota_de_compra};
pub use conferencia::EstadoChave;
pub(crate) use conferencia::Notas;
pub use falhas::{Falha, Quando, RegraDeFalha};
pub use gravacao::{Gravador, Reprodutor};
pub use incidente::{Incidente, RetornoReal, Veredito};
pub use tempo::{Instante, RelogioControlado};

/// The value of `verAplic` in every answer of the sandbox.
pub const VER_APLIC: &str = "RAVENRS-SANDBOX";
/// The start of `xMotivo` in every answer of the sandbox.
pub const PREFIXO_MOTIVO: &str = "[SANDBOX] ";
/// The common name of the certificate of [`SandboxSigner`].
pub const CN_CERTIFICADO: &str = certificados::CN_CURINGA;

/// A signer with a generated certificate of the laboratory.
///
/// [`SandboxSigner::new`] gives the certificate that fits any issuer, with no CNPJ.
/// [`SandboxSigner::para`] gives one with the issuer's CNPJ, which exercises the rule on the CNPJ.
#[derive(Clone, Debug)]
pub struct SandboxSigner(CertificadoGerado);

impl SandboxSigner {
    /// The certificate that fits any issuer.
    pub fn new() -> Result<Self, NfeError> {
        certificados::certificado_curinga().map(Self).map_err(NfeError::from)
    }

    /// A certificate with this company's CNPJ.
    pub fn para(razao_social: &str, cnpj: &str) -> Result<Self, NfeError> {
        GeradorCertificado::novo(razao_social, cnpj).emitir().map(Self).map_err(NfeError::from)
    }
}

impl Signer for SandboxSigner {
    fn certificate_der(&self) -> &[u8] {
        self.0.der()
    }

    fn sign_rsa_sha1(&self, message: &[u8]) -> Result<Vec<u8>, DfeError> {
        self.0.sign_rsa_sha1(message)
    }

    fn sign_rsa(&self, hash: HashAssinatura, message: &[u8]) -> Result<Vec<u8>, DfeError> {
        self.0.sign_rsa(hash, message)
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
    /// Injects this failure.
    Falha(Falha),
    /// Leaves it to the next handler, and in the end to the default checks.
    Proximo,
}

/// What a handler receives.
#[derive(Clone, Debug)]
pub struct SandboxRequest {
    /// The request as sent.
    pub http: HttpRequest,
    /// The service of the action in `Content-Type`, if it is one of the known ones.
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

/// Handles the national distribution.
pub fn distribuicao(f: impl Fn(&SandboxRequest) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Alvo::Servico(Servico::Distribuicao), f)
}

/// Handles any request whose address contains this text, like `http.post` in MSW.
pub fn post(
    trecho_url: impl Into<String>,
    f: impl Fn(&SandboxRequest) -> Resposta + Send + Sync + 'static,
) -> Handler {
    handler(Alvo::Url(trecho_url.into()), f)
}

/// What to do with a request nothing answers, like `onUnhandledRequest` in MSW. Shared with the
/// other sandboxes; here `Warn` records it in [`SefazSandbox::warnings`] and answers a SOAP fault
/// with HTTP 404.
pub use ravenrs_dfe::OnUnhandled;

/// A request the sandbox received, and what became of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistroPedido {
    /// The request as sent.
    pub http: HttpRequest,
    /// The service of its action, if any.
    pub servico: Option<Servico>,
    /// Its position, starting at 1.
    pub numero: usize,
    /// The sandbox clock at that moment.
    pub quando: String,
    /// The key it concerns, if any.
    pub chave: Option<String>,
    /// The status answered, if a return arrived and could be read.
    pub c_stat: Option<u32>,
    /// The text answered.
    pub x_motivo: Option<String>,
    /// The failure injected on this call.
    pub falha: Option<String>,
    /// The error the sender got, when no response arrived.
    pub erro_transporte: Option<String>,
    /// The state of the key after this call.
    pub estado_chave: Option<EstadoChave>,
    /// In pt-BR, why this came out as it did, when there is something to say.
    pub explicacao: Option<String>,
}

impl RegistroPedido {
    /// The body as text; empty if it is not UTF-8.
    #[must_use]
    pub fn body(&self) -> &str {
        std::str::from_utf8(&self.http.body).unwrap_or("")
    }
}

/// One step of the story, with what the catalog finds in the message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PassoFiscal {
    /// The call.
    pub registro: RegistroPedido,
    /// What the catalog finds in the message of the call.
    pub diagnosticos: Vec<Diagnostico>,
}

struct Estado {
    handlers: Cadeia<Handler>,
    on_unhandled: OnUnhandled,
    pedidos: Vec<RegistroPedido>,
    avisos: Vec<String>,
    pub(crate) notas: Notas,
    conferir: bool,
    relogio: Arc<dyn Fn() -> String + Send + Sync>,
    controlado: Option<RelogioControlado>,
    tem_relogio: bool,
    limite_latencia: Option<std::time::Duration>,
    falhas: Vec<RegraDeFalha>,
    ancoras: Vec<Vec<u8>>,
    revogados: Vec<String>,
}

impl Estado {
    fn agora(&self) -> Option<Instante> {
        if !self.tem_relogio {
            return None;
        }
        Instante::de_xml(&(self.relogio)())
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
    /// A sandbox that checks every message of the known services.
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
        let ancoras = AcTeste::padrao().map(|ac| vec![ac.spki().to_vec()]).unwrap_or_default();
        let estado = Estado {
            handlers: Cadeia::com_iniciais(iniciais),
            on_unhandled: OnUnhandled::Error,
            pedidos: Vec::new(),
            avisos: Vec::new(),
            notas: Notas::default(),
            conferir,
            relogio: Arc::new(|| "2026-01-01T00:00:00-03:00".to_string()),
            controlado: None,
            tem_relogio: false,
            limite_latencia: None,
            falhas: Vec::new(),
            ancoras,
            revogados: vec![certificados::serial_revogado_padrao()],
        };
        Self { estado: Arc::new(Mutex::new(estado)), protocolos: Arc::new(AtomicU64::new(1)) }
    }

    fn lock(&self) -> MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn opcoes(&self) -> Opcoes {
        let estado = self.lock();
        Opcoes {
            agora: estado.agora(),
            tp_amb_esperado: Some("2".to_string()),
            ancoras: estado.ancoras.clone(),
            revogados: estado.revogados.clone(),
            certificado_transmite: true,
        }
    }

    /// Replaces the fixed moment of `dhRecbto` with a clock of yours, which turns on the rules
    /// with a date. Text that is not in the form of the layout leaves them off.
    pub fn relogio(&self, relogio: impl Fn() -> String + Send + Sync + 'static) {
        let mut estado = self.lock();
        estado.relogio = Arc::new(relogio);
        estado.tem_relogio = true;
        estado.controlado = None;
    }

    /// Reads the time from a controlled clock: rules with a date apply, and injected latency moves it.
    pub fn usar_relogio(&self, relogio: &RelogioControlado) {
        let mut estado = self.lock();
        let copia = relogio.clone();
        estado.relogio = Arc::new(move || copia.agora().para_xml());
        estado.tem_relogio = true;
        estado.controlado = Some(relogio.clone());
    }

    /// Latency above this limit becomes a timeout, after the message was processed.
    pub fn limite_de_latencia(&self, limite: std::time::Duration) {
        self.lock().limite_latencia = Some(limite);
    }

    /// Trusts another test authority, besides the default one.
    pub fn confiar_em(&self, ac: &AcTeste) {
        self.lock().ancoras.push(ac.spki().to_vec());
    }

    /// Adds a certificate to the revocation list of this sandbox.
    pub fn revogar(&self, certificado: &CertificadoGerado) {
        if let Some(info) = InfoCertificado::ler(certificado.der()) {
            self.lock().revogados.push(info.serial_hex().to_string());
        }
    }

    /// Adds a handler that wins over the older ones, like `server.use`.
    pub fn use_handler(&self, handler: Handler) {
        self.lock().handlers.adicionar(handler, false);
    }

    /// Adds a handler that answers once and goes away.
    pub fn use_once(&self, handler: Handler) {
        self.lock().handlers.adicionar(handler, true);
    }

    /// Goes back to the initial handlers. What was already issued stays, and injected failures go.
    pub fn reset_handlers(&self) {
        let mut estado = self.lock();
        estado.handlers.restaurar();
        estado.falhas.clear();
    }

    /// Injects a failure. The first rule that applies to a request wins.
    pub fn injetar(&self, regra: RegraDeFalha) {
        self.lock().falhas.push(regra);
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

    /// Puts a purchase in the distribution of its recipient, as `nfeProc` or `NFe` text.
    pub fn colocar_nota_recebida(&self, xml_texto: &str) -> Result<(), NfeError> {
        conferencia::colocar_nota(self, xml_texto)
    }

    /// The requests received so far, in order, with what became of each.
    #[must_use]
    pub fn requests(&self) -> Vec<RegistroPedido> {
        self.lock().pedidos.clone()
    }

    /// The warnings of `OnUnhandled::Warn`.
    #[must_use]
    pub fn warnings(&self) -> Vec<String> {
        self.lock().avisos.clone()
    }

    /// What the sandbox knows about a key right now.
    #[must_use]
    pub fn estado_da_chave(&self, chave: &str) -> EstadoChave {
        self.lock().notas.estado(chave)
    }

    /// The story of the calls, each with what the catalog finds in its message.
    #[must_use]
    pub fn linha_do_tempo(&self) -> Vec<PassoFiscal> {
        let opcoes = self.opcoes();
        self.requests()
            .into_iter()
            .map(|registro| {
                let diagnosticos = analisar(registro.body(), &opcoes).diagnosticos;
                PassoFiscal { registro, diagnosticos }
            })
            .collect()
    }

    /// The report of what happened to a key, or to everything, in Markdown, masked.
    #[must_use]
    pub fn relatorio_markdown(&self, chave: Option<&str>) -> String {
        let passos: Vec<PassoFiscal> = self
            .linha_do_tempo()
            .into_iter()
            .filter(|passo| chave.is_none() || passo.registro.chave.as_deref() == chave)
            .collect();
        let mut texto = String::from("# Relatório de incidente fiscal\n\n");
        texto.push_str(
            "Gerado pelo sandbox do ravenrs-nfe, prova de conceito beta. Nada aqui passou pela SEFAZ.\n\n",
        );
        texto.push_str("## O que aconteceu\n\n| # | Quando | Serviço | cStat | Falha injetada | Estado da chave |\n|---|---|---|---|---|---|\n");
        for passo in &passos {
            let registro = &passo.registro;
            let _ = writeln!(
                texto,
                "| {} | {} | {} | {} | {} | {} |",
                registro.numero,
                registro.quando,
                registro.servico.map_or("-", Servico::nome),
                registro.c_stat.map_or_else(
                    || registro.erro_transporte.clone().unwrap_or_else(|| "-".to_string()),
                    |c| c.to_string()
                ),
                registro.falha.as_deref().unwrap_or("-"),
                registro.estado_chave.as_ref().map_or_else(|| "-".to_string(), rotulo_estado),
            );
        }
        texto.push_str("\n## Causa\n\n");
        let mut sem_causa = true;
        for passo in &passos {
            if let Some(explicacao) = &passo.registro.explicacao {
                let _ = writeln!(texto, "- Pedido {}: {explicacao}", passo.registro.numero);
                sem_causa = false;
            }
        }
        if sem_causa {
            texto.push_str("Nada a explicar: nenhum pedido foi recusado ou perdido.\n");
        }
        texto.push_str("\n## Correção\n\n");
        let mut achados: Vec<&Diagnostico> =
            passos.iter().flat_map(|passo| passo.diagnosticos.iter()).collect();
        achados.dedup_by_key(|achado| achado.regra);
        if achados.is_empty() {
            texto.push_str("Nenhuma regra do catálogo foi violada nas mensagens enviadas.\n");
        }
        for achado in &achados {
            let _ =
                writeln!(texto, "- `{}` (cStat {}): {}", achado.caminho, achado.c_stat, achado.como_corrigir);
        }
        texto.push_str("\n## Fonte\n\n");
        if achados.is_empty() {
            texto.push_str("Sem regra aplicável.\n");
        }
        for achado in &achados {
            let _ = writeln!(texto, "- {}: {} ({})", achado.regra, achado.fonte, achado.confianca);
        }
        texto.push_str("\n## O que não foi possível concluir\n\n");
        texto.push_str("- Isto é o comportamento do sandbox. O que a SEFAZ real faria só um teste em homologação com A1 comprova.\n");
        if passos.iter().any(|passo| passo.registro.falha.is_some()) {
            texto.push_str("- As falhas foram injetadas: elas mostram a reação do sistema, não a causa de um problema real.\n");
        }
        mascara::mascarar_texto_de_relatorio(&texto)
    }

    /// Runs the handlers, newest first, removing a `use_once` when it answers.
    ///
    /// Handlers run with the lock released, so one can call back into the sandbox.
    fn rodar_handlers(&self, pedido: &SandboxRequest) -> Option<Resposta> {
        let candidatos = self.lock().handlers.candidatos(|handler| handler.atende(pedido));
        for (id, once, handler) in candidatos {
            // Another thread may have used it first.
            if once && !self.lock().handlers.consumir(id) {
                continue;
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

    fn padrao(&self, pedido: &SandboxRequest) -> Resposta {
        let conferir = self.lock().conferir;
        if conferir && pedido.servico.is_some() {
            conferencia::conferir(pedido, self)
        } else {
            Resposta::Proximo
        }
    }

    fn resolver(
        &self,
        resposta: Resposta,
        pedido: &SandboxRequest,
        acao: Option<String>,
    ) -> Result<HttpResponse, TransportError> {
        match resposta {
            Resposta::Http(http) => Ok(http),
            Resposta::Timeout => Err(TransportError::Timeout),
            Resposta::Erro(erro) => Err(erro),
            Resposta::Falha(falha) => self.aplicar(&falha, pedido, acao),
            Resposta::Proximo => self.nao_tratado(pedido, acao),
        }
    }

    /// Runs a failure. The ones that fall after processing do process: that is the point of them.
    fn aplicar(
        &self,
        falha: &Falha,
        pedido: &SandboxRequest,
        acao: Option<String>,
    ) -> Result<HttpResponse, TransportError> {
        match falha {
            Falha::Latencia(tempo) => {
                let (controlado, limite) = {
                    let estado = self.lock();
                    (estado.controlado.clone(), estado.limite_latencia)
                };
                match &controlado {
                    Some(relogio) => relogio.avancar(*tempo),
                    None => dormir(*tempo),
                }
                let resposta = self.padrao(pedido);
                let resultado = self.resolver(resposta, pedido, acao);
                if limite.is_some_and(|limite| *tempo > limite) {
                    return Err(TransportError::Timeout);
                }
                resultado
            }
            Falha::QuedaAntesDaResposta => {
                let resposta = self.padrao(pedido);
                self.resolver(resposta, pedido, acao)?;
                Err(TransportError::Conexao("conexão encerrada antes da resposta".to_string()))
            }
            Falha::SoapFault(texto) => Ok(resposta::fault(500, texto)),
            Falha::Http(status) => Ok(HttpResponse {
                status: *status,
                headers: vec![("Content-Type".to_string(), "text/html".to_string())],
                body: format!("<html><body>HTTP {status}</body></html>").into_bytes(),
            }),
            Falha::Tls(texto) => Err(TransportError::Tls(texto.clone())),
            Falha::RespostaTruncada(parte) => {
                let resposta = self.padrao(pedido);
                let mut http = self.resolver(resposta, pedido, acao)?;
                let tamanho = http.body.len() * usize::from((*parte).min(100)) / 100;
                http.body.truncate(tamanho);
                Ok(http)
            }
            Falha::XmlInvalido => Ok(HttpResponse {
                status: 200,
                headers: vec![("Content-Type".to_string(), "application/soap+xml".to_string())],
                body: b"<soap:Envelope><isto nao e xml".to_vec(),
            }),
            Falha::Paralisado(c_stat) => {
                let resposta = pedido.responder(*c_stat);
                self.resolver(resposta, pedido, acao)
            }
        }
    }
}

fn dormir(tempo: std::time::Duration) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::sleep(tempo);
    #[cfg(target_arch = "wasm32")]
    let _ = tempo;
}

fn rotulo_estado(estado: &EstadoChave) -> String {
    match estado {
        EstadoChave::Desconhecida => "desconhecida".to_string(),
        EstadoChave::Rejeitada { c_stat } => format!("rejeitada ({c_stat})"),
        EstadoChave::Autorizada { .. } => "autorizada".to_string(),
        EstadoChave::AutorizadaSemRetorno { .. } => "autorizada, sem retorno ao emissor".to_string(),
        EstadoChave::Cancelada { .. } => "cancelada".to_string(),
    }
}

/// The key a message concerns, read from it.
fn chave_da_mensagem(pedido: &SandboxRequest) -> Option<String> {
    let dados = pedido.dados()?;
    dados
        .find("infNFe")
        .and_then(|inf| inf.attr("Id"))
        .and_then(|id| id.strip_prefix("NFe"))
        .map(str::to_string)
        .or_else(|| dados.find_text("chNFe"))
}

/// The status inside a response, the document's before the batch's.
fn c_stat_da_resposta(resposta: &HttpResponse) -> Option<(u32, String)> {
    let raiz = xml::parse(std::str::from_utf8(&resposta.body).ok()?).ok()?;
    let grupo = raiz
        .find("infProt")
        .or_else(|| raiz.find("retEvento").and_then(|r| r.child("infEvento")))
        .or_else(|| raiz.find("retEnviNFe"))
        .or_else(|| raiz.find("retEnvEvento"))
        .or_else(|| raiz.find("retConsSitNFe"))
        .or_else(|| raiz.find("retConsStatServ"))
        .or_else(|| raiz.find("retDistDFeInt"))?;
    Some((grupo.child_text("cStat")?.parse().ok()?, grupo.child_text("xMotivo").unwrap_or_default()))
}

/// Says why a step came out as it did, for the steps where that matters.
fn explicar(
    servico: Option<Servico>,
    c_stat: Option<u32>,
    erro: Option<&str>,
    estado: Option<&EstadoChave>,
) -> Option<String> {
    if erro.is_some() {
        return Some(match estado {
            Some(EstadoChave::AutorizadaSemRetorno { .. }) => {
                "A conexão caiu depois que a SEFAZ processou: a nota está autorizada e o emissor não sabe. Consulte a chave antes de reenviar ou de gerar outra numeração.".to_string()
            }
            _ => "Nenhuma resposta chegou. Não se sabe se a mensagem foi processada: consulte a chave antes de reenviar.".to_string(),
        });
    }
    let c_stat = c_stat?;
    Some(match (servico?, c_stat, estado) {
        (Servico::Autorizacao, 100, _) => return None,
        (Servico::Autorizacao, 204, _) => {
            "A nota já existia: quem reenviou provavelmente não recebeu o primeiro retorno. Consulte a chave e guarde o protocolo que já existe.".to_string()
        }
        (Servico::Autorizacao, 539, _) => {
            "Este número já foi usado com outra chave, quase sempre porque o cNF mudou entre tentativas.".to_string()
        }
        (Servico::Evento, 494, Some(EstadoChave::Rejeitada { c_stat })) => format!(
            "Não se cancela uma nota que não foi autorizada: ela foi rejeitada com {c_stat} e nunca existiu para a SEFAZ. Corrija e envie de novo."
        ),
        (Servico::Evento, 494, _) => {
            "Não se cancela uma chave que a SEFAZ não conhece: confira se a autorização chegou a acontecer.".to_string()
        }
        (Servico::Evento, 501, _) => {
            "A nota foi autorizada há mais de 24 horas, o prazo de cancelamento da regra 2P12-14. A UF pode ter exceção.".to_string()
        }
        (Servico::Evento, 580, Some(EstadoChave::Cancelada { .. })) => {
            "A nota já está cancelada: o evento exige uma nota autorizada.".to_string()
        }
        (Servico::Evento, 222, _) => {
            "O protocolo informado não é o da autorização desta nota.".to_string()
        }
        (Servico::Distribuicao, 656, _) => {
            "Consumo indevido: consulta antes de passar 1 hora, ou com um NSU anterior ao último entregue.".to_string()
        }
        (_, 108 | 109, _) => "O serviço está parado: nada foi processado.".to_string(),
        (_, 100..=199, _) => return None,
        (_, c_stat, _) => regra_para_explicar(c_stat)?,
    })
}

fn regra_para_explicar(c_stat: u32) -> Option<String> {
    catalogo::regras_do_c_stat(c_stat).next().map(|regra| regra.explicacao.to_string())
}

impl Transport for SefazSandbox {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let acao = request.header("Content-Type").and_then(acao_do_content_type).map(str::to_string);
        let servico = acao.as_deref().and_then(Servico::from_acao);
        let (dh_recbto, numero) = {
            let mut estado = self.lock();
            let quando = (estado.relogio)();
            let numero = estado.pedidos.len() + 1;
            estado.pedidos.push(RegistroPedido {
                http: request.clone(),
                servico,
                numero,
                quando: quando.clone(),
                chave: None,
                c_stat: None,
                x_motivo: None,
                falha: None,
                erro_transporte: None,
                estado_chave: None,
                explicacao: None,
            });
            (quando, numero)
        };
        let pedido = SandboxRequest {
            http: request.clone(),
            servico,
            dh_recbto,
            protocolos: Arc::clone(&self.protocolos),
        };
        let injetada = {
            let mut estado = self.lock();
            let agora = estado.agora();
            estado
                .falhas
                .iter_mut()
                .find_map(|regra| regra.aplica(servico, agora).then(|| regra.falha.clone()))
        };
        let descricao_falha = injetada.as_ref().map(Falha::descricao);
        let resultado = if let Some(falha) = &injetada {
            self.aplicar(falha, &pedido, acao)
        } else {
            let resposta = self.rodar_handlers(&pedido).unwrap_or_else(|| self.padrao(&pedido));
            self.resolver(resposta, &pedido, acao)
        };
        self.fechar_registro(numero, &pedido, &resultado, descricao_falha);
        resultado
    }

    fn intercepta(&self) -> bool {
        true
    }
}

impl SefazSandbox {
    fn fechar_registro(
        &self,
        numero: usize,
        pedido: &SandboxRequest,
        resultado: &Result<HttpResponse, TransportError>,
        falha: Option<String>,
    ) {
        let chave = chave_da_mensagem(pedido);
        let lido = resultado.as_ref().ok().and_then(c_stat_da_resposta);
        let erro = resultado.as_ref().err().map(ToString::to_string);
        let mut estado = self.lock();
        if let Some(chave) = &chave {
            // A queda deixa a nota autorizada sem que o emissor saiba; a consulta resolve isso.
            let processou = matches!(
                estado.notas.estado(chave),
                EstadoChave::Autorizada { .. } | EstadoChave::AutorizadaSemRetorno { .. }
            );
            if erro.is_some() && processou && pedido.servico == Some(Servico::Autorizacao) {
                estado.notas.marcar_sem_retorno(chave);
            }
            if lido.is_some() && matches!(pedido.servico, Some(Servico::Consulta | Servico::Autorizacao)) {
                estado.notas.marcar_com_retorno(chave);
            }
        }
        let estado_chave = chave.as_deref().map(|c| estado.notas.estado(c));
        let c_stat = lido.as_ref().map(|(c, _)| *c);
        let explicacao = explicar(pedido.servico, c_stat, erro.as_deref(), estado_chave.as_ref());
        if let Some(registro) = estado.pedidos.get_mut(numero - 1) {
            registro.chave = chave;
            registro.c_stat = c_stat;
            registro.x_motivo = lido.map(|(_, texto)| texto);
            registro.falha = falha;
            registro.erro_transporte = erro;
            registro.estado_chave = estado_chave;
            registro.explicacao = explicacao;
        }
    }
}
