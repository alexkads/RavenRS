//! Translations of the model that come with the framework, as differences in JSON for
//! `translation`: the captions of the entities and navigation its modules add.

use crate::Language;

/// The entities of signing in and of the audit trail, in Brazilian Portuguese.
pub const SECURITY_PT_BR: &str = include_str!("../model/pt-BR/security.json");

/// The entities of stored reports and of analytics dashboards, in Brazilian Portuguese.
pub const REPORTS_PT_BR: &str = include_str!("../model/pt-BR/reports.json");

/// The model of signing in, in this language, when it is not the one it was written in.
pub fn security(language: Language) -> Option<&'static str> {
    match language {
        Language::BrazilianPortuguese => Some(SECURITY_PT_BR),
        _ => None,
    }
}

/// The model of reports, in this language, when it is not the one it was written in.
pub fn reports(language: Language) -> Option<&'static str> {
    match language {
        Language::BrazilianPortuguese => Some(REPORTS_PT_BR),
        _ => None,
    }
}
