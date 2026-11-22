//! `PatchPanel`, every feature (#578): the back of a rack with a front that turns around, a
//! switchboard whose cords go home and work in pairs, a panel with its own rule, and the
//! same panel shown smaller on a phone. Every event writes what happened in a line.

use ravenrs::core::Color;
use ravenrs::prelude::*;
use ravenrs::widgets::cable::CableTexture;

/// The control's name, as the session knows it.
pub const ID: &str = "PatchPanelDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Painel de tomadas";

/// The uses, in the order of the tabs.
pub const USES: [&str; 3] = ["Rack", "Central", "Regra própria"];

fn rack() -> Vec<Jack> {
    vec![
        Jack::output("pedal-l", "Saída L").signal("áudio").at(60.0, 60.0),
        Jack::output("pedal-r", "Saída R").signal("áudio").at(160.0, 60.0),
        Jack::output("tap", "Tap").signal("controle").at(300.0, 60.0),
        Jack::output("clock", "Clock").signal("gatilho").at(420.0, 60.0),
        Jack::input("in-1", "Entrada 1").signal("áudio").at(60.0, 220.0),
        Jack::input("in-2", "Entrada 2").signal("áudio").at(160.0, 220.0),
        Jack::input("pedal", "Pedal").signal("controle").at(300.0, 220.0),
        Jack::input("sync", "Sync").signal("gatilho").at(420.0, 220.0),
    ]
}

fn switchboard() -> Vec<Jack> {
    let mut jacks: Vec<Jack> = (21..=26)
        .enumerate()
        .map(|(i, n)| Jack::input(n.to_string(), format!("Ramal {n}")).at(50.0 + i as f32 * 76.0, 50.0))
        .collect();
    for (i, (id, label)) in [
        ("a-atender", "Atender A"),
        ("a-chamar", "Chamar A"),
        ("b-atender", "Atender B"),
        ("b-chamar", "Chamar B"),
    ]
    .into_iter()
    .enumerate()
    {
        jacks.push(Jack::output(id, label).at(90.0 + i as f32 * 100.0, 250.0));
    }
    jacks
}

fn columns() -> Vec<Jack> {
    vec![
        Jack::output("rio", "Rio").at(70.0, 50.0),
        Jack::output("lisboa", "Lisboa").at(70.0, 140.0),
        Jack::output("toquio", "Tóquio").at(70.0, 230.0),
        Jack::input("japao", "Japão").at(380.0, 50.0),
        Jack::input("brasil", "Brasil").at(380.0, 140.0),
        Jack::input("portugal", "Portugal").at(380.0, 230.0),
    ]
}

/// The page: a tab per use, and a line with the last event.
#[derive(Debug, Default)]
pub struct PatchPanelPage;

impl View for PatchPanelPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let tab = cx.use_state(|| 0_usize);
        let said =
            cx.use_state(|| "Puxe um cabo de uma saída e solte numa entrada do mesmo sinal.".to_string());
        let rack_links = cx.use_state(|| vec![PatchLink::new("pedal-l", "in-1")]);
        let board_links = cx.use_state(Vec::<PatchLink>::new);
        let column_links = cx.use_state(Vec::<PatchLink>::new);
        let flipped = cx.use_state(|| false);
        let narrow = cx.viewport().width < 720.0;
        let room = if narrow { (cx.viewport().width - 96.0).max(240.0) } else { 2000.0 };
        let (plugged, unplugged, joined) = (said.clone(), said.clone(), said.clone());
        let body: AnyView = match tab.get() {
            0 => Box::new(
                PatchPanel::new(rack(), rack_links.binding())
                    .size(480.0, 300.0)
                    .fit(room)
                    .label("Traseira do amplificador")
                    .front(
                        VStack::new((
                            Text::new("Amplificador A-40").style(TextStyle::Headline),
                            Text::new("Ganho · Tom · Volume"),
                        ))
                        .padding(24.0)
                        .spacing(8.0),
                    )
                    .flipped(flipped.binding())
                    .on_connect(move |l| plugged.set(format!("Ligado: {} → {}", l.from, l.to)))
                    .on_disconnect(move |l| unplugged.set(format!("Desligado: {} → {}", l.from, l.to))),
            ),
            1 => Box::new(
                PatchPanel::new(switchboard(), board_links.binding())
                    .size(480.0, 320.0)
                    .fit(room)
                    .label("Central telefônica")
                    .background(Color::hex(0x4A2A1A))
                    .color_of(|_| Color::hex(0xC9A46A))
                    .texture_of(|_| CableTexture::Braided)
                    .thickness(5.0)
                    .retract(true)
                    .pair("a-atender", "a-chamar")
                    .pair("b-atender", "b-chamar")
                    .on_join(move |a, b| joined.set(format!("Ramais {a} e {b} em conversa"))),
            ),
            _ => Box::new(
                PatchPanel::new(columns(), column_links.binding())
                    .size(460.0, 290.0)
                    .fit(room)
                    .label("Ligue cada cidade ao seu país")
                    .accept(|from, to| {
                        let right = matches!(
                            (from.id.as_str(), to.id.as_str()),
                            ("rio", "brasil") | ("lisboa", "portugal") | ("toquio", "japao")
                        );
                        if right { Ok(()) } else { Err(format!("{} não fica em {}", from.label, to.label)) }
                    }),
            ),
        };
        let picker: AnyView = if narrow {
            Box::new(Select::new(tab.binding(), USES).label("Uso"))
        } else {
            Box::new(SegmentedControl::new(tab.binding(), USES))
        };
        cx.child(
            &VStack::new((picker, Keyed::new(tab.get() as u64, body), Text::new(said.get())))
                .spacing(12.0)
                .padding(if narrow { 0.0 } else { 16.0 })
                .align(Align::Start),
        )
    }
}
