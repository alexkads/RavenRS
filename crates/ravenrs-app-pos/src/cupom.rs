//! The cart of a sale on the screen, as the printer will give it.

use ravenrs_pos::{Carrinho, valor_em_texto};
use ravenrs_widgets::{Receipt, ReceiptLine};
use rust_decimal::Decimal;

/// How many of something, as the receipt prints it: `2`, `1,5`.
fn quantidade_em_texto(quantidade: Decimal) -> String {
    quantidade.normalize().to_string().replace('.', ",")
}

/// The receipt of `carrinho` on the screen while the sale is made: each line with how many at
/// what price under it, the subtotal with the count of items, and the total to receive in the
/// band. With no lines it says where they will show.
pub fn cupom_na_tela(carrinho: &Carrinho) -> Receipt {
    let mut cupom = Receipt::new();
    for linha in &carrinho.linhas {
        let detalhe = format!(
            "{} {} x {}",
            quantidade_em_texto(linha.quantidade),
            linha.unidade,
            valor_em_texto(linha.preco)
        );
        cupom = cupom
            .line(ReceiptLine::new(linha.descricao.clone(), valor_em_texto(linha.total())).detail(detalhe));
    }
    let itens = match carrinho.linhas.len() {
        1 => "1 ITEM".to_owned(),
        n => format!("{n} ITENS"),
    };
    let total = valor_em_texto(carrinho.total());
    cupom
        .summary(format!("SUBTOTAL ({itens})"), total.clone())
        .total("TOTAL A RECEBER", format!("R$ {total}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravenrs_pos::Linha;
    use ravenrs_widgets::Ui;

    fn linha(descricao: &str, preco: &str, quantidade: &str) -> Linha {
        Linha {
            codigo: descricao.to_owned(),
            gtin: None,
            descricao: descricao.to_owned(),
            unidade: "UN".to_owned(),
            preco: preco.parse().unwrap_or_default(),
            quantidade: quantidade.parse().unwrap_or_default(),
            ncm: "49119900".to_owned(),
            cfop: "5102".to_owned(),
            cest: None,
            csosn: "102".to_owned(),
        }
    }

    /// The name, the value and the description of what the screen says, in order.
    fn said(carrinho: &Carrinho) -> Vec<(Option<String>, Option<String>, Option<String>)> {
        let mut ui = Ui::new(cupom_na_tela(carrinho), ravenrs_style::Theme::raven_dark());
        ui.update();
        ui.access_tree()
            .nodes
            .iter()
            .filter(|n| n.value.is_some())
            .map(|n| (n.name.clone(), n.value.clone(), n.description.clone()))
            .collect()
    }

    #[test]
    fn the_total_on_the_screen_is_the_total_of_the_cart() {
        let mut carrinho = Carrinho::default();
        assert!(carrinho.adicionar(linha("FOTO 10X15", "15", "2")).is_ok());
        assert!(carrinho.adicionar(linha("ALBUM 20X30", "189.90", "1")).is_ok());
        assert!(carrinho.adicionar(linha("PAPEL KG", "12.40", "1.5")).is_ok());
        assert!(carrinho.adicionar(linha("ESTORNO", "-10", "1")).is_err(), "the cart refuses it");
        let said = said(&carrinho);
        let some = |s: &str| Some(s.to_owned());
        assert_eq!(
            said,
            [
                (some("FOTO 10X15"), some("30,00"), some("2 UN x 15,00")),
                (some("ALBUM 20X30"), some("189,90"), some("1 UN x 189,90")),
                (some("PAPEL KG"), some("18,60"), some("1,5 UN x 12,40")),
                (some("TOTAL A RECEBER"), some("R$ 238,50"), None),
            ],
            "the three lines in order, no line for what was refused, and the total of the cart"
        );
        assert_eq!(valor_em_texto(carrinho.total()), "238,50");
    }

    #[test]
    fn an_empty_cart_shows_where_the_lines_go_and_a_zero_total() {
        let said = said(&Carrinho::default());
        assert_eq!(said, [(Some("TOTAL A RECEBER".to_owned()), Some("R$ 0,00".to_owned()), None)]);
    }
}
