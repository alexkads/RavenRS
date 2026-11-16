//! What the repository gives an AI besides the libraries: the installable tools of `tools/`,
//! each described by its `tool.json`, and the skills, the instructions written for coding agents
//! (`.claude/skills`, `plugins/*/skills`, `tools/*/claude-pane/skills`).

use serde_json::{Value, json};

use crate::api::{GUIDES, SKILLS, TOOL_MANIFESTS};

/// The tools, or one of them with everything it offers and its README.
pub fn tools(name: Option<&str>) -> Result<Value, String> {
    let manifests: Vec<(&str, Value)> = TOOL_MANIFESTS
        .iter()
        .filter_map(|(path, text)| Some((*path, serde_json::from_str::<Value>(text).ok()?)))
        .collect();
    let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) else {
        let tools: Vec<Value> = manifests
            .iter()
            .map(|(path, tool)| {
                json!({
                    "name": tool["name"],
                    "also_called": tool["also_called"],
                    "package": tool["package"],
                    "version": tool["version"],
                    "summary": tool["summary"],
                    "manifest": path,
                })
            })
            .collect();
        return Ok(json!({
            "total": tools.len(),
            "tools": tools,
            "note": "Each tool installs and updates itself, with its own version; ask for one by name or package for its commands, scenarios, report and AI tools.",
        }));
    };
    let wanted = name.to_lowercase();
    let (path, tool) = manifests
        .iter()
        .find(|(_, tool)| {
            // The name, the package, or what people call it for short, such as "RTS".
            let short = tool["also_called"].as_array().into_iter().flatten();
            [&tool["package"], &tool["name"]]
                .into_iter()
                .chain(short)
                .any(|field| field.as_str().is_some_and(|text| text.to_lowercase() == wanted))
        })
        .or_else(|| {
            manifests
                .iter()
                .find(|(_, tool)| tool["package"].as_str().is_some_and(|package| package.contains(&wanted)))
        })
        .ok_or_else(|| {
            let known: Vec<&str> =
                manifests.iter().filter_map(|(_, tool)| tool["package"].as_str()).collect();
            format!("there is no tool {name:?}; the tools are {}", known.join(", "))
        })?;
    let folder = path.trim_end_matches("tool.json");
    let mut answer = tool.clone();
    answer["manifest"] = json!(path);
    if let Some((readme, text)) = GUIDES.iter().find(|(guide, _)| *guide == format!("{folder}README.md")) {
        answer["readme"] = json!({ "document": readme, "text": text });
    }
    Ok(answer)
}

/// One skill as the index knows it.
struct Skill {
    name: String,
    description: String,
    path: &'static str,
    body: &'static str,
}

/// Where a skill comes from: the project, a plugin or a tool.
fn origin(path: &str) -> &'static str {
    if path.starts_with("plugins/") {
        "plugin"
    } else if path.starts_with("tools/") {
        "tool"
    } else {
        "project"
    }
}

fn parse(path: &'static str, text: &'static str) -> Skill {
    let folder = path.trim_end_matches("/SKILL.md").rsplit('/').next().unwrap_or(path);
    let mut skill = Skill { name: folder.to_owned(), description: String::new(), path, body: text };
    if let Some(rest) = text.strip_prefix("---\n")
        && let Some(end) = rest.find("\n---")
    {
        for line in rest[..end].lines() {
            if let Some(value) = line.strip_prefix("name:") {
                skill.name = value.trim().to_owned();
            } else if let Some(value) = line.strip_prefix("description:") {
                skill.description = value.trim().to_owned();
            }
        }
        skill.body = rest[end + 4..].trim_start_matches('\n');
    }
    skill
}

/// The skills, those whose name or description has every word of `query`, or one whole.
pub fn skills(name: Option<&str>, query: Option<&str>) -> Result<Value, String> {
    let all: Vec<Skill> = SKILLS.iter().map(|(path, text)| parse(path, text)).collect();
    if let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) {
        let skill = all.iter().find(|skill| skill.name.eq_ignore_ascii_case(name)).ok_or_else(|| {
            let known: Vec<&str> = all.iter().map(|skill| skill.name.as_str()).collect();
            format!("there is no skill {name:?}; the skills are {}", known.join(", "))
        })?;
        return Ok(json!({
            "name": skill.name,
            "origin": origin(skill.path),
            "path": skill.path,
            "description": skill.description,
            "text": skill.body,
        }));
    }
    let words: Vec<String> = query.unwrap_or_default().split_whitespace().map(str::to_lowercase).collect();
    let listed: Vec<Value> = all
        .iter()
        .filter(|skill| {
            let text = format!("{} {}", skill.name, skill.description).to_lowercase();
            words.iter().all(|word| text.contains(word))
        })
        .map(|skill| {
            json!({
                "name": skill.name,
                "origin": origin(skill.path),
                "path": skill.path,
                "description": skill.description,
            })
        })
        .collect();
    Ok(json!({
        "total": listed.len(),
        "skills": listed,
        "note": "Ask for one by name for its whole text. Project skills are how this repository is worked on; plugin and tool skills travel with the plugin or tool.",
    }))
}
