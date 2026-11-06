//! The certificates of the laboratory come from `ravenrs-dfe`; what is NF-e here is which rule of
//! the catalog each kind of failure should trigger.

pub use ravenrs_dfe::certificado_x509::eh_do_laboratorio;
pub use ravenrs_dfe::certificados::*;

use super::catalogo::{Regra, regra};

/// The rule of the NF-e catalog a failure of the certificate triggers.
pub trait RegraDoCaso {
    /// The catalog rule, with its code, source, trust and how to fix it.
    fn regra(self) -> Option<&'static Regra>;
}

impl RegraDoCaso for CasoFalha {
    fn regra(self) -> Option<&'static Regra> {
        regra(self.id_regra())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_case_points_at_a_rule_of_the_catalog() {
        for caso in CasoFalha::TODOS {
            let regra = caso.regra().expect("rule");
            assert!(!regra.como_corrigir.is_empty(), "{caso:?}");
        }
        assert_eq!(CasoFalha::CnpjDiferente.regra().expect("rule").c_stat, 213);
        assert_eq!(CasoFalha::Vencido.regra().expect("rule").c_stat, 291);
        assert_eq!(CasoFalha::Revogado.regra().expect("rule").c_stat, 294);
        assert_eq!(CasoFalha::PinErrado.regra().expect("rule").c_stat, 0);
    }
}
