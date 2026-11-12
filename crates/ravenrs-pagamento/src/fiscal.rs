//! What a fiscal document needs from a payment, and the tables to fill it in.
//!
//! Nothing here depends on `ravenrs-nfe`: these are plain values for the `detPag/card` group.
//! A value that cannot be trusted stays empty and becomes an [`AlertaFiscal`]. Nothing is invented.

use serde::{Deserialize, Serialize};

/// `tpIntegra`: whether the payment is tied to the automation system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TpIntegra {
    /// `1`: integrated (TEF, e-commerce, integrated POS).
    Integrado,
    /// `2`: not integrated.
    NaoIntegrado,
}

impl TpIntegra {
    /// The value of the tag.
    #[must_use]
    pub const fn codigo(self) -> &'static str {
        match self {
            Self::Integrado => "1",
            Self::NaoIntegrado => "2",
        }
    }
}

/// A reason not to issue the fiscal document blindly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertaFiscal {
    /// `cAut` did not come from the provider. Rule YA05-10 may reject (392), depending on the state.
    CAutAusente {
        /// Why it is missing.
        motivo: String,
    },
    /// `CNPJ` of the payment institution did not come and no table has it.
    CnpjAusente {
        /// The network name that was looked for, if any.
        rede: Option<String>,
    },
    /// `CNPJ` came from the table of this crate, not from the provider, and needs a check.
    CnpjDeTabelaNaoConferida {
        /// The network name that was looked for.
        rede: String,
        /// Where the table got it.
        fonte: String,
    },
    /// `tBand` is empty because this brand is not in the table.
    BandeiraDesconhecida {
        /// The brand as the provider wrote it, or nothing if it sent none.
        recebida: Option<String>,
    },
    /// A value came in a format the schema refuses, so it was dropped.
    ValorDescartado {
        /// The tag.
        tag: String,
        /// Why.
        motivo: String,
    },
    /// For Pix, `cAut` holds the end-to-end id. This reading of the rule needs a fiscal review.
    CAutPixPeloEndToEnd,
}

/// The `detPag/card` group of NF-e and NFC-e, as this payment can fill it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DadosFiscaisPagamento {
    /// `tpIntegra`.
    pub tp_integra: TpIntegra,
    /// `CNPJ`: the payment institution.
    pub cnpj: Option<String>,
    /// `tBand`: two digits.
    pub t_band: Option<String>,
    /// `cAut`: the authorization number, 1 to 128 characters in the current schema.
    pub c_aut: Option<String>,
    /// `CNPJReceb`: the beneficiary.
    pub cnpj_receb: Option<String>,
    /// `idTermPag`: the payment terminal, up to 40 characters.
    pub id_term_pag: Option<String>,
    /// What the application must look at before issuing.
    pub alertas: Vec<AlertaFiscal>,
}

impl DadosFiscaisPagamento {
    /// Starts an integrated payment with nothing filled and no alerts yet.
    #[must_use]
    pub fn integrado() -> Self {
        Self {
            tp_integra: TpIntegra::Integrado,
            cnpj: None,
            t_band: None,
            c_aut: None,
            cnpj_receb: None,
            id_term_pag: None,
            alertas: Vec::new(),
        }
    }

    /// Sets `cAut`; an absent, empty or oversized value becomes an alert and stays empty.
    pub fn com_c_aut(&mut self, valor: Option<&str>, motivo_se_ausente: &str) {
        match valor.map(str::trim).filter(|v| !v.is_empty()) {
            Some(valor) if valor.chars().count() <= 128 => self.c_aut = Some(valor.to_owned()),
            Some(_) => {
                self.alertas.push(AlertaFiscal::ValorDescartado {
                    tag: "cAut".into(),
                    motivo: "mais de 128 caracteres".into(),
                });
                self.alertas.push(AlertaFiscal::CAutAusente { motivo: motivo_se_ausente.into() });
            }
            None => self.alertas.push(AlertaFiscal::CAutAusente { motivo: motivo_se_ausente.into() }),
        }
    }

