//! `raven-ci-agent`: see the [crate](raven_ci_agent).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--versao" || a == "--version") {
        println!("raven-ci-agent {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.is_empty() || args.iter().any(|a| a == "--ajuda" || a == "--help") {
        println!(
            "raven-ci-agent --servidor URL --nome NOME [--token T] [--pasta DIR] [--rotulos a,b] [--executores N]\n\
             [--nucleos N] [--memoria 4G] [--disco 20G] [--disco-livre 5G] [--vigia-ms 2000]\n\n\
             O token de uso único também vem em {}.",
            raven_ci_agent::TOKEN_VAR
        );
        return;
    }
    let config = match raven_ci_agent::Config::from_args(&args, std::env::var(raven_ci_agent::TOKEN_VAR).ok())
    {
        Ok(config) => config,
        Err(e) => {
            eprintln!("raven-ci-agent: {e}");
            std::process::exit(2);
        }
    };
    // The token used once does not reach the steps: `ravenrs-process` gives a program only the
    // basic variables and the build's own.
    if let Err(e) = raven_ci_agent::run(config, Arc::new(AtomicBool::new(false))) {
        eprintln!("raven-ci-agent: {e}");
        std::process::exit(1);
    }
}
