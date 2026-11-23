//! The format `Jenkinsfile` (#909): what it converts, what it lists with the line, and the
//! converted pipeline run for real in the core with the step `sh`.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::sync::Arc;

use raven_ci_core::{Ci, JobSpec, PluginState};
use raven_ci_plugin_api::{BuildRequest, BuildStatus, CliCommand, PipelineFormat, STAGE_KEY};
use raven_ci_plugin_format_jenkinsfile::{
    ConvertCommand, JenkinsfileFormat, JenkinsfilePlugin, NoteKind, convert,
};
use raven_ci_plugin_format_toml::{TomlFormat, TomlFormatPlugin};
use raven_ci_plugin_shell::ShellPlugin;

const FULL: &str = r#"
// Build e testes do site
pipeline {
    agent { label 'linux' }
    environment {
        CARGO_TERM_COLOR = 'always'
    }
    parameters {
        booleanParam(name: 'E2E', defaultValue: false, description: 'Roda os testes de ponta a ponta')
        choice(name: 'ALVO', choices: ['web', 'desktop'], description: 'O que montar')
    }
    options {
        timeout(time: 30, unit: 'MINUTES')
        buildDiscarder(logRotator(numToKeepStr: '10'))
        timestamps()
    }
    triggers {
        cron('H 2 * * *')
    }
    stages {
        stage('Build') {
            steps {
                checkout scm
                sh 'cargo build'
                sh(script: 'cargo doc', label: 'Documentação')
            }
        }
        stage('Teste') {
            parallel {
                stage('unit') {
                    steps { sh 'cargo test' }
                }
                stage('e2e') {
                    steps {
                        dir('e2e') {
                            sh './run.sh'
                        }
                    }
                }
            }
        }
        stage('Publica') {
            when { branch 'main' }
            steps {
                timeout(time: 5, unit: 'MINUTES') {
                    sh "echo publicando ${params.ALVO} ${env.BUILD_NUMBER}"
                }
            }
        }
    }
    post {
        always {
            junit '**/junit.xml'
            archiveArtifacts artifacts: 'target/*.tar.gz', fingerprint: true
        }
        failure {
            echo 'falhou'
        }
    }
}
"#;

fn sh_steps(pipeline: &raven_ci_plugin_api::Pipeline) -> Vec<(String, String, Option<u64>)> {
    pipeline
        .flow
        .nodes
        .iter()
        .filter(|n| n.kind == "sh")
        .map(|n| {
            (
                n.parameters.get(STAGE_KEY).and_then(|v| v.as_str()).unwrap_or_default().to_owned(),
                n.parameters.get("script").and_then(|v| v.as_str()).unwrap_or_default().to_owned(),
                n.timeout_ms,
            )
        })
        .collect()
}

#[test]
fn a_declarative_pipeline_becomes_a_toml_that_format_toml_reads_with_the_same_steps() {
    let conversion = convert(FULL);
    assert!(conversion.complete(), "{:#?}", conversion.notes);
    let pipeline = TomlFormat.read("site/main", &conversion.toml).expect(&conversion.toml);

    assert_eq!(pipeline.timeout_ms, Some(30 * 60_000));
    assert_eq!(pipeline.keep_builds, Some(10));
    assert_eq!(pipeline.environment.get("CARGO_TERM_COLOR").map(String::as_str), Some("always"));
    let names: Vec<&str> = pipeline.parameters.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["E2E", "ALVO"]);
    assert_eq!(
        sh_steps(&pipeline),
        [
            ("Build".to_owned(), "cargo build".to_owned(), None),
            ("Build".to_owned(), "cargo doc".to_owned(), None),
            ("Teste / unit".to_owned(), "cargo test".to_owned(), None),
            ("Teste / e2e".to_owned(), "cd e2e && ./run.sh".to_owned(), None),
            ("Publica".to_owned(), "echo publicando ${ALVO} ${BUILD_NUMBER}".to_owned(), Some(5 * 60_000)),
        ]
    );
    let publishers: Vec<&str> = pipeline.publishers.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(publishers, ["artifacts", "junit"]);
    assert_eq!(pipeline.post.len(), 1, "always only had publishers; failure has its echo");

    let ignored: Vec<&str> =
        conversion.notes.iter().filter(|n| n.kind == NoteKind::Ignored).map(|n| n.what.as_str()).collect();
    assert_eq!(
        ignored,
        ["timestamps()", "cron('H 2 * * *')", "checkout scm", "archiveArtifacts: fingerprint"]
    );
    assert!(conversion.toml.contains("ponha cron = \"H 2 * * *\" no job"), "{}", conversion.toml);
}

