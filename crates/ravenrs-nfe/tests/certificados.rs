//! The certificates of the laboratory, and the walls that keep them there.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

mod comum;

use comum::*;
use ravenrs_nfe::sandbox::certificados::{
    CasoFalha, NOME_AC, cita_icp_brasil, eh_do_laboratorio, pem_certificado,
};
use ravenrs_nfe::sandbox::{
    CertificadoGerado, Confianca, ErroToken, Falha, GeradorCertificado, InfoCertificado, RegraDeFalha,
    SandboxSigner, SefazSandbox, TokenSimulado, catalogo, diagnosticar,
};
use ravenrs_nfe::{Ambiente, BetaAck, Emissor, Modelo, NfeError, RsaSigner, Signer};

const CNPJ: &str = "11222333000181";

fn gerador() -> GeradorCertificado {
    GeradorCertificado::novo("MERCADO DE TESTE LTDA", CNPJ)
}

fn emissor_com_certificado(server: &SefazSandbox, signer: impl Signer + 'static) -> Emissor {
    Emissor::builder(BetaAck::i_understand_this_is_experimental())
        .ambiente(Ambiente::Sandbox)
        .emitente(emitente(CNPJ))
        .signer(signer)
        .transport(server.clone())
        .build()
        .expect("emitter")
}

#[test]
fn valid_generated_certificate_is_accepted() {
    let server = SefazSandbox::new();
    let emissor = emissor_com_certificado(&server, gerador().emitir().expect("issued"));
    let nota = emissor.emitir(&nfe55(proximo_numero())).expect("emission");
    assert_eq!(nota.autorizacao.c_stat(), 100, "{}", nota.autorizacao.x_motivo());
}

#[test]
fn branch_of_the_same_company_passes_the_cnpj_rule() {
    let server = SefazSandbox::new();
    let filial =
        GeradorCertificado::novo("MERCADO DE TESTE LTDA", "11222333000262").emitir().expect("issued");
    let emissor = emissor_com_certificado(&server, filial);
    assert_eq!(emissor.emitir(&nfe55(proximo_numero())).expect("emission").autorizacao.c_stat(), 100);
}

#[test]
fn each_failure_gives_its_rejection_through_the_sandbox() {
    let casos = [
        (CasoFalha::Vencido, 291),
        (CasoFalha::AindaNaoValido, 291),
        (CasoFalha::CnpjDiferente, 213),
        (CasoFalha::CadeiaQuebrada, 293),
        (CasoFalha::Revogado, 294),
        (CasoFalha::EkuErrado, 280),
        (CasoFalha::UsoDaChaveErrado, 290),
        (CasoFalha::ChaveFraca, 290),
        (CasoFalha::Sha1NoCertificado, 290),
        (CasoFalha::SemCnpj, 292),
    ];
    for (caso, esperado) in casos {
        let server = SefazSandbox::new();
        let certificado = gerador().com_falha(caso).expect("issued");
        let emissor = emissor_com_certificado(&server, certificado);
        let retorno = emissor.emitir(&nfe55(proximo_numero())).expect("emission").autorizacao;
        assert_eq!(retorno.c_stat(), esperado, "{caso:?}: {}", retorno.x_motivo());
        assert_eq!(caso.regra().expect("rule").c_stat, esperado, "{caso:?}");
    }
}

#[test]
fn each_failure_is_marked_with_source_and_trust() {
    assert_eq!(CasoFalha::Vencido.regra().expect("rule").confianca, Confianca::LidaNaFonte);
    assert_eq!(CasoFalha::CnpjDiferente.regra().expect("rule").confianca, Confianca::LidaNaFonte);
    // No rule of the MOC cites these two: they are inferences and say so.
    assert_eq!(CasoFalha::ChaveFraca.regra().expect("rule").confianca, Confianca::Inferida);
    assert_eq!(CasoFalha::Sha1NoCertificado.regra().expect("rule").confianca, Confianca::Inferida);
    for id in ["A02", "A03", "A04", "A05", "A06", "A07", "E05", "E07", "F03A"] {
        let regra = catalogo::regra(id).expect("rule");
        assert_eq!(regra.confianca, Confianca::LidaNaFonte, "{id}");
    }
}

#[test]
fn diagnosis_lists_a_bad_certificate_too() {
    let server = SefazSandbox::new();
    let emissor = emissor_com_certificado(&server, gerador().vencido().expect("issued"));
    let documento = emissor.preparar(&nfe55(proximo_numero())).expect("document");
    assert!(diagnosticar(&documento.xml).iter().any(|a| a.c_stat == 291));
}

#[test]
fn revocation_list_of_the_sandbox_grows() {
    let server = SefazSandbox::new();
    let certificado = GeradorCertificado::novo("OUTRA LTDA", CNPJ).emitir().expect("issued");
    server.revogar(&certificado);
    let emissor = emissor_com_certificado(&server, certificado);
    assert_eq!(emissor.emitir(&nfe55(proximo_numero())).expect("emission").autorizacao.c_stat(), 294);
}

