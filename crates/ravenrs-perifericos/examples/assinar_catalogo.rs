//! Signs a catalog. The private key stays in a file of yours, never in the repository.
//!
//! `cargo run -p ravenrs-perifericos --example assinar_catalogo -- CHAVE CONTEUDO SAIDA`
//!
//! `CHAVE` holds 32 bytes in hexadecimal, as `openssl rand -hex 32` makes. `CONTEUDO` is the catalog's
//! JSON. It is opened with the same checks a client makes before anything is written to `SAIDA`.

use std::process::ExitCode;

use ravenrs_perifericos::{Catalogo, assinar, chave_publica_de, hoje_utc};

fn chave_de(texto: &str) -> Result<[u8; 32], String> {
    let texto = texto.trim();
    if texto.len() != 64 || !texto.is_ascii() {
        return Err("a chave precisa ter 64 dígitos hexadecimais".to_owned());
    }
    let mut chave = [0; 32];
    for (indice, byte) in chave.iter_mut().enumerate() {
        let par = texto.get(indice * 2..indice * 2 + 2).ok_or("chave inválida")?;
        *byte = u8::from_str_radix(par, 16).map_err(|_| "a chave tem caractere que não é hexadecimal")?;
    }
    Ok(chave)
}

fn executar() -> Result<(), String> {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    let [chave, conteudo, saida] = argumentos.as_slice() else {
        return Err("uso: assinar_catalogo CHAVE CONTEUDO SAIDA".to_owned());
    };
    let texto_chave = std::fs::read_to_string(chave).map_err(|erro| format!("{chave}: {erro}"))?;
    let chave = chave_de(&texto_chave)?;
    let conteudo = std::fs::read_to_string(conteudo).map_err(|erro| format!("{conteudo}: {erro}"))?;
    let assinado = assinar(&conteudo, &chave).map_err(|erro| erro.to_string())?;
    let publica = chave_publica_de(&chave);
    let catalogo =
        Catalogo::abrir(assinado.as_bytes(), &publica, &hoje_utc(), 0).map_err(|erro| erro.to_string())?;
    std::fs::write(saida, assinado).map_err(|erro| format!("{saida}: {erro}"))?;
    println!(
        "{saida}: catálogo {} com {} drivers, válido até {}",
        catalogo.sequencia(),
        catalogo.itens().len(),
        catalogo.validade()
    );
    println!("chave pública: {}", publica.para_hex());
    Ok(())
}

fn main() -> ExitCode {
    match executar() {
        Ok(()) => ExitCode::SUCCESS,
        Err(erro) => {
            eprintln!("{erro}");
            ExitCode::FAILURE
        }
    }
}
