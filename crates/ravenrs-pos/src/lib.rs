//! The rules of a point of sale, with nothing about screens or fiscal documents: a cart, what a
//! scanner reads, totals in exact decimals, payments and change, what arrives from suppliers and what
//! a price has to cover.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub mod entrada;
pub mod preco;

pub use entrada::{
    EstoqueAtualizado, LinhaCompra, PREFIXO_INTERNO, SugestaoEtiqueta, codigo_interno, conferir_codigo,
    e_codigo_interno, entrar_no_estoque, gtin_fiscal, sugerir_etiqueta,
};
pub use preco::{Composicao, Formacao, arredondar_preco};

/// What was read at the counter: a code, and how many of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leitura {
    /// How many.
    pub quantidade: Decimal,
    /// The code.
    pub codigo: String,
}

/// Reads `code`, `3*code` or `1,5*code`. Says why when it cannot.
pub fn ler_codigo(texto: &str) -> Result<Leitura, String> {
    let texto = texto.trim();
    let (quantidade, codigo) = match texto.split_once('*') {
        Some((quantidade, codigo)) => {
            let quantidade: Decimal =
                quantidade.trim().replace(',', ".").parse().map_err(|_| {
                    format!("Quantidade \u{201c}{}\u{201d} não é um número.", quantidade.trim())
                })?;
            (quantidade, codigo.trim())
        }
        None => (Decimal::ONE, texto),
    };
    if codigo.is_empty() {
        return Err("Falta o código do produto.".to_owned());
    }
    if quantidade <= Decimal::ZERO {
        return Err("A quantidade precisa ser maior que zero.".to_owned());
    }
    Ok(Leitura { quantidade, codigo: codigo.to_owned() })
}

/// One line of a sale, with what a fiscal document asks of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Linha {
    /// The product's own code.
    pub codigo: String,
    /// Its barcode, if it has one.
    pub gtin: Option<String>,
    /// What it is.
    pub descricao: String,
    /// The unit it is sold by.
    pub unidade: String,
    /// The price of each.
    pub preco: Decimal,
    /// How many.
    pub quantidade: Decimal,
    /// NCM.
    pub ncm: String,
    /// CFOP.
    pub cfop: String,
    /// CEST, for goods under tax substitution.
    pub cest: Option<String>,
    /// CSOSN.
    pub csosn: String,
}

impl Linha {
    /// What this line comes to.
    pub fn total(&self) -> Decimal {
        (self.preco * self.quantidade)
            .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
    }
}

/// A sale being made.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Carrinho {
    /// Its lines, in the order they came.
    pub linhas: Vec<Linha>,
    /// The CPF or CNPJ of who is buying, when they asked for it on the note.
    pub cliente: Option<String>,
    /// What goes along on the document.
    pub observacao: Option<String>,
    /// What comes off the whole sale, in money; [`Carrinho::desconto_por_linha`] splits it among
    /// the lines, as the fiscal document carries it. A sale kept before it existed opens with none.
    #[serde(default)]
    pub desconto: Decimal,
}

impl Carrinho {
    /// Adds a line, or says why not: a price below zero, or a quantity that is not above it.
    pub fn adicionar(&mut self, linha: Linha) -> Result<(), String> {
        if linha.preco < Decimal::ZERO {
            return Err(format!("O preço de {} não pode ser negativo.", linha.descricao));
        }
        if linha.quantidade <= Decimal::ZERO {
            return Err("A quantidade precisa ser maior que zero.".to_owned());
        }
        self.linhas.push(linha);
        Ok(())
    }

    /// Takes out the last line, and gives it. A discount above what is left goes away.
    pub fn cancelar_ultimo(&mut self) -> Option<Linha> {
        let linha = self.linhas.pop();
        self.conferir_desconto();
        linha
    }

    /// Takes out the line in this position, from 1 as the receipt counts. A discount above what
    /// is left goes away.
    pub fn cancelar_item(&mut self, numero: usize) -> Option<Linha> {
        let at = numero.checked_sub(1)?;
        let linha = (at < self.linhas.len()).then(|| self.linhas.remove(at));
        self.conferir_desconto();
        linha
    }

    /// A discount bigger than the lines goes away, so the total is never below zero.
    fn conferir_desconto(&mut self) {
        if self.desconto > self.subtotal() {
            self.desconto = Decimal::ZERO;
        }
    }

