//! The emitter: writes, signs and sends the DPS, and finds a lost answer by the Id of the DPS.

use std::sync::Arc;

use ravenrs_dfe::certificado_x509::eh_do_laboratorio;
use ravenrs_dfe::{Algoritmo, HttpRequest, HttpResponse, Signer, Transport, TransportError};
use serde_json::{Value, json};

use crate::ambiente::{Ambiente, BetaAck};
use crate::chave::ChaveNfse;
use crate::dps;
use crate::erro::{Mensagem, NfseError};
use crate::evento::{self, MotivoCancelamento};
use crate::modelo::{Dps, Inscricao};
use crate::retorno;

/// The code with which the Sefin says a DPS with this Id already exists. From a third-party
/// catalog, not the official table: see `docs/NFSE.md`.
pub const DPS_JA_EXISTE: &str = "E0014";

/// An NFS-e issued.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Emitida {
    /// The Id of the DPS that became it.
    pub id_dps: String,
    /// Its access key.
    pub chave: ChaveNfse,
    /// The XML of the NFS-e, as the Sefin gave it.
    pub xml: String,
    /// The alerts that came with it.
    pub alertas: Vec<Mensagem>,
    /// Whether it came from a recovery: the answer of the send was lost, and the NFS-e was found
    /// by the Id of the DPS.
    pub recuperada: bool,
}

/// Builds an [`Emissor`].
pub struct EmissorBuilder {
    ambiente: Ambiente,
    signer: Option<Arc<dyn Signer>>,
    transport: Option<Arc<dyn Transport>>,
    algoritmo: Algoritmo,
}

impl std::fmt::Debug for EmissorBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmissorBuilder").field("ambiente", &self.ambiente).finish_non_exhaustive()
    }
}

impl EmissorBuilder {
    /// Where it sends.
    #[must_use]
    pub fn ambiente(mut self, ambiente: Ambiente) -> Self {
        self.ambiente = ambiente;
        self
    }

    /// Who signs.
    #[must_use]
    pub fn signer(mut self, signer: impl Signer + 'static) -> Self {
        self.signer = Some(Arc::new(signer));
        self
    }

    /// A signer already shared.
    #[must_use]
    pub fn signer_compartilhado(mut self, signer: Arc<dyn Signer>) -> Self {
        self.signer = Some(signer);
        self
    }

    /// How it reaches the Sefin.
    #[must_use]
    pub fn transport(mut self, transport: impl Transport + 'static) -> Self {
        self.transport = Some(Arc::new(transport));
        self
    }

    /// A transport already shared.
    #[must_use]
    pub fn transport_compartilhado(mut self, transport: Arc<dyn Transport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// The signature algorithm. The vigent one is not confirmed for the national standard: the only
    /// official text is a historical manual with RSA-SHA1, and projects in use sign with SHA-256.
    /// The default is SHA-1, like the NF-e, until the first real send settles it.
    #[must_use]
    pub fn algoritmo(mut self, algoritmo: Algoritmo) -> Self {
        self.algoritmo = algoritmo;
        self
    }

    /// The emitter, if the configuration can work.
    pub fn build(self) -> Result<Emissor, NfseError> {
        let signer = self.signer.ok_or_else(|| NfseError::Configuracao("falta quem assina".to_owned()))?;
        let transport =
            self.transport.ok_or_else(|| NfseError::Configuracao("falta o transporte".to_owned()))?;
        match self.ambiente {
            Ambiente::Sandbox if !transport.intercepta() => {
                return Err(NfseError::Configuracao(
                    "o sandbox só aceita um transporte que não sai do computador".to_owned(),
                ));
            }
            Ambiente::ProducaoRestrita if eh_do_laboratorio(signer.certificate_der()) => {
                return Err(NfseError::Configuracao(
                    "o certificado do laboratório não vale fora do laboratório: use um A1 ou A3 ICP-Brasil"
                        .to_owned(),
                ));
            }
            _ => {}
        }
        Ok(Emissor { ambiente: self.ambiente, signer, transport, algoritmo: self.algoritmo })
    }
}

/// Issues NFS-e of the national standard. Only `tpAmb=2`.
pub struct Emissor {
    ambiente: Ambiente,
    signer: Arc<dyn Signer>,
    transport: Arc<dyn Transport>,
    algoritmo: Algoritmo,
}

impl std::fmt::Debug for Emissor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Emissor")
            .field("ambiente", &self.ambiente)
            .field("algoritmo", &self.algoritmo)
            .finish_non_exhaustive()
    }
}