    /// Sets `CNPJ` from a value that came from the provider; an invalid one is dropped with an alert.
    pub fn com_cnpj_do_provedor(&mut self, valor: &str) {
        let limpo = somente_cnpj(valor);
        if cnpj_valido(&limpo) {
            self.cnpj = Some(limpo);
        } else {
            self.alertas.push(AlertaFiscal::ValorDescartado {
                tag: "CNPJ".into(),
                motivo: "formato ou dígito verificador inválido".into(),
            });
            self.alertas.push(AlertaFiscal::CnpjAusente { rede: None });
        }
    }

    /// Sets `CNPJ` from the table by network name, always with an alert saying where it came from.
    pub fn com_cnpj_da_rede(&mut self, rede: Option<&str>) {
        match rede.and_then(cnpj_da_rede) {
            Some(linha) => {
                self.cnpj = Some(linha.cnpj.to_owned());
                self.alertas.push(AlertaFiscal::CnpjDeTabelaNaoConferida {
                    rede: linha.rede.to_owned(),
                    fonte: linha.fonte.to_owned(),
                });
            }
            None => self.alertas.push(AlertaFiscal::CnpjAusente { rede: rede.map(str::to_owned) }),
        }
    }

    /// Sets `tBand` by brand name; an unknown one becomes an alert, never the code 99 on its own.
    pub fn com_bandeira(&mut self, nome: Option<&str>) {
        match nome.and_then(t_band_da_bandeira) {
            Some(linha) => self.t_band = Some(linha.t_band.to_owned()),
            None => {
                self.alertas.push(AlertaFiscal::BandeiraDesconhecida { recebida: nome.map(str::to_owned) })
            }
        }
    }

    /// Sets `idTermPag`; an oversized value is dropped with an alert.
    pub fn com_terminal(&mut self, valor: Option<&str>) {
        match valor.map(str::trim).filter(|v| !v.is_empty()) {
            Some(valor) if valor.chars().count() <= 40 => self.id_term_pag = Some(valor.to_owned()),
            Some(_) => self.alertas.push(AlertaFiscal::ValorDescartado {
                tag: "idTermPag".into(),
                motivo: "mais de 40 caracteres".into(),
            }),
            None => {}
        }
    }

    /// True when `cAut` and `CNPJ` are both here, which is what rule YA05-10 asks of `tpIntegra=1`.
    #[must_use]
    pub fn atende_ya05_10(&self) -> bool {
        self.tp_integra != TpIntegra::Integrado || (self.cnpj.is_some() && self.c_aut.is_some())
    }
}

/// One line of the brand table, with where it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinhaBandeira {
    /// The `tBand` code.
    pub t_band: &'static str,
    /// The name of the brand.
    pub nome: &'static str,
    /// Other names providers use for it, already normalized.
    pub apelidos: &'static [&'static str],
    /// The document this code was read in.
    pub fonte: &'static str,
}

const MOC_7: &str = "MOC 7.0 Anexo I, YA06 (lido); a tabela vigente do Portal NF-e não foi conferida";

/// The codes read in the manual. NT 2020.006 sends the current table to the national portal, which
/// was not read, so it may have more lines. A brand beyond these stays unknown instead of guessed.
/// The aliases of Mercado Pago (`debvisa`, `master`...) come from memory, except `master`.
pub const BANDEIRAS: &[LinhaBandeira] = &[
    LinhaBandeira { t_band: "01", nome: "Visa", apelidos: &["VISA", "DEBVISA"], fonte: MOC_7 },
    LinhaBandeira {
        t_band: "02",
        nome: "Mastercard",
        apelidos: &["MASTERCARD", "MASTER", "DEBMASTER"],
        fonte: MOC_7,
    },
    LinhaBandeira {
        t_band: "03",
        nome: "American Express",
        apelidos: &["AMERICANEXPRESS", "AMEX"],
        fonte: MOC_7,
    },
    LinhaBandeira { t_band: "04", nome: "Sorocred", apelidos: &["SOROCRED"], fonte: MOC_7 },
    LinhaBandeira { t_band: "05", nome: "Diners Club", apelidos: &["DINERSCLUB", "DINERS"], fonte: MOC_7 },
    LinhaBandeira { t_band: "06", nome: "Elo", apelidos: &["ELO", "DEBELO"], fonte: MOC_7 },
    LinhaBandeira { t_band: "07", nome: "Hipercard", apelidos: &["HIPERCARD", "HIPER"], fonte: MOC_7 },
    LinhaBandeira { t_band: "08", nome: "Aura", apelidos: &["AURA"], fonte: MOC_7 },
    LinhaBandeira { t_band: "09", nome: "Cabal", apelidos: &["CABAL", "DEBCABAL"], fonte: MOC_7 },
];

