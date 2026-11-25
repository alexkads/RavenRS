//! Stat: a headline number with its label, the change since before (up or down, good or bad),
//! the large size for the one number a screen is about, and a muted caption under the value.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "StatDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Indicadores";

/// Every public feature of Stat on one page.
#[derive(Debug, Default)]
pub struct StatPage;

impl View for StatPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let waiting = cx.use_state(|| 2_u32);
        let large = cx.use_state(|| false);
        let (more, less) = (waiting.clone(), waiting.clone());
        let caption = match waiting.get() {
            0 => "Ninguém espera".to_owned(),
            n => format!("O mais antigo espera há {} s", 20 * n),
        };
        let mut queue = Stat::new("Na fila", waiting.get().to_string()).caption(caption.clone());
        if large.get() {
            queue = queue.large();
        }
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new(
                    "Um número com o seu rótulo, a mudança desde antes, o tamanho grande para o número que importa e \
                     uma legenda que diz de onde ele vem.",
                )
                .foreground(Semantic::InkMuted),
                Text::new("Com legenda").style(TextStyle::Headline),
                HStack::new((
                    Card::new(queue),
                    Card::new(Stat::new("Memória do servidor", "50,6 MB").caption("Lida do processo, ao vivo; meta ≤ 60 MB")),
                ))
                .spacing(16.0)
                .wrap(),
                HStack::new((
                    Button::new("Chegou um build").style(ButtonStyle::Secondary).on_click(move || more.update(|n| *n += 1)),
                    Button::new("Saiu um build").style(ButtonStyle::Ghost).on_click(move || {
                        less.update(|n| *n = n.saturating_sub(1));
                    }),
                    Toggle::new(large.binding()).label("Grande"),
                ))
                .spacing(12.0)
                .align(Align::Center)
                .wrap(),
                Text::new(format!("Legenda: {caption}")).foreground(Semantic::InkMuted),
                Text::new("A mudança").style(TextStyle::Headline),
                HStack::new((
                    Card::new(Stat::new("Receita", "R$ 128.450").delta("+12,4%", true)),
                    Card::new(Stat::new("Visitas", "8.210").delta("-3,1%", false)),
                    Card::new(Stat::new("Gastos", "R$ 9.300").delta_toned("+8%", true, false)),
                    Card::new(Stat::new("Cancelamentos", "1,8%").delta_toned("-0,4%", false, true)),
                ))
                .spacing(16.0)
                .wrap(),
                Text::new("O tom do valor").style(TextStyle::Headline),
                Text::new("Um total cuja cor diz o que ele é, como o dinheiro que entrou. O tom vai em direção à tinta, como as palavras de um selo, para ficar legível em todo tema.")
                    .foreground(Semantic::InkMuted),
                HStack::new((
                    Card::new(Stat::new("Caixa", "R$ 2.180")),
                    Card::new(Stat::new("Pós-venda", "R$ 1.240").value_tone(Tone::Success)),
                    Card::new(Stat::new("A conferir", "R$ 310").value_tone(Tone::Warning)),
                    Card::new(Stat::new("Estornado", "R$ 90").value_tone(Tone::Danger)),
                ))
                .spacing(16.0)
                .wrap(),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(960.0)
            .align(Align::Stretch),
        )
    }
}
