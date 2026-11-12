//! O centro de cursos usa o mesmo pacote com outra etapa e outra regra, sobre os dados sintéticos.
#![allow(clippy::expect_used)]

use contrato_cursos::{COORDENACAO, entidades, matriculas};
use ravenrs_app::{Access, Identity, Operation, Role, SpaceError};
use ravenrs_app_requests::{APPROVED, DECISION, Decided, Decision, OUTBOX, Outcome, REQUEST};
use ravenrs_app_testkit::Fixture;

fn pessoa(nome: &str, papel: &str) -> Identity {
    let todas = [Operation::Read, Operation::Write, Operation::Create];
    let papel = Role::new(papel)
        .type_permission(REQUEST, todas, Access::Allow)
        .type_permission(DECISION, todas, Access::Allow)
        .type_permission(OUTBOX, todas, Access::Allow);
    Identity::new(nome, nome, vec![papel])
}

fn decisao(id: &str, pedido: i64) -> Decision {
    Decision {
        id: id.to_owned(),
        request: pedido,
        version: 1,
        outcome: Outcome::Approve,
        reason: "Vaga".to_owned(),
    }
}

#[test]
fn uma_etapa_basta_e_quem_pede_pode_ser_da_coordenacao() {
    let dados = Fixture::memory(entidades().expect("entidades")).expect("dados");
    let pacote = matriculas().expect("pacote");
    let coordenador = pessoa("coordenador", COORDENACAO);
    let mut espaco = dados.t1.space_as(coordenador.clone());
    pacote.submit(&mut espaco, &coordenador, 1, "Matrícula tardia", "U1").expect("aberto");
    let decidido = pacote.decide(&mut espaco, "T1", &coordenador, &decisao("c1", 1), 0);
    assert_eq!(decidido.expect("autorizado"), Decided::Recorded { state: APPROVED.to_owned() });
}

#[test]
fn um_pedido_sem_polo_nao_e_autorizado() {
    let dados = Fixture::memory(entidades().expect("entidades")).expect("dados");
    let pacote = matriculas().expect("pacote");
    let coordenador = pessoa("coordenador", COORDENACAO);
    let mut espaco = dados.t1.space_as(coordenador.clone());
    pacote.submit(&mut espaco, &coordenador, 1, "Matrícula tardia", "").expect("aberto");
    let recusado = pacote.decide(&mut espaco, "T1", &coordenador, &decisao("c1", 1), 0);
    assert!(matches!(recusado, Err(SpaceError::Denied(_))));
}