#[test]
fn a_stage_outside_the_branch_is_switched_off_as_in_the_hand_written_toml() {
    let pipeline = JenkinsfileFormat.read("site/PR-3", FULL).expect("reads");
    let publish =
        pipeline.flow.nodes.iter().find(|n| {
            n.parameters.get(STAGE_KEY).and_then(|v| v.as_str()) == Some("Publica") && n.kind == "sh"
        });
    assert!(publish.expect("the step").disabled);
}

fn ci() -> Ci {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(JenkinsfilePlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.apply_plugins().expect("on");
    ci
}

fn run(ci: &Ci, job: &str, format: &str, text: &str) -> (BuildStatus, Vec<String>) {
    ci.save_job(JobSpec::new(job, format, text).every_request()).expect("saved");
    let number = ci
        .request_build(BuildRequest::new(job, "teste").parameter("NOME", "Ana".into()))
        .expect("asked")
        .number;
    ci.work_until_idle("e1").expect("runs");
    let log =
        ci.build_log(job, number, 0).expect("log").into_iter().map(|l| l.text.trim().to_owned()).collect();
    (ci.build(job, number).expect("build").status, log)
}

#[test]
fn the_converted_pipeline_runs_in_the_core_with_the_same_log_as_the_toml_written_by_hand() {
    let jenkinsfile = r#"
pipeline {
    agent any
    parameters { string(name: 'NOME', defaultValue: 'mundo') }
    stages {
        stage('Diz') {
            steps {
                echo 'oi'
                sh "echo ola ${params.NOME}"
            }
        }
    }
}
"#;
    let by_hand = r#"
[[parameters]]
name = "NOME"
type = "text"
default = "mundo"

[[stages]]
name = "Diz"
steps = ["echo oi", "echo ola ${NOME}"]
"#;
    if cfg!(windows) {
        // `${NOME}` is a variable of sh, not of cmd.
        return;
    }
    let ci = ci();
    let (status, from_jenkins) = run(&ci, "a", "jenkinsfile", jenkinsfile);
    assert_eq!(status, BuildStatus::Succeeded, "{from_jenkins:?}");
    let (_, from_toml) = run(&ci, "b", "toml", by_hand);
    let said = |log: &[String]| log.iter().filter(|l| l.starts_with("o")).cloned().collect::<Vec<_>>();
    assert_eq!(said(&from_jenkins), ["oi", "ola Ana"]);
    assert_eq!(said(&from_jenkins), said(&from_toml));
}

const PENDING: &str = r#"pipeline {
    agent any
    stages {
        stage('Build') {
            steps {
                sh 'make'
            }
        }
        stage('Groovy') {
            steps {
                sh 'echo antes'
                script {
                    def v = readFile('VERSION').trim()
                    echo "versão ${v}"
                }
                sh "echo ${currentBuild.number}"
            }
        }
        stage('Segredo') {
            steps {
                withCredentials([string(credentialsId: 'token', variable: 'T')]) {
                    sh 'deploy'
                }
            }
        }
    }
}
"#;

