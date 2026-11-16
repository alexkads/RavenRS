//! `Text`, `Icon`, `Image` and `AspectRatio`, every feature: the type scale side by side, text that
//! follows a state, icons in sizes and colors, pictures in each way of filling a frame, and boxes
//! that keep their proportion. A line says what the buttons changed.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "TextIconsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Texto e ícones";

/// The pictures the crossfade goes through, by name.
pub const PICTURES: [&str; 3] = ["Aurora", "Oceano", "Pôr do sol"];

/// An 8 × 8 PNG, so that `Image::from_bytes` decodes real bytes with nothing to download.
const TILE_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00,
    0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08, 0x08, 0x06, 0x00, 0x00, 0x00, 0xc4, 0x0f, 0xbe, 0x8b, 0x00,
    0x00, 0x00, 0x8e, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x15, 0xca, 0xa1, 0x01, 0x02, 0x01, 0x08, 0x40,
    0x51, 0xc7, 0xb9, 0x35, 0x5c, 0x83, 0x0d, 0xc8, 0x16, 0xf2, 0x15, 0xca, 0x65, 0xb2, 0x85, 0x62, 0x26,
    0x5f, 0xa1, 0xb8, 0x81, 0xe3, 0xe0, 0xe7, 0xe5, 0xf7, 0x38, 0xe4, 0x3b, 0x4f, 0xb9, 0x47, 0xa4, 0xe6,
    0x25, 0x9f, 0xb9, 0xe4, 0x3d, 0x29, 0x31, 0xb7, 0x5c, 0xf3, 0x93, 0x73, 0x1e, 0x87, 0x12, 0x94, 0xa0,
    0x04, 0x25, 0x28, 0x41, 0x09, 0x4a, 0xd0, 0x0d, 0x46, 0x30, 0x82, 0x11, 0x8c, 0x60, 0x04, 0x23, 0x18,
    0xc1, 0x36, 0x38, 0xc1, 0x09, 0x4e, 0x70, 0x82, 0x13, 0x9c, 0xe0, 0x04, 0xdf, 0x10, 0x84, 0x20, 0x04,
    0x21, 0x08, 0x41, 0x08, 0x42, 0x10, 0x62, 0x43, 0x12, 0x92, 0x90, 0x84, 0x24, 0x24, 0x21, 0x09, 0x49,
    0xc8, 0x0d, 0x45, 0x28, 0x42, 0x11, 0x8a, 0x50, 0x84, 0x22, 0x14, 0xa1, 0x36, 0x34, 0xa1, 0x09, 0x4d,
    0x68, 0x42, 0x13, 0x9a, 0xd0, 0x84, 0x3e, 0xe7, 0x0f, 0xec, 0xf3, 0xac, 0x81, 0xe2, 0xf8, 0x5b, 0xe5,
    0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

fn note(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Caption).foreground(Semantic::InkMuted)
}

/// A picture made in Rust, its colors turned by `hue`, so the page needs no file.
fn picture(hue: f32) -> ImageData {
    ImageData::from_fn(160, 90, move |x, y| {
        let (u, v) = (x as f32 / 160.0, y as f32 / 90.0);
        // ravenrs-lint: allow(raw_color) the pixels of a picture, not a color of the interface
        Color::rgb(0.2 + 0.6 * u * hue, 0.3 + 0.4 * v, 0.9 - 0.5 * u)
    })
    .unwrap_or_else(|| unreachable!("the size is not zero"))
}

/// The page: the type scale, text from a state, icons, images and proportions.
#[derive(Debug, Default)]
pub struct TextIconsPage;

