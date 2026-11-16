//! `Alert`, `Toaster`, `Badge` and `Avatar`, every feature: alerts in each tone, with and without a
//! title, a button and the thin strip; toasts that come and go in any corner; badges for status,
//! counts and categories; and people as initials or a picture, with the ring of the chosen one.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "AlertsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Avisos";

/// The corners the toasts can stack in, in the order of the picker.
pub const CORNERS: [&str; 6] = [
    "Acima à esquerda",
    "Acima ao centro",
    "Acima à direita",
    "Abaixo à esquerda",
    "Abaixo ao centro",
    "Abaixo à direita",
];

/// The people of the team row, the first of them with a picture.
pub const TEAM: [&str; 4] = ["Ana Souza", "Bia Lima", "Caio Prado", "Davi Reis"];

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

fn corner(index: usize) -> ToastPosition {
    match index {
        0 => ToastPosition::TopLeft,
        1 => ToastPosition::TopCenter,
        2 => ToastPosition::TopRight,
        3 => ToastPosition::BottomLeft,
        4 => ToastPosition::BottomCenter,
        _ => ToastPosition::BottomRight,
    }
}

/// A small portrait drawn in code, so that the page needs no file.
fn portrait() -> Option<Image> {
    let data = ImageData::from_fn(48, 48, |x, y| {
        let (dx, dy) = (x as f32 - 24.0, y as f32 - 18.0);
        if dx * dx + dy * dy < 100.0 || (y > 30 && (x as f32 - 24.0).abs() < (y as f32 - 22.0)) {
            Color::hex(0xF2C14E)
        } else {
            Color::hex(0x2563EB)
        }
    });
    data.map(|data| Image::new(data).alt("Retrato de Ana Souza"))
}

/// The page: alerts, toasts, badges and avatars, and a line with the last event.
#[derive(Debug, Default)]
pub struct AlertsPage;

