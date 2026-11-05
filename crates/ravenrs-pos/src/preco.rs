//! What a price has to cover: from a cost, the taxes and the fixed costs to the price and the markup,
//! and back from a price to what is left of it.

use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

const CEM: Decimal = Decimal::ONE_HUNDRED;
/// Half a cent goes up, as on a fiscal document.
const MEIO: RoundingStrategy = RoundingStrategy::MidpointAwayFromZero;

/// What goes into the price of a product. Percentages are of the selling price, as 12.5 for 12.5%.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Composicao {
    /// What a unit was bought for.
    pub custo: Decimal,
    /// Freight, for a unit.
    pub frete: Decimal,
    /// Other costs of buying, for a unit, such as tax paid at purchase that cannot be recovered.
    pub outros_custos: Decimal,
    /// Taxes on the sale, such as the rate of the Simples Nacional.
    pub impostos: Decimal,
    /// The share of fixed costs each sale carries.
    pub custos_fixos: Decimal,
    /// Commission and card fees.
    pub comissao: Decimal,
    /// The profit wanted.
    pub margem: Decimal,
}

/// What comes out of a [`Composicao`], or of a price given.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Formacao {
    /// What a unit costs, with freight and the rest.
    pub custo_total: Decimal,
    /// The price.
    pub preco: Decimal,
    /// Price divided by total cost: 2.0 sells for twice the cost.
    pub markup: Decimal,
    /// What goes to taxes.
    pub valor_impostos: Decimal,
    /// What goes to fixed costs.
    pub valor_custos_fixos: Decimal,
    /// What goes to commission and fees.
    pub valor_comissao: Decimal,
    /// What is left as profit.
    pub lucro: Decimal,
    /// The profit as a share of the price, in percent.
    pub margem: Decimal,
    /// The price at which there is no profit and no loss.
    pub ponto_de_equilibrio: Decimal,
}

impl Composicao {
    /// What is taken out of each price, before the profit, in percent.
    fn despesas(&self) -> Decimal {
        self.impostos + self.custos_fixos + self.comissao
    }

    fn custo_total(&self) -> Decimal {
        self.custo + self.frete + self.outros_custos
    }

    fn conferir(&self) -> Result<(), String> {
        let negativos = [self.custo, self.frete, self.outros_custos, self.impostos, self.custos_fixos, self.comissao];
        if negativos.iter().any(|value| *value < Decimal::ZERO) {
            return Err("Custos e percentuais não podem ser negativos.".to_owned());
        }
        Ok(())
    }

    /// The price that gives the margin wanted, with what it comes to.
    pub fn calcular(&self) -> Result<Formacao, String> {
        self.conferir()?;
        let soma = self.despesas() + self.margem;
        if soma >= CEM {
            return Err(format!(
                "Impostos, custos fixos, comissão e margem somam {}%. Precisam ficar abaixo de 100%.",
                soma.normalize()
            ));
        }
        if self.custo_total() <= Decimal::ZERO {
            return Err("Informe o custo.".to_owned());
        }
        let preco = (self.custo_total() / ((CEM - soma) / CEM)).round_dp_with_strategy(2, MEIO);
        Ok(self.para_o_preco(preco))
    }

    /// What is left at this price, whatever margin was wanted.
    pub fn para_o_preco(&self, preco: Decimal) -> Formacao {
        let custo_total = self.custo_total();
        let parte = |percentual: Decimal| (preco * percentual / CEM).round_dp_with_strategy(2, MEIO);
        let (valor_impostos, valor_custos_fixos, valor_comissao) =
            (parte(self.impostos), parte(self.custos_fixos), parte(self.comissao));
        let lucro = preco - custo_total - valor_impostos - valor_custos_fixos - valor_comissao;
        let despesas = self.despesas();
        Formacao {
            custo_total,
            preco,
            markup: if custo_total > Decimal::ZERO { (preco / custo_total).round_dp(4) } else { Decimal::ZERO },
            valor_impostos,
            valor_custos_fixos,
            valor_comissao,
            lucro,
            margem: if preco > Decimal::ZERO { (lucro / preco * CEM).round_dp_with_strategy(2, MEIO) } else { Decimal::ZERO },
            ponto_de_equilibrio: if despesas < CEM {
                (custo_total / ((CEM - despesas) / CEM)).round_dp_with_strategy(2, MEIO)
            } else {
                Decimal::ZERO
            },
        }
    }
}

/// A price rounded up to end in these cents, as a shelf price does: 5,37 with 90 gives 5,90.
pub fn arredondar_preco(preco: Decimal, centavos: u8) -> Decimal {
    let inteiro = preco.trunc();
    let final_desejado = Decimal::new(i64::from(centavos.min(99)), 2);
    let candidato = inteiro + final_desejado;
    if candidato >= preco { candidato } else { candidato + Decimal::ONE }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(text: &str) -> Decimal {
        text.parse().unwrap_or_else(|_| panic!("{text}"))
    }

    fn composicao() -> Composicao {
        Composicao {
            custo: d("10"),
            frete: d("0.50"),
            outros_custos: d("0"),
            impostos: d("6"),
            custos_fixos: d("10"),
            comissao: d("4"),
            margem: d("10"),
        }
    }

    #[test]
    fn the_price_covers_it_all_and_gives_the_margin() {
        // 10,50 / (1 - 0,30) = 15,00
        let f = composicao().calcular().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(f.preco, d("15"));
        assert_eq!(f.custo_total, d("10.50"));
        assert_eq!(f.markup, d("1.4286"));
        assert_eq!((f.valor_impostos, f.valor_custos_fixos, f.valor_comissao), (d("0.90"), d("1.50"), d("0.60")));
        assert_eq!(f.lucro, d("1.50"));
        assert_eq!(f.margem, d("10"));
        assert_eq!(f.ponto_de_equilibrio, d("13.13"));
    }

    #[test]
    fn a_price_given_says_what_is_left() {
        let f = composicao().para_o_preco(d("12"));
        assert_eq!(f.lucro, d("-0.90"), "below break-even it shows the loss");
        assert_eq!(f.margem, d("-7.5"));
        assert_eq!(composicao().para_o_preco(d("0")).margem, d("0"));
    }

    #[test]
    fn what_cannot_be_says_why() {
        let tudo = Composicao { margem: d("80"), ..composicao() };
        assert!(tudo.calcular().unwrap_err().contains("100%"));
        assert!(Composicao { custo: d("0"), frete: d("0"), ..composicao() }.calcular().is_err());
        assert!(Composicao { custo: d("-1"), ..composicao() }.calcular().is_err());
    }

    #[test]
    fn a_negative_margin_is_a_price_below_cost_on_purpose() {
        let f = Composicao { margem: d("-5"), ..composicao() }.calcular().unwrap_or_else(|e| panic!("{e}"));
        assert!((f.margem - d("-5")).abs() < d("0.1"), "{}", f.margem);
    }

    #[test]
    fn shelf_prices_round_up() {
        assert_eq!(arredondar_preco(d("5.37"), 90), d("5.90"));
        assert_eq!(arredondar_preco(d("5.95"), 90), d("6.90"));
        assert_eq!(arredondar_preco(d("5.90"), 90), d("5.90"));
        assert_eq!(arredondar_preco(d("5.01"), 99), d("5.99"));
    }
}