#[test]
fn what_it_cannot_convert_is_listed_on_its_line_and_the_rest_is_written() {
    let conversion = convert(PENDING);
    assert!(!conversion.complete());
    let pending: Vec<(usize, &str)> = conversion.pending().map(|n| (n.line, n.what.as_str())).collect();
    assert_eq!(pending, [(12, "script { }"), (16, "${currentBuild.number}")]);
    // The rest is converted, and the notes are comments the TOML still reads.
    let pipeline = TomlFormat.read("x", &conversion.toml).expect(&conversion.toml);
    let scripts: Vec<String> = sh_steps(&pipeline).into_iter().map(|(_, s, _)| s).collect();
    assert_eq!(scripts, ["make", "echo antes", "deploy"]);
    assert!(conversion.toml.contains("# linha 12:17, não convertido: script { }"), "{}", conversion.toml);
    // withCredentials is converted (#922): the step names the credential, which the note says.
    assert!(conversion.toml.contains("credentials = { T = \"token\" }"), "{}", conversion.toml);
    let note = conversion.notes.iter().find(|n| n.what == "withCredentials").expect("a note");
    assert_eq!(note.kind, NoteKind::Warning);
    assert!(note.why.contains("tem que existir no cofre: token"), "{}", note.why);
    let node = pipeline
        .flow
        .nodes
        .iter()
        .find(|n| n.parameters.get("script") == Some(&serde_json::json!("deploy")))
        .expect("deploy");
    assert_eq!(node.parameters["credentials"], serde_json::json!({ "T": "token" }));
}

#[test]
fn as_a_format_a_part_not_converted_refuses_the_job_on_each_line() {
    let errors = JenkinsfileFormat.read("x", PENDING).expect_err("refused");
    let lines: Vec<Option<usize>> = errors.iter().map(|e| e.line).collect();
    assert_eq!(lines, [Some(12), Some(16)]);
    assert!(errors[0].message.contains("script"), "{errors:?}");
    // And the core says it when the job is saved or built.
    let ci = ci();
    let refused = ci
        .save_job(JobSpec::new("x", "jenkinsfile", PENDING))
        .and_then(|()| ci.request_build(BuildRequest::new("x", "teste")).map(|_| ()));
    assert!(refused.is_err());
}

#[test]
fn a_scripted_pipeline_or_a_file_that_does_not_close_is_said_with_its_line() {
    let scripted = convert("node {\n  stage('a') { sh 'x' }\n}\n");
    assert_eq!(scripted.pending().map(|n| (n.line, n.what.as_str())).collect::<Vec<_>>(), [(1, "node { }")]);

    let open = convert("pipeline {\n  agent any\n  stages {\n    stage('a') {\n      steps { sh 'x\n");
    assert!(!open.complete());
    assert!(open.pending().any(|n| n.line == 5 && n.why.contains("não fecha")), "{:#?}", open.notes);

    let nothing = convert("// vazio\n");
    assert_eq!(
        nothing.pending().map(|n| n.why.as_str()).collect::<Vec<_>>(),
        ["não achei o bloco pipeline { }"]
    );
}

#[test]
fn the_parts_raven_ci_does_alone_are_ignored_with_why_and_do_not_refuse() {
    let text = r#"
pipeline {
    agent any
    options { ansiColor('xterm') }
    stages {
        stage('a') {
            steps {
                checkout scm
                sh 'make'
                cleanWs()
            }
        }
    }
}
"#;
    let conversion = convert(text);
    assert!(conversion.complete(), "{:#?}", conversion.notes);
    let ignored: Vec<(&str, &str)> =
        conversion.notes.iter().map(|n| (n.what.as_str(), n.why.as_str())).collect();
    assert_eq!(
        ignored,
        [
            ("ansiColor('xterm')", "o visualizador de log já mostra as cores"),
            ("checkout scm", "o núcleo busca o código antes dos passos"),
            ("cleanWs()", "o plugin workspace-cleanup apaga a pasta do build no fim"),
        ]
    );
    assert!(JenkinsfileFormat.read("a", text).is_ok());
}

