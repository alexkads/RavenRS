//! `CodeEditor` and `Playground`, every feature: an editor of Rust with its caret, history and
//! focus in the app's hands, suggestions, tips and the way to a definition asked of the app,
//! marked problems, its options as switches, and a console that follows its end; then a
//! playground with examples, its code kept by the app, its own buttons and a download.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "CodeEditorDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Editor e playground";

const CODE: &str = "fn main() {\n    let total = soma(2, 3);\n    println!(\"{total}\");\n}\n\nfn soma(a: i32, b: i32) -> i32 {\n    a + b\n}";

/// The examples of the playground, by name.
pub const EXAMPLES: [(&str, &str); 2] = [
    ("Saudação", "diga Olá, Raven\ndiga Tudo pronto"),
    ("Com erro", "diga Uma linha\ngrite Isso não existe"),
];

/// The playground's language: each line `diga <texto>` shows the text; anything else is a problem.
fn run(source: &str) -> Result<AnyView, Vec<CodeDiagnostic>> {
    let mut lines = Vec::new();
    let mut problems = Vec::new();
    for (i, line) in source.lines().enumerate() {
        match line.trim().strip_prefix("diga ") {
            Some(words) => lines.push(Text::new(words.to_owned())),
            None if line.trim().is_empty() => {}
            None => problems.push(
                CodeDiagnostic::error(i + 1, 1, line.trim().len(), "Comando desconhecido")
                    .hint("Comece a linha com `diga`"),
            ),
        }
    }
    if problems.is_empty() { Ok(Box::new(VStack::new(lines).spacing(4.0))) } else { Err(problems) }
}

/// The page.
#[derive(Debug, Default)]
pub struct CodePage;

