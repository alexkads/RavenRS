//! The real network, only with the `http` feature and only on native targets.
//!
//! [`HttpTransport`] uses `ureq` with `rustls` and a client certificate from an A1 PFX. The roots
//! come from a PEM file the caller provides (the ICP-Brasil chain), no redirect is followed,
//! plain HTTP is refused and certificate verification has no switch to turn off. The A1 password
//! lives only in memory and is dropped as soon as the file is read.

use std::fmt;
use std::path::Path;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use p12_keystore::{KeyStore, Pkcs12ImportPolicy};
use ureq::Agent;
use ureq::config::Config;
use ureq::tls::{Certificate, ClientCert, PemItem, PrivateKey, RootCerts, TlsConfig, TlsProvider};
use zeroize::{Zeroize, Zeroizing};

use crate::assinatura::{RsaSigner, Signer};
use crate::error::NfeError;
use crate::sandbox::certificados::eh_do_laboratorio;
use crate::transporte::{HttpRequest, HttpResponse, Transport, TransportError};

/// The largest response read, far above any return of these services.
const LIMITE_RESPOSTA: u64 = 10 * 1024 * 1024;

/// The password of an A1 file. It only exists in memory, is hidden from `Debug` and is wiped on drop.
#[derive(Clone)]
pub struct SenhaA1(String);

impl SenhaA1 {
    /// Takes the password typed by the user.
    #[must_use]
    pub fn new(senha: impl Into<String>) -> Self {
        Self(senha.into())
    }
}

impl SenhaA1 {
    pub(crate) fn expor(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SenhaA1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SenhaA1(<oculta>)")
    }
}

impl Drop for SenhaA1 {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// An A1 certificate read from a PFX: the chain and the key, with no password kept.
pub struct CertificadoA1 {
    cadeia: Vec<Vec<u8>>,
    chave: Zeroizing<Vec<u8>>,
    signer: RsaSigner,
}

impl fmt::Debug for CertificadoA1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CertificadoA1")
            .field("certificados", &self.cadeia.len())
            .field("chave", &"<oculta>")
            .finish()
    }
}

impl CertificadoA1 {
    /// Reads a PFX. A wrong password and a damaged file give the same error text.
    pub fn from_pfx(pfx: &[u8], senha: &SenhaA1) -> Result<Self, NfeError> {
        let erro =
            || NfeError::Assinatura("não foi possível abrir o PFX: senha ou arquivo inválido".to_string());
        let cofre = KeyStore::from_pkcs12(pfx, &senha.0, Pkcs12ImportPolicy::Strict).map_err(|_| erro())?;
        let (_, entrada) = cofre.private_key_chain().ok_or_else(erro)?;
        let cadeia: Vec<Vec<u8>> =
            entrada.certs().iter().map(|certificado| certificado.as_der().to_vec()).collect();
        let certificado = cadeia.first().ok_or_else(erro)?.clone();
        let chave = Zeroizing::new(entrada.key().as_der().to_vec());
        let signer = RsaSigner::from_der(certificado, &chave)?;
        Ok(Self { cadeia, chave, signer })
    }
}

impl Signer for CertificadoA1 {
    fn certificate_der(&self) -> &[u8] {
        self.signer.certificate_der()
    }

    fn sign_rsa_sha1(&self, message: &[u8]) -> Result<Vec<u8>, NfeError> {
        self.signer.sign_rsa_sha1(message)
    }
}

/// Builds an [`HttpTransport`].
#[derive(Debug)]
pub struct HttpTransportBuilder<'a> {
    certificado: &'a CertificadoA1,
    raizes: Option<Vec<u8>>,
    timeout: Duration,
}

