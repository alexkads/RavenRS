//! LogView: a build log with the colors the programs wrote, that follows the end as lines come,
//! with search, stages that fold and a line pointed at by its number.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "LogViewDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Log de build";

/// A short build, as a CI runner writes it.
fn build_log() -> LogBuffer {
    let log = LogBuffer::new();
    log.section("Checkout");
    log.push("$ git fetch origin +refs/pull/803/head");
    log.push("HEAD is now at a1b2c3d");
    log.section("Testes");
    log.push("$ cargo nextest run");
    log.push("    \u{1b}[32mPASS\u{1b}[0m [0.012s] plan::rejects_cycles\n    \u{1b}[32mPASS\u{1b}[0m [0.040s] claim::lease_is_exclusive");
    log.push("    \u{1b}[33mSLOW\u{1b}[0m [12.4s] registry::reloads_without_restart");
    log.push("    \u{1b}[1;31mFAIL\u{1b}[0m [0.210s] docker::cleans_up_on_cancel");
    // A long line runs on, as in a terminal, and the list scrolls across to it.
    log.push(
        "$ docker run --rm -v /var/lib/raven-ci/espacos/apps/site/42:/w -w /w -u 1000:1000 -e CARGO_TARGET_DIR=/w/target rust:1 cargo nextest run --workspace --retries 2",
    );
    log
}

/// Every public feature of LogView on one page.
#[derive(Debug, Default)]
pub struct LogPage;

impl View for LogPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let log = cx.use_state(build_log).peek().clone();
        let follow = cx.use_state(|| true);
        let numbers = cx.use_state(|| true);
        let search = cx.use_state(|| true);
        let said = cx.use_state(|| "Nenhuma linha apontada.".to_string());
        let added = cx.use_state(|| 0_u32);
        let told = said.clone();
        let view = LogView::new(log.clone())
            .follow_end(follow.get())
            .line_numbers(numbers.get())
            .searchable(search.get())
            .height(260.0)
            .label("Log do build 42")
            .on_line(move |number| told.set(format!("Linha apontada: {number}.")));
        let (more, count) = (log.clone(), added.clone());
        let lines = log.len();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new(
                    "Cores ANSI do próprio programa, fonte monoespaçada e só as linhas visíveis montadas. Clique no \
                     título de um estágio para dobrar, e no número de uma linha para apontá-la.",
                )
                .foreground(Semantic::InkMuted),
                HStack::new((
                    Toggle::new(follow.binding()).label("Seguir o fim"),
                    Toggle::new(numbers.binding()).label("Números das linhas"),
                    Toggle::new(search.binding()).label("Busca"),
                    Button::new("Chegar linha nova").style(ButtonStyle::Secondary).on_click(move || {
                        count.update(|n| *n += 1);
                        more.push(&format!("    \u{1b}[32mPASS\u{1b}[0m [0.001s] nova::linha_{}", count.get()));
                    }),
                ))
                .spacing(16.0)
                .wrap(),
                view,
                Text::new(format!("{lines} linhas no log.")).foreground(Semantic::InkMuted),
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Stretch),
        )
    }
}
