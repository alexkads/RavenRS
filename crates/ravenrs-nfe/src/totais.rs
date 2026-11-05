//! The totals of a document, always calculated and never typed.
//!
//! # Rule adopted
//!
//! - `vProd` of a line is `qCom * vUnCom`, rounded to 2 places with half away from zero.
//! - The totals `vProd` and `vDesc` are the sum of the lines, so they never round again.
//! - `vNF` is `vProd - vDesc`: freight, insurance, others, ST, IPI and II do not exist in this POC.
//! - `vTroco` is what was paid above `vNF`, and paying less than `vNF` is refused.
//! - A value with more places than the layout allows is refused, never rounded in silence.
//!
//! # Without a source
//!
//! The official MOC could not be read during the POC. The midpoint rule and the tolerance the
//! SEFAZ applies to rejections 629 and 610 stay unconfirmed: third-party pages disagree on the
//! tolerance. This crate needs none, because it writes exact values. See `REGRAS.md`.

use rust_decimal::{Decimal, RoundingStrategy};

use crate::error::NfeError;
use crate::modelo::{Item, Rascunho};

/// The calculated values of one line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TotaisItem {
    /// `vProd` of the line.
    pub v_prod: Decimal,
    /// `vDesc` of the line.
    pub v_desc: Decimal,
}

/// The calculated values of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Totais {
    /// The values of each line, in order.
    pub itens: Vec<TotaisItem>,
    /// Sum of `vProd`.
    pub v_prod: Decimal,
    /// Sum of `vDesc`.
    pub v_desc: Decimal,
    /// Total of the document.
    pub v_nf: Decimal,
    /// Sum of the payments.
    pub v_pago: Decimal,
    /// Change.
    pub v_troco: Decimal,
}

/// Rounds a value of money to 2 places, half away from zero.
#[must_use]
pub fn arredondar(valor: Decimal) -> Decimal {
    valor.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

fn exigir_casas(campo: &str, valor: Decimal, casas: u32) -> Result<(), NfeError> {
    if valor.normalize().scale() > casas {
        return Err(NfeError::validacao(campo, format!("no máximo {casas} casas decimais")));
    }
    if valor.is_sign_negative() && !valor.is_zero() {
        return Err(NfeError::validacao(campo, "não pode ser negativo"));
    }
    Ok(())
}

fn estouro(campo: &str) -> NfeError {
    NfeError::validacao(campo, "valor grande demais")
}

fn calcular_item(numero: usize, item: &Item) -> Result<TotaisItem, NfeError> {
    let campo = |nome: &str| format!("det[{numero}]/{nome}");
    exigir_casas(&campo("qCom"), item.quantidade, 4)?;
    exigir_casas(&campo("vUnCom"), item.valor_unitario, 10)?;
    exigir_casas(&campo("vDesc"), item.desconto, 2)?;
    if item.quantidade.is_zero() {
        return Err(NfeError::validacao(&campo("qCom"), "precisa ser maior que zero"));
    }
    let bruto = item.quantidade.checked_mul(item.valor_unitario).ok_or_else(|| estouro(&campo("vProd")))?;
    let v_prod = arredondar(bruto);
    if item.desconto > v_prod {
        return Err(NfeError::validacao(&campo("vDesc"), "maior que o valor do item"));
    }
    Ok(TotaisItem { v_prod, v_desc: item.desconto })
}

/// Calculates the totals of a draft.
pub fn calcular(rascunho: &Rascunho) -> Result<Totais, NfeError> {
    let mut itens = Vec::with_capacity(rascunho.itens.len());
    let mut v_prod = Decimal::ZERO;
    let mut v_desc = Decimal::ZERO;
    for (posicao, item) in rascunho.itens.iter().enumerate() {
        let totais = calcular_item(posicao + 1, item)?;
        v_prod = v_prod.checked_add(totais.v_prod).ok_or_else(|| estouro("vProd"))?;
        v_desc = v_desc.checked_add(totais.v_desc).ok_or_else(|| estouro("vDesc"))?;
        itens.push(totais);
    }
    let v_nf = v_prod - v_desc;
    let mut v_pago = Decimal::ZERO;
    for pagamento in &rascunho.pagamentos {
        exigir_casas("vPag", pagamento.valor, 2)?;
        v_pago = v_pago.checked_add(pagamento.valor).ok_or_else(|| estouro("vPag"))?;
    }
    if v_pago < v_nf {
        return Err(NfeError::validacao("vPag", "os pagamentos não cobrem o total da nota"));
    }
    Ok(Totais { itens, v_prod, v_desc, v_nf, v_pago, v_troco: v_pago - v_nf })
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::ambiente::Modelo;
    use crate::modelo::{Csosn, FormaPagamento};

    fn d(texto: &str) -> Decimal {
        Decimal::from_str(texto).expect("decimal")
    }

    fn item(quantidade: &str, valor: &str, desconto: &str) -> Item {
        Item {
            codigo: "1".into(),
            gtin: None,
            descricao: "X".into(),
            ncm: "10063021".into(),
            cest: None,
            cfop: "5102".into(),
            unidade: "UN".into(),
            quantidade: d(quantidade),
            valor_unitario: d(valor),
            desconto: d(desconto),
            csosn: Csosn::S102,
            origem: 0,
        }
    }

    fn rascunho(itens: Vec<Item>, pago: &str) -> Rascunho {
        let mut r = Rascunho::novo(Modelo::Nfce65, 1, 1, "VENDA", "2026-11-05T14:30:00-03:00");
        r.itens = itens;
        r.com_pagamento(FormaPagamento::Dinheiro, d(pago))
    }

    #[test]
    fn half_cent_rounds_away_from_zero() {
        assert_eq!(arredondar(d("1.005")), d("1.01"));
        assert_eq!(arredondar(d("1.0049")), d("1.00"));
    }

    #[test]
    fn line_rounds_once_and_totals_are_the_sum() {
        let totais = calcular(&rascunho(vec![item("0.333", "9.99", "0"), item("3", "1.333", "0.50")], "10"))
            .expect("totals");
        assert_eq!(totais.itens[0].v_prod, d("3.33"));
        assert_eq!(totais.itens[1].v_prod, d("4.00"));
        assert_eq!(totais.v_prod, d("7.33"));
        assert_eq!(totais.v_desc, d("0.50"));
        assert_eq!(totais.v_nf, d("6.83"));
        assert_eq!(totais.v_troco, d("3.17"));
    }

    #[test]
    fn too_many_places_are_refused_not_rounded() {
        assert!(calcular(&rascunho(vec![item("1.00001", "1", "0")], "10")).is_err());
        assert!(calcular(&rascunho(vec![item("1", "1", "0.001")], "10")).is_err());
    }

    #[test]
    fn trailing_zeros_do_not_count_as_places() {
        assert!(calcular(&rascunho(vec![item("1.00000", "1", "0")], "10")).is_ok());
    }

    #[test]
    fn short_payment_negative_and_zero_are_refused() {
        assert!(calcular(&rascunho(vec![item("1", "10", "0")], "9.99")).is_err());
        assert!(calcular(&rascunho(vec![item("-1", "10", "0")], "10")).is_err());
        assert!(calcular(&rascunho(vec![item("0", "10", "0")], "10")).is_err());
    }

    #[test]
    fn discount_above_the_line_is_refused() {
        assert!(calcular(&rascunho(vec![item("1", "1", "1.01")], "10")).is_err());
    }
}
