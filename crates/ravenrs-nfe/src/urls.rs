//! The homologation addresses of the POC. There are no production addresses in the crate.
//!
//! # Origin
//!
//! The web service addresses were read from the table `storage/wsnfe_4.00_mod55.xml` and
//! `wsnfe_4.00_mod65.xml` of sped-nfe, in November 2026. They are facts about public servers and
//! were written here by hand, for a few states only.
//!
//! # Not checked
//!
//! - The national environment comes from the entry `AN` of the same table.
//! - None of them was called: that needs an A1 certificate and an accredited taxpayer.
//! - The official list of states served by SVRS was not read. SC, RJ, ES and DF go to SVRS because
//!   the two tables give them no address of their own.
//! - The QR Code addresses come from the same table. `url_consulta_chave` has no table at all:
//!   its values are from memory, and a wrong one gets rejection 878 from the real SEFAZ. Use
//!   `EmissorBuilder::url_consulta` to replace it. See `REGRAS.md`.

use crate::ambiente::{Modelo, Uf};
use crate::soap::Servico;

/// The authorizer of a state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Autorizador {
    Sp,
    Rs,
    Pr,
    Mg,
    Svrs,
}

const fn autorizador(uf: Uf) -> Autorizador {
    match uf {
        Uf::SP => Autorizador::Sp,
        Uf::RS => Autorizador::Rs,
        Uf::PR => Autorizador::Pr,
        Uf::MG => Autorizador::Mg,
        Uf::SC | Uf::RJ | Uf::ES | Uf::DF => Autorizador::Svrs,
    }
}

/// The national environment in homologation: distribution of documents.
pub const URL_DISTRIBUICAO: &str =
    "https://hom1.nfe.fazenda.gov.br/NFeDistribuicaoDFe/NFeDistribuicaoDFe.asmx";

/// The national environment in homologation: events of the recipient's manifestation.
pub const URL_EVENTO_NACIONAL: &str =
    "https://hom1.nfe.fazenda.gov.br/NFeRecepcaoEvento4/NFeRecepcaoEvento4.asmx";

