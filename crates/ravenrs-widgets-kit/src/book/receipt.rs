//! The stories of `Receipt`.

use ravenrs_widgets_base::AnyView;
use ravenrs_widgets_base::book::{ArgType, Args, ComponentBook, Meta};

use crate::{Receipt, ReceiptLine};

fn receipt(args: &Args) -> AnyView {
    let mut receipt = Receipt::new().title(args.text("title")).subtitle(args.text("subtitle"));
    let lines = args.number("lines") as usize;
    let sold = [
        ("FOTO 10X15 BRILHO", "30,00", "2 x 15,00"),
        ("ALBUM 20X30 CAPA DURA", "189,90", "1 x 189,90"),
        ("ENSAIO GESTANTE 1H", "350,00", "1 x 350,00"),
    ];
    for (description, amount, detail) in sold.into_iter().take(lines) {
        receipt = receipt.line(ReceiptLine::new(description, amount).detail(detail));
    }
    let subtotal = ["0,00", "30,00", "219,90", "569,90"][lines.min(3)];
    let items = match lines {
        1 => "1 ITEM".to_owned(),
        n => format!("{n} ITENS"),
    };
    receipt = receipt.summary(format!("SUBTOTAL ({items})"), subtotal);
    let total = if args.bool("discount") && lines > 0 {
        receipt = receipt.summary("DESCONTO", "-20,00");
        ["0,00", "10,00", "199,90", "549,90"][lines.min(3)]
    } else {
        subtotal
    };
    receipt = receipt.summary("FOTOGRAFOU · ATENDEU · AUXILIOU", "F3 escolhe");
    Box::new(receipt.total("TOTAL A RECEBER", format!("R$ {total}")))
}

impl ComponentBook for Receipt {
    fn book() -> Meta {
        Meta::new("Data/Receipt", receipt)
            .summary("The receipt of a sale on the screen: its lines, the subtotal and the total in a band.")
            .arg(ArgType::text("title", "Cupom"))
            .arg(ArgType::text("subtitle", "Caixa 1 · venda 128"))
            .arg(ArgType::number("lines", 3.0, 0.0, 3.0, 1.0).description("How many lines the sale has."))
            .arg(ArgType::bool("discount", false).description("A discount row under the subtotal."))
            .story("With lines", Args::new())
            .story("Empty", Args::new().with("lines", 0.0))
            .story("With a discount", Args::new().with("discount", true))
    }
}