/// Finds the line of a brand by any of its names, ignoring case, spaces and punctuation.
#[must_use]
pub fn t_band_da_bandeira(nome: &str) -> Option<&'static LinhaBandeira> {
    let chave = normalizar(nome);
    BANDEIRAS.iter().find(|linha| linha.apelidos.contains(&chave.as_str()))
}

/// One line of the network table, with where it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinhaRede {
    /// The name of the network.
    pub rede: &'static str,
    /// The CNPJ, digits only.
    pub cnpj: &'static str,
    /// Other names providers use for it, already normalized.
    pub apelidos: &'static [&'static str],
    /// Where this was read.
    pub fonte: &'static str,
    /// Whether someone compared it with the Receita Federal. No line was.
    pub conferido_na_receita: bool,
}

/// CNPJs of networks. Only the check digits are verified by a test of this crate.
pub const REDES: &[LinhaRede] = &[
    LinhaRede {
        rede: "Stone",
        cnpj: "16501555000157",
        apelidos: &["STONE"],
        fonte: "[S] linha lida no arquivo RedesPayGo.txt do espelho do ACBr",
        conferido_na_receita: false,
    },
    LinhaRede {
        rede: "Cielo",
        cnpj: "01027058000191",
        apelidos: &["CIELO", "VISANET"],
        fonte: "[S] linha lida no arquivo RedesPayGo.txt do espelho do ACBr, como VISANET",
        conferido_na_receita: false,
    },
    LinhaRede {
        rede: "Getnet",
        cnpj: "10440482000154",
        apelidos: &["GETNET"],
        fonte: "[S] linha lida no arquivo RedesPayGo.txt do espelho do ACBr",
        conferido_na_receita: false,
    },
    LinhaRede {
        rede: "Mercado Pago",
        cnpj: "10573521000191",
        apelidos: &["MERCADOPAGO"],
        fonte: "[S] consulta em agregador de terceiros, cnpj-api.com",
        conferido_na_receita: false,
    },
];

/// Finds the line of a network by any of its names, ignoring case, spaces and punctuation.
#[must_use]
pub fn cnpj_da_rede(nome: &str) -> Option<&'static LinhaRede> {
    let chave = normalizar(nome);
    REDES.iter().find(|linha| linha.apelidos.contains(&chave.as_str()))
}

fn normalizar(texto: &str) -> String {
    texto.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_uppercase()).collect()
}

fn somente_cnpj(texto: &str) -> String {
    normalizar(texto)
}