impl View for TextIconsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let likes = cx.use_state(|| 0_u32);
        let shown = cx.use_state(|| 0_usize);
        let event = cx.use_state(|| String::from("Nada mudou ainda"));

        // The type scale, each step beside the others.
        let scale = VStack::new((
            Text::new("Título de exibição").style(TextStyle::Display),
            Text::new("Título").style(TextStyle::Title),
            Text::new("Manchete").style(TextStyle::Headline),
            Text::new("Corpo do texto, o tamanho de leitura.").style(TextStyle::Body),
            Text::new("Chamada, um pouco menor").style(TextStyle::Callout),
            Text::new("Legenda, para notas").style(TextStyle::Caption),
        ))
        .spacing(6.0)
        .align(Align::Start);

        let overrides = VStack::new((
            Text::new("Tamanho 22, sem trocar de estilo").font_size(22.0),
            Text::new("Peso negrito").font_weight(Weight::BOLD),
            Text::new("Peso médio, em destaque").font_weight(Weight::MEDIUM).foreground(Semantic::Accent),
            Text::new("Cor de perigo").foreground(Semantic::Danger),
            Text::new("Cor apagada, para o que é secundário").foreground(Semantic::InkMuted),
        ))
        .spacing(6.0)
        .align(Align::Start);

        let width = if narrow { 260.0 } else { 360.0 };
        let alignment = VStack::new((
            Text::new("No início da linha").align(TextAlign::Start),
            Text::new("No meio da linha").align(TextAlign::Center),
            Text::new("No fim da linha").align(TextAlign::End),
            Text::new("Um texto longo demais para caber numa linha só termina em reticências").truncate(),
        ))
        .spacing(6.0)
        .align(Align::Stretch)
        .width(width);

        let (more, reset) = (likes.clone(), likes.clone());
        let (said, said_reset) = (event.clone(), event.clone());
        let live = VStack::new((
            Text::bound(&likes, |n| format!("Curtidas: {n}")).style(TextStyle::Headline),
            Text::dynamic({
                let likes = likes.clone();
                move || match likes.get() {
                    0 => String::from("Ninguém curtiu ainda"),
                    1 => String::from("Uma pessoa curtiu"),
                    n => format!("{n} pessoas curtiram"),
                }
            })
            .foreground(Semantic::InkMuted),
            HStack::new((
                Button::new("Curtir").icon(IconName::ThumbsUp).on_click(move || {
                    more.update(|n| *n += 1);
                    said.set(String::from("Curtida somada"));
                }),
                Button::new("Zerar").style(ButtonStyle::Secondary).on_click(move || {
                    reset.set(0);
                    said_reset.set(String::from("Curtidas zeradas"));
                }),
            ))
            .spacing(8.0),
        ))
        .spacing(6.0)
        .align(Align::Start);

        // Icons: sizes, then colors by role and by value.
        let sizes = HStack::new((
            Icon::new(IconName::Star),
            Icon::new(IconName::Star).size(20.0),
            Icon::new(IconName::Star).size(28.0),
            Icon::new(IconName::Star).size(40.0),
        ))
        .spacing(12.0)
        .align(Align::Center);
        let colors = HStack::new((
            Icon::new(IconName::Heart).size(24.0).color(Semantic::Danger),
            Icon::new(IconName::CircleCheck).size(24.0).color(Semantic::Success),
            Icon::new(IconName::TriangleAlert).size(24.0).color(Semantic::Warning),
            Icon::new(IconName::Info).size(24.0).color(Semantic::Accent),
            // ravenrs-lint: allow(raw_color) shows that a literal color is accepted too
            Icon::new(IconName::Palette).size(24.0).color(Color::hex(0x9b59b6)),
        ))
        .spacing(12.0)
        .align(Align::Center);

        // Images: decoded from bytes, made in Rust, and each way of filling the frame.
        let side = if narrow { 88.0 } else { 120.0 };
        let decoded: AnyView = match Image::from_bytes(TILE_PNG) {
            Ok(image) => Box::new(
                VStack::new((
                    image.content_mode(ContentMode::Fill).size(48.0, 48.0).corner_radius(8.0),
                    note("PNG lido dos bytes"),
                ))
                .spacing(4.0),
            ),
            Err(why) => Box::new(Text::new(format!("PNG ilegível: {why}"))),
        };
        let refused = match Image::from_bytes(b"isto nao e uma imagem") {
            Ok(_) => String::from("Bytes errados foram aceitos"),
            Err(_) => String::from("Bytes que não são imagem: recusados com erro"),
        };
        let mode = |mode: ContentMode, label: &str| {
            VStack::new((
                Image::new(picture(1.0))
                    .content_mode(mode)
                    .width(side)
                    .height(side)
                    .corner_radius(12.0)
                    .alt(format!("Gradiente em {label}")),
                note(label),
            ))
            .spacing(4.0)
            .align(Align::Center)
        };
        let modes = HStack::new((
            mode(ContentMode::Cover, "Cobrir"),
            mode(ContentMode::Fit, "Caber"),
            mode(ContentMode::Fill, "Esticar"),
        ))
        .spacing(12.0);
        let faded = HStack::new((
            Image::new(picture(0.4)).size(56.0, 56.0).corner_radius(28.0),
            Image::new(picture(0.4)).size(56.0, 56.0).corner_radius(28.0).opacity(0.6),
            Image::new(picture(0.4)).size(56.0, 56.0).corner_radius(28.0).opacity(0.3),
            decoded,
        ))
        .spacing(12.0)
        .align(Align::Center);

        let hues = [1.4_f32, 0.3, 1.0];
        let index = shown.get() % PICTURES.len();
        let (next, said_next) = (shown.clone(), event.clone());
        let banner = VStack::new((
            Image::new(picture(hues[index]))
                .stretch()
                .height(if narrow { 120.0 } else { 160.0 })
                .corner_radius(12.0)
                .crossfade(0.4)
                .alt(format!("Foto: {}", PICTURES[index])),
            HStack::new((
                Text::new(format!("Foto: {}", PICTURES[index])),
                Spacer::new(),
                Button::new("Trocar a foto").style(ButtonStyle::Secondary).on_click(move || {
                    next.update(|i| *i += 1);
                    said_next.set(String::from("Foto trocada com dissolução"));
                }),
            ))
            .align(Align::Center),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        // Boxes that keep their proportion whatever width they get.
        let ratio_box = |ratio: f32, label: &str| {
            AspectRatio::new(
                ratio,
                Card::new(Text::new(label.to_owned()).align(TextAlign::Center)).padding(8.0),
            )
        };
        let box_width = if narrow { 96.0 } else { 160.0 };
        let ratios = HStack::new((
            VStack::new((ratio_box(16.0 / 9.0, "16:9"),)).width(box_width),
            VStack::new((ratio_box(4.0 / 3.0, "4:3"),)).width(box_width),
            VStack::new((ratio_box(1.0, "1:1"),)).width(box_width * 0.6),
        ))
        .spacing(12.0)
        .align(Align::Start);

        cx.child(
            &VStack::new((
                VStack::new((heading("Escala de tipos"), scale)).spacing(8.0),
                VStack::new((heading("Tamanho, peso e cor"), overrides)).spacing(8.0),
                VStack::new((heading("Alinhamento e corte"), alignment)).spacing(8.0),
                VStack::new((heading("Texto que segue um estado"), live)).spacing(8.0),
                VStack::new((heading("Ícones"), sizes, colors)).spacing(8.0),
                VStack::new((
                    heading("Imagens"),
                    modes,
                    faded,
                    Text::new(refused).foreground(Semantic::InkMuted),
                ))
                .spacing(8.0),
                VStack::new((heading("Troca com dissolução"), banner)).spacing(8.0),
                VStack::new((heading("Proporção fixa"), ratios)).spacing(8.0),
                Text::bound(&event, |said| format!("Evento: {said}")),
            ))
            .spacing(20.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
