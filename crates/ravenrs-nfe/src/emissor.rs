//! The emitter: from a draft to a signed document, and from there to the services.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use ravenrs_barcode::{Pattern, Symbology};
use rust_decimal::Decimal;

use crate::ambiente::{Ambiente, BetaAck, Modelo, Origem};
use crate::assinatura::{Signer, sign_enveloped};
use crate::chave::ChaveAcesso;
use crate::documento::{self, DadosCancelamento, Entrada, QrCode, TipoManifestacao};
use crate::error::NfeError;
use crate::modelo::{Emitente, Rascunho, validar_data_hora};
use crate::retorno::{self, Autorizacao, ConsultaProtocolo, LoteDfe, RetornoEvento, StatusServico};
use crate::sandbox::certificados::eh_do_laboratorio;
use crate::sandbox::tempo::{Instante, RelogioControlado};
use crate::soap::{self, Servico};
use crate::totais::{self, Totais};
use crate::transporte::Transport;
use crate::urls;

/// How long to wait after nothing was found: NT 2014.002, item 3.11.4.
const ESPERA_NACIONAL: Duration = Duration::from_secs(3600);

/// A signed document, ready to send, with what the application keeps of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Documento {
    /// The access key.
    pub chave: ChaveAcesso,
    /// The model.
    pub modelo: Modelo,
    /// The series.
    pub serie: u16,
    /// The number.
    pub numero: u32,
    /// The environment it was prepared for.
    pub ambiente: Ambiente,
    /// Who produced it, kept so a test note never passes for a real one.
    pub origem: Origem,
    /// The calculated totals.
    pub totais: Totais,
    /// The signed XML, `<NFe>` as it goes in the batch.
    pub xml: String,
    /// The text of the QR Code, in an NFC-e.
    pub qr_code: Option<String>,
}

impl Documento {
    /// The total of the document.
    #[must_use]
    pub fn valor_total(&self) -> Decimal {
        self.totais.v_nf
    }

    /// The QR Code as modules to draw, in an NFC-e.
    pub fn qr_code_modulos(&self) -> Option<Result<Pattern, String>> {
        self.qr_code.as_deref().map(|texto| ravenrs_barcode::encode(Symbology::Qr, texto))
    }
}

/// The outcome of [`Emissor::emitir`]: the document and what the service said about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Emissao {
    /// The document as sent.
    pub documento: Documento,
    /// The answer of the service.
    pub autorizacao: Autorizacao,
}

/// A request to cancel an authorized document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cancelamento {
    /// The key of the document.
    pub chave: ChaveAcesso,
    /// The protocol of its authorization.
    pub protocolo: String,
    /// Why, from 15 to 255 characters.
    pub justificativa: String,
    /// When, as `2026-11-05T14:30:00-03:00`.
    pub quando: String,
    /// The event's sequence, 1 for the first.
    pub sequencia: u8,
}

/// Builds an [`Emissor`]. Needs the explicit beta acknowledgement to exist.
pub struct EmissorBuilder {
    ambiente: Ambiente,
    emitente: Option<Emitente>,
    signer: Option<Arc<dyn Signer>>,
    transport: Option<Arc<dyn Transport>>,
    qr_code: QrCode,
    url_consulta: Option<String>,
    relogio: Option<RelogioControlado>,
}

impl std::fmt::Debug for EmissorBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmissorBuilder").field("ambiente", &self.ambiente).finish_non_exhaustive()
    }
}

impl EmissorBuilder {
    /// Sets the environment. Starts as [`Ambiente::Sandbox`].
    #[must_use]
    pub fn ambiente(mut self, ambiente: Ambiente) -> Self {
        self.ambiente = ambiente;
        self
    }

    /// Sets the issuer.
    #[must_use]
    pub fn emitente(mut self, emitente: Emitente) -> Self {
        self.emitente = Some(emitente);
        self
    }

    /// Sets who signs.
    #[must_use]
    pub fn signer(mut self, signer: impl Signer + 'static) -> Self {
        self.signer = Some(Arc::new(signer));
        self
    }

    /// Sets the way out to the network.
    #[must_use]
    pub fn transport(mut self, transport: impl Transport + 'static) -> Self {
        self.transport = Some(Arc::new(transport));
        self
    }

    /// Sets the QR Code version. Starts as [`QrCode::V3`].
    #[must_use]
    pub fn qr_code(mut self, qr_code: QrCode) -> Self {
        self.qr_code = qr_code;
        self
    }