fn cabecalhos_json() -> Vec<(String, String)> {
    vec![
        ("Content-Type".to_owned(), "application/json".to_owned()),
        ("Accept".to_owned(), "application/json".to_owned()),
    ]
}

/// Whether the answer was lost on the way: the DPS may or may not have become an NFS-e.
fn incerto(erro: &NfseError) -> bool {
    match erro {
        NfseError::Transporte(TransportError::Timeout | TransportError::Conexao(_)) => true,
        NfseError::RetornoInvalido { status, .. } => *status >= 500,
        _ => false,
    }
}

impl Emissor {
    /// Starts building an emitter. It only exists for whoever acknowledges the beta.
    #[must_use]
    pub fn builder(_ack: BetaAck) -> EmissorBuilder {
        EmissorBuilder {
            ambiente: Ambiente::Sandbox,
            signer: None,
            transport: None,
            algoritmo: Algoritmo::NFE,
        }
    }

    /// Where it sends.
    #[must_use]
    pub const fn ambiente(&self) -> Ambiente {
        self.ambiente
    }

    fn url(&self, caminho: &str) -> String {
        format!("{}{caminho}", self.ambiente.base_sefin())
    }

    fn enviar(&self, pedido: &HttpRequest) -> Result<HttpResponse, NfseError> {
        Ok(self.transport.send(pedido)?)
    }

    /// The DPS as it would go, signed. Useful to look at, and to keep with the sale.
    pub fn assinar(&self, dps: &Dps) -> Result<String, NfseError> {
        dps::assinar(dps, self.signer.as_ref(), self.algoritmo)
    }

    /// Sends the DPS and returns the NFS-e.
    ///
    /// If the answer is lost (time out, connection cut, 5xx) or the Sefin says the DPS already
    /// exists, it looks for the NFS-e by the Id of the DPS before saying anything. When it is not
    /// found either, the error is [`NfseError::NaoConfirmada`]: send the same DPS again, never with
    /// another number, or the same service gets two NFS-e.
    pub fn emitir(&self, dps: &Dps) -> Result<Emitida, NfseError> {
        let id_dps = dps::id_dps(dps);
        let assinado = self.assinar(dps)?;
        let corpo = json!({ "dpsXmlGZipB64": ravenrs_dfe::compactacao::gzip_base64(&assinado)? });
        let pedido = HttpRequest::post(self.url("/nfse"), cabecalhos_json(), corpo.to_string().into_bytes());
        let enviado = self.enviar(&pedido).and_then(|resposta| self.ler_emissao(&id_dps, &resposta));
        match enviado {
            Ok(emitida) => Ok(emitida),
            Err(erro) if incerto(&erro) || erro.codigos().contains(&DPS_JA_EXISTE) => {
                match self.recuperar(&id_dps) {
                    Ok(Some(emitida)) => Ok(emitida),
                    Ok(None) if erro.codigos().contains(&DPS_JA_EXISTE) => Err(erro),
                    Ok(None) => Err(NfseError::NaoConfirmada { id_dps, motivo: erro.to_string() }),
                    Err(consulta) => Err(NfseError::NaoConfirmada {
                        id_dps,
                        motivo: format!("{erro}; a consulta também falhou: {consulta}"),
                    }),
                }
            }
            Err(erro) => Err(erro),
        }
    }

    fn ler_emissao(&self, id_dps: &str, resposta: &HttpResponse) -> Result<Emitida, NfseError> {
        if let Some(recusa) = retorno::recusa(resposta) {
            return Err(recusa);
        }
        if !(200..300).contains(&resposta.status) {
            return Err(NfseError::RetornoInvalido {
                status: resposta.status,
                motivo: String::from_utf8_lossy(&resposta.body).chars().take(200).collect(),
            });
        }
        let corpo = retorno::json(resposta)?;
        let xml = retorno::documento(resposta, "nfseXmlGZipB64")?;
        let chave = match retorno::texto(&corpo, &["chaveAcesso"]) {
            Some(chave) => ChaveNfse::parse(&chave)?,
            None => chave_do_xml(&xml)?,
        };
        Ok(Emitida {
            id_dps: id_dps.to_owned(),
            chave,
            xml,
            alertas: retorno::mensagens(&corpo, &["alertas"]),
            recuperada: false,
        })
    }