// ---------------------------------------------------------------- walls

#[test]
fn homologation_refuses_a_certificate_of_the_laboratory() {
    for assinante in [
        Box::new(SandboxSigner::new().expect("signer")) as Box<dyn Signer>,
        Box::new(gerador().emitir().expect("issued")),
    ] {
        struct Dono(Box<dyn Signer>);
        impl Signer for Dono {
            fn certificate_der(&self) -> &[u8] {
                self.0.certificate_der()
            }
            fn sign_rsa_sha1(&self, message: &[u8]) -> Result<Vec<u8>, NfeError> {
                self.0.sign_rsa_sha1(message)
            }
        }
        let resultado = Emissor::builder(BetaAck::i_understand_this_is_experimental())
            .ambiente(Ambiente::Homologacao)
            .emitente(emitente(CNPJ))
            .signer(Dono(assinante))
            .transport(SefazSandbox::new())
            .build();
        match resultado {
            Err(NfeError::Configuracao(motivo)) => {
                assert!(motivo.contains("fora do laboratório"), "{motivo}");
            }
            other => panic!("deveria recusar: {other:?}"),
        }
    }
}

fn certificado_externo() -> RsaSigner {
    RsaSigner::from_pem(
        include_str!("../fixtures/laboratorio/externo-cert.pem"),
        include_str!("../fixtures/laboratorio/externo-key.pem"),
    )
    .expect("signer")
}

#[test]
fn sandbox_refuses_a_real_looking_certificate() {
    let externo = certificado_externo();
    assert!(cita_icp_brasil(externo.certificate_der()));
    assert!(!eh_do_laboratorio(externo.certificate_der()));
    let server = SefazSandbox::new();
    let emissor = emissor_com_certificado(&server, externo);
    let retorno = emissor.emitir(&nfe55(proximo_numero())).expect("emission").autorizacao;
    assert_eq!(retorno.c_stat(), 293, "{}", retorno.x_motivo());
    assert!(
        retorno.x_motivo().contains("Certificado real não entra")
            || retorno.x_motivo().contains("AC de teste"),
        "{}",
        retorno.x_motivo()
    );
}

#[test]
fn outside_certificate_goes_to_homologation() {
    let emissor = Emissor::builder(BetaAck::i_understand_this_is_experimental())
        .ambiente(Ambiente::Homologacao)
        .emitente(emitente(CNPJ))
        .signer(certificado_externo())
        .transport(SefazSandbox::new())
        .build();
    assert!(emissor.is_ok());
}

#[test]
fn another_authority_needs_to_be_trusted_first() {
    let externo_cn =
        InfoCertificado::ler(gerador().emitir().expect("issued").der()).expect("read").emissor_cn;
    assert_eq!(externo_cn.as_deref(), Some(NOME_AC));
}

// ---------------------------------------------------------------- A3

fn token(pin: &str) -> TokenSimulado {
    gerador().emitir().expect("issued").em_token(pin)
}

#[test]
fn a3_signs_through_the_same_path_and_the_sandbox_accepts() {
    let server = SefazSandbox::new();
    let a3 = token("1234");
    a3.entrar(0, "1234").expect("pin");
    let emissor = emissor_com_certificado(&server, a3.clone());
    let nota = emissor.emitir(&nfe55(proximo_numero())).expect("emission");
    assert_eq!(nota.autorizacao.c_stat(), 100, "{}", nota.autorizacao.x_motivo());
    assert_eq!(a3.assinaturas_feitas(), 1);
}

#[test]
fn a3_with_a_wrong_pin_stops_before_the_sefaz() {
    let server = SefazSandbox::new();
    let a3 = gerador().para_pin_errado("1234").expect("token");
    assert_eq!(a3.entrar(0, "9999"), Err(ErroToken::PinErrado { restantes: 2 }));
    let emissor = emissor_com_certificado(&server, a3.clone());
    let resultado = emissor.emitir(&nfe55(proximo_numero()));
    assert!(matches!(resultado, Err(NfeError::Assinatura(_))), "{resultado:?}");
    assert!(server.requests().is_empty(), "nada pode ter saído");
    assert_eq!(CasoFalha::PinErrado.regra().expect("rule").c_stat, 0);
    assert!(!format!("{a3:?}").contains("1234"));
}

#[test]
fn a3_key_cannot_be_asked_for() {
    // The token has no method that returns the key; only the signature comes out of it.
    let a3 = token("1234");
    a3.entrar(0, "1234").expect("pin");
    assert_eq!(a3.sign_rsa_sha1(b"mensagem").expect("signed").len(), 256);
    assert!(format!("{a3:?}").contains("<fica no token>"));
}

// ---------------------------------------------------------------- external tools

fn openssl() -> Option<std::path::PathBuf> {
    let candidato =
        std::env::var_os("OPENSSL_BIN").map_or_else(|| "openssl".into(), std::path::PathBuf::from);
    std::process::Command::new(&candidato).arg("version").output().ok().map(|_| candidato)
}