impl View for CodePage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let code = cx.use_state(|| CODE.to_string());
        let caret = cx.use_state(|| Cursor::at(0));
        let history = cx.use_state(TextHistory::new);
        let focus = cx.use_state(|| false);
        let completions = cx.use_state(Vec::<CodeCompletion>::new);
        let tip = cx.use_state(|| None::<String>);
        let numbers = cx.use_state(|| true);
        let problems = cx.use_state(|| true);
        let read_only = cx.use_state(|| false);
        let plain = cx.use_state(|| 0_usize);
        let indent = cx.use_state(|| 1_usize);
        let console = cx.use_state(|| "Pronto.".to_string());
        let source = cx.use_state(String::new);
        let example = cx.use_state(|| 0_usize);
        let said = cx.use_state(|| "Nada aconteceu ainda.".to_string());
        let narrow = cx.viewport().width < 720.0;
        let wide = if narrow { (cx.viewport().width - 32.0).max(240.0) } else { 760.0 };

        let (asked, offer) = (said.clone(), completions.clone());
        let (rested, answer) = (said.clone(), tip.clone());
        let went = said.clone();
        let editor = CodeEditor::new(code.binding())
            .label("Código de exemplo")
            .language(if plain.get() == 0 { CodeLanguage::Rust } else { CodeLanguage::Plain })
            .line_numbers(numbers.get())
            .indent([2, 4][indent.get().min(1)])
            .rows(if narrow { 8 } else { 10 })
            .read_only(read_only.get())
            .caret(caret.clone())
            .history(history.clone())
            .focus_request(focus.clone())
            .diagnostics(if problems.get() {
                vec![
                    CodeDiagnostic::warning(2, 9, 5, "Variável usada só uma vez").hint("Imprima direto"),
                    CodeDiagnostic::error(6, 4, 4, "Nome em português").hint("Tudo bem aqui"),
                ]
            } else {
                Vec::new()
            })
            // The app answers what is asked: suggestions, tips and where a name is defined.
            .on_complete(move |at| {
                asked.set(format!("Sugestões pedidas na posição {at}"));
                offer.set(vec![
                    CodeCompletion::new("soma").detail("fn(i32, i32) -> i32").insert("soma(1, 2)"),
                    CodeCompletion::new("total").detail("i32"),
                ]);
            })
            .completions(completions.get())
            .on_hover(move |at| {
                rested.set(format!("Dica pedida na posição {at}"));
                answer.set(Some("fn soma(a: i32, b: i32) -> i32".into()));
            })
            .hover(tip.get())
            .on_go_to(move |at| went.set(format!("Ir para a definição na posição {at}")));

        let (to, ask) = (caret.clone(), focus.clone());
        let go_to_line = Button::new("Ir para a linha 6").on_click(move || {
            to.set(Cursor::at(offset_of(CODE, 6, 4)));
            ask.set(true);
        });
        let (into, kept, at) = (code.clone(), history.clone(), caret.clone());
        let comment = Button::new("Inserir um comentário").on_click(move || {
            let before = into.get();
            let after = format!("// Soma dois números.\n{before}");
            kept.update(|h| h.record_change(&before, &after, at.get()));
            into.set(after);
        });
        let (back, from, place) = (history.clone(), code.clone(), caret.clone());
        let undo = Button::new("Desfazer").disabled(!history.get().can_undo()).on_click(move || {
            let (mut text, mut cursor, mut steps) = (from.get(), place.get(), back.get());
            if steps.undo(&mut text, &mut cursor) {
                from.set(text);
                place.set(cursor);
                back.set(steps);
            }
        });
        let (line, column) = line_and_column(&code.get(), caret.get().caret);

        let printing = console.clone();
        let log = VStack::new((CodeEditor::new(console.binding())
            .label("Console")
            .language(CodeLanguage::Plain)
            .line_numbers(false)
            .read_only(true)
            .follow_end(true)
            .fill(),))
        .height(120.0)
        .width(wide);

        let (copied, saved) = (said.clone(), said.clone());
        let playground = Playground::new(run)
            .example(EXAMPLES[0].0, EXAMPLES[0].1)
            .example(EXAMPLES[1].0, EXAMPLES[1].1)
            .source(source.binding())
            .chosen_example(example.binding())
            .language(CodeLanguage::Plain)
            .file_name("roteiro.txt")
            .title("Roteiro")
            .note("Cada linha que começa com diga aparece no resultado.")
            .action("Contar as linhas", IconName::Sigma, move |code| {
                copied.set(format!("O roteiro tem {} linhas", code.lines().count()));
            })
            .download(move |code| {
                saved.set("Baixou o roteiro".into());
                Ok(("roteiro.txt".into(), code.as_bytes().to_vec()))
            })
            .output_open(!narrow)
            .height(if narrow { 420.0 } else { 360.0 });

        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Grid::new(
                    if narrow { 1 } else { 3 },
                    (
                        Toggle::new(numbers.binding()).label("Números das linhas"),
                        Toggle::new(problems.binding()).label("Marcar problemas"),
                        Toggle::new(read_only.binding()).label("Só leitura"),
                    ),
                ),
                HStack::new((
                    SegmentedControl::new(plain.binding(), ["Rust", "Texto puro"]),
                    SegmentedControl::new(indent.binding(), ["Recuo 2", "Recuo 4"]),
                ))
                .spacing(12.0)
                .wrap(),
                VStack::new((editor,)).width(wide),
                HStack::new((go_to_line, comment, undo)).spacing(8.0).wrap(),
                Text::new(format!("Ln {line}, Col {column} · Linhas: {}", code.get().lines().count())),
                VStack::new((
                    Text::new("Console").style(TextStyle::Headline),
                    Button::new("Imprimir uma linha").style(ButtonStyle::Secondary).on_click(move || {
                        printing.update(|c| {
                            let n = c.lines().count();
                            c.push_str(&format!("\nLinha {n} impressa"));
                        });
                    }),
                    log,
                ))
                .spacing(12.0)
                .align(Align::Start),
                Text::new("Playground").style(TextStyle::Headline),
                VStack::new((playground,)).width(wide),
                Text::new(format!("Exemplo escolhido: {}", EXAMPLES[example.get().min(1)].0)),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Start),
        )
    }
}
