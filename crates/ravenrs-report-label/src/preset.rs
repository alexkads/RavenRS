//! The printers among the drivers that come with the program. This is only a view of that catalog,
//! for choosing by name: what is known about each one lives in `ravenrs-perifericos`.

use std::sync::OnceLock;

use ravenrs_perifericos::{Ajustes, Manifesto, TipoPeriferico, manifestos_embutidos};

use crate::language::Language;

/// A printer model and the languages it takes, the first one preferred. The choice stays editable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrinterPreset {
    /// The exact model.
    pub name: String,
    /// The driver this comes from.
    pub driver_id: String,
    /// Its languages. PPLB is [`Language::Epl`].
    pub languages: Vec<Language>,
    /// What differs on this model, for [`LabelJob::ajustes`](crate::LabelJob::ajustes).
    pub ajustes: Ajustes,
    /// What to set on the printer before using it, to show to whoever installs it.
    pub setup: Option<String>,
}

fn preset_de(driver: &Manifesto) -> Option<PrinterPreset> {
    if !matches!(driver.tipo, TipoPeriferico::ImpressoraTermica | TipoPeriferico::Etiquetadora) {
        return None;
    }
    let protocolos = std::iter::once(&driver.protocolo).chain(&driver.protocolos_alternativos);
    Some(PrinterPreset {
        name: driver.nome_do_modelo().to_owned(),
        driver_id: driver.id.clone(),
        languages: protocolos.filter_map(|protocolo| Language::do_protocolo(protocolo.nome)).collect(),
        ajustes: driver.ajustes,
        setup: driver.instalacao.clone(),
    })
}

/// The printers known by name.
pub fn presets() -> &'static [PrinterPreset] {
    static PRESETS: OnceLock<Vec<PrinterPreset>> = OnceLock::new();
    PRESETS.get_or_init(|| manifestos_embutidos().iter().filter_map(preset_de).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_has_a_language_and_a_distinct_name() {
        assert_eq!(presets().len(), 9);
        for (index, preset) in presets().iter().enumerate() {
            assert!(!preset.languages.is_empty(), "{}", preset.name);
            assert!(presets().iter().skip(index + 1).all(|other| other.name != preset.name));
        }
    }

    #[test]
    fn no_preset_has_only_a_brand_for_a_name() {
        for vague in ["TSC", "Argox", "Elgin", "Epson", "Zebra"] {
            assert!(presets().iter().all(|preset| preset.name != vague), "{vague}");
        }
    }

    #[test]
    fn printers_keep_their_languages_and_adjustments() {
        let find = |name: &str| presets().iter().find(|preset| preset.name == name);
        for name in ["Epson TM-T20", "Elgin i9", "Bematech MP-4200 TH"] {
            assert_eq!(
                find(name).map(|preset| preset.languages.clone()),
                Some(vec![Language::EscPos]),
                "{name}"
            );
        }
        assert_eq!(
            find("Elgin L42 Pro").map(|preset| preset.languages.clone()),
            Some(vec![Language::Zpl, Language::Epl, Language::Ppla])
        );
        let bematech = find("Bematech MP-4200 TH");
        assert!(bematech.is_some_and(|preset| preset.setup.is_some() && preset.ajustes.bipe.is_some()));
    }
}
