//! The server of the tests as the panel reads it: what each page asks for answers, a build asked
//! for goes to the queue, and a reader may not act on builds.
#![allow(clippy::expect_used, reason = "tests fail loudly")]
#![cfg(not(target_arch = "wasm32"))]

use std::cell::RefCell;
use std::rc::Rc;

use raven_ci_painel::RemoteClient;
use raven_ci_painel::e2e::{PASSWORD, folder, server};
use serde_json::{Value, json};

fn signed(client: &RemoteClient, name: &str) {
    client.sign_in(name, PASSWORD, |answer| {
        answer.expect("signed in");
    });
}

fn get(client: &RemoteClient, path: &str) -> (u16, Value) {
    let got = Rc::new(RefCell::new((0, Value::Null)));
    let keep = got.clone();
    client.get_json(path, move |answer| *keep.borrow_mut() = answer.expect("an answer"));
    got.borrow().clone()
}

fn post(client: &RemoteClient, path: &str, body: Value) -> (u16, Value) {
    let got = Rc::new(RefCell::new((0, Value::Null)));
    let keep = got.clone();
    client.send_json("POST", path, &body, move |answer| *keep.borrow_mut() = answer.expect("an answer"));
    got.borrow().clone()
}

#[test]
fn every_page_reads_what_it_shows() {
    let dir = folder("paginas");
    let (_ci, client) = server(&dir).expect("the server of the tests");
    signed(&client, "ana");
    for path in [
        "/api/me",
        "/api/ci/server",
        "/api/ci/screens",
        "/api/ci/overview",
        "/api/ci/occupancy?since_ms=0",
        "/api/ci/jobs",
        "/api/ci/plugins",
        "/api/ci/credentials",
        "/api/ci/users",
        "/api/ci/audit?limit=5",
        "/api/ci/build?job=apps%2Fsite&number=2",
        "/api/ci/log?job=apps%2Fsite&number=2&after=0",
        "/api/ci/demonstracao/travas",
    ] {
        let (status, body) = get(&client, path);
        assert_eq!(status, 200, "{path}: {body}");
    }
    let (_, overview) = get(&client, "/api/ci/overview");
    assert_eq!(overview["jobs"].as_array().map(Vec::len), Some(5), "{overview}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_build_asked_for_goes_to_the_queue_and_a_reader_may_not_act() {
    let dir = folder("disparo");
    let (_ci, client) = server(&dir).expect("the server of the tests");
    signed(&client, "bia");
    let (status, said) = post(&client, "/api/ci/run", json!({ "job": "imagens-base", "parameters": {} }));
    assert_eq!(status, 201, "{said}");
    assert_eq!(said["number"], 1);
    let (_, overview) = get(&client, "/api/ci/overview");
    assert_eq!(overview["queued"], 1, "{overview}");
    let reader = server(&folder("disparo-leitor")).expect("another").1;
    signed(&reader, "caio");
    let (status, _) = post(&reader, "/api/ci/rebuild", json!({ "job": "apps/site", "number": 1 }));
    assert_eq!(status, 403);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(folder("disparo-leitor"));
}

/// The panel opened on `page`, not signed in, over the core of the tests in `dir`, and the core.
fn opened_on(dir: &std::path::Path, page: raven_ci_painel::Pagina) -> ravenrs_e2e::E2e {
    let (_ci, client) = server(dir).expect("the server of the tests");
    let painel = raven_ci_painel::Painel::new(client, false).abrir(page);
    ravenrs_e2e::E2e::new(raven_ci_painel::app_of(painel), ravenrs::prelude::Theme::raven_dark())
        .viewport(ravenrs::prelude::Size::new(1440.0, 900.0), 1.0)
}

/// Signs in as bia, then waits for `shown`, as a scenario of Raven Test Studio does.
fn sign_in_and_see(session: &mut ravenrs_e2e::E2e, shown: &str) {
    let text = format!(
        "scenario: Raven CI: abrir pelo endereço\n\
         open Raven CI | Abrir o painel no endereço de um aviso\n\
         fill Usuário = bia | Escrever o usuário\n\
         fill Senha = {PASSWORD} | Escrever a senha\n\
         click Entrar | Entrar\n\
         wait {shown} | A página do endereço abre\n"
    );
    let scenario = ravenrs_e2e_wire::Scenario::parse(&text).expect("the scenario reads");
    scenario.play(session).expect("the scenario plays");
}

#[test]
fn the_link_of_a_notice_opens_its_build_once_someone_signs_in() {
    let dir = folder("endereco");
    let (ci, _) = server(&folder("endereco-nucleo")).expect("the server of the tests");
    ci.store().set_setting("url", "http://ci.exemplo:8090").expect("the address");
    // What the core links to is what the panel opens.
    let link = ci.build_url("apps/site", 2).expect("a link");
    assert_eq!(link, "http://ci.exemplo:8090/painel/#/build/apps%2Fsite/2");
    let page = raven_ci_painel::Pagina::from_path(&link).expect("the panel reads the link");
    assert_eq!(page, raven_ci_painel::Pagina::Build("apps/site".to_owned(), 2));
    let mut session = opened_on(&dir, page);
    sign_in_and_see(&mut session, "Build #2");
    drop(session);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(folder("endereco-nucleo"));
}

#[test]
fn the_link_to_a_build_that_is_not_there_lands_on_the_jobs_saying_so() {
    let dir = folder("endereco-sem-build");
    let mut session = opened_on(&dir, raven_ci_painel::Pagina::Build("apps/site".to_owned(), 99));
    sign_in_and_see(&mut session, "Não há o build #99 de apps/site.");
    drop(session);
    let _ = std::fs::remove_dir_all(&dir);
}
