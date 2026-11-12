//! The drivers that come with the program, as the same JSON files anyone would write.

use crate::manifesto::Manifesto;

pub(crate) const FONTES_EMBUTIDAS: [&str; 12] = [
    include_str!("../drivers/epson.tm-t20.cupom.json"),
    include_str!("../drivers/epson.tm-t20ii.cupom.json"),
    include_str!("../drivers/elgin.i9.cupom.json"),
    include_str!("../drivers/bematech.mp-4200-th.cupom.json"),
    include_str!("../drivers/elgin.l42-pro.etiqueta.json"),
    include_str!("../drivers/elgin.l42-dt.etiqueta.json"),
    include_str!("../drivers/zebra.zd220.etiqueta.json"),
    include_str!("../drivers/tsc.ttp-244me.etiqueta.json"),
    include_str!("../drivers/argox.x-2300.etiqueta.json"),
    include_str!("../drivers/generico.hid-teclado.leitor.json"),
    include_str!("../drivers/gertec.ppc930.pinpad.json"),
    include_str!("../drivers/ingenico.lane-3000.pinpad.json"),
];

/// The manifests that come with the program.
pub fn manifestos_embutidos() -> Vec<Manifesto> {
    // Each one is proven valid by a test, so none is left out here.
    FONTES_EMBUTIDAS.iter().filter_map(|fonte| Manifesto::de_json(fonte.as_bytes()).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifesto::{Situacao, TipoPeriferico};

    #[test]
    fn every_embedded_driver_is_valid() {
        for fonte in FONTES_EMBUTIDAS {
            if let Err(erro) = Manifesto::de_json(fonte.as_bytes()) {
                panic!("{erro}\n{fonte}");
            }
        }
        assert_eq!(manifestos_embutidos().len(), FONTES_EMBUTIDAS.len());
    }

    #[test]
    fn no_embedded_driver_claims_a_test_on_a_device() {
        for manifesto in manifestos_embutidos() {
            assert!(
                !matches!(manifesto.situacao, Situacao::TestadoNoAparelho { .. }),
                "{}: nenhum foi testado em aparelho",
                manifesto.id
            );
        }
    }

    #[test]
    fn the_catalog_covers_printers_scanners_and_pinpads() {
        let manifestos = manifestos_embutidos();
        let conta = |tipo| manifestos.iter().filter(|m| m.tipo == tipo).count();
        assert_eq!(conta(TipoPeriferico::ImpressoraTermica), 4);
        assert_eq!(conta(TipoPeriferico::Etiquetadora), 5);
        assert_eq!(conta(TipoPeriferico::LeitorCodigo), 1);
        assert_eq!(conta(TipoPeriferico::Pinpad), 2);
    }
}
