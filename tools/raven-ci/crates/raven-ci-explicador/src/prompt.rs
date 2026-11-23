//! What the model is told (#942): the rules of the answer, and the summary of the build (#941) as
//! a short text, so a small model reads only what matters.
//!
//! The rules ask for Brazilian Portuguese, three sentences at most, the file and the line when the
//! summary says them, and nothing the summary does not say: a model that does not know says so and
//! names the step to look at, instead of making up a file or a number.

use serde_json::Value;

/// The rules of the answer.
pub const SYSTEM: &str = "Você explica por que um build de CI falhou, para quem não vai ler o log. \
Responda em português do Brasil, em no máximo três frases curtas: o que falhou (o teste, o comando, \
e o arquivo e a linha quando o resumo os disser), por quê, e o que mudar para passar. Use só o que o \
resumo diz. Se ele não disser a causa, diga que pelo resumo não dá para saber e qual passo olhar. \
Nunca invente nome de arquivo, número de linha, comando ou valor.";

/// How many lines of the tail of the step that failed the model reads.
const TAIL: usize = 20;
/// How many tests and diagnostics it reads.
const MOST: usize = 5;

fn text<'v>(value: &'v Value, field: &str) -> &'v str {
    value.get(field).and_then(Value::as_str).unwrap_or_default()
}

/// The summary of a build (the JSON of `GET /api/ci/build/summary`) as the text the model reads.
pub fn user(summary: &Value) -> String {
    let mut out = format!(
        "Build {} #{}: {}.\n",
        text(summary, "job"),
        summary.get("number").and_then(Value::as_u64).unwrap_or_default(),
        text(summary, "status_words")
    );
    if let Some(reason) = summary.get("reason").and_then(Value::as_str) {
        out.push_str(&format!("Motivo: {reason}\n"));
    }
    if let Some(failed) = summary.get("failed").filter(|f| f.is_object()) {
        out.push_str(&format!("Estágio que falhou: {}; passo: {}.\n", text(failed, "stage"), text(failed, "title")));
        if let Some(error) = failed.get("error").and_then(Value::as_str) {
            out.push_str(&format!("Erro do passo: {error}\n"));
        }
        let tail: Vec<&str> = failed.get("tail").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
        if !tail.is_empty() {
            out.push_str("Últimas linhas do passo:\n");
            for line in &tail[tail.len().saturating_sub(TAIL)..] {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    if let Some(tests) = summary.get("tests").filter(|t| t.is_object()) {
        out.push_str(&format!(
            "Testes: {} rodaram, {} falharam, {} com erro.\n",
            tests.get("tests").and_then(Value::as_u64).unwrap_or_default(),
            tests.get("failures").and_then(Value::as_u64).unwrap_or_default(),
            tests.get("errors").and_then(Value::as_u64).unwrap_or_default()
        ));
        for test in tests.get("failed").and_then(Value::as_array).into_iter().flatten().take(MOST) {
            out.push_str(&format!("- teste {}: {}\n", text(test, "name"), text(test, "message")));
        }
    }
    if let Some(diagnostics) = summary.get("diagnostics").filter(|d| d.is_object()) {
        for d in diagnostics.get("first").and_then(Value::as_array).into_iter().flatten().take(MOST) {
            out.push_str(&format!("- {} {} {}: {}\n", text(d, "level"), text(d, "code"), text(d, "place"), text(d, "message")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_model_reads_what_failed_the_tests_and_the_diagnostics_and_no_more() {
        let tail: Vec<String> = (1..=40).map(|n| format!("linha {n}")).collect();
        let summary = json!({
            "job": "loja", "number": 7, "status_words": "falhou",
            "reason": "o passo e1-2 falhou",
            "failed": { "stage": "Testes", "title": "cargo test", "error": "código 101", "tail": tail },
            "tests": { "tests": 301, "failures": 1, "errors": 0, "failed": [{ "name": "testes::soma", "message": "esperava 5, veio 4" }] },
            "diagnostics": { "first": [{ "level": "warning", "code": "unused_variables", "place": "src/lib.rs:1:42", "message": "unused variable" }] }
        });
        let text = user(&summary);
        assert!(text.starts_with("Build loja #7: falhou."), "{text}");
        assert!(text.contains("passo: cargo test") && text.contains("- teste testes::soma: esperava 5, veio 4"), "{text}");
        assert!(text.contains("src/lib.rs:1:42"), "{text}");
        assert!(text.contains("linha 40") && !text.contains("linha 20\n"), "only the last {TAIL} lines: {text}");
        assert!(SYSTEM.contains("Nunca invente"));
    }
}
