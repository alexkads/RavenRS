//! The JSON Schema of `.raven-ci.toml` (#941): what an editor or a session of AI reads to write a
//! pipeline right the first time.
//!
//! # How it stays true
//!
//! Each table of the schema lists exactly the fields the reader takes, from the same lists
//! ([`read::TOP`](crate::read), `STAGE`, `STEP`…): a field the reader learns and the schema does not
//! describe fails the test of this module, and so does one the schema has and the reader refuses.
//! The examples of the guides are read both ways in `tests/schema.rs`, and the two have to agree.
//!
//! The schema says the shape: names, types, which values a field takes. What only reading says,
//! such as a duration that does not parse or two stages with one name, `raven-ci validar` says,
//! with the line.

use serde_json::{Map, Value, json};

use crate::read::{
    AGENT, BRANCH, CONTAINER, CREDENTIAL_FILE, LOCK, PARAMETER, PIPELINE, POST, STAGE, STEP, TOP, WHEN,
};

/// A period as the house writes it.
fn duration(what: &str) -> Value {
    json!({ "type": "string", "pattern": "^([0-9]+(ms|s|m|h|d))+$", "description": format!("{what}: 30s, 10m, 1h30m, 500ms") })
}

fn text(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn texts(description: &str) -> Value {
    json!({ "oneOf": [{ "type": "string" }, { "type": "array", "items": { "type": "string" } }], "description": description })
}

fn integer(description: &str) -> Value {
    json!({ "type": "integer", "description": description })
}

fn boolean(description: &str) -> Value {
    json!({ "type": "boolean", "description": description })
}

/// A table with exactly `fields`, each described by `describe`; `required` are needed.
fn table(fields: &[&str], required: &[&str], description: &str, describe: impl Fn(&str) -> Value) -> Value {
    let properties: Map<String, Value> =
        fields.iter().map(|field| ((*field).to_owned(), describe(field))).collect();
    let mut table = json!({
        "type": "object",
        "description": description,
        "additionalProperties": false,
        "properties": properties,
    });
    if !required.is_empty() {
        table["required"] = json!(required);
    }
    table
}

fn credentials() -> Value {
    json!({
        "type": "object",
        "description": "Variáveis do comando com credenciais do cofre: \"nome\", \"nome:campo\" ou { file = \"nome\" }",
        "propertyNames": { "pattern": "^[A-Za-z_][A-Za-z0-9_]*$" },
        "additionalProperties": { "oneOf": [
            { "type": "string", "description": "nome, ou nome:campo" },
            table(CREDENTIAL_FILE, &["file"], "Um arquivo secreto, só do dono, com o caminho na variável", |field| match field {
                "file" => text("A credencial"),
                "name" => text("O nome do arquivo (o da variável, quando não dito)"),
                _ => json!({}),
            }),
        ]},
    })
}

/// `unstable = [3]`: the exit codes that leave the build unstable (#947).
fn codes() -> Value {
    json!({
        "type": "array",
        "items": { "type": "integer", "minimum": 1, "maximum": 255 },
        "description": "Códigos de saída que deixam o build instável (amarelo) e seguem, como o unstable() do Jenkins",
    })
}

fn step() -> Value {
    json!({ "oneOf": [
        { "type": "string", "description": "Um comando, rodado por sh -e -c" },
        table(STEP, &[], "Um passo: diz um, e só um, de sh, cargo ou uses", |field| match field {
            "sh" => text("O comando"),
            "cargo" => text("As palavras depois de cargo: test -p site"),
            "uses" => text("O passo de um plugin: plugin/passo, como kubernetes/kubectl"),
            "with" => json!({ "type": "object", "description": "Os parâmetros do passo de uses" }),
            "name" => text("O que o painel e o log mostram"),
            "timeout" => duration("O limite do passo"),
            "retry" => integer("Quantas vezes mais o passo é tentado quando falha"),
            "nice" => integer("A prioridade: nice -n N (Linux e macOS)"),
            "credentials" => credentials(),
            "keep_daemons" => boolean("Deixa vivo o que o comando deixou rodando"),
            "unstable" => codes(),
            _ => json!({}),
        }),
    ]})
}

fn steps() -> Value {
    json!({ "type": "array", "items": step(), "description": "Os passos, em ordem" })
}

fn stage() -> Value {
    table(STAGE, &["name"], "Um estágio: steps ou parallel", |field| match field {
        "name" => text("O nome, um por estágio"),
        "steps" => steps(),
        "parallel" => json!({ "type": "array", "description": "Ramos no lugar de steps", "items":
        table(BRANCH, &["name", "steps"], "Um ramo", |field| match field {
            "name" => text("O nome do ramo"),
            "steps" => steps(),
            _ => json!({}),
        })}),
        "when" => {
            table(WHEN, &[], "Quando o estágio roda: toda condição dada tem de valer", |field| match field
            {
                "branch" => texts("A branch, ou a lista; um padrão que acaba em * pega o começo"),
                "pull_request" => boolean("Só num pull request (true) ou só numa branch (false)"),
                "parameter" => text("Um parâmetro bool que tem de ser verdadeiro"),
                _ => json!({}),
            })
        }
        "timeout" => duration("O limite de cada passo do estágio"),
        "retry" => integer("Quantas vezes mais cada passo é tentado"),
        "lock" => json!({ "oneOf": [
            text("Uma trava: um build por vez a segura (plugin lock)"),
            table(LOCK, &["name"], "Uma trava num arquivo dito, que scripts de fora também seguram com flock", |field| match field {
                "name" => text("O nome da trava"),
                "path" => text("O arquivo da trava; ~/ é a pasta do usuário de quem roda o passo"),
                _ => json!({}),
            }),
        ]}),
        "nice" => integer("Cada sh roda com nice -n N"),
        "container" => {
            table(CONTAINER, &["image"], "Cada sh roda na imagem (plugin docker)", |field| match field {
                "image" => text("A imagem"),
                "args" => texts("Mais palavras do docker run"),
                "env" => {
                    json!({ "type": "object", "additionalProperties": { "type": "string" }, "description": "Variáveis do contêiner" })
                }
                _ => json!({}),
            })
        }
        "credentials" => credentials(),
        "unstable" => codes(),
        _ => json!({}),
    })
}

/// The JSON Schema of `.raven-ci.toml`.
pub fn schema() -> Value {
    let mut root = table(TOP, &["stages"], "O pipeline de um job do Raven CI: .raven-ci.toml", |field| {
        match field {
            "pipeline" => table(PIPELINE, &[], "O build inteiro", |field| match field {
                "timeout" => duration("O limite do build inteiro"),
                "keep" => integer("Quantos builds o job guarda"),
                "agent" => json!({ "description": "Onde o build roda", "oneOf": [
                    { "const": "any" },
                    table(AGENT, &[], "Um, e só um, de label, docker ou kubernetes", |field| match field {
                        "label" => text("Um agente com este rótulo"),
                        "docker" => json!({ "type": "object", "description": "Um agente num contêiner: image, network, args, tentativas…" }),
                        "kubernetes" => json!({ "type": "object", "description": "Um agente num pod" }),
                        _ => json!({}),
                    }),
                ]}),
                _ => json!({}),
            }),
            "environment" => {
                json!({ "type": "object", "additionalProperties": { "type": "string" }, "description": "Variáveis de todo passo" })
            }
            "parameters" => {
                json!({ "type": "array", "description": "O que quem pede o build informa", "items":
                table(PARAMETER, &["name"], "Um parâmetro", |field| match field {
                    "name" => text("O nome, que vira variável do build"),
                    "type" => json!({ "enum": ["bool", "text", "choice"], "description": "text quando não dito" }),
                    "default" => json!({ "description": "O valor quando não informado" }),
                    "description" => text("O que o painel mostra"),
                    "choices" => json!({ "type": "array", "items": { "type": "string" }, "description": "As opções de um choice" }),
                    _ => json!({}),
                })})
            }
            "stages" => {
                json!({ "type": "array", "minItems": 1, "items": stage(), "description": "Os estágios, em ordem" })
            }
            "post" => {
                table(POST, &[], "O que roda no fim, na mesma pasta e no mesmo log", |field| match field {
                    "always" => steps(),
                    "success" => steps(),
                    "failure" => steps(),
                    _ => json!({}),
                })
            }
            "artifacts" => texts("O que o build guarda, por padrão de caminho (plugin artifacts)"),
            "junit" => texts("Os relatórios de teste JUnit (plugin junit)"),
            _ => json!({}),
        }
    });
    root["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    root["$id"] = json!("https://github.com/alexkads/RavenRS/tree/master/tools/raven-ci/docs/pipeline.md");
    root["title"] = json!(".raven-ci.toml");
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every table of the schema has exactly the fields its list has, each described.
    #[test]
    fn every_field_the_reader_takes_is_in_the_schema_and_described() {
        let root = schema();
        let stage = &root["properties"]["stages"]["items"];
        let step = &stage["properties"]["steps"]["items"]["oneOf"][1];
        let tables: [(&str, &Value, &[&str]); 9] = [
            ("o arquivo", &root, TOP),
            ("pipeline", &root["properties"]["pipeline"], PIPELINE),
            ("stage", stage, STAGE),
            ("when", &stage["properties"]["when"], WHEN),
            ("step", step, STEP),
            ("parameter", &root["properties"]["parameters"]["items"], PARAMETER),
            ("branch", &stage["properties"]["parallel"]["items"], BRANCH),
            ("container", &stage["properties"]["container"], CONTAINER),
            ("post", &root["properties"]["post"], POST),
        ];
        for (name, table, fields) in tables {
            let properties =
                table["properties"].as_object().unwrap_or_else(|| panic!("{name} has no properties"));
            let mut have: Vec<&str> = properties.keys().map(String::as_str).collect();
            let mut want = fields.to_vec();
            have.sort_unstable();
            want.sort_unstable();
            assert_eq!(have, want, "{name}");
            for (field, value) in properties {
                assert!(value != &json!({}), "{name}.{field} is not described");
            }
            assert_eq!(table["additionalProperties"], false, "{name}");
        }
    }
}
