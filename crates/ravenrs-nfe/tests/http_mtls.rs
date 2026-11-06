//! `HttpTransport` against a local HTTPS server that demands a client certificate.
#![cfg(all(feature = "http", not(target_arch = "wasm32")))]
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use ravenrs_nfe::http::{CertificadoA1, HttpTransport, SenhaA1};
use ravenrs_nfe::{DfeError, HttpRequest, NfeError, Servico, Signer, Transport, TransportError};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::WebPkiClientVerifier;
use rustls::{RootCertStore, ServerConfig, ServerConnection, StreamOwned};

const SENHA: &str = "teste-sem-valor";

fn fixture(nome: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/http-test").join(nome)
}

fn certs(nome: &str) -> Vec<CertificateDer<'static>> {
    CertificateDer::pem_file_iter(fixture(nome)).expect("pem").map(|c| c.expect("cert")).collect()
}

fn config_servidor(ca_dos_clientes: &str) -> Arc<ServerConfig> {
    let provedor = Arc::new(rustls::crypto::ring::default_provider());
    let mut raizes = RootCertStore::empty();
    for certificado in certs(ca_dos_clientes) {
        raizes.add(certificado).expect("root");
    }
    let verificador = WebPkiClientVerifier::builder_with_provider(Arc::new(raizes), provedor.clone())
        .build()
        .expect("verifier");
    let mut cadeia = certs("server.pem");
    cadeia.extend(certs("ca.pem"));
    let chave = PrivateKeyDer::from_pem_file(fixture("server-key.pem")).expect("key");
    let config = ServerConfig::builder_with_provider(provedor)
        .with_safe_default_protocol_versions()
        .expect("versions")
        .with_client_cert_verifier(verificador)
        .with_single_cert(cadeia, chave)
        .expect("config");
    Arc::new(config)
}

/// What the server saw: the request text and how many client certificates came.
struct Visto {
    texto: String,
    certificados_cliente: usize,
}

fn atender(tcp: TcpStream, config: Arc<ServerConfig>, resposta: &str) -> Option<Visto> {
    tcp.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let conexao = ServerConnection::new(config).ok()?;
    let mut tls = StreamOwned::new(conexao, tcp);
    let mut recebido = Vec::new();
    let mut pedaco = [0_u8; 4096];
    loop {
        let lidos = tls.read(&mut pedaco).ok()?;
        if lidos == 0 {
            return None;
        }
        recebido.extend_from_slice(&pedaco[..lidos]);
        let texto = String::from_utf8_lossy(&recebido);
        if let Some(fim_cabecalho) = texto.find("\r\n\r\n") {
            let tamanho: usize = texto
                .lines()
                .find_map(|linha| {
                    linha
                        .to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse().unwrap_or(0))
                })
                .unwrap_or(0);
            if recebido.len() >= fim_cabecalho + 4 + tamanho {
                break;
            }
        }
    }
    let certificados_cliente = tls.conn.peer_certificates().map_or(0, <[_]>::len);
    tls.write_all(resposta.as_bytes()).ok()?;
    tls.flush().ok()?;
    tls.conn.send_close_notify();
    let _ = tls.flush();
    Some(Visto { texto: String::from_utf8_lossy(&recebido).into_owned(), certificados_cliente })
}

/// Serves one TLS connection on a port of its own and reports what it saw.
fn servidor(ca_dos_clientes: &str, resposta: &'static str) -> (SocketAddr, Receiver<Option<Visto>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("port");
    let endereco = listener.local_addr().expect("address");
    let config = config_servidor(ca_dos_clientes);
    let (envia, recebe) = channel();
    std::thread::spawn(move || {
        if let Ok((tcp, _)) = listener.accept() {
            let _ = envia.send(atender(tcp, config, resposta));
        }
    });
    (endereco, recebe)
}

fn certificado(arquivo: &str) -> CertificadoA1 {
    let pfx = std::fs::read(fixture(arquivo)).expect("pfx");
    CertificadoA1::from_pfx(&pfx, &SenhaA1::new(SENHA)).expect("certificate")
}

