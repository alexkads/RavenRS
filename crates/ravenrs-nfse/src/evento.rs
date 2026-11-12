//! The request for an event: here only the cancellation, `e101101`.

use ravenrs_dfe::xml::escape_text;
use ravenrs_dfe::{Algoritmo, Signer};

use crate::ambiente::{NS_NFSE, VER_APLIC, VERSAO};
use crate::chave::ChaveNfse;
use crate::erro::NfseError;
use crate::modelo::Inscricao;

/// The code of the cancellation event.
pub const CANCELAMENTO: &str = "101101";

/// Why an NFS-e is canceled, `cMotivo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MotivoCancelamento {
    /// 1, a mistake in the issue.
    ErroNaEmissao,
    /// 2, the service was not provided.
    ServicoNaoPrestado,
    /// 9, anything else.
    Outros,
}

impl MotivoCancelamento {
    /// The code.
    #[must_use]
    pub const fn codigo(self) -> u8 {
        match self {
            Self::ErroNaEmissao => 1,
            Self::ServicoNaoPrestado => 2,
            Self::Outros => 9,
        }
    }
}

/// The Id of the request: `PRE` + key (50) + event (6), the pattern `PRE[0-9]{56}`. The annotation
/// of the XSD also mentions a sequence; this follows what the ACBr does. Test before relying on it.
#[must_use]
pub fn id_pedido(chave: &ChaveNfse, evento: &str) -> String {
    format!("PRE{}{evento}", chave.as_str())
}

/// Writes and signs the request for cancellation.
pub fn pedido_cancelamento(
    chave: &ChaveNfse,
    autor: &Inscricao,
    motivo: MotivoCancelamento,
    justificativa: &str,
    quando: &str,
    signer: &dyn Signer,
    algoritmo: Algoritmo,
) -> Result<String, NfseError> {
    let justificativa = justificativa.trim();
    if justificativa.chars().count() < 15 || justificativa.chars().count() > 255 {
        return Err(NfseError::validacao("xMotivo", "a justificativa tem de 15 a 255 caracteres"));
    }
    if ravenrs_dfe::Instante::de_xml(quando).is_none() {
        return Err(NfseError::validacao("dhEvento", "use AAAA-MM-DDThh:mm:ss-03:00"));
    }
    let id = id_pedido(chave, CANCELAMENTO);
    let tag = match autor {
        Inscricao::Cnpj(_) => "CNPJAutor",
        Inscricao::Cpf(_) => "CPFAutor",
    };
    let xml = format!(
        "<pedRegEvento xmlns=\"{NS_NFSE}\" versao=\"{VERSAO}\"><infPedReg Id=\"{id}\"><tpAmb>2</tpAmb><verAplic>{VER_APLIC}</verAplic><dhEvento>{quando}</dhEvento><{tag}>{autor}</{tag}><chNFSe>{chave}</chNFSe><e{CANCELAMENTO}><xDesc>Cancelamento de NFS-e</xDesc><cMotivo>{motivo}</cMotivo><xMotivo>{justificativa}</xMotivo></e{CANCELAMENTO}></infPedReg></pedRegEvento>",
        quando = escape_text(quando),
        autor = autor.numero(),
        chave = chave.as_str(),
        motivo = motivo.codigo(),
        justificativa = escape_text(justificativa),
    );
    Ok(ravenrs_dfe::assinatura::assinar_envelopado(&xml, "pedRegEvento", &id, signer, algoritmo)?)
}
