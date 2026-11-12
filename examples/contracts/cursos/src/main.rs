//! Uma matrícula fora do prazo autorizada pela coordenação, com o aviso ao aluno.

use std::cell::RefCell;

use contrato_cursos::{COORDENACAO, entidades, matriculas};
use ravenrs_app::{CAN_TELL_STATUS, EventSink, Identity, MemoryDatabase, Provider, Role, SpaceError};
use ravenrs_app_requests::{Decision, Dispatcher, Outcome};

/// Mensagens ao aluno: aqui só escritas na tela.
#[derive(Default)]
struct Mensagens(RefCell<Vec<String>>);

impl Provider for Mensagens {
    fn name(&self) -> &str {
        "mensagens"
    }

    fn capabilities(&self) -> Vec<String> {
        vec![CAN_TELL_STATUS.to_owned()]
    }
}

impl EventSink for Mensagens {
    fn send(&self, chave: &str, conteudo: &str) -> Result<(), SpaceError> {
        println!("  mensagem {chave}: {conteudo}");
        self.0.borrow_mut().push(chave.to_owned());
        Ok(())
    }

    fn status(&self, chave: &str) -> Result<bool, SpaceError> {
        Ok(self.0.borrow().iter().any(|enviada| enviada == chave))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let banco = MemoryDatabase::new(entidades()?)?;
    let mut espaco = banco.space();
    let pacote = matriculas()?;
    let secretaria = Identity::new("secretaria", "secretaria", vec![Role::new("Secretaria")]);
    let coordenador = Identity::new("coordenador", "coordenador", vec![Role::new(COORDENACAO)]);

    pacote.submit(&mut espaco, &secretaria, 1, "Matrícula tardia em Desenho Técnico", "", 0)?;
    println!("Pedido 1 aberto pela secretaria, sem polo.");
    let sem_polo = pacote.decide(&mut espaco, "cursos", &coordenador, &decisao("c1"), 0);
    println!("A coordenação tenta autorizar sem polo: {sem_polo:?}");

    pacote.submit(&mut espaco, &secretaria, 2, "Matrícula tardia em Informática", "Polo Centro", 0)?;
    let autorizada = Decision { request: 2, ..decisao("c2") };
    println!("Coordenação: {:?}", pacote.decide(&mut espaco, "cursos", &coordenador, &autorizada, 0)?);

    let mensagens = Mensagens::default();
    let relatorio = Dispatcher::new("cursos", 3).run(&mut espaco, &mensagens)?;
    println!("Mensagens confirmadas: {}", relatorio.confirmed);
    Ok(())
}

fn decisao(id: &str) -> Decision {
    Decision {
        id: id.to_owned(),
        request: 1,
        version: 1,
        outcome: Outcome::Approve,
        reason: "Vaga disponível".to_owned(),
    }
}