fn transporte(raizes: &str, timeout: Duration) -> HttpTransport {
    HttpTransport::builder(&certificado("client.p12"))
        .raizes_pem_arquivo(&fixture(raizes))
        .expect("roots")
        .timeout(timeout)
        .build()
        .expect("transport")
}

fn pedido(endereco: SocketAddr) -> HttpRequest {
    ravenrs_nfe::soap::request(Servico::Status, &format!("https://127.0.0.1:{}/ws", endereco.port()), "<x/>")
}

const OK: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/soap+xml\r\nContent-Length: 5\r\nConnection: close\r\n\r\n<ok/>";

#[test]
fn mtls_request_carries_certificate_action_and_body() {
    let (endereco, visto) = servidor("ca.pem", OK);
    let resposta = transporte("ca.pem", Duration::from_secs(10)).send(&pedido(endereco)).expect("response");
    assert_eq!(resposta.status, 200);
    assert_eq!(resposta.body, b"<ok/>");
    let visto = visto.recv().expect("seen").expect("request");
    assert_eq!(visto.certificados_cliente, 2, "client certificate plus its CA");
    assert!(visto.texto.starts_with("POST /ws HTTP/1.1"));
    assert!(
        visto.texto.to_lowercase().contains(
            "action=\"http://www.portalfiscal.inf.br/nfe/wsdl/nfestatusservico4/nfestatusserviconf\""
        )
    );
    assert!(visto.texto.contains("<nfeDadosMsg"));
}

#[test]
fn server_errors_come_back_as_responses_with_their_status() {
    let (endereco, _visto) = servidor(
        "ca.pem",
        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 4\r\nConnection: close\r\n\r\nfail",
    );
    let resposta = transporte("ca.pem", Duration::from_secs(10)).send(&pedido(endereco)).expect("response");
    assert_eq!(resposta.status, 500);
    assert_eq!(resposta.body, b"fail");
}

#[test]
fn legacy_pfx_opens_too() {
    let (endereco, visto) = servidor("ca.pem", OK);
    let antigo = certificado("client-legacy.p12");
    let transporte = HttpTransport::builder(&antigo)
        .raizes_pem_arquivo(&fixture("ca.pem"))
        .expect("roots")
        .build()
        .expect("transport");
    assert_eq!(transporte.send(&pedido(endereco)).expect("response").status, 200);
    assert_eq!(visto.recv().expect("seen").expect("request").certificados_cliente, 2);
}

#[test]
fn untrusted_server_is_refused() {
    let (endereco, _visto) = servidor("ca.pem", OK);
    let erro =
        transporte("other-ca.pem", Duration::from_secs(10)).send(&pedido(endereco)).expect_err("refused");
    assert!(matches!(erro, TransportError::Tls(_)), "{erro:?}");
}

#[test]
fn server_refusing_the_client_certificate_fails() {
    let (endereco, visto) = servidor("other-ca.pem", OK);
    let erro = transporte("ca.pem", Duration::from_secs(10)).send(&pedido(endereco));
    assert!(erro.is_err(), "{erro:?}");
    assert!(visto.recv().expect("seen").is_none());
}

#[test]
fn plain_http_is_refused() {
    let erro = transporte("ca.pem", Duration::from_secs(2))
        .send(&ravenrs_nfe::soap::request(Servico::Status, "http://127.0.0.1:9/ws", "<x/>"))
        .expect_err("refused");
    assert!(matches!(erro, TransportError::Tls(_)), "{erro:?}");
}