    /// Replaces the `urlChave` of the state, whose table was not checked.
    #[must_use]
    pub fn url_consulta(mut self, url: impl Into<String>) -> Self {
        self.url_consulta = Some(url.into());
        self
    }

    /// Reads the time from this clock, for the limiter of the national environment.
    /// Without one it reads the system clock; in WebAssembly there the limiter cannot count hours.
    #[must_use]
    pub fn relogio(mut self, relogio: RelogioControlado) -> Self {
        self.relogio = Some(relogio);
        self
    }

    /// Finishes the emitter, refusing a sandbox whose transport would reach the network, and a
    /// certificate of the laboratory on the way to a real server.
    pub fn build(self) -> Result<Emissor, NfeError> {
        let emitente =
            self.emitente.ok_or_else(|| NfeError::Configuracao("emitente não informado".to_string()))?;
        emitente.validar()?;
        let signer =
            self.signer.ok_or_else(|| NfeError::Configuracao("assinador não informado".to_string()))?;
        let transport =
            self.transport.ok_or_else(|| NfeError::Configuracao("transporte não informado".to_string()))?;
        if self.ambiente == Ambiente::Sandbox && !transport.intercepta() {
            return Err(NfeError::Configuracao(
                "no ambiente Sandbox nenhuma nota pode sair daqui: use SefazSandbox como transporte"
                    .to_string(),
            ));
        }
        if self.ambiente == Ambiente::Homologacao && eh_do_laboratorio(signer.certificate_der()) {
            return Err(NfeError::Configuracao(
                "este certificado foi gerado pelo sandbox e não vale fora do laboratório: use um certificado ICP-Brasil real em homologação"
                    .to_string(),
            ));
        }
        if let Some(url) = &self.url_consulta
            && !(21..=85).contains(&url.len())
        {
            return Err(NfeError::Configuracao("urlChave precisa de 21 a 85 caracteres".to_string()));
        }
        Ok(Emissor {
            ambiente: self.ambiente,
            emitente,
            signer,
            transport,
            qr_code: self.qr_code,
            url_consulta: self.url_consulta,
            proximo_lote: AtomicU64::new(1),
            relogio: self.relogio,
            proxima_consulta: Mutex::new(None),
        })
    }
}

/// The fiscal emitter of the POC. Thread-safe; its sends block, so call them off the screen thread.
pub struct Emissor {
    ambiente: Ambiente,
    emitente: Emitente,
    signer: Arc<dyn Signer>,
    transport: Arc<dyn Transport>,
    qr_code: QrCode,
    url_consulta: Option<String>,
    proximo_lote: AtomicU64,
    relogio: Option<RelogioControlado>,
    proxima_consulta: Mutex<Option<Instante>>,
}

impl std::fmt::Debug for Emissor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Emissor")
            .field("ambiente", &self.ambiente)
            .field("emitente", &self.emitente.cnpj)
            .finish_non_exhaustive()
    }
}

impl Emissor {
    /// Starts building an emitter. The acknowledgement is what makes this compile.
    ///
    /// ```compile_fail,E0061
    /// let builder = ravenrs_nfe::Emissor::builder();
    /// ```
    #[must_use]
    pub fn builder(_ack: BetaAck) -> EmissorBuilder {
        EmissorBuilder {
            ambiente: Ambiente::Sandbox,
            emitente: None,
            signer: None,
            transport: None,
            qr_code: QrCode::V3,
            url_consulta: None,
            relogio: None,
        }
    }

    /// The environment of this emitter.
    #[must_use]
    pub fn ambiente(&self) -> Ambiente {
        self.ambiente
    }

