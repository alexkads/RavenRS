//! The table of rules with a start date. Dates come from the technical notes named on each line.

use super::catalogo::{Confianca, Fonte};
use super::tempo::Instante;

/// A dated rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vigencia {
    /// What this line is about.
    pub id: &'static str,
    /// What changes on the date.
    pub descricao: &'static str,
    /// Start in homologation, as (year, month, day); `None` when no date is announced.
    pub homologacao: Option<(i64, i64, i64)>,
    /// Start in production, as (year, month, day); `None` when no date is announced.
    pub producao: Option<(i64, i64, i64)>,
    /// Where the dates come from.
    pub fonte: Fonte,
    /// How sure the date is.
    pub confianca: Confianca,
}

/// The dated rules this crate knows.
pub const VIGENCIAS: &[Vigencia] = &[
    Vigencia {
        id: "ASSINCRONO-452",
        descricao: "Lote de uma nota com indSinc=0 passa a dar 452",
        homologacao: None,
        producao: Some((2025, 10, 13)),
        fonte: Fonte::Nt("NT 2025.001 v1.02, GAP03a-3"),
        confianca: Confianca::LidaNaFonte,
    },
    Vigencia {
        id: "ASSINCRONO-EXTINTO",
        descricao: "Toda autorização assíncrona passa a dar 129",
        homologacao: Some((2027, 2, 1)),
        producao: Some((2027, 4, 5)),
        fonte: Fonte::Nt("NT 2023.002 v1.02, cronograma e GAP03a-6"),
        confianca: Confianca::LidaNaFonte,
    },
    Vigencia {
        id: "INUTILIZACAO-AC-BA-SP",
        descricao: "Serviço de inutilização desativado em AC, BA e SP, com 998",
        homologacao: Some((2026, 11, 11)),
        producao: Some((2026, 11, 11)),
        fonte: Fonte::Nt("NT 2023.002 v1.02, I01a"),
        confianca: Confianca::LidaNaFonte,
    },
    Vigencia {
        id: "IBSCBS-1115",
        descricao: "CRT 3 sem grupo IBSCBS dá 1115, pela data de emissão; produção sem data anunciada",
        homologacao: Some((2026, 7, 1)),
        producao: None,
        fonte: Fonte::Nt("NT 2025.002 v1.53, UB12-10"),
        confianca: Confianca::LidaNaFonte,
    },
    Vigencia {
        id: "URLCHAVE-878",
        descricao: "urlChave diferente da prevista pode dar 878, a critério da UF",
        homologacao: Some((2019, 4, 1)),
        producao: Some((2019, 4, 1)),
        fonte: Fonte::Moc("Anexo I, ZX03-20"),
        confianca: Confianca::LidaNaFonte,
    },
];

/// Whether a dated rule already applies at this moment.
///
/// `homologacao` says which column to read. A column with no date means it does not apply yet,
/// and an unknown line never applies.
#[must_use]
pub fn em_vigor(id: &str, homologacao: bool, agora: Instante) -> bool {
    let Some(linha) = VIGENCIAS.iter().find(|linha| linha.id == id) else {
        return false;
    };
    let coluna = if homologacao { linha.homologacao.or(linha.producao) } else { linha.producao };
    coluna.is_some_and(|(ano, mes, dia)| agora >= Instante::utc(ano, mes, dia, 3, 0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asynchronous_ends_on_different_days_per_environment() {
        let dia = |a, m, d| Instante::utc(a, m, d, 12, 0, 0);
        assert!(!em_vigor("ASSINCRONO-EXTINTO", true, dia(2027, 1, 31)));
        assert!(em_vigor("ASSINCRONO-EXTINTO", true, dia(2027, 2, 1)));
        assert!(!em_vigor("ASSINCRONO-EXTINTO", false, dia(2027, 2, 1)));
        assert!(em_vigor("ASSINCRONO-EXTINTO", false, dia(2027, 4, 5)));
    }

    #[test]
    fn column_with_no_date_or_unknown_line_never_applies() {
        let longe = Instante::utc(2090, 1, 1, 0, 0, 0);
        assert!(!em_vigor("IBSCBS-1115", false, longe));
        assert!(em_vigor("IBSCBS-1115", true, longe));
        assert!(!em_vigor("NAO-EXISTE", true, longe));
    }
}
