//! What arrives from a supplier: the units of the invoice become units on the shelf, and the codes of
//! a store's own products. It knows nothing about where the invoice came from.

use ravenrs_barcode::{gtin_check_digit, is_valid_gtin};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// The first digits of the codes a store makes for itself: the range GS1 keeps for use inside a
/// company, away from the 20 that scales usually print on weighed goods.
pub const PREFIXO_INTERNO: &str = "29";

/// The EAN-13 of a store's own product, from its running number. It has no meaning outside the store.
pub fn codigo_interno(sequencia: u64) -> Result<String, String> {
    let corpo = format!("{PREFIXO_INTERNO}{sequencia:010}");
    if corpo.len() != 12 {
        return Err("A numeração de códigos internos acabou.".to_owned());
    }
    let digito = gtin_check_digit(&corpo).ok_or("Código interno inválido.")?;
    Ok(format!("{corpo}{digito}"))
}

/// Whether a code is one a store made for itself. It must not go out as a GTIN on a fiscal document.
pub fn e_codigo_interno(codigo: &str) -> bool {
    is_valid_gtin(codigo) && codigo.len() == 13 && codigo.starts_with('2')
}

/// A code that can go as the GTIN of a fiscal document.
pub fn gtin_fiscal(codigo: &str) -> Option<&str> {
    (is_valid_gtin(codigo) && !e_codigo_interno(codigo)).then_some(codigo)
}

/// A barcode as it will be kept, or why it cannot be one. It takes what Code 128 prints; digits with
/// the length of a GTIN have to add up, since a scanner reads them as one.
pub fn conferir_codigo(codigo: &str) -> Result<String, String> {
    let codigo = codigo.trim();
    if codigo.is_empty() {
        return Err("O código está vazio.".to_owned());
    }
    if codigo.len() > 48 {
        return Err("O código passa de 48 caracteres.".to_owned());
    }
    if !codigo.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
        return Err("O código só aceita letras, números e símbolos, sem espaço nem acento.".to_owned());
    }
    let parece_gtin =
        matches!(codigo.len(), 8 | 12 | 13 | 14) && codigo.bytes().all(|byte| byte.is_ascii_digit());
    if parece_gtin && !is_valid_gtin(codigo) {
        return Err(format!("O dígito verificador de {codigo} não confere."));
    }
    Ok(codigo.to_owned())
}

/// One line of an invoice, in the supplier's units, with what its cost has to carry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinhaCompra {
    /// How many of the supplier's units.
    pub quantidade: Decimal,
    /// What the whole line came to, before the rest.
    pub valor_total: Decimal,
    /// Discount on the line.
    pub desconto: Decimal,
    /// Freight, insurance and other costs on the line.
    pub acrescimos: Decimal,
    /// Tax on the line that adds to the cost: IPI and ICMS-ST.
    pub impostos_no_custo: Decimal,
    /// How many of the store's units come in one of the supplier's: 12 for a box of 12.
    pub fator: Decimal,
}

impl LinhaCompra {
    /// How many of the store's units arrive.
    pub fn unidades(&self) -> Decimal {
        self.quantidade * self.fator
    }

    /// What one of the store's units cost, to 4 places, or why it cannot be said.
    pub fn custo_unitario(&self) -> Result<Decimal, String> {
        if self.unidades() <= Decimal::ZERO {
            return Err("A quantidade e o fator de conversão precisam ser maiores que zero.".to_owned());
        }
        let total = self.valor_total - self.desconto + self.acrescimos + self.impostos_no_custo;
        Ok((total / self.unidades()).round_dp(4))
    }
}

/// What a product's stock and average cost become after arriving.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstoqueAtualizado {
    /// How many there are now.
    pub quantidade: Decimal,
    /// What each cost, on average.
    pub custo_medio: Decimal,
}

/// Adds what arrived to what there was. A stock below zero, from selling what was never entered, does
/// not weigh on the cost: only what is really there does.
pub fn entrar_no_estoque(
    estoque: Decimal,
    custo_medio: Decimal,
    chegou: Decimal,
    custo: Decimal,
) -> EstoqueAtualizado {
    let havia = estoque.max(Decimal::ZERO);
    let total = havia + chegou;
    let custo_medio = if total > Decimal::ZERO {
        ((havia * custo_medio + chegou * custo) / total).round_dp(4)
    } else {
        custo
    };
    EstoqueAtualizado { quantidade: estoque + chegou, custo_medio }
}

