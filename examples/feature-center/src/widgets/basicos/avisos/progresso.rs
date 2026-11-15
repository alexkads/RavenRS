//! `ProgressBar`, `Spinner`, `Skeleton` and `SplashScreen`, every feature: bars with a label, a
//! percentage and a detail in each tone and thickness, a copy that advances by steps, spinners of
//! every size, the placeholder of a card while it loads and the opening screen of an app, with its
//! icon or its own artwork, a spinner or a bar, and a status that changes as it goes.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "ProgressDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Progresso e espera";

/// How long the opening screen stays, in seconds.
pub const SPLASH_SECONDS: f64 = 3.0;

/// What the opening screen says along the way, a third of the time each.
pub const STEPS: [&str; 3] = ["Abrindo o banco de dados", "Lendo as contas", "Preparando o painel"];

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

/// The artwork of the opening screen, drawn in code: a ring in the accent color.
fn artwork() -> Option<Image> {
    let data = ImageData::from_fn(64, 64, |x, y| {
        let (dx, dy) = (x as f32 - 32.0, y as f32 - 32.0);
        let d = (dx * dx + dy * dy).sqrt();
        if (18.0..28.0).contains(&d) { Color::hex(0x2563EB) } else { Color::TRANSPARENT }
    });
    data.map(|data| Image::new(data).size(64.0, 64.0).alt("Logotipo do Raven Finanças"))
}

/// The page: progress bars, spinners, placeholders and the opening screen.
#[derive(Debug, Default)]
pub struct ProgressPage;

impl View for ProgressPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let copied = cx.use_state(|| 0_u32);
        let loading = cx.use_state(|| true);
        let started = cx.use_state(|| None::<f64>);
        let with_logo = cx.use_state(|| false);
        let with_bar = cx.use_state(|| true);

        // A copy of 8 photos that moves one step per click.
        let done = copied.get().min(8);
        let value = done as f32 / 8.0;
        let (step, restart) = (copied.clone(), copied.clone());
        let copy = VStack::new((
            ProgressBar::new(value).label("Copiando as fotos").percent().detail(format!("{done} de 8 fotos")),
            HStack::new((
                Button::new("Copiar mais duas")
                    .disabled(done >= 8)
                    .on_click(move || step.update(|n| *n = (*n + 2).min(8))),
                Button::new("Recomeçar a cópia")
                    .style(ButtonStyle::Secondary)
                    .on_click(move || restart.set(0)),
            ))
            .spacing(8.0),
            Text::new(if done >= 8 {
                "Cópia concluída".to_string()
            } else {
                format!("Cópia em {:.0}%", value * 100.0)
            }),
        ))
        .spacing(8.0)
        .align(Align::Stretch);
        let tones = VStack::new((
            ProgressBar::new(0.35).label("Armazenamento").tone(Tone::Accent),
            ProgressBar::new(0.92).label("Cota de envios").percent().tone(Tone::Danger).height(10.0),
            ProgressBar::new(0.7).label("Meta do mês").tone(Tone::Success).detail("R$ 7.000 de R$ 10.000"),
            ProgressBar::new(0.55).label("Revisão").tone(Tone::Warning).height(3.0),
            ProgressBar::new(0.6).label("Orçamento de viagem").color(Color::hex(0x7C3AED)).percent(),
            ProgressBar::new(0.2),
        ))
        .spacing(10.0)
        .align(Align::Stretch);

        let spinners = HStack::new((
            Spinner::new().size(12.0),
            Spinner::new(),
            Spinner::new().size(32.0),
            Spinner::new().size(if narrow { 40.0 } else { 48.0 }).color(Color::hex(0x0E9F6E)),
            Button::new("Enviando").loading(true),
        ))
        .spacing(16.0)
        .align(Align::Center);

        let card: AnyView = if loading.get() {
            Box::new(
                HStack::new((
                    Skeleton::circle(40.0),
                    VStack::new((
                        Skeleton::line(14.0),
                        Skeleton::block(if narrow { 120.0 } else { 180.0 }, 12.0),
                    ))
                    .spacing(6.0)
                    .align(Align::Stretch),
                ))
                .spacing(12.0)
                .align(Align::Center),
            )
        } else {
            Box::new(
                HStack::new((
                    Avatar::new("Ana Souza").size(40.0),
                    VStack::new((
                        Text::new("Ana Souza"),
                        Text::new("Contas a receber").style(TextStyle::Caption),
                    ))
                    .spacing(2.0)
                    .align(Align::Start),
                ))
                .spacing(12.0)
                .align(Align::Center),
            )
        };

        // The opening screen follows the clock from the click that showed it.
        let now = cx.now();
        let elapsed = started.get().map(|at| now - at);
        let visible = elapsed.is_some_and(|e| e < SPLASH_SECONDS);
        if let (Some(at), true) = (started.get(), visible) {
            let third = SPLASH_SECONDS / 3.0;
            let next = ((now - at) / third).floor() + 1.0;
            cx.wake_at(at + (next * third).min(SPLASH_SECONDS));
        }
        let part = elapsed.unwrap_or(0.0) / SPLASH_SECONDS;
        let status = STEPS[((part * 3.0) as usize).min(STEPS.len() - 1)];
        let mut splash = SplashScreen::new(Binding::constant(visible), "Raven Finanças")
            .icon(IconName::Wallet)
            .status(status);
        if with_logo.get() {
            if let Some(logo) = artwork() {
                splash = splash.logo(logo);
            }
            splash = splash.reveal_after(0.4);
        }
        if with_bar.get() {
            splash = splash.progress(part as f32);
        }
        let show = started.clone();
        let splash_line = match elapsed {
            None => "A abertura ainda não foi mostrada".to_string(),
            Some(_) if visible => format!("Abertura: {status}"),
            Some(_) => "A abertura terminou e o app ficou à mão".to_string(),
        };
        let splash_controls = VStack::new((
            HStack::new((
                Toggle::new(with_logo.binding()).label("Com o logotipo do app"),
                Toggle::new(with_bar.binding()).label("Com barra de progresso"),
            ))
            .spacing(16.0)
            .wrap(),
            Button::new("Mostrar a abertura").icon(IconName::Play).on_click(move || show.set(Some(now))),
            Text::new(splash_line),
            splash,
        ))
        .spacing(8.0)
        .align(Align::Start);

        cx.child(
            &VStack::new((
                VStack::new((heading("Barras de progresso"), copy, tones)).spacing(8.0).align(Align::Stretch),
                VStack::new((heading("Girando"), spinners)).spacing(8.0),
                VStack::new((
                    heading("Enquanto carrega"),
                    Toggle::new(loading.binding()).label("Cartão carregando"),
                    card,
                ))
                .spacing(8.0)
                .align(Align::Stretch),
                VStack::new((heading("Tela de abertura"), splash_controls)).spacing(8.0),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
