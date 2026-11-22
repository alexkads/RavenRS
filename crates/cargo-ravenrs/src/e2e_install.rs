//! Installs the shared template artifacts only where the existing entry is recognized.
use std::io::Write;
use std::path::{Component, Path};
use std::process::{Command, Stdio};

use ravenrs_project::ProjectFile;
use toml_edit::{DocumentMut, InlineTable, Item, Table, TableLike, Value, value};

use crate::{read, template, versions, write};

const NATIVE: &str = "cfg(not(target_arch = \"wasm32\"))";

/// Prepare and validate the complete plan before writing any generated file.
pub(crate) fn install(folder: &Path) -> Result<usize, String> {
    let root = folder.canonicalize().map_err(|error| format!("{}: {error}", folder.display()))?;
    let files = plan(&root)?;
    for file in &files {
        let mut path = root.clone();
        for component in Path::new(&file.path).components() {
            path.push(component);
            if path.is_symlink() {
                return Err(format!(
                    "{} é um link; instale manualmente sem redirecionar arquivos.",
                    path.display()
                ));
            }
        }
        if path.exists() && !path.is_file() {
            return Err(format!("{} não é um arquivo; nenhum arquivo foi alterado.", path.display()));
        }
    }
    let changed: Vec<_> = files
        .into_iter()
        .filter(|file| std::fs::read(root.join(&file.path)).ok().as_deref() != Some(file.contents.as_bytes()))
        .collect();
    for file in &changed {
        write(&root, &file.path, &file.contents)?;
    }
    Ok(changed.len())
}

fn plan(root: &Path) -> Result<Vec<ProjectFile>, String> {
    let original = read(&root.join("Cargo.toml"))?;
    let mut manifest: DocumentMut = original.parse().map_err(|error| format!("Cargo.toml: {error}"))?;
    let name = manifest.get("package").and_then(|item| item.get("name")).and_then(Item::as_str)
        .ok_or("Selecione a pasta de um app gerado com --project; bibliotecas e workspaces virtuais não têm entrada para instalar.")?.to_owned();
    if manifest.get("bin").is_some()
        || manifest["package"].get("autobins").and_then(Item::as_bool) == Some(false)
    {
        return Err(
            "Entrada de binário personalizada; configure o canal manualmente. Nenhum arquivo foi alterado."
                .into(),
        );
    }
    let state: DocumentMut = read(&root.join(".ravenrs-project.toml"))
        .map_err(|_| "e2e install reconhece projetos de cargo ravenrs new; para outra entrada, configure o canal manualmente. Nenhum arquivo foi alterado.".to_owned())?
        .parse().map_err(|error| format!(".ravenrs-project.toml: {error}"))?;
    let kind =
        state.get("template").and_then(Item::as_str).ok_or("Modelo ausente em .ravenrs-project.toml")?;
    let target = state.get("target").and_then(Item::as_str).ok_or("Alvo ausente em .ravenrs-project.toml")?;
    versions::ensure_current(root)?;
    let generated = template::project_files(&name, kind, target)?;
    let project = ravenrs_e2e_runner::project::read(root)?;
    project.check_configs()?;
    let app = project
        .found
        .iter()
        .find(|found| found.folder == root && found.package == name && found.is_app())
        .ok_or("Este pacote não é um app RavenRS com entrada nativa reconhecida.")?;
    if app.binaries != [name.clone()] {
        return Err("Selecione um app com um único binário de mesmo nome do pacote; outras entradas exigem configuração manual.".into());
    }
    for (file, scenario) in &app.scenarios {
        if let Err(why) = scenario {
            return Err(format!("{}: {why}", file.display()));
        }
    }
    if !app.config.folder.components().all(|part| matches!(part, Component::Normal(_) | Component::CurDir)) {
        return Err("A instalação escreve somente dentro do app; escolha uma pasta relativa de cenários sem .. em e2e.toml.".into());
    }
    let mut scenario_parts = Vec::new();
    for part in app.config.folder.components() {
        if let Component::Normal(name) = part {
            scenario_parts.push(name.to_str().ok_or("A pasta de cenários precisa ser UTF-8.")?);
        }
    }
    scenario_parts.push("start.raventest");
    let scenario_path = scenario_parts.join("/");
    let default_main = artifact(&generated, "src/main.rs")?;
    let new_main = default_main
        .replace("\"../tests/e2e/start.raventest\"", &format!("{:?}", format!("../{scenario_path}")));
    let current_main = read(&root.join("src/main.rs"))?;
    let legacy_main = template::legacy_main(&name)?;
    let exact = [&legacy_main, &default_main, &new_main]
        .iter()
        .any(|known| known.trim_end() == current_main.trim_end());
    let (recognized, already_current) = if exact {
        (true, current_main.trim_end() == new_main.trim_end())
    } else if let Some(current) = formatted(root, &current_main) {
        let already = formatted(root, &new_main).as_deref() == Some(current.as_str());
        (
            already
                || [&legacy_main, &default_main]
                    .iter()
                    .any(|known| formatted(root, known).as_deref() == Some(current.as_str())),
            already,
        )
    } else {
        (false, false)
    };
    if !recognized {
        return Err("src/main.rs tem uma entrada personalizada; configure --e2e-serve manualmente antes da janela/persistência. Nenhum arquivo foi alterado.".into());
    }
    let expected: DocumentMut =
        artifact(&generated, "Cargo.toml")?.parse().map_err(|error| format!("Modelo: {error}"))?;
    if manifest.get("dependencies").and_then(|deps| deps.get("ravenrs")).is_none() {
        return Err("A dependência ravenrs do modelo foi renomeada; configure a entrada manualmente.".into());
    }
    for dependency in ["ravenrs-e2e", "ravenrs-e2e-wire"] {
        let global = manifest.get("dependencies").is_some_and(|deps| deps.get(dependency).is_some());
        let deps = if global {
            table_at(manifest.as_table_mut(), &["dependencies"])?
        } else {
            table_at(manifest.as_table_mut(), &["target", NATIVE, "dependencies"])?
        };
        if deps.get(dependency).is_none() {
            deps.insert(dependency, expected["target"][NATIVE]["dependencies"][dependency].clone());
        }
        let item = deps.get_mut(dependency).ok_or("Dependência não inserida")?;
        if item.get("package").and_then(Item::as_str).is_some_and(|package| package != dependency) {
            return Err(format!("{dependency} aponta para outro pacote; configure manualmente."));
        }
        if dependency == "ravenrs-e2e-wire" {
            dependency_feature(item, "serve")?;
        }
    }
    let metadata = table_at(manifest.as_table_mut(), &["package", "metadata", "ravenrs", "e2e"])?;
    for key in ["app", "binary"] {
        if let Some(current) = metadata.get(key) {
            if current.as_str() != Some(&name) {
                return Err(format!(
                    "package.metadata.ravenrs.e2e.{key} difere do app gerado; nenhum arquivo foi alterado."
                ));
            }
        } else {
            metadata.insert(key, value(&name));
        }
    }
    append_feature(metadata, "features", "e2e-source")?;
    let features = table_at(manifest.as_table_mut(), &["features"])?;
    append_feature(features, "e2e-source", "ravenrs/e2e-source")?;
    let scenario = if root.join(&scenario_path).exists() {
        read(&root.join(&scenario_path))?
    } else {
        artifact(&generated, "tests/e2e/start.raventest")?
    };
    ravenrs_e2e_wire::Scenario::parse(&scenario).map_err(|error| format!("{scenario_path}: {error}"))?;
    let mut files = vec![
        ProjectFile { path: "Cargo.toml".into(), contents: manifest.to_string() },
        ProjectFile {
            path: "src/main.rs".into(),
            contents: if already_current { current_main } else { new_main },
        },
        ProjectFile { path: scenario_path, contents: scenario },
    ];
    if kind == "app-sqlite" && root.join("tests/screen.rs").is_file() {
        let screen = artifact(&generated, "tests/screen.rs")?;
        let legacy = screen.replace("::tasks().into_view()", "::app()");
        if read(&root.join("tests/screen.rs"))?.trim_end() == legacy.trim_end() {
            files.push(ProjectFile { path: "tests/screen.rs".into(), contents: screen });
        }
    }
    Ok(files)
}

