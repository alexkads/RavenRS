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
}

const PRESETS: [PrinterPreset; 5] = [
    PrinterPreset { name: "Elgin L42 Pro", languages: &[Language::Zpl, Language::Epl, Language::Ppla] },
    PrinterPreset { name: "Elgin L42 DT", languages: &[Language::Zpl, Language::Epl, Language::Tspl] },
    PrinterPreset { name: "Zebra ZD220", languages: &[Language::Zpl, Language::Epl] },
    PrinterPreset { name: "TSC", languages: &[Language::Tspl] },
    PrinterPreset { name: "Argox", languages: &[Language::Ppla, Language::PPLB] },
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
}
