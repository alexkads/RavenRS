//! `ReleaseNotes`, `CodeBlock` and `DaySeparator`, every feature (#579): what each version of
//! the shop brought, with the command that installs it, and a history of messages divided by
//! day. A line says what happened last.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "ReleaseNotesDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Novidades da versão";

/// The versions, newest first.
pub const VERSIONS: [&str; 3] = ["Versão 2.4", "Versão 2.3", "Versão 2.2"];
/// The command that installs the newest version.
pub const INSTALL: &str = "raven-loja atualizar --versao 2.4";

/// The page: the notes, the command, the history and a line with the last event.
#[derive(Debug, Default)]
pub struct ReleaseNotesPage;

impl View for ReleaseNotesPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Escolha uma versão.".to_string());
        let selected = cx.use_state(|| 0_usize);

        let issue = said.clone();
        let notes = ReleaseNotes::new(
            vec![
                ReleaseNote::new(VERSIONS[0], "Caixa mais rápido")
                    .date("15 de nov. de 2026")
                    .badge("Nova", Tone::Success)
                    .changes(["A venda fecha com uma tecla", "O cupom sai enquanto o cartão é lido"])
                    .section("Por que atualizar", "Nas horas de pico, a fila anda em metade do tempo.")
                    .code(INSTALL)
                    .links(
                        "De onde veio",
                        [("Pedido #579", move || issue.set("Abriu o pedido #579".to_string()))],
                    ),
                ReleaseNote::new(VERSIONS[1], "Tema escuro")
                    .date("2 de nov. de 2026")
                    .badge("Instalada", Tone::Neutral)
                    .changes(["Tema escuro em todas as telas"]),
                ReleaseNote::new(VERSIONS[2], "Etiquetas")
                    .date("20 de out. de 2026")
                    .changes(["Impressão de etiquetas"]),
            ],
            selected.binding(),
        )
        .height(if narrow { 520.0 } else { 420.0 });

        // A fixed day, so that the words for today and yesterday read the same on any date.
        let today = Date::new(2026, 11, 15).unwrap_or(Date::from_days(0));
        let history = VStack::new((
            DaySeparator::new("Semana passada"),
            Text::new("Bia: o fornecedor mandou a nota."),
            DaySeparator::of(today.add_days(-3), today),
            Text::new("Caio: estoque de café conferido."),
            DaySeparator::of(today.add_days(-1), today),
            Text::new("Ana: a versão 2.4 já está no caixa 2."),
            DaySeparator::of(today, today),
            Text::new("Ana: instalar nos outros caixas."),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        cx.child(
            &VStack::new((
                Text::new("Novidades").style(TextStyle::Headline),
                notes,
                Text::new(format!("Versão aberta: {}", VERSIONS[selected.get().min(2)])),
                Text::new("Para instalar").style(TextStyle::Headline),
                CodeBlock::new(INSTALL),
                Text::new("Conversa da equipe").style(TextStyle::Headline),
                history,
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