    /// What the lines come to, before the discount.
    pub fn subtotal(&self) -> Decimal {
        self.linhas.iter().map(Linha::total).sum()
    }

    /// What it all comes to: the lines less the discount.
    pub fn total(&self) -> Decimal {
        self.subtotal() - self.desconto
    }

    /// Takes `valor` off the whole sale, in place of any discount it had, or says why not: below
    /// zero, with cents past the second place, or above what the lines come to. Zero takes the
    /// discount away.
    pub fn dar_desconto(&mut self, valor: Decimal) -> Result<(), String> {
        if valor < Decimal::ZERO {
            return Err("O desconto não pode ser negativo.".to_owned());
        }
        if valor.round_dp(2) != valor {
            return Err("O desconto vai até os centavos.".to_owned());
        }
        if valor > self.subtotal() {
            return Err(format!(
                "O desconto de R$ {} passa do subtotal, R$ {}.",
                valor_em_texto(valor),
                valor_em_texto(self.subtotal())
            ));
        }
        self.desconto = valor;
        Ok(())
    }

    /// The discount split among the lines by what each comes to, to the cent, in the order of the
    /// lines: the cents left by rounding go to the line that comes to most (the first of them),
    /// so the parts add up to the discount exactly. No line gets more than it comes to.
    pub fn desconto_por_linha(&self) -> Vec<Decimal> {
        ratear(self.desconto, &self.linhas.iter().map(Linha::total).collect::<Vec<_>>())
    }

    /// The last line.
    pub fn ultimo(&self) -> Option<&Linha> {
        self.linhas.last()
    }

    /// Whether it has nothing.
    pub fn vazio(&self) -> bool {
        self.linhas.is_empty()
    }

    /// As text, to keep it and pick it up later.
    pub fn para_texto(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// From what [`para_texto`](Self::para_texto) gave.
    pub fn de_texto(texto: &str) -> Result<Self, String> {
        serde_json::from_str(texto).map_err(|error| error.to_string())
    }
}

/// Splits `valor` among `partes` by their weight, to the cent: each gets its share rounded down,
/// and the cents left go to the largest part (the first of them). The result adds up to `valor`
/// whenever `valor` is no more than their sum; with no weight at all, nothing is split.
pub fn ratear(valor: Decimal, partes: &[Decimal]) -> Vec<Decimal> {
    let soma: Decimal = partes.iter().sum();
    if soma <= Decimal::ZERO || valor <= Decimal::ZERO {
        return vec![Decimal::ZERO; partes.len()];
    }
    let mut cotas: Vec<Decimal> = partes
        .iter()
        .map(|parte| (valor * parte / soma).round_dp_with_strategy(2, rust_decimal::RoundingStrategy::ToZero))
        .collect();
    let sobra = valor - cotas.iter().sum::<Decimal>();
    if sobra != Decimal::ZERO
        && let Some(maior) = partes
            .iter()
            .enumerate()
            .fold(None::<(usize, Decimal)>, |best, (i, p)| match best {
                Some((_, b)) if *p <= b => best,
                _ => Some((i, *p)),
            })
            .map(|(i, _)| i)
    {
        cotas[maior] += sobra;
    }
    cotas
}

/// How someone pays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Forma {
    /// Cash.
    #[default]
    Dinheiro,
    /// Debit card.
    Debito,
    /// Credit card.
    Credito,
    /// Pix.
    Pix,
}

impl Forma {
    /// All of them, in the order of their keys.
    pub const TODAS: [Self; 4] = [Self::Dinheiro, Self::Debito, Self::Credito, Self::Pix];

    /// What people call it.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Dinheiro => "Dinheiro",
            Self::Debito => "Cartão de débito",
            Self::Credito => "Cartão de crédito",
            Self::Pix => "Pix",
        }
    }

    /// Whether someone can hand over more than the total and get change.
    pub fn tem_troco(self) -> bool {
        self == Self::Dinheiro
    }
}

/// Reads an amount as people type it: `1.234,56`, `1234,56` or `1234.56`.
pub fn ler_valor(texto: &str) -> Option<Decimal> {
    let texto = texto.trim().trim_start_matches("R$").trim();
    let limpo = if texto.contains(',') { texto.replace('.', "").replace(',', ".") } else { texto.to_owned() };
    limpo.parse().ok()
}