#[test]
fn redirect_is_not_followed() {
    let (endereco, _visto) = servidor(
        "ca.pem",
        "HTTP/1.1 302 Found\r\nLocation: https://outro.example/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let erro = transporte("ca.pem", Duration::from_secs(10)).send(&pedido(endereco)).expect_err("refused");
    assert_eq!(erro, TransportError::Redirecionamento("https://outro.example/".to_string()));
}

#[test]
fn silent_server_times_out() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("port");
    let endereco = listener.local_addr().expect("address");
    let segura = std::thread::spawn(move || listener.accept().map(|(tcp, _)| tcp));
    let erro = transporte("ca.pem", Duration::from_millis(300)).send(&pedido(endereco)).expect_err("timeout");
    assert_eq!(erro, TransportError::Timeout);
    drop(segura);
}

#[test]
fn wrong_password_says_nothing_about_it() {
    let pfx = std::fs::read(fixture("client.p12")).expect("pfx");
    let erro = CertificadoA1::from_pfx(&pfx, &SenhaA1::new("senha-errada-123")).expect_err("wrong");
    let texto = format!("{erro} {erro:?}");
    assert!(matches!(erro, DfeError::Assinatura(_)));
    assert!(!texto.contains("senha-errada-123"));
}

#[test]
fn debug_hides_password_and_key() {
    let senha = SenhaA1::new("segredo-do-usuario");
    assert!(!format!("{senha:?}").contains("segredo"));
    let a1 = certificado("client.p12");
    let texto = format!("{a1:?}");
    assert!(texto.contains("<oculta>"));
    assert!(!texto.contains("BEGIN"));
}

#[test]
fn certificate_also_signs() {
    let a1 = certificado("client.p12");
    assert_eq!(a1.sign_rsa_sha1(b"x").expect("signature").len(), 256);
    let esperado = certs("client.pem");
    assert_eq!(a1.certificate_der(), esperado[0].as_ref());
}

#[test]
fn roots_are_required_and_must_be_certificates() {
    let a1 = certificado("client.p12");
    assert!(matches!(HttpTransport::builder(&a1).build(), Err(DfeError::Configuracao(_))));
    assert!(matches!(
        HttpTransport::builder(&a1).raizes_pem(b"nada aqui").build(),
        Err(DfeError::Configuracao(_))
    ));
    assert!(matches!(
        HttpTransport::builder(&a1).raizes_pem_arquivo(&fixture("nao-existe.pem")),
        Err(DfeError::Configuracao(_))
    ));
}

#[test]
fn real_transport_fits_homologation_and_is_refused_by_the_sandbox() {
    use ravenrs_nfe::{Ambiente, BetaAck, Emissor, Endereco, Uf};
    // The address table is fixed, so nothing is sent here: a real call needs a real A1.
    let a1 = certificado("client.p12");
    let transporte = HttpTransport::builder(&a1)
        .raizes_pem_arquivo(&fixture("ca.pem"))
        .expect("roots")
        .timeout(Duration::from_millis(1))
        .build()
        .expect("transport");
    let montar = |ambiente: Ambiente, a1: CertificadoA1, transporte: HttpTransport| {
        Emissor::builder(BetaAck::i_understand_this_is_experimental())
            .ambiente(ambiente)
            .emitente(ravenrs_nfe::Emitente {
                cnpj: "11222333000181".into(),
                razao_social: "TESTE".into(),
                nome_fantasia: None,
                inscricao_estadual: "123456789012".into(),
                endereco: Endereco {
                    logradouro: "RUA".into(),
                    numero: "1".into(),
                    bairro: "CENTRO".into(),
                    codigo_municipio: "3550308".into(),
                    municipio: "SAO PAULO".into(),
                    uf: Uf::SP,
                    cep: "01001000".into(),
                },
            })
            .signer(a1)
            .transport(transporte)
            .build()
    };
    let emissor = montar(Ambiente::Homologacao, a1, transporte).expect("emitter");
    assert_eq!(emissor.ambiente(), Ambiente::Homologacao);
    let a1 = certificado("client.p12");
    let transporte = HttpTransport::builder(&a1)
        .raizes_pem(include_bytes!("../fixtures/http-test/ca.pem"))
        .build()
        .expect("transport");
    assert!(matches!(montar(Ambiente::Sandbox, a1, transporte), Err(NfeError::Configuracao(_))));
}