/// The address of a service in homologation.
#[must_use]
pub const fn url_servico(uf: Uf, modelo: Modelo, servico: Servico) -> &'static str {
    use Autorizador as A;
    use Modelo::{Nfce65 as C, Nfe55 as E};
    use Servico as S;
    match (autorizador(uf), modelo, servico) {
        (_, _, S::Distribuicao) => URL_DISTRIBUICAO,
        (A::Sp, E, S::Status) => "https://homologacao.nfe.fazenda.sp.gov.br/ws/nfestatusservico4.asmx",
        (A::Sp, E, S::Autorizacao) => "https://homologacao.nfe.fazenda.sp.gov.br/ws/nfeautorizacao4.asmx",
        (A::Sp, E, S::Consulta) => "https://homologacao.nfe.fazenda.sp.gov.br/ws/nfeconsultaprotocolo4.asmx",
        (A::Sp, E, S::Evento) => "https://homologacao.nfe.fazenda.sp.gov.br/ws/nferecepcaoevento4.asmx",
        (A::Sp, C, S::Status) => "https://homologacao.nfce.fazenda.sp.gov.br/ws/NFeStatusServico4.asmx",
        (A::Sp, C, S::Autorizacao) => "https://homologacao.nfce.fazenda.sp.gov.br/ws/NFeAutorizacao4.asmx",
        (A::Sp, C, S::Consulta) => "https://homologacao.nfce.fazenda.sp.gov.br/ws/NFeConsultaProtocolo4.asmx",
        (A::Sp, C, S::Evento) => "https://homologacao.nfce.fazenda.sp.gov.br/ws/NFeRecepcaoEvento4.asmx",

        (A::Rs, E, S::Status) => {
            "https://nfe-homologacao.sefazrs.rs.gov.br/ws/NfeStatusServico/NfeStatusServico4.asmx"
        }
        (A::Rs, E, S::Autorizacao) => {
            "https://nfe-homologacao.sefazrs.rs.gov.br/ws/NfeAutorizacao/NFeAutorizacao4.asmx"
        }
        (A::Rs, E, S::Consulta) => {
            "https://nfe-homologacao.sefazrs.rs.gov.br/ws/NfeConsulta/NfeConsulta4.asmx"
        }
        (A::Rs, E, S::Evento) => {
            "https://nfe-homologacao.sefazrs.rs.gov.br/ws/recepcaoevento/recepcaoevento4.asmx"
        }
        (A::Rs, C, S::Status) => {
            "https://nfce-homologacao.sefazrs.rs.gov.br/ws/NfeStatusServico/NfeStatusServico4.asmx"
        }
        (A::Rs, C, S::Autorizacao) => {
            "https://nfce-homologacao.sefazrs.rs.gov.br/ws/NfeAutorizacao/NFeAutorizacao4.asmx"
        }
        (A::Rs, C, S::Consulta) => {
            "https://nfce-homologacao.sefazrs.rs.gov.br/ws/NfeConsulta/NfeConsulta4.asmx"
        }
        (A::Rs, C, S::Evento) => {
            "https://nfce-homologacao.sefazrs.rs.gov.br/ws/recepcaoevento/recepcaoevento4.asmx"
        }

        (A::Pr, E, S::Status) => "https://homologacao.nfe.sefa.pr.gov.br/nfe/NFeStatusServico4",
        (A::Pr, E, S::Autorizacao) => "https://homologacao.nfe.sefa.pr.gov.br/nfe/NFeAutorizacao4",
        (A::Pr, E, S::Consulta) => "https://homologacao.nfe.sefa.pr.gov.br/nfe/NFeConsultaProtocolo4",
        (A::Pr, E, S::Evento) => "https://homologacao.nfe.sefa.pr.gov.br/nfe/NFeRecepcaoEvento4",
        (A::Pr, C, S::Status) => "https://homologacao.nfce.sefa.pr.gov.br/nfce/NFeStatusServico4",
        (A::Pr, C, S::Autorizacao) => "https://homologacao.nfce.sefa.pr.gov.br/nfce/NFeAutorizacao4",
        (A::Pr, C, S::Consulta) => "https://homologacao.nfce.sefa.pr.gov.br/nfce/NFeConsultaProtocolo4",
        (A::Pr, C, S::Evento) => "https://homologacao.nfce.sefa.pr.gov.br/nfce/NFeRecepcaoEvento4",

        (A::Mg, E, S::Status) => "https://hnfe.fazenda.mg.gov.br/nfe2/services/NFeStatusServico4",
        (A::Mg, E, S::Autorizacao) => "https://hnfe.fazenda.mg.gov.br/nfe2/services/NFeAutorizacao4",
        (A::Mg, E, S::Consulta) => "https://hnfe.fazenda.mg.gov.br/nfe2/services/NFeConsultaProtocolo4",
        (A::Mg, E, S::Evento) => "https://hnfe.fazenda.mg.gov.br/nfe2/services/NFeRecepcaoEvento4",
        (A::Mg, C, S::Status) => "https://hnfce.fazenda.mg.gov.br/nfce/services/NFeStatusServico4",
        (A::Mg, C, S::Autorizacao) => "https://hnfce.fazenda.mg.gov.br/nfce/services/NFeAutorizacao4",
        (A::Mg, C, S::Consulta) => "https://hnfce.fazenda.mg.gov.br/nfce/services/NFeConsultaProtocolo4",
        (A::Mg, C, S::Evento) => "https://hnfce.fazenda.mg.gov.br/nfce/services/NFeRecepcaoEvento4",

        (A::Svrs, E, S::Status) => {
            "https://nfe-homologacao.svrs.rs.gov.br/ws/NfeStatusServico/NfeStatusServico4.asmx"
        }
        (A::Svrs, E, S::Autorizacao) => {
            "https://nfe-homologacao.svrs.rs.gov.br/ws/NfeAutorizacao/NFeAutorizacao4.asmx"
        }
        (A::Svrs, E, S::Consulta) => {
            "https://nfe-homologacao.svrs.rs.gov.br/ws/NfeConsulta/NfeConsulta4.asmx"
        }
        (A::Svrs, E, S::Evento) => {
            "https://nfe-homologacao.svrs.rs.gov.br/ws/recepcaoevento/recepcaoevento4.asmx"
        }
        (A::Svrs, C, S::Status) => {
            "https://nfce-homologacao.svrs.rs.gov.br/ws/NfeStatusServico/NfeStatusServico4.asmx"
        }
        (A::Svrs, C, S::Autorizacao) => {
            "https://nfce-homologacao.svrs.rs.gov.br/ws/NfeAutorizacao/NFeAutorizacao4.asmx"
        }
        (A::Svrs, C, S::Consulta) => {
            "https://nfce-homologacao.svrs.rs.gov.br/ws/NfeConsulta/NfeConsulta4.asmx"
        }
        (A::Svrs, C, S::Evento) => {
            "https://nfce-homologacao.svrs.rs.gov.br/ws/recepcaoevento/recepcaoevento4.asmx"
        }
    }
}

