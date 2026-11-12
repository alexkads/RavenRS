//! A associação usa o pacote com suas etapas e sua regra, sobre os dados sintéticos.
#![allow(clippy::expect_used)]

use contrato_associacao::{CONSELHO, DIRETORIA, compras, entidades};
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

fn decisao(id: &str, versao: i64) -> Decision {
    Decision {
        id: id.to_owned(),
        request: 1,
        version: versao,
        outcome: Outcome::Approve,
        reason: String::new(),
    }
}

#[test]
fn a_compra_passa_pelo_conselho_e_pela_diretoria() {
    let dados = Fixture::memory(entidades().expect("entidades")).expect("dados");
    let pacote = compras().expect("pacote");
    // Ela é do conselho, e mesmo assim não decide o que ela mesma pediu.
    let tesoureira = pessoa("tesoureira", CONSELHO);
    pacote
        .submit(&mut dados.t1.space_as(tesoureira.clone()), &tesoureira, 1, "Cadeiras", "U1")
        .expect("aberto");

    let propria =
        pacote.decide(&mut dados.t1.space_as(tesoureira.clone()), &tesoureira, &decisao("a1", 1), 0);
    assert!(matches!(propria, Err(SpaceError::Denied(_))));

    let conselheiro = pessoa("conselheiro", CONSELHO);
    let conselho =
        pacote.decide(&mut dados.t1.space_as(conselheiro.clone()), &conselheiro, &decisao("a2", 1), 0);
    assert_eq!(conselho.expect("conferido"), Decided::Recorded { state: "diretoria".to_owned() });

    let diretora = pessoa("diretora", DIRETORIA);
    let diretoria = pacote.decide(&mut dados.t1.space_as(diretora.clone()), &diretora, &decisao("a3", 2), 0);
    assert_eq!(diretoria.expect("aprovado"), Decided::Recorded { state: APPROVED.to_owned() });
}
