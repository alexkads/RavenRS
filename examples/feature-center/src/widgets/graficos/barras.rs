//! `Barcode`, every feature: each format side by side, a QR Code of what is typed, the text
//! under the bars left out, other sizes, and what screen readers hear in place of the data.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "BarcodeDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Código de barras";

/// Each linear format with data it takes, and its name.
pub const FORMATS: [(&str, Symbology, &str); 7] = [
    ("Code 128", Symbology::Code128, "RAVEN-2026"),
    ("Code 39", Symbology::Code39, "PEDIDO 42"),
    ("EAN-13", Symbology::Ean13, "789123456789"),
    ("EAN-8", Symbology::Ean8, "9638507"),
    ("UPC-A", Symbology::UpcA, "03600029145"),
    ("ITF", Symbology::Itf, "12345678"),
    ("ITF-14", Symbology::Itf14, "1789123456789"),
];

/// The page.
#[derive(Debug, Default)]
pub struct BarcodePage;

impl View for BarcodePage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let link = cx.use_state(|| "https://ravenrs.dev".to_string());
        let text_under = cx.use_state(|| true);
        let big = cx.use_state(|| false);
        let narrow = cx.viewport().width < 720.0;
        let (width, height) = match (big.get(), narrow) {
            (true, false) => (300.0, 130.0),
            (_, true) => (200.0, 90.0),
            _ => (240.0, 110.0),
        };
        let codes: Vec<AnyView> = FORMATS
            .iter()
            .map(|(name, symbology, data)| {
                let code = Barcode::new(*symbology, *data).size(width, height);
                let code = if text_under.get() { code } else { code.hide_text() };
                Box::new(VStack::new((Text::new(*name).style(TextStyle::Caption), code)).spacing(4.0))
                    as AnyView
            })
            .collect();
        let side = if narrow { 140.0 } else { 180.0 };
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                HStack::new((
                    Toggle::new(text_under.binding()).label("Texto sob as barras"),
                    Toggle::new(big.binding()).label("Maiores"),
                ))
                .spacing(16.0)
                .wrap(),
                Grid::new(if narrow { 1 } else { 3 }, codes).spacing(16.0),
                Text::new("QR Code").style(TextStyle::Headline),
                FormField::new(
                    "Endereço do QR Code",
                    TextField::new(link.binding()).width(if narrow { 260.0 } else { 360.0 }),
                ),
                HStack::new((
                    Barcode::qr(link.get()).size(side, side),
                    // Screen readers hear this, not the address.
                    Barcode::qr("PIX: chave 00020126").alt("QR Code do Pix da loja").size(side, side),
                ))
                .spacing(24.0)
                .wrap(),
                Text::new(format!("O QR Code guarda: {}", link.get())),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Start),
        )
    }
}
