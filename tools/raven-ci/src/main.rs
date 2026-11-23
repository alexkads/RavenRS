//! Raven CI: a small CI/CD server whose core holds only the queue, the model of jobs and builds
//! and the register of plugins. Everything else is a plugin.
//!
//! ```text
//! raven-ci plugins listar [--format json]
//! raven-ci plugins info NOME [--format json]
//! raven-ci plugins ligar NOME
//! raven-ci plugins desligar NOME
//! raven-ci plugins compilar PASTA
//! raven-ci servir [--endereco HOST:PORTA] [--executores N]
//! echo SENHA | raven-ci usuario criar NOME [--papel PAPEL]
//! raven-ci rodar JOB [--param NOME=VALOR]... [--esperar]
//! raven-ci status JOB [NUMERO] | log JOB NUMERO [--seguir] | resumo | evidencia JOB NUMERO PASTA
//! raven-ci aplicar ARQUIVO [--remover] [--servidor URL]
//! raven-ci servico instalar|remover|estado [--memoria 1G] [--endereco HOST:PORTA]
//! raven-ci agentes listar|token NOME|remover NOME|modelos|modelo NOME …
//! raven-ci <comando de um plugin> ...
//! raven-ci --self-test
//! raven-ci --versao
//! ```
//!
//! The data is in `--dados PASTA`, `RAVEN_CI_HOME` or `~/.raven-ci`; external plugins are found in
//! its `plugins/` folder and in each `--plugins PASTA`.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a tool run from the terminal")]

mod agentes;
mod apply;
mod backup;
mod chave;
mod cli;
mod cliente;
mod compilar;
mod ia;
mod mcp;
mod plugins;
mod plugins_no_servidor;
mod remote;
mod saida;
mod self_test;
mod server;
mod service;
mod versao;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The installer proves a new binary opens with this.
    if args.iter().any(|a| a == "--versao") {
        println!("{}", versao::VERSION);
        return;
    }
    if args.iter().any(|a| a == "--self-test") {
        match self_test::run() {
            Ok(said) => println!("{said}"),
            Err(why) => {
                eprintln!("o autoteste falhou: {why}");
                std::process::exit(1);
            }
        }
        return;
    }
    let mut out = std::io::stdout();
    let code = match cli::Invocation::parse(&args) {
        Ok(invocation) => invocation.run(&mut out).unwrap_or_else(|why| {
            eprintln!("{why}");
            1
        }),
        Err(why) => {
            eprintln!("{why}");
            eprintln!("{}", cli::USAGE);
            2
        }
    };
    std::process::exit(code);
}
