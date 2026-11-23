//! List and ListItem: rows with an icon or an avatar, a title, a subtitle, words at the end, an
//! action and a tone for the icon; picked or opened with a click. Compact, the rows are 48 tall
//! with the subtitle beside the title, as a list of stages or a queue.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "ListDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Lista";

/// The stages a build ran: name, what ran it, icon, tone and how long it took.
const STAGES: [(&str, &str, IconName, Tone, &str); 4] = [
    ("Checkout", "git", IconName::CircleCheck, Tone::Success, "8 s"),
    ("Lint", "cargo clippy", IconName::CircleCheck, Tone::Success, "1 min 40 s"),
    ("Testes", "cargo nextest", IconName::LoaderCircle, Tone::Accent, "rodando"),
    ("Imagem", "docker build", IconName::Clock, Tone::Neutral, "esperando"),
];

/// Every public feature of List and ListItem on one page.
#[derive(Debug, Default)]
pub struct ListPage;

impl View for ListPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let compact = cx.use_state(|| true);
        let picked = cx.use_state(|| 2_usize);
        let said = cx.use_state(|| "Nada aconteceu ainda.".to_owned());
        let rows: Vec<AnyView> = STAGES
            .iter()
            .enumerate()
            .map(|(i, (name, tool, icon, tone, took))| -> AnyView {
                let (pick, say) = (picked.clone(), said.clone());
                let name = (*name).to_owned();
                Box::new(
                    ListItem::new(name.clone())
                        .icon(*icon)
                        .tone(*tone)
                        .subtitle(*tool)
                        .trailing(*took)
                        .selected(picked.get() == i)
                        .on_click(move || {
                            pick.set(i);
                            say.set(format!("Aberto: {name}"));
                        }),
                )
            })
            .collect();
        let stages = List::new(rows);
        let stages: AnyView = if compact.get() { Box::new(stages.compact()) } else { Box::new(stages) };
        let lift = said.clone();
        let queue = List::new((ListItem::new("imagens-base #7")
            .icon(IconName::Plug)
            .tone(Tone::Warning)
            .subtitle("pede o rótulo docker · o plugin docker está desligado")
            .action(
                Button::new("Ligar o plugin")
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .on_click(move || lift.set("Ação: ligar o plugin".to_owned())),
            ),))
        .compact();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new(
                    "Linhas com ícone ou avatar, título, subtítulo, palavras no fim, uma ação e o tom do ícone. Compacta, \
                     cada linha tem 48 e o subtítulo vai ao lado do título.",
                )
                .foreground(Semantic::InkMuted),
                Toggle::new(compact.binding()).label("Compacta"),
                Text::new("Os estágios").style(TextStyle::Headline),
                stages,
                Text::new("A fila").style(TextStyle::Headline),
                queue,
                Text::new("Pessoas").style(TextStyle::Headline),
                List::new((
                    ListItem::new("Ana Lima").avatar("Ana Lima").subtitle("ana@exemplo.dev").trailing("Dona"),
                    ListItem::new("Bia Souza").avatar("Bia Souza").subtitle("bia@exemplo.dev").trailing("Editora"),
                )),
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(720.0)
            .align(Align::Stretch),
        )
    }
}
