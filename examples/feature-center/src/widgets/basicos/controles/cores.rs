//! `ColorPicker`, `ColorPair`, `ColorSwatch`, `SwatchGrid` and `ThemeSwitcher`, every feature: the
//! colors of a label being designed, a palette where Ctrl or Cmd picks the background, the accent
//! of an account with a "no color" choice, and the theme of the whole center, with the app's own
//! themes as well. A line says what was picked.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "ColorsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Cores e temas";

/// The colors of the palette, with the names screen readers say.
pub const PALETTE: [(u32, &str); 8] = [
    (0xE11D48, "Rosa"),
    (0xF97316, "Laranja"),
    (0xEAB308, "Amarelo"),
    (0x16A34A, "Verde"),
    (0x0891B2, "Ciano"),
    (0x2563EB, "Azul"),
    (0x7C3AED, "Roxo"),
    (0x111827, "Grafite"),
];

/// The accents an account can have, in the order of the swatches.
pub const ACCENTS: [(u32, &str); 4] =
    [(0x2563EB, "Oceano"), (0x16A34A, "Floresta"), (0xE11D48, "Coral"), (0xF97316, "Âmbar")];

/// The application's own themes, each with a name of its own.
pub const NIGHT: Theme = Theme { name: "Noite da loja", ..Theme::raven_dark() };
/// The light theme of the application.
pub const DAY: Theme = Theme { name: "Dia da loja", ..Theme::raven_light() };
/// The glass theme of the application.
pub const GLASS: Theme = Theme { name: "Vitrine da loja", ..Theme::raven_glass() };

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

fn hex(color: Color) -> String {
    let [r, g, b, _] = color.to_array();
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", byte(r), byte(g), byte(b))
}

fn named(colors: &[(u32, &'static str)]) -> Vec<(Color, &'static str)> {
    colors.iter().map(|(code, name)| (Color::hex(*code), *name)).collect()
}

/// The page: pickers, a pair, swatches, the theme, and a line with the last pick.
#[derive(Debug, Default)]
pub struct ColorsPage;

impl View for ColorsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Escolha uma cor ou um tema.".to_string());
        let ink = cx.use_state(|| Some(Color::hex(0x111827)));
        let glow = cx.use_state(|| Some(Color::hex(0x2563EB).with_alpha(0.5)));
        let brand = cx.use_state(|| Some(Color::hex(0x7C3AED)));
        let front = cx.use_state(|| Color::hex(0x111827));
        let back = cx.use_state(|| Color::WHITE);
        let accent = cx.use_state(|| Some(0_usize));
        let width = if narrow { 160.0 } else { 220.0 };

        let pickers = HStack::new((
            ColorPicker::new(ink.binding()).label("Cor do texto").palette(named(&PALETTE)).width(width),
            ColorPicker::new(glow.binding()).label("Brilho, com transparência").alpha(true).width(width),
            ColorPicker::new(brand.binding()).label("Cor da marca (só leitura)").read_only(true),
            ColorPicker::new(brand.binding()).label("Cor do rodapé (desligada)").disabled(true),
        ))
        .spacing(12.0)
        .wrap();

        let (asked_front, asked_back) = (said.clone(), said.clone());
        let pair = HStack::new((
            ColorPair::new(front.binding(), back.binding())
                .defaults(Color::hex(0x111827), Color::WHITE)
                .on_foreground(move || asked_front.set("Escolher a cor da frente".to_string()))
                .on_background(move || asked_back.set("Escolher a cor do fundo".to_string())),
            ColorPair::new(front.binding(), back.binding()).size(36.0),
        ))
        .spacing(16.0)
        .align(Align::Center);
        let pair_line = format!("Frente {} sobre fundo {}", hex(front.get()), hex(back.get()));

        let (to_front, to_back, picked) = (front.clone(), back.clone(), said.clone());
        let grid = SwatchGrid::new(named(&PALETTE))
            .label("Paleta da etiqueta")
            .columns(if narrow { 4 } else { 8 })
            .selected(Some(front.get()))
            .on_pick(move |color, held| {
                if held.ctrl || held.meta {
                    to_back.set(color);
                    picked.set(format!("Fundo: {}", hex(color)));
                } else {
                    to_front.set(color);
                    picked.set(format!("Frente: {}", hex(color)));
                }
            });

        let mut swatches: Vec<ColorSwatch> = ACCENTS
            .iter()
            .enumerate()
            .map(|(index, (code, name))| {
                let (chosen, line) = (accent.clone(), said.clone());
                ColorSwatch::new(Color::hex(*code), *name).selected(accent.get() == Some(index)).on_click(
                    move || {
                        chosen.set(Some(index));
                        line.set(format!("Destaque: {name}"));
                    },
                )
            })
            .collect();
        let (cleared, clearing) = (accent.clone(), said.clone());
        swatches.push(ColorSwatch::none("Sem destaque").selected(accent.get().is_none()).on_click(
            move || {
                cleared.set(None);
                clearing.set("Destaque: nenhum".to_string());
            },
        ));
        let accents = HStack::new(swatches).spacing(4.0);

        let (told, own_told) = (said.clone(), said.clone());
        let themes = VStack::new((
            ThemeSwitcher::new().on_change(move |theme| told.set(format!("Tema: {}", theme.name))),
            ThemeSwitcher::new()
                .themes(NIGHT, DAY, GLASS)
                .on_change(move |theme| own_told.set(format!("Tema da loja: {}", theme.name))),
        ))
        .spacing(8.0)
        .align(Align::Start);

        cx.child(
            &VStack::new((
                VStack::new((heading("Seletores de cor"), pickers)).spacing(8.0),
                VStack::new((heading("Frente e fundo"), pair, Text::new(pair_line))).spacing(8.0),
                VStack::new((
                    heading("Paleta"),
                    Text::new("Clique para a frente; com Ctrl ou ⌘, para o fundo.")
                        .foreground(Semantic::InkMuted),
                    grid,
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((heading("Destaque da conta"), accents)).spacing(8.0),
                VStack::new((heading("Tema"), themes)).spacing(8.0),
                Text::new(said.get()),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