#[test]
fn openssl_reads_the_certificate_and_the_chain_closes() {
    let Some(openssl) = openssl() else {
        eprintln!("openssl ausente: conferência não rodou");
        return;
    };
    let pasta = pasta_temporaria("cert-openssl");
    let certificado: CertificadoGerado = gerador().emitir().expect("issued");
    let folha = pasta.join("folha.pem");
    let ac = pasta.join("ac.pem");
    std::fs::write(&folha, certificado.pem()).expect("write");
    std::fs::write(&ac, pem_certificado(certificado.cadeia())).expect("write");
    let saida = std::process::Command::new(&openssl)
        .args([
            "x509",
            "-noout",
            "-subject",
            "-issuer",
            "-ext",
            "subjectAltName,extendedKeyUsage,keyUsage",
            "-in",
        ])
        .arg(&folha)
        .output()
        .expect("openssl");
    let texto = String::from_utf8_lossy(&saida.stdout);
    assert!(saida.status.success(), "{}", String::from_utf8_lossy(&saida.stderr));
    assert!(texto.contains("RAVENRS SANDBOX AC - SEM VALOR"), "{texto}");
    assert!(texto.contains("MERCADO DE TESTE LTDA:11222333000181"), "{texto}");
    let verifica = std::process::Command::new(&openssl)
        .args(["verify", "-CAfile"])
        .arg(&ac)
        .arg(&folha)
        .output()
        .expect("openssl");
    assert!(verifica.status.success(), "{}", String::from_utf8_lossy(&verifica.stderr));

    // The broken chain has to fail in openssl too, or the check above means nothing.
    let quebrado = pasta.join("quebrado.pem");
    std::fs::write(&quebrado, gerador().cadeia_quebrada().expect("issued").pem()).expect("write");
    let falha = std::process::Command::new(&openssl)
        .args(["verify", "-CAfile"])
        .arg(&ac)
        .arg(&quebrado)
        .output()
        .expect("openssl");
    assert!(!falha.status.success());
}

#[cfg(all(feature = "http", not(target_arch = "wasm32")))]
mod pfx {
    use super::*;
    use ravenrs_nfe::http::{CertificadoA1, HttpTransport, SenhaA1};

    #[test]
    fn generated_pfx_opens_in_the_production_path_and_signs() {
        let gerado = gerador().emitir().expect("issued");
        let senha = SenhaA1::new("senha-de-teste");
        let pfx = gerado.pfx(&senha).expect("pfx");
        let a1 = CertificadoA1::from_pfx(&pfx, &senha).expect("a1");
        assert_eq!(a1.certificate_der(), gerado.der());
        assert!(CertificadoA1::from_pfx(&pfx, &SenhaA1::new("errada")).is_err());

        let server = SefazSandbox::new();
        let emissor = emissor_com_certificado(&server, a1);
        assert_eq!(emissor.emitir(&nfe55(proximo_numero())).expect("emission").autorizacao.c_stat(), 100);
    }

    #[test]
    fn http_transport_refuses_the_laboratory() {
        let gerado = gerador().emitir().expect("issued");
        let senha = SenhaA1::new("senha-de-teste");
        let a1 = CertificadoA1::from_pfx(&gerado.pfx(&senha).expect("pfx"), &senha).expect("a1");
        let resultado =
            HttpTransport::builder(&a1).raizes_pem(pem_certificado(gerado.cadeia()).as_bytes()).build();
        match resultado {
            Err(NfeError::Configuracao(motivo)) => {
                assert!(motivo.contains("fora do laboratório"), "{motivo}");
            }
            other => panic!("deveria recusar: {other:?}"),
        }
    }

    #[test]
    fn openssl_opens_the_generated_pfx() {
        let Some(openssl) = openssl() else {
            eprintln!("openssl ausente: conferência não rodou");
            return;
        };
        let pasta = pasta_temporaria("pfx-openssl");
        let gerado = gerador().emitir().expect("issued");
        let arquivo = pasta.join("a1.pfx");
        std::fs::write(&arquivo, gerado.pfx(&SenhaA1::new("senha-de-teste")).expect("pfx")).expect("write");
        let abre = std::process::Command::new(&openssl)
            .args(["pkcs12", "-noout", "-passin", "pass:senha-de-teste", "-in"])
            .arg(&arquivo)
            .output()
            .expect("openssl");
        assert!(abre.status.success(), "{}", String::from_utf8_lossy(&abre.stderr));
        let errada = std::process::Command::new(&openssl)
            .args(["pkcs12", "-noout", "-passin", "pass:errada", "-in"])
            .arg(&arquivo)
            .output()
            .expect("openssl");
        assert!(!errada.status.success());
    }
}

#[test]
fn faults_and_certificates_do_not_touch_each_other() {
    let server = SefazSandbox::new();
    server.injetar(RegraDeFalha::nova(Falha::Paralisado(108)).vezes(1));
    let emissor = emissor_com_certificado(&server, gerador().vencido().expect("issued"));
    let parado = emissor.status(Modelo::Nfe55).expect("status");
    assert_eq!(parado.retorno.c_stat, 108);
}