#[test]
fn a_docker_agent_stage_options_and_when_become_their_toml_fields() {
    let text = r#"
pipeline {
    agent { docker { image 'rust:1'; args '-v /cache:/cache' } }
    stages {
        stage('Teste') {
            when {
                expression { return params.E2E }
                changeRequest()
            }
            options {
                timeout(time: 10, unit: 'MINUTES')
                retry(3)
                lock('maquina')
            }
            steps { sh 'cargo test' }
        }
        stage('Lint') {
            agent { docker 'node:22' }
            steps { sh 'npm run lint' }
        }
    }
}
"#;
    let conversion = convert(text);
    assert!(conversion.complete(), "{:#?}", conversion.notes);
    let toml = &conversion.toml;
    assert!(toml.contains(r#"when = { pull_request = true, parameter = "E2E" }"#), "{toml}");
    assert!(toml.contains("timeout = \"10m\"\nretry = 2\nlock = \"maquina\""), "{toml}");
    assert!(toml.contains(r#"container = { image = "rust:1", args = "-v /cache:/cache" }"#), "{toml}");
    assert!(toml.contains(r#"container = { image = "node:22" }"#), "{toml}");
    let warnings = conversion.notes.iter().filter(|n| n.kind == NoteKind::Warning).count();
    assert_eq!(warnings, 2, "the timeout and the retry of a stage go to each step");
    TomlFormat.read("x/PR-1", toml).expect(toml);
}

fn command(args: &[&str]) -> (i32, String) {
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let mut out = Vec::new();
    let code = ConvertCommand.run(&args, &mut out).expect("runs");
    (code, String::from_utf8(out).expect("utf-8"))
}

#[test]
fn the_command_writes_the_file_refuses_to_overwrite_and_says_how_it_is_used() {
    let folder = std::env::temp_dir().join(format!("raven-ci-jenkinsfile-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("folder");
    let jenkinsfile = folder.join("Jenkinsfile");
    let toml = folder.join(".raven-ci.toml");
    std::fs::write(&jenkinsfile, FULL).expect("written");
    let (input, output) = (jenkinsfile.to_string_lossy().into_owned(), toml.to_string_lossy().into_owned());

    let (code, said) = command(&[]);
    assert_eq!(code, 2);
    assert!(said.contains("uso: raven-ci converter"), "{said}");

    let (code, said) = command(&[&input]);
    assert_eq!(code, 0);
    assert!(said.starts_with("# Convertido de um Jenkinsfile"), "{said}");

    let (code, said) = command(&[&input, "--saida", &output]);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("0 não convertido(s)"), "{said}");
    let written = std::fs::read_to_string(&toml).expect("the toml");

    std::fs::write(&jenkinsfile, PENDING).expect("written");
    let (code, said) = command(&[&input, "--saida", &output]);
    assert_eq!(code, 1);
    assert!(said.contains("já existe"), "{said}");
    assert_eq!(std::fs::read_to_string(&toml).expect("the toml"), written, "nothing was written");

    let (code, said) = command(&[&input, "--saida", &output, "--substituir"]);
    assert_eq!(code, 1, "a part was not converted");
    assert!(said.contains("2 não convertido(s)"), "{said}");

    std::fs::remove_dir_all(&folder).expect("removed");
}

#[test]
fn without_format_toml_format_jenkinsfile_stays_out_with_why() {
    let alone = Ci::in_memory().expect("a core");
    alone.add_plugin(Arc::new(JenkinsfilePlugin));
    alone.apply_plugins().expect("applied");
    let state = alone.registry().info("format-jenkinsfile").expect("listed").state;
    assert!(matches!(&state, PluginState::Refused(why) if why.contains("format-toml")), "{state:?}");

    let ci = ci();
    let changes = ci.disable_plugin("format-toml").expect("switched off");
    assert!(changes.iter().any(|c| c.contains("format-jenkinsfile")), "{changes:?}");
    assert_ne!(ci.registry().info("format-jenkinsfile").expect("listed").state, PluginState::On);
}

/// The `Jenkinsfile` of a real repository (the e-commerce's, at the Jenkins of the oficina): its
/// options, `post { cleanup { deleteDir() } }` and a function of its own (#803, test of the real
/// world).
const REAL: &str = r##"
pipeline {
  agent { label 'oficina' }
  options {
    buildDiscarder(logRotator(numToKeepStr: '10', daysToKeepStr: '30'))
    timeout(time: 3, unit: 'HOURS')
  }
  stages {
    stage('api-lint') {
      steps { sh 'make api-lint' }
    }
  }
  post {
    cleanup { deleteDir() }
  }
}

def naOficina(String comando) {
  sh """#!/usr/bin/env bash
nice -n 10 ${comando}
"""
}
"##;

#[test]
fn days_to_keep_goes_to_the_job_and_cleanup_with_delete_dir_needs_nothing() {
    let conversion = convert(REAL);
    let notes: Vec<(NoteKind, usize, &str, &str)> =
        conversion.notes.iter().map(|n| (n.kind, n.line, n.what.as_str(), n.why.as_str())).collect();
    assert!(
        notes.contains(&(
            NoteKind::Ignored,
            5,
            "logRotator: daysToKeepStr",
            "ponha keep_days = 30 no job: o pipeline só guarda o keep",
        )),
        "{notes:#?}"
    );
    assert!(notes.iter().any(|n| n.0 == NoteKind::Ignored && n.2 == "deleteDir()"), "{notes:#?}");
    assert!(!notes.iter().any(|n| n.2.contains("cleanup")), "{notes:#?}");
    // The function of the file is said at its `def`, by its name, and is the one part left.
    let pending: Vec<&raven_ci_plugin_format_jenkinsfile::Note> = conversion.pending().collect();
    assert_eq!(pending.len(), 1, "{pending:#?}");
    assert_eq!((pending[0].line, pending[0].column), (18, 1));
    assert!(pending[0].why.starts_with("def naOficina é código Groovy"), "{}", pending[0].why);
    assert!(conversion.toml.contains("keep = 10"), "{}", conversion.toml);
}

#[test]
fn cleanup_joins_always_in_one_key_that_reads() {
    let text = r#"
pipeline {
  agent any
  stages {
    stage('a') { steps { sh 'make' } }
  }
  post {
    always { sh './coleta.sh' }
    cleanup { sh './limpa.sh' }
  }
}
"#;
    let conversion = convert(text);
    assert!(conversion.complete(), "{:#?}", conversion.notes);
    assert_eq!(conversion.toml.matches("always =").count(), 1, "{}", conversion.toml);
    let always = conversion.toml.split_once("always =").map_or("", |(_, after)| after);
    let (coleta, limpa) = (always.find("coleta").unwrap_or(usize::MAX), always.find("limpa").unwrap_or(0));
    assert!(coleta < limpa, "{always}");
    assert!(conversion.notes.iter().any(|n| n.kind == NoteKind::Warning && n.what == "post { cleanup }"));
    assert!(TomlFormat.read("a", &conversion.toml).is_ok(), "{}", conversion.toml);
}

/// Every kind of withCredentials the converter knows (#922): a token, a user and a password
/// apart, a secret file and an SSH key; the TOML it writes reads, and each step gets them.
#[test]
fn with_credentials_becomes_the_credentials_of_the_steps_inside_it() {
    let text = r#"
pipeline {
    agent any
    stages {
        stage('Publica') {
            steps {
                sh 'antes'
                withCredentials([
                    string(credentialsId: 'npm', variable: 'NPM_TOKEN'),
                    usernamePassword(credentialsId: 'github-alexkads', usernameVariable: 'GH_USER', passwordVariable: 'GH_TOKEN'),
                    file(credentialsId: 'kube', variable: 'KUBECONFIG'),
                    sshUserPrivateKey(credentialsId: 'deploy-key', keyFileVariable: 'KEY', passphraseVariable: 'FRASE')
                ]) {
                    sh 'publica'
                }
            }
        }
    }
}
"#;
    let conversion = convert(text);
    assert!(conversion.complete(), "{:#?}", conversion.notes);
    let pipeline = TomlFormat.read("x", &conversion.toml).expect(&conversion.toml);
    let publish = pipeline
        .flow
        .nodes
        .iter()
        .find(|n| n.parameters.get("script") == Some(&serde_json::json!("publica")))
        .expect("publica");
    assert_eq!(
        publish.parameters["credentials"],
        serde_json::json!({
            "NPM_TOKEN": "npm",
            "GH_USER": "github-alexkads:user",
            "GH_TOKEN": "github-alexkads:password",
            "KUBECONFIG": { "file": "kube" },
            "KEY": { "file": "deploy-key" },
            "FRASE": "deploy-key:passphrase",
        })
    );
    let before = pipeline
        .flow
        .nodes
        .iter()
        .find(|n| n.parameters.get("script") == Some(&serde_json::json!("antes")))
        .expect("antes");
    assert!(before.parameters.get("credentials").is_none(), "only the steps inside get them");
    let note = conversion.notes.iter().find(|n| n.what == "withCredentials").expect("a note");
    assert!(note.why.contains("deploy-key, github-alexkads, kube, npm"), "{}", note.why);
}