/// The address the QR Code of an NFC-e points to, in homologation.
#[must_use]
pub const fn url_qr_code(uf: Uf) -> &'static str {
    match uf {
        Uf::SP => "https://www.homologacao.nfce.fazenda.sp.gov.br/qrcode",
        Uf::RS => "https://www.sefaz.rs.gov.br/NFCE/NFCE-COM.aspx",
        Uf::PR => "http://www.fazenda.pr.gov.br/nfce/qrcode",
        Uf::MG => "https://portalsped.fazenda.mg.gov.br/portalnfce/sistema/qrcode.xhtml",
        Uf::SC => "https://hom.sat.sef.sc.gov.br/nfce/consulta",
        Uf::RJ => "http://www4.fazenda.rj.gov.br/consultaNFCe/QRCode",
        Uf::ES => "http://homologacao.sefaz.es.gov.br/ConsultaNFCe/qrcode.aspx",
        Uf::DF => "http://dec.fazenda.df.gov.br/ConsultarNFCe.aspx",
    }
}

/// The address of the query by access key, `urlChave`. Not checked against any table.
#[must_use]
pub const fn url_consulta_chave(uf: Uf) -> &'static str {
    match uf {
        Uf::SP => "https://www.homologacao.nfce.fazenda.sp.gov.br/consulta",
        Uf::RS => "https://www.sefaz.rs.gov.br/nfce/consulta",
        Uf::PR => "http://www.fazenda.pr.gov.br/nfce/consulta",
        Uf::MG => "https://hportalsped.fazenda.mg.gov.br/portalnfce",
        Uf::SC => "https://hom.sat.sef.sc.gov.br/nfce/consulta",
        Uf::RJ => "http://www.fazenda.rj.gov.br/nfce/consulta",
        Uf::ES => "http://homologacao.sefaz.es.gov.br/ConsultaNFCe",
        Uf::DF => "http://www.fazenda.df.gov.br/nfce/consulta",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UFS: [Uf; 8] = [Uf::SP, Uf::RS, Uf::PR, Uf::MG, Uf::SC, Uf::RJ, Uf::ES, Uf::DF];
    const SERVICOS: [Servico; 4] =
        [Servico::Status, Servico::Autorizacao, Servico::Consulta, Servico::Evento];

    #[test]
    fn every_address_is_https_and_homologation() {
        for uf in UFS {
            for modelo in [Modelo::Nfe55, Modelo::Nfce65] {
                for servico in SERVICOS {
                    let url = url_servico(uf, modelo, servico);
                    assert!(url.starts_with("https://"), "{url}");
                    let lower = url.to_lowercase();
                    assert!(
                        lower.contains("homologacao") || lower.contains("hnf"),
                        "not homologation: {url}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_addresses_the_plan_names_are_there() {
        assert_eq!(
            url_servico(Uf::SC, Modelo::Nfe55, Servico::Status),
            "https://nfe-homologacao.svrs.rs.gov.br/ws/NfeStatusServico/NfeStatusServico4.asmx"
        );
        assert_eq!(
            url_servico(Uf::SP, Modelo::Nfe55, Servico::Status),
            "https://homologacao.nfe.fazenda.sp.gov.br/ws/nfestatusservico4.asmx"
        );
    }

    #[test]
    fn consulta_addresses_fit_the_schema_size() {
        for uf in UFS {
            assert!(url_consulta_chave(uf).len() >= 21 && url_consulta_chave(uf).len() <= 85);
        }
    }
}
