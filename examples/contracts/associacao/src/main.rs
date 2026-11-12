//! Uma compra da associação passando pelas duas etapas, com o aviso de cada decisão.

use std::cell::RefCell;

use contrato_associacao::{CONSELHO, DIRETORIA, compras, entidades};
use ravenrs_app::{CAN_TELL_STATUS, EventSink, Identity, MemoryDatabase, Provider, Role, SpaceError};
use ravenrs_app_requests::{Decision, Dispatcher, Outcome};

/// Avisos da associação: aqui só escritos na tela.
#[derive(Default)]
struct Avisos(RefCell<Vec<String>>);

impl Provider for Avisos {
    fn name(&self) -> &str {
        "avisos"
    }

    fn capabilities(&self) -> Vec<String> {
        vec![CAN_TELL_STATUS.to_owned()]
    }
}

impl EventSink for Avisos {
    fn send(&self, chave: &str, conteudo: &str) -> Result<(), SpaceError> {
        println!("  aviso {chave}: {conteudo}");
        self.0.borrow_mut().push(chave.to_owned());
        Ok(())
    }

    fn status(&self, chave: &str) -> Result<bool, SpaceError> {
        Ok(self.0.borrow().iter().any(|enviado| enviado == chave))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let banco = MemoryDatabase::new(entidades()?)?;
    let mut espaco = banco.space();
    let pacote = compras()?;
    // A tesoureira também é do conselho, e mesmo assim não decide o que ela mesma pediu.
    let tesoureira = Identity::new("tesoureira", "tesoureira", vec![Role::new(CONSELHO)]);
    let conselheiro = Identity::new("conselheiro", "conselheiro", vec![Role::new(CONSELHO)]);
    let diretora = Identity::new("diretora", "diretora", vec![Role::new(DIRETORIA)]);

    pacote.submit(&mut espaco, &tesoureira, 1, "Cadeiras para o salão", "Sede")?;
    println!("Pedido 1 aberto pela tesoureira.");
    let recusa = pacote.decide(&mut espaco, "associacao", &tesoureira, &decisao("a1", 1), 0);
    println!("A tesoureira tenta decidir o próprio pedido: {recusa:?}");
    println!("Conselho: {:?}", pacote.decide(&mut espaco, "associacao", &conselheiro, &decisao("a2", 1), 0)?);
    println!("Diretoria: {:?}", pacote.decide(&mut espaco, "associacao", &diretora, &decisao("a3", 2), 0)?);

    let avisos = Avisos::default();
    let relatorio = Dispatcher::new("associacao", 3).run(&mut espaco, &avisos)?;
    println!("Avisos confirmados: {}", relatorio.confirmed);
    Ok(())
}

fn decisao(id: &str, versao: i64) -> Decision {
    Decision {
        id: id.to_owned(),
        request: 1,
        version: versao,
        outcome: Outcome::Approve,
        reason: "Orçamento conferido".to_owned(),
    }
}
