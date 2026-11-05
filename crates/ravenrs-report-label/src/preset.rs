//! Brands only pick a language. How far each list is confirmed is in FONTES.md.

use crate::language::Language;

/// A printer model and the languages it takes, the first one preferred. The choice stays editable: a
/// preset only fills in [`LabelJob::language`](crate::LabelJob::language).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrinterPreset {
    /// The model.
    pub name: &'static str,
    /// Its languages. PPLB is [`Language::Epl`].
    pub languages: &'static [Language],
    /// What to set on the printer before it takes these languages, to show to whoever installs it.
    pub setup: Option<&'static str>,
}

const fn preset(name: &'static str, languages: &'static [Language]) -> PrinterPreset {
    PrinterPreset { name, languages, setup: None }
}

const PRESETS: [PrinterPreset; 9] = [
    preset("Elgin L42 Pro", &[Language::Zpl, Language::Epl, Language::Ppla]),
    preset("Elgin L42 DT", &[Language::Zpl, Language::Epl, Language::Tspl]),
    preset("Zebra ZD220", &[Language::Zpl, Language::Epl]),
    preset("TSC", &[Language::Tspl]),
    preset("Argox", &[Language::Ppla, Language::PPLB]),
    preset("Epson TM-T20", &[Language::EscPos]),
    preset("Epson TM-T20II", &[Language::EscPos]),
    preset("Elgin i9", &[Language::EscPos]),
    PrinterPreset {
        name: "Bematech MP-4200 TH",
        languages: &[Language::EscPos],
        setup: Some(
            "A impressora tem dois modos, ESC/Bema e ESC/POS, e só um fica ativo. Ela precisa estar em \
             ESC/POS: o corte usado aqui só existe nesse modo.",
        ),
    },
];

/// The printers known by name.
pub fn presets() -> &'static [PrinterPreset] {
    &PRESETS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_has_a_language_and_a_distinct_name() {
        for (index, preset) in presets().iter().enumerate() {
            assert!(!preset.languages.is_empty(), "{}", preset.name);
            assert!(presets().iter().skip(index + 1).all(|other| other.name != preset.name));
        }
    }

    #[test]
    fn receipt_printers_are_there() {
        let find = |name: &str| presets().iter().find(|preset| preset.name == name);
        for name in ["Epson TM-T20", "Elgin i9", "Bematech MP-4200 TH"] {
            assert_eq!(find(name).map(|preset| preset.languages), Some(&[Language::EscPos][..]), "{name}");
        }
        assert!(find("Bematech MP-4200 TH").is_some_and(|preset| preset.setup.is_some()));
    }
}