impl HttpTransportBuilder<'_> {
    /// Sets the trusted roots, from PEM text.
    #[must_use]
    pub fn raizes_pem(mut self, pem: &[u8]) -> Self {
        self.raizes = Some(pem.to_vec());
        self
    }

    /// Sets the trusted roots, from a PEM file.
    pub fn raizes_pem_arquivo(mut self, caminho: &Path) -> Result<Self, NfeError> {
        let pem = std::fs::read(caminho).map_err(|erro| {
            NfeError::Configuracao(format!("não foi possível ler {}: {erro}", caminho.display()))
        })?;
        self.raizes = Some(pem);
        Ok(self)
    }

    /// Sets the time limit of a whole request. Starts at 30 seconds.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Finishes the transport. Having no root is an error: nothing here trusts by default.
    pub fn build(self) -> Result<HttpTransport, NfeError> {
        if self.certificado.cadeia.iter().any(|der| eh_do_laboratorio(der)) {
            return Err(NfeError::Configuracao(
                "este certificado foi gerado pelo sandbox e não vale fora do laboratório: use um certificado ICP-Brasil real"
                    .to_string(),
            ));
        }
        let pem = self.raizes.ok_or_else(|| {
            NfeError::Configuracao("informe as raízes confiáveis (cadeia ICP-Brasil)".to_string())
        })?;
        let mut raizes = Vec::new();
        for item in ureq::tls::parse_pem(&pem) {
            let item =
                item.map_err(|erro| NfeError::Configuracao(format!("arquivo de raízes inválido: {erro}")))?;
            if let PemItem::Certificate(certificado) = item {
                raizes.push(certificado);
            }
        }
        if raizes.is_empty() {
            return Err(NfeError::Configuracao("o arquivo de raízes não tem certificados".to_string()));
        }
        let cadeia: Vec<Certificate<'static>> =
            self.certificado.cadeia.iter().map(|der| Certificate::from_der(der).to_owned()).collect();
        // ureq only takes a key of a known kind through PEM, so PKCS#8 goes in that wrapper.
        let pem_chave = Zeroizing::new(format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            Zeroizing::new(STANDARD.encode(self.certificado.chave.as_slice())).as_str()
        ));
        let chave = PrivateKey::from_pem(pem_chave.as_bytes())
            .map_err(|_| NfeError::Assinatura("chave do PFX ilegível".to_string()))?;
        let tls = TlsConfig::builder()
            .provider(TlsProvider::Rustls)
            .client_cert(Some(ClientCert::new_with_certs(&cadeia, chave)))
            .root_certs(RootCerts::new_with_certs(&raizes))
            .build();
        let config: Config = Agent::config_builder()
            .tls_config(tls)
            .https_only(true)
            .http_status_as_error(false)
            .max_redirects(0)
            .max_redirects_will_error(false)
            .timeout_global(Some(self.timeout))
            .build();
        Ok(HttpTransport { agent: config.into() })
    }
}

/// The real way out to the SEFAZ.
pub struct HttpTransport {
    agent: Agent,
}

impl fmt::Debug for HttpTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpTransport").finish_non_exhaustive()
    }
}

impl HttpTransport {
    /// Starts building a transport that presents this certificate.
    #[must_use]
    pub fn builder(certificado: &CertificadoA1) -> HttpTransportBuilder<'_> {
        HttpTransportBuilder { certificado, raizes: None, timeout: Duration::from_secs(30) }
    }
}

fn mapear_erro(erro: ureq::Error) -> TransportError {
    match erro {
        ureq::Error::Timeout(_) => TransportError::Timeout,
        ureq::Error::Rustls(motivo) => TransportError::Tls(motivo.to_string()),
        ureq::Error::Tls(motivo) => TransportError::Tls(motivo.to_string()),
        ureq::Error::Io(motivo) => match motivo.kind() {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => TransportError::Timeout,
            // This is how rustls reports a refused certificate through the stream.
            std::io::ErrorKind::InvalidData => TransportError::Tls(motivo.to_string()),
            _ => TransportError::Conexao(motivo.to_string()),
        },
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => {
            TransportError::Conexao(erro.to_string())
        }
        ureq::Error::RequireHttpsOnly(motivo) => TransportError::Tls(motivo),
        other => TransportError::Outro(other.to_string()),
    }
}

impl Transport for HttpTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut pedido = self.agent.post(&request.url);
        for (nome, valor) in &request.headers {
            pedido = pedido.header(nome.as_str(), valor.as_str());
        }
        let mut resposta = pedido.send(request.body.as_slice()).map_err(mapear_erro)?;
        let status = resposta.status().as_u16();
        let headers: Vec<(String, String)> = resposta
            .headers()
            .iter()
            .map(|(nome, valor)| {
                (nome.as_str().to_string(), String::from_utf8_lossy(valor.as_bytes()).into_owned())
            })
            .collect();
        if (300..400).contains(&status) {
            let destino = headers
                .iter()
                .find(|(nome, _)| nome.eq_ignore_ascii_case("location"))
                .map_or_else(|| status.to_string(), |(_, valor)| valor.clone());
            return Err(TransportError::Redirecionamento(destino));
        }
        let body =
            resposta.body_mut().with_config().limit(LIMITE_RESPOSTA).read_to_vec().map_err(mapear_erro)?;
        Ok(HttpResponse { status, headers, body })
    }
}