impl View for AlertsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Use os botões dos avisos.".to_string());
        let toasts = cx.use_state(Toasts::new).get();
        let place = cx.use_state(|| 5_usize);
        let lifted = cx.use_state(|| false);
        let chosen = cx.use_state(|| 0_usize);

        let retry = said.clone();
        let review = said.clone();
        let alerts = VStack::new((
            Alert::new("Tudo certo", "As 32 notas foram enviadas à prefeitura.").tone(Tone::Success),
            Alert::new("Atenção", "O certificado digital vence em 12 dias.").tone(Tone::Warning),
            Alert::new("Falha na sincronização", "Três lançamentos não chegaram ao servidor.")
                .tone(Tone::Danger)
                .action("Tentar de novo", move || retry.set("Sincronização refeita".to_string())),
            Alert::new("Novidade", "A busca agora entende datas como \"ontem\".").tone(Tone::Accent),
            Alert::message("Só a mensagem, quando um título repetiria o lugar.").tone(Tone::Neutral),
            Alert::message("Este campo é lido pelo contador.")
                .tone(Tone::Warning)
                .compact()
                .action("Revisar", move || review.set("Revisão pedida pela faixa".to_string())),
            Alert::new("Faixa fina", "Ao lado do que fala, como num painel lateral.").compact(),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        let (quick, quick_line) = (toasts.clone(), said.clone());
        let (slow, slow_line) = (toasts.clone(), said.clone());
        let (fixed, fixed_line) = (toasts.clone(), said.clone());
        let (undo, undo_line) = (toasts.clone(), said.clone());
        let toast_buttons = HStack::new((
            Button::new("Mostrar aviso de sucesso").on_click(move || {
                quick.show("Salvo", "Suas alterações estão guardadas.", Tone::Success);
                quick_line.set(format!("Avisos na tela: {}", quick.len()));
            }),
            Button::new("Aviso de 10 segundos").style(ButtonStyle::Secondary).on_click(move || {
                slow.show_for("Conexão lenta", "O envio pode demorar.", Tone::Warning, 10.0);
                slow_line.set(format!("Avisos na tela: {}", slow.len()));
            }),
            Button::new("Aviso fixo").style(ButtonStyle::Secondary).on_click(move || {
                fixed.show_until_closed("Enviando", "Faltam três arquivos.", Tone::Accent);
                fixed_line.set(format!("Avisos na tela: {}", fixed.len()));
            }),
            Button::new("Aviso com ação").style(ButtonStyle::Secondary).on_click(move || {
                let (done, gone) = (undo_line.clone(), undo_line.clone());
                let extras = ToastExtras::new()
                    .action("Desfazer", move || done.set("Exclusão desfeita".to_string()))
                    .on_close(move || gone.set("O aviso da exclusão saiu".to_string()))
                    .position(ToastPosition::TopCenter);
                undo.show_with(
                    "Excluído",
                    "Uma fatura foi para a lixeira.",
                    Tone::Danger,
                    f64::INFINITY,
                    extras,
                );
                undo_line.set(format!("Avisos na tela: {}", undo.len()));
            }),
        ))
        .spacing(8.0)
        .wrap();
        let corner_picker: AnyView = if narrow {
            Box::new(Select::new(place.binding(), CORNERS).label("Canto dos avisos").fill())
        } else {
            Box::new(SegmentedControl::new(place.binding(), CORNERS).label("Canto dos avisos"))
        };
        let inset = if lifted.get() { 48.0 } else { 0.0 };
        let toaster = Toaster::new(toasts.clone()).position(corner(place.get())).bottom_inset(inset);

        let badges = HStack::new((
            Badge::new("Rascunho"),
            Badge::new("Paga").tone(Tone::Success).icon(IconName::CircleCheck),
            Badge::new("Vence hoje").tone(Tone::Warning).icon(IconName::Clock),
            Badge::new("Atrasada").tone(Tone::Danger),
            Badge::new("Nova").tone(Tone::Accent).outline(),
            Badge::new("Mercado").color(Color::hex(0x7C3AED)),
            Badge::new("Viagem").color(Color::hex(0x0E9F6E)).outline(),
            Badge::new("12").tone(Tone::Accent).height(20.0),
        ))
        .spacing(6.0)
        .wrap()
        .align(Align::Center);
        let sample = Badge::new("Fatura paga").tone(Tone::Success);
        let badge_line = format!("A etiqueta diz: {}", sample.text());

        let picked = chosen.get().min(TEAM.len() - 1);
        let ring = |index: usize, avatar: Avatar| {
            if index == picked { avatar.border(3.0, Semantic::Accent) } else { avatar }
        };
        let team = HStack::new((
            ring(0, portrait().into_iter().fold(Avatar::new(TEAM[0]).size(48.0), Avatar::image)),
            ring(1, Avatar::new(TEAM[1]).size(48.0)),
            ring(2, Avatar::new(TEAM[2]).size(48.0)),
            ring(3, Avatar::new(TEAM[3]).size(48.0)),
        ))
        .spacing(8.0)
        .align(Align::Center);
        let sizes = HStack::new((
            Avatar::new("Eva Nunes").size(24.0),
            Avatar::new("Eva Nunes"),
            Avatar::new("Eva Nunes").size(56.0),
            Avatar::new("Eva Nunes").size(if narrow { 64.0 } else { 80.0 }).border(2.0, Color::hex(0xF2C14E)),
        ))
        .spacing(8.0)
        .align(Align::Center);
        let next = chosen.clone();
        let choose = Button::new("Escolher o próximo")
            .style(ButtonStyle::Secondary)
            .on_click(move || next.update(|i| *i = (*i + 1) % TEAM.len()));

        cx.child(
            &VStack::new((
                VStack::new((heading("Alertas"), alerts)).spacing(8.0).align(Align::Stretch),
                VStack::new((
                    heading("Avisos passageiros"),
                    toast_buttons,
                    corner_picker,
                    Toggle::new(lifted.binding()).label("Acima da barra de status"),
                    toaster,
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((heading("Etiquetas"), badges, Text::new(badge_line))).spacing(8.0),
                VStack::new((
                    heading("Pessoas"),
                    team,
                    HStack::new((choose, Text::new(format!("Escolhida: {}", TEAM[picked])))).spacing(8.0),
                    sizes,
                ))
                .spacing(8.0)
                .align(Align::Start),
                Text::new(said.get()),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
