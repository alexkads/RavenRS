//! A sample DPS, for tests, documentation and the sandbox. The CNPJs are the usual test ones.

use rust_decimal::Decimal;

use crate::modelo::{Dps, Inscricao, Prestador, Servico, Tomador};

/// The CNPJ of the sample provider.
pub const CNPJ_PRESTADOR: &str = "11222333000181";
/// São Paulo, by its IBGE code.
pub const MUNICIPIO: &str = "3550308";

/// The sample provider: ME/EPP in São Paulo.
#[must_use]
pub fn prestador() -> Prestador {
    Prestador {
        inscricao: Inscricao::Cnpj(CNPJ_PRESTADOR.to_owned()),
        inscricao_municipal: Some("12345678".to_owned()),
        razao_social: "MERCADO EXEMPLO LTDA".to_owned(),
        municipio: MUNICIPIO.to_owned(),
    }
}

/// A DPS with this number: an installation service of R$ 150,00, to a company.
#[must_use]
pub fn dps(numero: u64) -> Dps {
    Dps {
        serie: 1,
        numero,
        emitida_em: "2026-11-06T10:00:00-03:00".to_owned(),
        competencia: "2026-11-06".to_owned(),
        prestador: prestador(),
        tomador: Some(Tomador {
            inscricao: Inscricao::Cnpj("11444777000161".to_owned()),
            nome: "ATACADO EXEMPLO LTDA".to_owned(),
            email: None,
        }),
        servico: Servico {
            codigo_tributacao_nacional: "140101".to_owned(),
            descricao: "Instalação e configuração de balança no caixa".to_owned(),
            municipio_prestacao: MUNICIPIO.to_owned(),
            valor: Decimal::new(15_000, 2),
        },
    }
}
