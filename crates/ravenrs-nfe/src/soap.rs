//! The SOAP 1.2 envelope of the four services, with the action inside `Content-Type`.

use crate::transporte::HttpRequest;

/// Namespace of the NF-e layout.
pub const NS_NFE: &str = "http://www.portalfiscal.inf.br/nfe";
/// Namespace of the SOAP 1.2 envelope.
pub const NS_SOAP12: &str = "http://www.w3.org/2003/05/soap-envelope";
/// The prefix of a service's namespace.
const NS_WSDL: &str = "http://www.portalfiscal.inf.br/nfe/wsdl";

/// A service of the SEFAZ used by this POC. Voiding number ranges is out on purpose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Servico {
    /// `NFeStatusServico4`: is the service up.
    Status,
    /// `NFeAutorizacao4`: send a document, synchronous only.
    Autorizacao,
    /// `NFeConsultaProtocolo4`: the situation of a key.
    Consulta,
    /// `NFeRecepcaoEvento4`: events, of which only cancellation is used.
    Evento,
}

impl Servico {
    /// The service name in the WSDL.
    #[must_use]
    pub const fn nome(self) -> &'static str {
        match self {
            Self::Status => "NFeStatusServico4",
            Self::Autorizacao => "NFeAutorizacao4",
            Self::Consulta => "NFeConsultaProtocolo4",
            Self::Evento => "NFeRecepcaoEvento4",
        }
    }

    /// The operation in the WSDL.
    #[must_use]
    pub const fn operacao(self) -> &'static str {
        match self {
            Self::Status => "nfeStatusServicoNF",
            Self::Autorizacao => "nfeAutorizacaoLote",
            Self::Consulta => "nfeConsultaNF",
            Self::Evento => "nfeRecepcaoEvento",
        }
    }

    /// The namespace of `nfeDadosMsg` and `nfeResultMsg`.
    #[must_use]
    pub fn namespace(self) -> String {
        format!("{NS_WSDL}/{}", self.nome())
    }

    /// The SOAP action.
    #[must_use]
    pub fn acao(self) -> String {
        format!("{NS_WSDL}/{}/{}", self.nome(), self.operacao())
    }

    /// The value of `Content-Type`.
    #[must_use]
    pub fn content_type(self) -> String {
        format!("application/soap+xml;charset=utf-8;action=\"{}\"", self.acao())
    }

    /// Finds the service of an action.
    #[must_use]
    pub fn from_acao(acao: &str) -> Option<Self> {
        [Self::Status, Self::Autorizacao, Self::Consulta, Self::Evento]
            .into_iter()
            .find(|servico| servico.acao() == acao)
    }
}

/// Reads the `action` parameter of a `Content-Type`, quoted or not.
#[must_use]
pub fn acao_do_content_type(content_type: &str) -> Option<&str> {
    content_type.split(';').find_map(|parte| {
        let (nome, valor) = parte.trim().split_once('=')?;
        nome.trim().eq_ignore_ascii_case("action").then(|| valor.trim().trim_matches('"'))
    })
}

/// Wraps a message in the envelope of a service.
#[must_use]
pub fn envelope(servico: Servico, dados: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><soap12:Envelope xmlns:soap12=\"{NS_SOAP12}\"><soap12:Body><nfeDadosMsg xmlns=\"{}\">{dados}</nfeDadosMsg></soap12:Body></soap12:Envelope>",
        servico.namespace()
    )
}

/// Builds the request of a service.
#[must_use]
pub fn request(servico: Servico, url: &str, dados: &str) -> HttpRequest {
    HttpRequest {
        url: url.to_string(),
        headers: vec![("Content-Type".to_string(), servico.content_type())],
        body: envelope(servico, dados).into_bytes(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_lives_in_the_content_type() {
        assert_eq!(
            Servico::Autorizacao.content_type(),
            "application/soap+xml;charset=utf-8;action=\"http://www.portalfiscal.inf.br/nfe/wsdl/NFeAutorizacao4/nfeAutorizacaoLote\""
        );
        assert_eq!(
            acao_do_content_type(&Servico::Status.content_type()),
            Some("http://www.portalfiscal.inf.br/nfe/wsdl/NFeStatusServico4/nfeStatusServicoNF")
        );
        assert_eq!(acao_do_content_type("application/soap+xml;action=x"), Some("x"));
        assert_eq!(acao_do_content_type("text/xml"), None);
    }

    #[test]
    fn every_service_has_its_operation() {
        assert_eq!(
            Servico::Consulta.acao(),
            "http://www.portalfiscal.inf.br/nfe/wsdl/NFeConsultaProtocolo4/nfeConsultaNF"
        );
        assert_eq!(
            Servico::Evento.acao(),
            "http://www.portalfiscal.inf.br/nfe/wsdl/NFeRecepcaoEvento4/nfeRecepcaoEvento"
        );
        assert_eq!(Servico::from_acao(&Servico::Evento.acao()), Some(Servico::Evento));
        assert_eq!(Servico::from_acao("urn:outra"), None);
    }

    #[test]
    fn envelope_is_soap12_with_the_message_inside() {
        let text = envelope(Servico::Status, "<x/>");
        assert!(text.contains("xmlns:soap12=\"http://www.w3.org/2003/05/soap-envelope\""));
        assert!(text.contains("<nfeDadosMsg xmlns=\"http://www.portalfiscal.inf.br/nfe/wsdl/NFeStatusServico4\"><x/></nfeDadosMsg>"));
    }
}
