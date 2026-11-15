//! `Lottie`, every feature: two animations made of curves, played by the engine of the house, at
//! three sizes, paused and played, once or over and over, slower or faster, and named for screen
//! readers. A line says how they are playing.

use ravenrs::prelude::*;

use super::{heading, note};

/// The control's name, as the session knows it.
pub const ID: &str = "LottieDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Animação";

/// The speeds, in the order of the choice.
pub const SPEEDS: [&str; 3] = ["Metade", "Normal", "Dobro"];
const SPEED_VALUES: [f64; 3] = [0.5, 1.0, 2.0];

/// A ring that draws itself and then wipes out, two seconds long.
const RING: &str = r#"{"v":"5.7.0","fr":30,"ip":0,"op":60,"w":100,"h":100,"layers":[{"ty":4,"ks":{},
"ip":0,"op":60,"st":0,"shapes":[{"ty":"gr","it":[{"ty":"el","p":{"a":0,"k":[50,50]},"s":{"a":0,"k":[70,70]}},
{"ty":"tm","s":{"a":1,"k":[{"t":0,"s":[0],"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":30,"s":[0],
"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":60,"s":[100]}]},"e":{"a":1,"k":[{"t":0,"s":[0],
"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":30,"s":[100]}]},"o":{"a":0,"k":0},"m":1},
{"ty":"st","c":{"a":0,"k":[0.25,0.34,0.86,1]},"o":{"a":0,"k":100},"w":{"a":0,"k":9},"lc":2,"lj":2},
{"ty":"tr","a":{"a":0,"k":[0,0]},"p":{"a":0,"k":[0,0]},"s":{"a":0,"k":[100,100]},"r":{"a":0,"k":0},"o":{"a":0,"k":100}}]}]}]}"#;

/// A dot that grows and shrinks, as a heartbeat, a little over a second long.
const PULSE: &str = r#"{"v":"5.7.0","fr":30,"ip":0,"op":40,"w":100,"h":100,"layers":[{"ty":4,"ks":{},
"ip":0,"op":40,"st":0,"shapes":[{"ty":"gr","it":[{"ty":"el","p":{"a":0,"k":[50,50]},"s":{"a":1,"k":[
{"t":0,"s":[30,30],"o":{"x":[0.4],"y":[0]},"i":{"x":[0.6],"y":[1]}},
{"t":20,"s":[72,72],"o":{"x":[0.4],"y":[0]},"i":{"x":[0.6],"y":[1]}},{"t":40,"s":[30,30]}]}},
{"ty":"fl","c":{"a":0,"k":[0.95,0.55,0.2,1]},"o":{"a":0,"k":100}},
{"ty":"tr","a":{"a":0,"k":[0,0]},"p":{"a":0,"k":[0,0]},"s":{"a":0,"k":[100,100]},"r":{"a":0,"k":0},"o":{"a":0,"k":100}}]}]}]}"#;

/// The page: the two animations at three sizes, and the switches that play them.
#[derive(Debug, Default)]
pub struct AnimationPage;

impl View for AnimationPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let ring = cx.use_state(|| LottieAnimation::from_json(RING.as_bytes()).ok()).get();
        let pulse = cx.use_state(|| LottieAnimation::from_json(PULSE.as_bytes()).ok()).get();
        let playing = cx.use_state(|| true);
        let looping = cx.use_state(|| true);
        let speed = cx.use_state(|| 1_usize);
        let rate = SPEED_VALUES[speed.get().min(2)];

        let player = |animation: &Option<LottieAnimation>, side: f32, alt: String| -> AnyView {
            match animation {
                Some(animation) => Box::new(
                    Lottie::new(animation.clone())
                        .size(side, side)
                        .looping(looping.get())
                        .speed(rate)
                        .playing(playing.binding())
                        .alt(alt),
                ),
                None => Box::new(Text::new("Animação inválida")),
            }
        };
        let sizes: [(f32, &str); 3] = if narrow {
            [(40.0, "pequeno"), (72.0, "médio"), (104.0, "grande")]
        } else {
            [(48.0, "pequeno"), (96.0, "médio"), (160.0, "grande")]
        };
        let row = |animation: &Option<LottieAnimation>, name: &str| {
            HStack::new(
                sizes
                    .iter()
                    .map(|(side, size)| player(animation, *side, format!("{name}, {size}")))
                    .collect::<Vec<_>>(),
            )
            .spacing(16.0)
            .align(Align::End)
        };
        let speeds: AnyView = if narrow {
            Box::new(Select::new(speed.binding(), SPEEDS).label("Velocidade"))
        } else {
            Box::new(SegmentedControl::new(speed.binding(), SPEEDS))
        };
        // Without a name, an animation is decoration that screen readers skip.
        let decoration: AnyView = match &pulse {
            Some(animation) => Box::new(Lottie::new(animation.clone()).size(24.0, 24.0)),
            None => Box::new(Text::new("Animação inválida")),
        };
        let state = format!(
            "{} · velocidade {} · {}",
            if playing.get() { "Tocando" } else { "Pausada" },
            SPEEDS[speed.get().min(2)].to_lowercase(),
            if looping.get() { "repete sem parar" } else { "para no último quadro" }
        );

        cx.child(
            &VStack::new((
                heading("Animações Lottie"),
                note("Curvas desenhadas na GPU, nítidas em qualquer tamanho. Com movimento reduzido, fica o último quadro."),
                HStack::new((
                    Toggle::new(playing.binding()).label("Tocando"),
                    Toggle::new(looping.binding()).label("Repetir"),
                    speeds,
                ))
                .spacing(12.0)
                .wrap(),
                row(&ring, "Anel que se desenha"),
                row(&pulse, "Ponto pulsando"),
                HStack::new((decoration, note("Sem nome: enfeite que o leitor de tela pula")))
                    .spacing(8.0)
                    .align(Align::Center),
                Text::new(state),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Start),
        )
    }
}