/// An amount as people read it: `1.234,56`.
pub fn valor_em_texto(valor: Decimal) -> String {
    let texto = format!("{:.2}", valor.round_dp(2));
    let (inteiro, centavos) = texto.split_once('.').unwrap_or((&texto, "00"));
    let (sinal, digitos) = inteiro.strip_prefix('-').map_or(("", inteiro), |resto| ("-", resto));
    let mut agrupado = String::new();
    for (posicao, digito) in digitos.chars().enumerate() {
        if posicao > 0 && (digitos.len() - posicao) % 3 == 0 {
            agrupado.push('.');
        }
        agrupado.push(digito);
    }
    format!("{sinal}{agrupado},{centavos}")
}

/// What comes of paying `recebido` for `total` in this way, or why it is not enough.
pub fn troco(forma: Forma, total: Decimal, recebido: Decimal) -> Result<Decimal, String> {
    if recebido < total {
        return Err(format!("Faltam R$ {}.", valor_em_texto(total - recebido)));
    }
    if recebido > total && !forma.tem_troco() {
        return Err(format!("{} não tem troco: o valor é R$ {}.", forma.nome(), valor_em_texto(total)));
    }
    Ok(recebido - total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linha(preco: &str, quantidade: &str) -> Linha {
        Linha {
            codigo: "1".into(),
            gtin: None,
            descricao: "Leite".into(),
            unidade: "UN".into(),
            preco: preco.parse().unwrap_or_default(),
            quantidade: quantidade.parse().unwrap_or_default(),
            ncm: "04012010".into(),
            cfop: "5102".into(),
            cest: None,
            csosn: "102".into(),
        }
    }

    #[test]
    fn a_code_can_come_with_how_many() {
        assert_eq!(ler_codigo("789").map(|l| l.quantidade), Ok(Decimal::ONE));
        let leitura = ler_codigo(" 3 * 789 ").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((leitura.quantidade, leitura.codigo.as_str()), (Decimal::from(3), "789"));
        assert_eq!(ler_codigo("1,5*789").map(|l| l.quantidade.to_string()), Ok("1.5".to_owned()));
        assert!(ler_codigo("x*789").is_err());
        assert!(ler_codigo("0*789").is_err());
        assert!(ler_codigo("3*").is_err());
        assert!(ler_codigo("  ").is_err());
    }

    #[test]
    fn totals_do_not_lose_a_cent() {
        let mut carrinho = Carrinho::default();
        assert!(carrinho.adicionar(linha("0.10", "3")).is_ok());
        assert!(carrinho.adicionar(linha("5.49", "3")).is_ok());
        assert_eq!(carrinho.total().to_string(), "16.77");
        assert_eq!(carrinho.ultimo().map(Linha::total).map(|t| t.to_string()), Some("16.47".to_owned()));
    }

    #[test]
    fn lines_come_out_by_position_or_the_last() {
        let mut carrinho = Carrinho::default();
        assert!(carrinho.adicionar(linha("1", "1")).is_ok());
        assert!(carrinho.adicionar(linha("2", "1")).is_ok());
        assert!(carrinho.adicionar(linha("3", "1")).is_ok());
        assert_eq!(carrinho.cancelar_item(0), None);
        assert_eq!(carrinho.cancelar_item(9), None);
        assert_eq!(carrinho.cancelar_item(1).map(|l| l.preco), Some(Decimal::ONE));
        assert_eq!(carrinho.cancelar_ultimo().map(|l| l.preco), Some(Decimal::from(3)));
        assert_eq!(carrinho.linhas.len(), 1);
    }

    #[test]
    fn a_sale_comes_back_as_it_was_kept() {
        let mut carrinho = Carrinho::default();
        assert!(carrinho.adicionar(linha("5.49", "1.500")).is_ok());
        carrinho.cliente = Some("12345678909".into());
        assert_eq!(Carrinho::de_texto(&carrinho.para_texto()), Ok(carrinho));
        assert!(Carrinho::de_texto("nada").is_err());
    }

    #[test]
    fn amounts_read_and_write_as_people_do() {
        assert_eq!(ler_valor("1.234,56"), "1234.56".parse().ok());
        assert_eq!(ler_valor("R$ 100"), Some(Decimal::from(100)));
        assert_eq!(ler_valor("1234.56"), "1234.56".parse().ok());
        assert_eq!(ler_valor("x"), None);
        assert_eq!(valor_em_texto("1234567.5".parse().unwrap_or_default()), "1.234.567,50");
        assert_eq!(valor_em_texto(Decimal::ZERO), "0,00");
        assert_eq!(valor_em_texto("-12.3".parse().unwrap_or_default()), "-12,30");
    }

    #[test]
    fn only_cash_has_change_and_nothing_pays_less() {
        let total: Decimal = "97.32".parse().unwrap_or_default();
        assert_eq!(
            troco(Forma::Dinheiro, total, Decimal::from(100)).map(|t| t.to_string()),
            Ok("2.68".to_owned())
        );
        assert!(troco(Forma::Dinheiro, total, Decimal::from(50)).is_err());
        assert!(troco(Forma::Pix, total, Decimal::from(100)).is_err());
        assert_eq!(troco(Forma::Pix, total, total), Ok(Decimal::ZERO));
    }

    #[test]
    fn a_line_below_zero_is_refused_and_the_cart_stays_as_it_was() {
        let mut carrinho = Carrinho::default();
        assert!(carrinho.adicionar(linha("2", "1")).is_ok());
        assert!(carrinho.adicionar(linha("-5", "1")).is_err(), "a negative price");
        assert!(carrinho.adicionar(linha("5", "0")).is_err(), "no quantity");
        assert_eq!(carrinho.linhas.len(), 1);
        assert_eq!(carrinho.total(), Decimal::from(2));
    }

    #[test]
    fn a_discount_comes_off_the_total_of_three_lines() {
        let mut carrinho = Carrinho::default();
        for (preco, quantidade) in [("15", "2"), ("189.90", "1"), ("12.40", "1.5")] {
            assert!(carrinho.adicionar(linha(preco, quantidade)).is_ok());
        }
        assert_eq!(carrinho.subtotal().to_string(), "238.50");
        assert_eq!(carrinho.dar_desconto(Decimal::from(20)), Ok(()));
        assert_eq!(carrinho.total().to_string(), "218.50");
        let partes = carrinho.desconto_por_linha();
        assert_eq!(partes.iter().sum::<Decimal>(), Decimal::from(20), "{partes:?}");
    }

    #[test]
    fn a_discount_split_among_equal_lines_loses_no_cent() {
        let mut carrinho = Carrinho::default();
        for _ in 0..3 {
            assert!(carrinho.adicionar(linha("3.33", "1")).is_ok());
        }
        assert_eq!(carrinho.dar_desconto(Decimal::from(9)), Ok(()));
        let partes = carrinho.desconto_por_linha();
        assert_eq!(partes.iter().sum::<Decimal>(), Decimal::from(9));
        assert_eq!(partes, [Decimal::from(3); 3], "three equal shares");
        assert_eq!(carrinho.dar_desconto("1".parse().unwrap_or_default()), Ok(()));
        let partes = carrinho.desconto_por_linha();
        assert_eq!(partes.iter().sum::<Decimal>(), Decimal::ONE, "{partes:?}");
        assert!(partes.iter().zip(&carrinho.linhas).all(|(p, l)| *p <= l.total()));
    }

    #[test]
    fn a_discount_below_zero_above_the_subtotal_or_past_the_cents_is_refused() {
        let mut carrinho = Carrinho::default();
        assert!(carrinho.adicionar(linha("10", "1")).is_ok());
        assert!(carrinho.dar_desconto(Decimal::from(-1)).is_err());
        assert!(carrinho.dar_desconto(Decimal::from(11)).is_err());
        assert!(carrinho.dar_desconto("0.001".parse().unwrap_or_default()).is_err());
        assert_eq!(carrinho.desconto, Decimal::ZERO, "it stays as it was");
        assert_eq!(carrinho.dar_desconto(Decimal::from(10)), Ok(()), "the whole subtotal is allowed");
        assert_eq!(carrinho.total(), Decimal::ZERO);
    }

    #[test]
    fn a_sale_kept_before_the_discount_opens_with_none() {
        let antiga = r#"{"linhas":[],"cliente":null,"observacao":null}"#;
        assert_eq!(Carrinho::de_texto(antiga).map(|c| c.desconto), Ok(Decimal::ZERO));
    }
}