    /// Validates, calculates, writes and signs a draft, without sending anything.
    pub fn preparar(&self, rascunho: &Rascunho) -> Result<Documento, NfeError> {
        rascunho.validar()?;
        let totais = totais::calcular(rascunho)?;
        let c_nf = documento::codigo_numerico(rascunho, &self.emitente.cnpj);
        let chave = documento::chave_de(rascunho, &self.emitente, &c_nf)?;
        let uf = self.emitente.endereco.uf;
        let qr_code = (rascunho.modelo == Modelo::Nfce65)
            .then(|| self.qr_code.texto(urls::url_qr_code(uf), &chave, self.ambiente));
        let url_chave = self.url_consulta.as_deref().unwrap_or(urls::url_consulta_chave(uf));
        let xml = documento::escrever_nfe(&Entrada {
            ambiente: self.ambiente,
            emitente: &self.emitente,
            rascunho,
            totais: &totais,
            chave: &chave,
            c_nf: &c_nf,
            suplemento: qr_code.as_deref().map(|qr| (qr, url_chave)),
        });
        let id = format!("NFe{chave}");
        let xml = sign_enveloped(&xml, "NFe", &id, self.signer.as_ref())?;
        Ok(Documento {
            chave,
            modelo: rascunho.modelo,
            serie: rascunho.serie,
            numero: rascunho.numero,
            ambiente: self.ambiente,
            origem: self.ambiente.into(),
            totais,
            xml,
            qr_code,
        })
    }

    /// Sends a prepared document for authorization, always synchronously.
    pub fn autorizar(&self, documento: &Documento) -> Result<Autorizacao, NfeError> {
        let dados = documento::lote(self.novo_lote(), &documento.xml);
        let resposta = self.enviar(documento.modelo, Servico::Autorizacao, &dados)?;
        retorno::ler_autorizacao(&resposta, &documento.xml)
    }

    /// Prepares and sends a draft.
    pub fn emitir(&self, rascunho: &Rascunho) -> Result<Emissao, NfeError> {
        let documento = self.preparar(rascunho)?;
        let autorizacao = self.autorizar(&documento)?;
        Ok(Emissao { documento, autorizacao })
    }

    /// Asks whether the service of a model is up.
    pub fn status(&self, modelo: Modelo) -> Result<StatusServico, NfeError> {
        let dados = documento::consulta_status(self.emitente.endereco.uf, self.ambiente);
        let resposta = self.enviar(modelo, Servico::Status, &dados)?;
        retorno::ler_status(&resposta)
    }

    /// Asks for the situation of a key.
    pub fn consultar(&self, chave: &ChaveAcesso) -> Result<ConsultaProtocolo, NfeError> {
        let dados = documento::consulta_protocolo(chave, self.ambiente);
        let resposta = self.enviar(modelo_da_chave(chave), Servico::Consulta, &dados)?;
        retorno::ler_consulta(&resposta)
    }

    /// Asks to cancel an authorized document.
    pub fn cancelar(&self, cancelamento: &Cancelamento) -> Result<RetornoEvento, NfeError> {
        let justificativa = cancelamento.justificativa.chars().count();
        if !(15..=255).contains(&justificativa) {
            return Err(NfeError::validacao("xJust", "de 15 a 255 caracteres"));
        }
        if cancelamento.sequencia == 0 || cancelamento.sequencia > 20 {
            return Err(NfeError::validacao("nSeqEvento", "de 1 a 20"));
        }
        validar_data_hora(&cancelamento.quando)?;
        let chave = &cancelamento.chave;
        let evento = documento::evento_cancelamento(&DadosCancelamento {
            ambiente: self.ambiente,
            uf: self.emitente.endereco.uf,
            cnpj: &self.emitente.cnpj,
            chave,
            protocolo: &cancelamento.protocolo,
            justificativa: &cancelamento.justificativa,
            quando: &cancelamento.quando,
            sequencia: cancelamento.sequencia,
        });
        let id = documento::id_cancelamento(chave, cancelamento.sequencia);
        let assinado = sign_enveloped(&evento, "evento", &id, self.signer.as_ref())?;
        let dados = documento::lote_evento(self.novo_lote(), &assinado);
        let resposta = self.enviar(modelo_da_chave(chave), Servico::Evento, &dados)?;
        retorno::ler_evento(&resposta)
    }

    /// Fetches the documents after this NSU from the national environment.
    ///
    /// Holds the query back, with [`NfeError::ConsultaAdiantada`], if the last answer asked to wait
    /// one hour; that avoids rejection 656, which blocks the CNPJ. A `resNFe` is only a summary:
    /// register [`TipoManifestacao::Ciencia`] with [`Self::manifestar`] and fetch again.
    pub fn buscar_documentos(&self, ultimo_nsu: u64) -> Result<LoteDfe, NfeError> {
        let agora = self.agora();
        {
            let proxima = self.proxima_consulta.lock().unwrap_or_else(PoisonError::into_inner);
            if let (Some(proxima), Some(agora)) = (*proxima, agora)
                && agora < proxima
            {
                return Err(NfeError::ConsultaAdiantada { proxima: proxima.para_xml() });
            }
        }
        let dados = documento::dist_por_nsu(
            self.ambiente,
            self.emitente.endereco.uf,
            &self.emitente.cnpj,
            ultimo_nsu,
        );
        let lote = self.distribuir(&dados)?;
        if lote.sem_novidade() || lote.c_stat == 656 {
            let mut proxima = self.proxima_consulta.lock().unwrap_or_else(PoisonError::into_inner);
            *proxima = agora.map(|momento| momento.depois(ESPERA_NACIONAL));
        }
        Ok(lote)
    }

