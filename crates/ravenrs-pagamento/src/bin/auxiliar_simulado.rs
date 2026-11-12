//! A helper process with a simulated provider inside, to test the contract over real pipes.
//!
//! With the argument `morrer-ao-confirmar`, it exits with no answer on the first confirmation. It
//! exists for tests. It has no library, no card and no value.

use std::io::{BufRead, Write};

use ravenrs_pagamento::auxiliar::atender;
use ravenrs_pagamento::simulador::SimuladorProvedor;

fn main() -> std::io::Result<()> {
    let morrer = std::env::args().any(|argumento| argumento == "morrer-ao-confirmar");
    let mut provedor = SimuladorProvedor::new();
    let mut saida = std::io::stdout().lock();
    for linha in std::io::stdin().lock().lines() {
        let linha = linha?;
        if morrer && linha.contains("\"confirmar\"") {
            std::process::exit(9);
        }
        writeln!(saida, "{}", atender(&mut provedor, &linha))?;
        saida.flush()?;
    }
    Ok(())
}
