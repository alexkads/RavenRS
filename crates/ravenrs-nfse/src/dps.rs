//! The XML of the DPS: deterministic, with no spaces or line breaks, in the order of the XSD v1.01.

use ravenrs_dfe::xml::escape_text;
use ravenrs_dfe::{Algoritmo, Signer};
use rust_decimal::Decimal;

use crate::ambiente::{NS_NFSE, VER_APLIC, VERSAO};
use crate::erro::NfseError;
use crate::modelo::Dps;

/// The Id of the DPS (45): `DPS` + city (7) + kind of registration (1) + registration (14) +
/// series (5) + number (15). The same DPS has the same Id, which is how a lost answer is found.
#[must_use]
pub fn id_dps(dps: &Dps) -> String {
    format!(
        "DPS{}{}{:0>14}{:05}{:015}",
        dps.prestador.municipio,
        dps.prestador.inscricao.tipo(),
        dps.prestador.inscricao.numero(),
        dps.serie,
        dps.numero
    )
}

/// Money with two decimals, as `TSDec15V2` asks.
#[must_use]
pub fn valor(valor: Decimal) -> String {
    format!("{:.2}", valor.round_dp(2))
}

/// The description with no carriage return: a CR in `xDescServ` is a reported cause of E0714.
fn descricao(texto: &str) -> String {
    escape_text(texto.trim().replace("\r\n", "\n").replace('\r', "\n").as_str())
}

/// Writes the DPS, unsigned.
pub fn escrever(dps: &Dps) -> Result<String, NfseError> {
    dps.validar()?;
    let prestador = &dps.prestador;
    let mut xml = format!(
        "<DPS xmlns=\"{NS_NFSE}\" versao=\"{VERSAO}\"><infDPS Id=\"{id}\"><tpAmb>2</tpAmb><dhEmi>{dh}</dhEmi><verAplic>{VER_APLIC}</verAplic><serie>{serie}</serie><nDPS>{numero}</nDPS><dCompet>{compet}</dCompet><tpEmit>1</tpEmit><cLocEmi>{mun}</cLocEmi><prest><{tag}>{insc}</{tag}>",
        id = id_dps(dps),
        dh = escape_text(&dps.emitida_em),
        serie = dps.serie,
        numero = dps.numero,
        compet = escape_text(&dps.competencia),
        mun = prestador.municipio,
        tag = prestador.inscricao.tag(),
        insc = prestador.inscricao.numero(),
    );
    if let Some(im) = &prestador.inscricao_municipal {
        xml.push_str(&format!("<IM>{}</IM>", escape_text(im)));
    }
    // ME/EPP in the Simples, everything calculated in it, no special regime.
    xml.push_str("<regTrib><opSimpNac>3</opSimpNac><regApTribSN>1</regApTribSN><regEspTrib>0</regEspTrib></regTrib></prest>");
    if let Some(tomador) = &dps.tomador {
        let tag = tomador.inscricao.tag();
        xml.push_str(&format!(
            "<toma><{tag}>{}</{tag}><xNome>{}</xNome>",
            tomador.inscricao.numero(),
            escape_text(tomador.nome.trim())
        ));
        if let Some(email) = &tomador.email {
            xml.push_str(&format!("<email>{}</email>", escape_text(email)));
        }
        xml.push_str("</toma>");
    }
    let servico = &dps.servico;
    xml.push_str(&format!(
        "<serv><locPrest><cLocPrestacao>{}</cLocPrestacao></locPrest><cServ><cTribNac>{}</cTribNac><xDescServ>{}</xDescServ></cServ></serv>",
        servico.municipio_prestacao,
        servico.codigo_tributacao_nacional,
        descricao(&servico.descricao)
    ));
    // ISS taxable (tribISSQN=1), not withheld (tpRetISSQN=1), no estimate of the total taxes.
    xml.push_str(&format!(
        "<valores><vServPrest><vServ>{}</vServ></vServPrest><trib><tribMun><tribISSQN>1</tribISSQN><tpRetISSQN>1</tpRetISSQN></tribMun><totTrib><indTotTrib>0</indTotTrib></totTrib></trib></valores></infDPS></DPS>",
        valor(servico.valor)
    ));
    Ok(xml)
}

/// Writes and signs the DPS: the reference is `infDPS`, and `Signature` goes inside `DPS`.
pub fn assinar(dps: &Dps, signer: &dyn Signer, algoritmo: Algoritmo) -> Result<String, NfseError> {
    let xml = escrever(dps)?;
    Ok(ravenrs_dfe::assinatura::assinar_envelopado(&xml, "DPS", &id_dps(dps), signer, algoritmo)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exemplo;

    #[test]
    fn the_id_has_45_characters_in_its_parts() {
        let id = id_dps(&exemplo::dps(42));
        assert_eq!(id.len(), 45);
        assert_eq!(&id[..3], "DPS");
        assert_eq!(&id[3..10], "3550308");
        assert_eq!(&id[10..11], "2");
        assert!(id.ends_with("00001000000000000042"));
    }

    #[test]
    fn writes_in_the_order_of_the_schema_with_no_formatting() {
        let xml = escrever(&exemplo::dps(1)).expect("xml");
        assert!(!xml.contains('\n') && !xml.contains("> <"));
        let ordem = [
            "<tpAmb>",
            "<dhEmi>",
            "<verAplic>",
            "<serie>",
            "<nDPS>",
            "<dCompet>",
            "<tpEmit>",
            "<cLocEmi>",
            "<prest>",
            "<toma>",
            "<serv>",
            "<valores>",
        ];
        let posicoes: Vec<usize> = ordem.iter().map(|tag| xml.find(tag).expect(tag)).collect();
        assert!(posicoes.windows(2).all(|par| par[0] < par[1]), "{xml}");
        assert!(xml.contains("<vServ>150.00</vServ>"));
    }

    #[test]
    fn a_carriage_return_never_reaches_the_description() {
        let mut dps = exemplo::dps(1);
        dps.servico.descricao = "Linha 1\r\nLinha 2".to_owned();
        assert!(!escrever(&dps).expect("xml").contains('\r'));
    }

    #[test]
    fn refuses_before_sending() {
        let mut dps = exemplo::dps(1);
        dps.servico.codigo_tributacao_nacional = "1401".to_owned();
        assert!(matches!(escrever(&dps), Err(NfseError::Validacao { campo, .. }) if campo == "cTribNac"));
        let mut dps = exemplo::dps(1);
        dps.servico.valor = Decimal::new(1001, 3);
        assert!(escrever(&dps).is_err());
    }
}
