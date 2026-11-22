//! `raven-ci --self-test`: a core in memory with a fake plugin for every extension point, each
//! plugged in and used as the core uses it, and builds run through them.

use std::path::Path;
use std::sync::Arc;

use raven_ci_core::{Ci, JobSpec, PluginState};
use raven_ci_plugin_api::testing::Fake;
use raven_ci_plugin_api::{
    BuildEvent, BuildInfo, BuildRequest, BuildStatus, CheckoutRequest, CiError, CommitState, CommitStatus,
    HttpRequest, LaunchRequest, Reference,
};
use serde_json::json;

fn check(ok: bool, what: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(what.to_owned()) }
}

fn said(e: CiError) -> String {
    e.to_string()
}

/// Runs it; what it checked, or what failed.
pub fn run() -> Result<String, String> {
    let ci = Arc::new(Ci::in_memory().map_err(said)?);
    let fakes = Fake::all();
    let probes: Vec<_> = fakes.iter().map(Fake::probe).collect();
    let count = fakes.len();
    for fake in fakes {
        ci.add_plugin(Arc::new(fake));
    }
    ci.apply_plugins().map_err(said)?;
    for view in ci.registry().list().map_err(said)? {
        check(
            view.state == PluginState::On,
            &format!("o plugin {} não ligou: {}", view.name, view.state.words()),
        )?;
        check(view.points.len() == 1, &format!("o plugin {} não oferece o seu ponto", view.name))?;
    }
    let on = ci.registry().on();

    // Step and pipeline format: a build that passes and one that fails, asked by the trigger.
    ci.save_job(JobSpec::new("autoteste/passa", "fake-lines", "Build: compila\nTeste: testa"))
        .map_err(said)?;
    ci.save_job(JobSpec::new("autoteste/falha", "fake-lines", "Build: compila\nTeste: !fail"))
        .map_err(said)?;
    let trigger = on.iter().flat_map(|p| p.triggers()).next().ok_or("sem gatilho")?;
    trigger.watch("autoteste/passa", &json!({}), ci.clone()).map_err(said)?;
    let trigger_probe = probes.iter().find(|p| !p.watched().is_empty()).ok_or("o gatilho não vigia o job")?;
    let passed = trigger_probe.fire("autoteste/passa").map_err(said)?;
    let failed = ci.request_build(BuildRequest::new("autoteste/falha", "autoteste")).map_err(said)?.number;
    ci.work_until_idle("autoteste").map_err(said)?;
    let build = ci.build("autoteste/passa", passed).map_err(said)?;
    check(build.status == BuildStatus::Succeeded, &format!("o build que passa ficou {:?}", build.status))?;
    let stages = ci.stages("autoteste/passa", passed).map_err(said)?;
    check(
        stages.len() == 2 && stages.iter().all(|s| s.status == "done"),
        "os estágios do build não saíram certos",
    )?;
    let build = ci.build("autoteste/falha", failed).map_err(said)?;
    check(build.status == BuildStatus::Failed, &format!("o build que falha ficou {:?}", build.status))?;

    // Scm and branch source.
    let scm = on.iter().flat_map(|p| p.scms()).next().ok_or("sem scm")?;
    let request = CheckoutRequest {
        url: "https://example.com/autoteste.git".into(),
        reference: Reference::Branch("main".into()),
        workspace: Path::new(".").to_path_buf(),
        depth: Some(1),
        credential: None,
        merge_onto_target: false,
    };
    check(scm.checkout(&request, &|_| {}).map_err(said)?.commit == "f00d", "o scm não buscou")?;
    let source = on.iter().flat_map(|p| p.branch_sources()).next().ok_or("sem fonte de branches")?;
    check(
        source.discover(&json!({})).map_err(said)?.len() == 2,
        "a fonte de branches não achou main e PR-1",
    )?;

    // Publisher, status reporter, notifier: what the build leaves and who hears of it.
    let info =
        BuildInfo { job: build.job.clone(), number: failed, status: build.status, ..BuildInfo::default() };
    let publisher = on.iter().flat_map(|p| p.publishers()).next().ok_or("sem publicador")?;
    for attachment in publisher.publish(&info, &json!({}), Path::new(".")).map_err(said)? {
        ci.attach(&info.job, failed, &attachment).map_err(said)?;
    }
    let kept = ci.attachments(&info.job, failed).map_err(said)?;
    check(kept.len() == 1, "o anexo do publicador não ficou com o build")?;
    let expected = format!("autoteste/falha #{failed} falhou");
    check(
        ci.attachment_bytes(kept[0].id).map_err(said)? == expected.as_bytes(),
        "o anexo não guardou os bytes",
    )?;
    let status = CommitStatus {
        repository: "autoteste/autoteste".into(),
        commit: "f00d".into(),
        context: "raven-ci/autoteste".into(),
        state: CommitState::Failure,
        description: "falhou".into(),
        url: None,
    };
    on.iter()
        .flat_map(|p| p.status_reporters())
        .next()
        .ok_or("sem status")?
        .report(&status, &json!({}))
        .map_err(said)?;
    on.iter()
        .flat_map(|p| p.notifiers())
        .next()
        .ok_or("sem aviso")?
        .notify(&BuildEvent::Ended(info), &json!({}))
        .map_err(said)?;
    check(probes.iter().any(|p| p.statuses().len() == 1), "o status não chegou")?;
    check(probes.iter().any(|p| p.events().len() == 1), "o aviso não chegou")?;

    // Credential kind, agent launcher, route and command.
    let kind = on.iter().flat_map(|p| p.credential_kinds()).next().ok_or("sem tipo de credencial")?;
    check(kind.hidden(&json!({ "token": "abc" })) == ["abc"], "a credencial não esconde o token")?;
    let launcher = on.iter().flat_map(|p| p.agent_launchers()).next().ok_or("sem lançador de agentes")?;
    let agent = launcher
        .launch(&LaunchRequest {
            build: None,
            labels: Vec::new(),
            settings: json!({}),
            server: "http://localhost".into(),
            token: "t".into(),
        })
        .map_err(said)?;
    agent.stop();
    check(!agent.alive(), "o agente não parou")?;
    let route = on.iter().flat_map(|p| p.routes()).next().ok_or("sem rota")?;
    check((route.handler)(&HttpRequest::default()).status == 200, "a rota não respondeu")?;
    let command = on.iter().flat_map(|p| p.cli_commands()).next().ok_or("sem comando")?;
    let mut out = Vec::new();
    check(
        command.run(&["ok".into()], &mut out).map_err(said)? == 0 && out == b"fake: ok\n",
        "o comando não respondeu",
    )?;

    // Switching off and on again with no restart.
    ci.disable_plugin("fake-step").map_err(said)?;
    check(
        ci.request_build(BuildRequest::new("autoteste/passa", "autoteste")).is_err(),
        "sem o passo, o build não pode ser pedido",
    )?;
    ci.enable_plugin("fake-step").map_err(said)?;
    check(ci.request_build(BuildRequest::new("autoteste/passa", "autoteste")).is_ok(), "o passo não voltou")?;

    Ok(format!(
        "Autoteste: {count} plugins falsos, um por ponto de extensão, ligados e usados; um build passou, outro falhou como devia, e um plugin desligou e voltou sem reiniciar"
    ))
}