    /// Looks for the NFS-e of a DPS by its Id: `HEAD /dps/{id}`, then the key in `GET /dps/{id}`,
    /// then the document in `GET /nfse/{chave}`. `None` means the DPS did not become an NFS-e.
    pub fn recuperar(&self, id_dps: &str) -> Result<Option<Emitida>, NfseError> {
        let caminho = format!("/dps/{id_dps}");
        let existe = self.enviar(&HttpRequest::head(self.url(&caminho), cabecalhos_json()))?;
        match existe.status {
            404 => return Ok(None),
            200..=299 => {}
            status => {
                return Err(NfseError::RetornoInvalido { status, motivo: "HEAD /dps inesperado".to_owned() });
            }
        }
        let dados = self.enviar(&HttpRequest::get(self.url(&caminho), cabecalhos_json()))?;
        if let Some(recusa) = retorno::recusa(&dados) {
            return Err(recusa);
        }
        let chave = retorno::texto(&retorno::json(&dados)?, &["chaveAcesso"]).ok_or_else(|| {
            NfseError::RetornoInvalido { status: dados.status, motivo: "chaveAcesso ausente".to_owned() }
        })?;
        let chave = ChaveNfse::parse(&chave)?;
        let xml = self.consultar(&chave)?;
        Ok(Some(Emitida { id_dps: id_dps.to_owned(), chave, xml, alertas: Vec::new(), recuperada: true }))
    }

    /// The XML of an NFS-e, by its key.
    pub fn consultar(&self, chave: &ChaveNfse) -> Result<String, NfseError> {
        let resposta =
            self.enviar(&HttpRequest::get(self.url(&format!("/nfse/{chave}")), cabecalhos_json()))?;
        if let Some(recusa) = retorno::recusa(&resposta) {
            return Err(recusa);
        }
        retorno::documento(&resposta, "nfseXmlGZipB64")
    }

    /// Cancels an NFS-e, and returns the XML of the event registered.
    pub fn cancelar(
        &self,
        chave: &ChaveNfse,
        autor: &Inscricao,
        motivo: MotivoCancelamento,
        justificativa: &str,
        quando: &str,
    ) -> Result<String, NfseError> {
        let pedido = evento::pedido_cancelamento(
            chave,
            autor,
            motivo,
            justificativa,
            quando,
            self.signer.as_ref(),
            self.algoritmo,
        )?;
        let corpo =
            json!({ "pedidoRegistroEventoXmlGZipB64": ravenrs_dfe::compactacao::gzip_base64(&pedido)? });
        let resposta = self.enviar(&HttpRequest::post(
            self.url(&format!("/nfse/{chave}/eventos")),
            cabecalhos_json(),
            corpo.to_string().into_bytes(),
        ))?;
        if let Some(recusa) = retorno::recusa(&resposta) {
            return Err(recusa);
        }
        retorno::documento(&resposta, "eventoXmlGZipB64")
    }

    /// The agreement of a city with the national standard, as the ADN answers it. The shape of the
    /// answer is not confirmed, so it comes as JSON.
    pub fn convenio(&self, municipio: &str) -> Result<Value, NfseError> {
        let url = format!("{}/parametros_municipais/{municipio}/convenio", self.ambiente.base_parametros());
        let resposta = self.enviar(&HttpRequest::get(url, cabecalhos_json()))?;
        if let Some(recusa) = retorno::recusa(&resposta) {
            return Err(recusa);
        }
        retorno::json(&resposta)
    }
}

/// The key from the Id of `infNFSe`, when the answer does not bring it apart.
fn chave_do_xml(xml: &str) -> Result<ChaveNfse, NfseError> {
    let raiz = ravenrs_dfe::xml::parse(xml)?;
    let id = raiz
        .find("infNFSe")
        .and_then(|info| info.attr("Id"))
        .ok_or_else(|| NfseError::RetornoInvalido { status: 200, motivo: "NFS-e sem Id".to_owned() })?;
    ChaveNfse::parse(id)
}
