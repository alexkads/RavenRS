//! Receipt: the receipt of a sale on the screen while it is made, from empty to the total, and one
//! with a discount row.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "ReceiptDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Cupom";

/// Every public feature of Receipt on one page.
#[derive(Debug, Default)]
pub struct ReceiptPage;

impl View for ReceiptPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let photos = cx.use_state(|| 0_u32);
        let (add, clear) = (photos.clone(), photos.clone());
        let count = photos.get();
        let mut sale = Receipt::new()
            .title("Cupom")
            .subtitle("Caixa 1 · venda 128")
            .empty_text("Passe o primeiro produto.");
        if count > 0 {
            sale = sale.line(
                ReceiptLine::new("FOTO 10X15 BRILHO", format!("{},00", 15 * count))
                    .detail(format!("{count} UN x 15,00")),
            );
        }
        let items = if count == 0 { "0 ITENS".to_owned() } else { "1 ITEM".to_owned() };
        let total = format!("{},00", 15 * count);
        sale = sale
            .summary(format!("SUBTOTAL ({items})"), total.clone())
            .total("TOTAL A RECEBER", format!("R$ {total}"));
        let discount = Receipt::new()
            .title("Com desconto")
            .line(ReceiptLine::new("ALBUM 20X30 CAPA DURA", "189,90").detail("1 UN x 189,90"))
            .line(ReceiptLine::new("ENSAIO GESTANTE 1H", "350,00").detail("1 UN x 350,00"))
            .summary("SUBTOTAL (2 ITENS)", "539,90")
            .summary("DESCONTO", "-20,00")
            .total("TOTAL A RECEBER", "R$ 519,90");
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("O cupom na tela enquanto a venda é montada, como ele vai sair na impressora.")
                    .foreground(Semantic::InkMuted),
                HStack::new((
                    Button::new("Passar foto 10x15").on_click(move || add.update(|n| *n += 1)),
                    Button::new("Nova venda").style(ButtonStyle::Secondary).on_click(move || clear.set(0)),
                ))
                .spacing(8.0),
                HStack::new((
                    VStack::new((sale,)).width(360.0).align(Align::Stretch),
                    VStack::new((discount,)).width(360.0).align(Align::Stretch),
                ))
                .spacing(24.0)
                .align(Align::Start),
            ))
            .spacing(16.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