/// Checks the format of the current schema (`[0-9A-Z]{12}[0-9]{2}`) and the check digits.
///
/// Letters count as their ASCII code minus 48, which is how the alphanumeric CNPJ works.
/// All zeros is refused, as rule YA05-20 refuses it.
#[must_use]
pub fn cnpj_valido(cnpj: &str) -> bool {
    let bytes = cnpj.as_bytes();
    if bytes.len() != 14
        || !bytes[..12].iter().all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
        || !bytes[12..].iter().all(u8::is_ascii_digit)
        || bytes.iter().all(|b| *b == b'0')
    {
        return false;
    }
    let digito = |parte: &[u8]| -> u8 {
        let mut soma = 0u32;
        let mut peso = 2u32;
        for byte in parte.iter().rev() {
            soma += u32::from(*byte - b'0') * peso;
            peso = if peso == 9 { 2 } else { peso + 1 };
        }
        let resto = soma % 11;
        // The value is below 10 in this branch, so the cast loses nothing.
        if resto < 2 { 0 } else { (11 - resto) as u8 }
    };
    digito(&bytes[..12]) == bytes[12] - b'0' && digito(&bytes[..13]) == bytes[13] - b'0'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_cnpj_da_tabela_tem_digito_valido() {
        for linha in REDES {
            assert!(cnpj_valido(linha.cnpj), "{}", linha.rede);
            assert!(!linha.conferido_na_receita, "{}", linha.rede);
            assert!(!linha.fonte.is_empty());
        }
    }

    #[test]
    fn cnpj_recusa_zeros_e_digito_errado_e_aceita_alfanumerico_do_esquema() {
        assert!(!cnpj_valido("00000000000000"));
        assert!(!cnpj_valido("16501555000158"));
        assert!(!cnpj_valido("1650155500015"));
        // The example used by the Receita Federal for the alphanumeric CNPJ.
        assert!(cnpj_valido("12ABC34501DE35"));
    }

    #[test]
    fn bandeira_ignora_caixa_e_pontuacao() {
        assert_eq!(t_band_da_bandeira("MasterCard").map(|l| l.t_band), Some("02"));
        assert_eq!(t_band_da_bandeira("master").map(|l| l.t_band), Some("02"));
        assert_eq!(t_band_da_bandeira("American Express").map(|l| l.t_band), Some("03"));
        assert!(t_band_da_bandeira("Bandeira Nova").is_none());
    }

    #[test]
    fn bandeira_desconhecida_nao_vira_99_sozinha() {
        let mut dados = DadosFiscaisPagamento::integrado();
        dados.com_bandeira(Some("Nova"));
        assert_eq!(dados.t_band, None);
        assert_eq!(dados.alertas, vec![AlertaFiscal::BandeiraDesconhecida { recebida: Some("Nova".into()) }]);
    }

    #[test]
    fn c_aut_ausente_vira_alerta_e_nao_valor() {
        let mut dados = DadosFiscaisPagamento::integrado();
        dados.com_c_aut(None, "a API não devolve");
        assert_eq!(dados.c_aut, None);
        assert!(matches!(dados.alertas[0], AlertaFiscal::CAutAusente { .. }));
        assert!(!dados.atende_ya05_10());
    }

    #[test]
    fn c_aut_acima_do_esquema_e_descartado() {
        let mut dados = DadosFiscaisPagamento::integrado();
        dados.com_c_aut(Some(&"9".repeat(129)), "fora do esquema");
        assert_eq!(dados.c_aut, None);
        assert_eq!(dados.alertas.len(), 2);
    }

    #[test]
    fn rede_da_tabela_sempre_avisa_que_nao_foi_conferida() {
        let mut dados = DadosFiscaisPagamento::integrado();
        dados.com_cnpj_da_rede(Some("STONE"));
        assert_eq!(dados.cnpj.as_deref(), Some("16501555000157"));
        assert!(matches!(dados.alertas[0], AlertaFiscal::CnpjDeTabelaNaoConferida { .. }));
        let mut outra = DadosFiscaisPagamento::integrado();
        outra.com_cnpj_da_rede(Some("C6PAY"));
        assert_eq!(outra.cnpj, None);
        assert_eq!(outra.alertas, vec![AlertaFiscal::CnpjAusente { rede: Some("C6PAY".into()) }]);
    }

    #[test]
    fn ya05_10_cobra_cnpj_e_c_aut() {
        let mut dados = DadosFiscaisPagamento::integrado();
        dados.com_cnpj_do_provedor("16.501.555/0001-57");
        dados.com_c_aut(Some("123"), "");
        assert!(dados.atende_ya05_10());
        assert_eq!(dados.cnpj.as_deref(), Some("16501555000157"));
    }

    #[test]
    fn terminal_longo_e_descartado() {
        let mut dados = DadosFiscaisPagamento::integrado();
        dados.com_terminal(Some(&"T".repeat(41)));
        assert_eq!(dados.id_term_pag, None);
        assert_eq!(dados.alertas.len(), 1);
    }
}