    /// Fetches one document by its key. It comes as a summary until a manifestation releases it.
    pub fn baixar_por_chave(&self, chave: &ChaveAcesso) -> Result<LoteDfe, NfeError> {
        let dados =
            documento::dist_por_chave(self.ambiente, self.emitente.endereco.uf, &self.emitente.cnpj, chave);
        self.distribuir(&dados)
    }

    /// Picks up the wait an earlier emissor left, from [`Self::proxima_consulta_em`], so that a new
    /// one, or the program started again, still holds back the query until then.
    pub fn aguardar_consulta_ate(&self, proxima: Option<Instante>) {
        let mut atual = self.proxima_consulta.lock().unwrap_or_else(PoisonError::into_inner);
        *atual = (*atual).max(proxima);
    }

    /// When the next [`Self::buscar_documentos`] may run; `None` means it may run now.
    #[must_use]
    pub fn proxima_consulta_em(&self) -> Option<Instante> {
        let proxima = *self.proxima_consulta.lock().unwrap_or_else(PoisonError::into_inner);
        match (proxima, self.agora()) {
            (Some(proxima), Some(agora)) if agora >= proxima => None,
            (proxima, _) => proxima,
        }
    }

    /// Registers the recipient's manifestation at the national environment.
    pub fn manifestar(
        &self,
        chave: &ChaveAcesso,
        tipo: TipoManifestacao,
        justificativa: Option<&str>,
        quando: &str,
    ) -> Result<RetornoEvento, NfeError> {
        validar_data_hora(quando)?;
        if tipo == TipoManifestacao::NaoRealizada
            && !justificativa.is_some_and(|texto| (15..=255).contains(&texto.chars().count()))
        {
            return Err(NfeError::validacao("xJust", "de 15 a 255 caracteres"));
        }
        let evento = documento::evento_manifestacao(
            self.ambiente,
            &self.emitente.cnpj,
            chave,
            tipo,
            justificativa,
            quando,
        );
        let id = format!("ID{}{chave}01", tipo.codigo());
        let assinado = sign_enveloped(&evento, "evento", &id, self.signer.as_ref())?;
        let dados = documento::lote_evento(self.novo_lote(), &assinado);
        let pedido = soap::request(Servico::Evento, urls::URL_EVENTO_NACIONAL, &dados);
        let resposta = self.transport.send(&pedido)?;
        retorno::ler_evento(&resposta)
    }

    fn distribuir(&self, dados: &str) -> Result<LoteDfe, NfeError> {
        let pedido = soap::request(Servico::Distribuicao, urls::URL_DISTRIBUICAO, dados);
        let resposta = self.transport.send(&pedido)?;
        retorno::ler_distribuicao(&resposta)
    }

    fn agora(&self) -> Option<Instante> {
        if let Some(relogio) = &self.relogio {
            return Some(relogio.agora());
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let segundos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .and_then(|tempo| i64::try_from(tempo.as_secs()).ok())?;
            Some(Instante(segundos))
        }
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
    }

    fn novo_lote(&self) -> u64 {
        self.proximo_lote.fetch_add(1, Ordering::Relaxed)
    }

    fn enviar(
        &self,
        modelo: Modelo,
        servico: Servico,
        dados: &str,
    ) -> Result<crate::transporte::HttpResponse, NfeError> {
        let url = urls::url_servico(self.emitente.endereco.uf, modelo, servico);
        let pedido = soap::request(servico, url, dados);
        Ok(self.transport.send(&pedido)?)
    }
}

fn modelo_da_chave(chave: &ChaveAcesso) -> Modelo {
    if chave.as_str().get(20..22) == Some("65") { Modelo::Nfce65 } else { Modelo::Nfe55 }
}
