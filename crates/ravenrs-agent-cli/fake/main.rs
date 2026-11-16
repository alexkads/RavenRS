//! A coding agent's CLI for the tests, which spends nothing: it answers `--version` and
//! `--help` (with or without the flags, by `FAKE_OLD`), and for a run it reads the prompt from
//! its input and writes recorded JSON lines, as Codex (`exec`) or Claude (`-p`) would. A message
//! says the arguments it got and the value of `API_TOKEN_TEST`, to check what reaches it.
#![allow(clippy::expect_used, reason = "a program for tests, which fails loudly")]

use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let old = std::env::var("FAKE_OLD").is_ok();
    if args.iter().any(|a| a == "--version") {
        println!("fake-agent 1.2.3");
        return;
    }
    if args.iter().any(|a| a == "--help") {
        if args.first().is_some_and(|a| a == "exec") {
            println!("Usage: codex exec [OPTIONS] [PROMPT]\n  --json\n  --cd <DIR>");
            if !old {
                println!("  -s, --sandbox <SANDBOX_MODE>\n  -m, --model <MODEL>");
            }
        } else {
            println!("Usage: claude [options]\n  --output-format <format> (choices: \"stream-json\")");
            if !old {
                println!(
                    "  --permission-mode <mode>\n  --allowedTools\n  --disallowedTools\n  --max-budget-usd <amount>\n  --model <model>"
                );
            }
        }
        return;
    }
    let mut prompt = String::new();
    std::io::stdin().read_to_string(&mut prompt).expect("the prompt");
    let prompt = prompt.trim();
    let told = format!(
        "args: {} | token: {}",
        args.join(" "),
        std::env::var("API_TOKEN_TEST").unwrap_or("<none>".into())
    );
    if args.first().is_some_and(|a| a == "exec") {
        let session = args
            .iter()
            .position(|a| a == "resume")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or("t-1".into());
        println!(r#"{{"type":"thread.started","thread_id":"{session}"}}"#);
        println!(r#"{{"type":"turn.started"}}"#);
        println!(
            "{}",
            serde_json::json!({ "type": "item.completed", "item": { "type": "agent_message", "text": told } })
        );
        println!(r#"{{"type":"item.started","item":{{"type":"command_execution","command":"ls"}}}}"#);
        if prompt.contains("FAIL") {
            println!(r#"{{"type":"turn.failed","error":{{"message":"the model refused"}}}}"#);
            return;
        }
        println!(
            "{}",
            serde_json::json!({ "type": "item.completed", "item": { "type": "agent_message", "text": format!("Done: {prompt}") } })
        );
        println!(r#"{{"type":"turn.completed","usage":{{"input_tokens":10}}}}"#);
    } else {
        println!(r#"{{"type":"system","subtype":"init","session_id":"c-1"}}"#);
        println!(
            "{}",
            serde_json::json!({ "type": "assistant", "message": { "content": [ { "type": "text", "text": told }, { "type": "tool_use", "name": "Read", "input": { "file": "a.rs" } } ] } })
        );
        println!(
            "{}",
            serde_json::json!({ "type": "result", "subtype": "success", "is_error": false, "result": format!("Done: {prompt}"), "total_cost_usd": 0.0123, "session_id": "c-1" })
        );
    }
}