fn artifact(files: &[ProjectFile], path: &str) -> Result<String, String> {
    files
        .iter()
        .find(|file| file.path == path)
        .map(|file| file.contents.clone())
        .ok_or_else(|| format!("Modelo sem {path}"))
}

/// Parse/format stdin only: no file is rewritten, compiled or executed. Both inputs use the
/// project's formatting configuration, so formatting alone never makes its entry "custom".
fn formatted(root: &Path, source: &str) -> Option<String> {
    let mut child = Command::new("rustfmt")
        .args(["--emit", "stdout", "--edition", "2024"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let written = child.stdin.take().is_some_and(|mut input| input.write_all(source.as_bytes()).is_ok());
    let output = child.wait_with_output().ok()?;
    if !written || !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok().map(|text| text.replace("\r\n", "\n"))
}

fn table_at<'a>(table: &'a mut dyn TableLike, path: &[&str]) -> Result<&'a mut dyn TableLike, String> {
    let Some((key, rest)) = path.split_first() else {
        return Ok(table);
    };
    if table.get(key).is_none() {
        table.insert(key, Item::Table(Table::new()));
    }
    let child = table
        .get_mut(key)
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| format!("{} precisa ser uma tabela em Cargo.toml", path.join(".")))?;
    table_at(child, rest)
}

fn append_feature(table: &mut dyn TableLike, key: &str, feature: &str) -> Result<(), String> {
    if table.get(key).is_none() {
        table.insert(key, value(toml_edit::Array::new()));
    }
    let list = table
        .get_mut(key)
        .and_then(Item::as_array_mut)
        .ok_or_else(|| format!("{key} precisa ser uma lista de textos"))?;
    if list.iter().any(|item| item.as_str().is_none_or(|text| text.trim().is_empty())) {
        return Err(format!("{key} precisa ser uma lista de textos não vazios"));
    }
    if !list.iter().any(|item| item.as_str() == Some(feature)) {
        list.push(feature);
    }
    Ok(())
}

fn dependency_feature(item: &mut Item, feature: &str) -> Result<(), String> {
    if let Some(version) = item.as_str() {
        let mut table = InlineTable::new();
        table.insert("version", version.into());
        let mut new = Value::InlineTable(table);
        if let Some(old) = item.as_value() {
            *new.decor_mut() = old.decor().clone();
        }
        *item = Item::Value(new);
    }
    let table =
        item.as_table_like_mut().ok_or("A dependência ravenrs-e2e-wire precisa ser uma versão ou tabela")?;
    append_feature(table, "features", feature)
}