/// Whether a product that arrived again should get a new shelf label, and how many.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SugestaoEtiqueta {
    /// The price on the shelf changes, so the old label is wrong.
    pub preco_mudou: bool,
    /// How many labels are suggested.
    pub copias: u32,
}

/// What to suggest for a product arriving. A product bought for the first time gets its label too,
/// and one bought again gets one because it goes back on the shelf; a new price makes it urgent.
pub fn sugerir_etiqueta(preco_atual: Decimal, preco_novo: Decimal) -> SugestaoEtiqueta {
    SugestaoEtiqueta { preco_mudou: preco_atual != preco_novo, copias: 1 }
}

#[cfg(test)]
mod tests {
    use ravenrs_barcode::is_valid_gtin;

    use super::*;

    fn d(text: &str) -> Decimal {
        text.parse().unwrap_or_else(|_| panic!("{text}"))
    }

    #[test]
    fn the_codes_of_a_store_are_valid_and_stay_out_of_fiscal_documents() {
        let codigo = codigo_interno(1).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(codigo.len(), 13);
        assert!(is_valid_gtin(&codigo), "a scanner reads it: {codigo}");
        assert!(codigo.starts_with("29"));
        assert!(e_codigo_interno(&codigo));
        assert_eq!(gtin_fiscal(&codigo), None);
        assert_eq!(gtin_fiscal("7891000100103"), Some("7891000100103"));
        assert_eq!(gtin_fiscal("7891000100100"), None, "a wrong digit is no GTIN");
        assert_ne!(codigo_interno(2).ok(), Some(codigo));
        assert!(codigo_interno(100_000_000_000).is_err());
    }

    #[test]
    fn a_code_of_a_store_can_be_anything_a_scanner_reads_but_a_gtin_has_to_add_up() {
        assert_eq!(conferir_codigo(" ABC-123 ").as_deref(), Ok("ABC-123"));
        assert_eq!(conferir_codigo("7891000100103").as_deref(), Ok("7891000100103"));
        assert!(conferir_codigo("7891000100100").is_err(), "wrong digit");
        assert!(conferir_codigo("").is_err());
        assert!(conferir_codigo("com espaço").is_err());
        assert!(conferir_codigo(&"x".repeat(49)).is_err());
        assert_eq!(conferir_codigo("12345").as_deref(), Ok("12345"), "short digits are a code of the store");
    }

    #[test]
    fn a_box_becomes_units_with_everything_it_carries() {
        let linha = LinhaCompra {
            quantidade: d("2"),
            valor_total: d("120"),
            desconto: d("10"),
            acrescimos: d("6"),
            impostos_no_custo: d("4"),
            fator: d("12"),
        };
        assert_eq!(linha.unidades(), d("24"));
        assert_eq!(linha.custo_unitario(), Ok(d("5")));
        assert!(LinhaCompra { fator: d("0"), ..linha }.custo_unitario().is_err());
    }

    #[test]
    fn the_average_cost_weighs_what_was_there() {
        let depois = entrar_no_estoque(d("10"), d("4"), d("10"), d("6"));
        assert_eq!((depois.quantidade, depois.custo_medio), (d("20"), d("5")));
        let sem_nada = entrar_no_estoque(d("0"), d("0"), d("5"), d("3"));
        assert_eq!(sem_nada.custo_medio, d("3"));
    }

    #[test]
    fn a_stock_below_zero_does_not_weigh_on_the_cost() {
        let depois = entrar_no_estoque(d("-3"), d("100"), d("10"), d("6"));
        assert_eq!((depois.quantidade, depois.custo_medio), (d("7"), d("6")));
    }

    #[test]
    fn a_label_is_suggested_and_a_new_price_says_so() {
        assert!(sugerir_etiqueta(d("5"), d("6")).preco_mudou);
        let igual = sugerir_etiqueta(d("5"), d("5"));
        assert!(!igual.preco_mudou);
        assert_eq!(igual.copias, 1);
    }
}
