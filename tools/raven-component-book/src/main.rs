//! `cargo run -p raven-component-book` opens the book.
//!
//! `--json <folder>` writes the JSON of every component and `index.json`, as Storybook's;
//! `--claude-design <folder>` writes the files of the design system the design tool reads under
//! `<folder>/project/`, with `--sha <commit>` naming the commit they came from; `--self-test`
//! draws and measures every story with no window and says how it went (exit code 0 when it
//! passed); `--versao` prints the version.

/// Every option the book reads; any other is refused.
const OPTIONS: [&str; 11] = [
    ravenrs_e2e_wire::SERVE_FLAG,
    "--versao",
    "--self-test",
    "--json",
    "--claude-design",
    "--sha",
    "--verify",
    "--replay",
    "--verbose",
    "--draw",
    "--help",
];

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    if let Some(unknown) = args.iter().find(|a| a.starts_with("--") && !OPTIONS.contains(&a.as_str())) {
        eprintln!("opção desconhecida: {unknown}; as que o livro entende: {}", OPTIONS.join(", "));
        return std::process::ExitCode::from(2);
    }
    if has("--help") {
        println!(
            "RavenComponentBook {}\n\n\
             raven-component-book                          abre o livro\n\
             --json <pasta>                                o JSON de cada componente e o index.json\n\
             --claude-design <pasta> [--sha <commit>]      os arquivos do design system do Claude Design em <pasta>/project\n\
             --verify <pasta>                              compara uma cópia do publicado com o que o RCB gera\n\
             --replay <tela.rcb.json>                      confere se o RavenRS monta cada peça de uma tela do design como foi desenhada\n\
             --replay <tela.rcb.json> --verbose            o mesmo, com a medida de cada peça no desenho e no RavenRS\n\
             --replay <tela.rcb.json> --draw <saída.json>  o mesmo, e grava cada peça como o RavenRS a desenha, em SVG\n\
             --self-test                                   desenha e mede cada história, sem janela\n\
             --versao                                      a versão",
            raven_component_book::VERSION
        );
        return std::process::ExitCode::SUCCESS;
    }
    // The installer proves a new binary opens with this, before any window or folder.
    if has("--versao") {
        println!("{}", raven_component_book::VERSION);
        return std::process::ExitCode::SUCCESS;
    }
    // A test tool such as Raven Test Studio drives the book with no window (`--e2e-serve`):
    // the same book the tests open, with no updates and no self-test in the status bar.
    if has(ravenrs_e2e_wire::SERVE_FLAG) {
        let session =
            ravenrs_e2e::E2e::new(raven_component_book::app(), ravenrs::prelude::Theme::raven_dark())
                .viewport(ravenrs::prelude::Size::new(1440.0, 900.0), 1.0);
        let hello =
            ravenrs_e2e_wire::Hello::of("RavenComponentBook").scenarios(raven_component_book::SCENARIOS);
        return match ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default()) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("o canal de teste parou: {error}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    if has("--self-test") {
        return match raven_component_book::self_test() {
            Ok(said) => {
                println!("{said}");
                std::process::ExitCode::SUCCESS
            }
            Err(why) => {
                eprintln!("o autoteste falhou: {why}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    if let Some(file) = value("--replay") {
        let checked = std::fs::read_to_string(&file)
            .map_err(|e| format!("não deu para ler {file}: {e}"))
            .and_then(|json| raven_component_book::replay::replay(&json));
        if let (Ok(replay), true) = (&checked, has("--verbose")) {
            for piece in &replay.measured {
                println!("{piece}");
            }
        }
        // Each piece as RavenRS draws it, for a board that follows the book (#921).
        if let (Ok(replay), Some(out)) = (&checked, value("--draw")) {
            let pieces: Vec<serde_json::Value> = replay
                .measured
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "element": m.element, "design": [m.design.0, m.design.1],
                        "built": [m.built.0, m.built.1], "margin": raven_component_book::measure::MARGIN,
                        "svg": m.drawing,
                    })
                })
                .collect();
            if let Err(e) = std::fs::write(&out, serde_json::Value::Array(pieces).to_string()) {
                eprintln!("não deu para escrever {out}: {e}");
                return std::process::ExitCode::from(2);
            }
        }
        return match checked {
            Ok(replay) if replay.divergences.is_empty() => {
                println!("{} peças conferidas: o RavenRS monta todas como foram desenhadas", replay.compared);
                std::process::ExitCode::SUCCESS
            }
            Ok(replay) => {
                for d in &replay.divergences {
                    println!("{d}");
                }
                println!("{}", replay.summary());
                std::process::ExitCode::FAILURE
            }
            Err(why) => {
                eprintln!("{why}");
                std::process::ExitCode::from(2)
            }
        };
    }
    if let Some(folder) = value("--verify") {
        return match raven_component_book::export::verify(std::path::Path::new(&folder)) {
            Ok(drift) if drift.is_clean() => {
                println!("o design system publicado é exatamente o que o RCB gera");
                std::process::ExitCode::SUCCESS
            }
            Ok(drift) => {
                for (what, list) in
                    [("mudou", &drift.changed), ("falta", &drift.missing), ("sobra", &drift.extra)]
                {
                    for path in list {
                        println!("{what}: {path}");
                    }
                }
                std::process::ExitCode::FAILURE
            }
            Err(e) => {
                eprintln!("não deu para comparar: {e}");
                std::process::ExitCode::from(2)
            }
        };
    }
    if has("--json") || has("--claude-design") {
        let Some(folder) = value("--json").or_else(|| value("--claude-design")) else {
            eprintln!("diga a pasta: --json <pasta> ou --claude-design <pasta>");
            return std::process::ExitCode::from(2);
        };
        let folder = std::path::PathBuf::from(folder);
        let written = if has("--json") {
            raven_component_book::export::write_book(&folder)
                .map(|n| format!("{n} componentes em {}", folder.display()))
        } else {
            let sha = value("--sha").unwrap_or_else(|| "master".to_owned());
            raven_component_book::export::write_claude_design(&folder, &sha)
                .map(|files| format!("{} arquivos em {}", files.len(), folder.join("project").display()))
        };
        return match written {
            Ok(said) => {
                println!("{said}");
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("não deu para escrever: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let options = ravenrs::native::WindowOptions::new("RavenComponentBook").size(1440.0, 900.0);
    // Closing the window keeps the book beside the clock, as Raven Finanças does; launching it
    // again opens the copy already running.
    let tray = ravenrs::native::SystemTray::new("RavenComponentBook")
        .open("Abrir o RavenComponentBook")
        .quit("Sair");
    let identity = ravenrs::native::AppIdentity::new(raven_component_book::versao::APP.package)
        .labels(ravenrs::prelude::Labels::pt_br())
        .new_issue_url("https://github.com/alexkads/RavenRS/issues/new")
        .close_to_tray(tray);
    match ravenrs::native::run_as(raven_component_book::installed_app(), options, identity) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::FAILURE
        }
    }
}
