use crate::{VERSION, read};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use toml_edit::DocumentMut;

/// Probe a real MCP command with a bounded initialize + tools/list exchange.
pub(crate) fn probe(command: &str, args: &[String], root: &Path) -> Result<(), String> {
    let mut child = Command::new(command)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("MCP {command}: {e}"))?;
    let result = (|| {
        let mut stdin = child.stdin.take().ok_or("MCP stdin ausente")?;
        let stdout = child.stdout.take().ok_or("MCP stdout ausente")?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let send = |stdin: &mut std::process::ChildStdin, value: Value| -> Result<(), String> {
            writeln!(stdin, "{value}").and_then(|()| stdin.flush()).map_err(|e| e.to_string())
        };
        let receive = |id: u64| -> Result<Value, String> {
            let deadline = std::time::Instant::now() + Duration::from_secs(20);
            loop {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                let line = rx
                    .recv_timeout(remaining)
                    .map_err(|e| format!("MCP não respondeu em 20s: {e}"))?
                    .map_err(|e| e.to_string())?;
                let reply: Value = serde_json::from_str(&line).map_err(|e| format!("MCP: {e}"))?;
                if reply["id"] == id {
                    if !reply["error"].is_null() {
                        return Err(format!("MCP: {}", reply["error"]));
                    }
                    return Ok(reply["result"].clone());
                }
            }
        };
        send(
            &mut stdin,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"cargo-ravenrs-doctor","version":VERSION}}}),
        )?;
        let initialized = receive(1)?;
        if initialized["serverInfo"]["version"] != VERSION {
            return Err(format!("MCP versão {}, CLI {VERSION}", initialized["serverInfo"]["version"]));
        }
        send(&mut stdin, json!({"jsonrpc":"2.0","method":"notifications/initialized"}))?;
        send(&mut stdin, json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))?;
        let tools = receive(2)?;
        if !tools["tools"].as_array().is_some_and(|t| t.iter().any(|t| t["name"] == "component_api")) {
            return Err("MCP não retornou component_api".into());
        }
        Ok(())
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

/// Verify the current project version, managed files and every registered MCP command.
pub fn doctor(root: &Path) -> Result<(), String> {
    let manifest: DocumentMut =
        read(&root.join("Cargo.toml"))?.parse().map_err(|e| format!("Cargo.toml: {e}"))?;
    let mut versions = Vec::new();
    collect_versions(manifest.as_item(), &mut versions);
    if versions.is_empty() {
        return Err("Nenhuma dependência RavenRS com versão explícita; doctor requer um Cargo.toml de projeto independente.".into());
    }
    for (name, version) in versions {
        if version.trim_start_matches(['=', '^', '~']) != VERSION {
            return Err(format!(
                "{name} usa {version}, mas cargo-ravenrs usa {VERSION}. Instale a tag correspondente."
            ));
        }
    }
    println!("✓ Crates e CLI: {VERSION}");
    if root.join(".ravenrs-project.toml").is_file() && !root.join(".ravenrs-ai.toml").is_file() {
        let state: DocumentMut =
            read(&root.join(".ravenrs-project.toml"))?.parse().map_err(|e| format!("Projeto: {e}"))?;
        if state["ai"].as_bool() == Some(false) {
            probe("cargo", &["ravenrs".into(), "mcp".into()], root)?;
            println!("✓ MCP; integração com assistentes desativada por --no-ai.");
            return Ok(());
        }
    }
    let state_path = root.join(".ravenrs-ai.toml");
    if !state_path.is_file() {
        return Err("Arquivos de IA ausentes. Execute cargo ravenrs ai.".into());
    }
    let state: DocumentMut = read(&state_path)?.parse().map_err(|e| format!("Estado de IA: {e}"))?;
    if state["version"].as_str() != Some(VERSION) {
        return Err("Arquivos de IA desatualizados. Execute cargo ravenrs ai.".into());
    }
    let plugin = state["plugin"].as_bool().unwrap_or(false);
    let expected = crate::ai::ai_files(root, plugin)?;
    for (path, contents) in expected {
        if read(&root.join(&path))? != contents {
            return Err(format!("{path} desatualizado. Execute cargo ravenrs ai."));
        }
    }
    println!("✓ Arquivos de IA e skill");
    for (path, key) in
        [(".mcp.json", "mcpServers"), (".cursor/mcp.json", "mcpServers"), (".vscode/mcp.json", "servers")]
    {
        let config: Value =
            serde_json::from_str(&read(&root.join(path))?).map_err(|e| format!("{path}: {e}"))?;
        let server = &config[key]["ravenrs"];
        let command = server["command"].as_str().ok_or("MCP command ausente")?;
        let args: Vec<String> = server["args"]
            .as_array()
            .ok_or("MCP args ausentes")?
            .iter()
            .map(|v| v.as_str().map(str::to_owned).ok_or("MCP argumento inválido".to_owned()))
            .collect::<Result<_, _>>()?;
        probe(command, &args, root)?;
        println!("✓ MCP {path}");
    }
    let codex: DocumentMut =
        read(&root.join(".codex/config.toml"))?.parse().map_err(|e| format!("Codex: {e}"))?;
    let server = &codex["mcp_servers"]["ravenrs"];
    let args = server["args"]
        .as_array()
        .ok_or("Codex args ausentes")?
        .iter()
        .map(|v| v.as_str().map(str::to_owned).ok_or("Codex argumento inválido".into()))
        .collect::<Result<Vec<String>, String>>()?;
    probe(server["command"].as_str().ok_or("Codex command ausente")?, &args, root)?;
    println!("✓ MCP .codex/config.toml\nDoctor: tudo certo.");
    Ok(())
}

fn collect_versions(item: &toml_edit::Item, versions: &mut Vec<(String, String)>) {
    if let Some(table) = item.as_table_like() {
        for (key, item) in table.iter() {
            if key.starts_with("ravenrs") {
                if let Some(version) =
                    item.as_str().or_else(|| item.get("version").and_then(toml_edit::Item::as_str))
                {
                    versions.push((key.into(), version.into()));
                }
            } else {
                collect_versions(item, versions);
            }
        }
    }
}
