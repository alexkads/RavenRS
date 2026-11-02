//! Letting a model choose what to do: it names one tool, and the app does the rest.

use std::collections::HashMap;

use serde_json::Value;

/// Something a model can ask the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tool {
    /// Its name.
    pub name: &'static str,
    /// When to choose it, for the model.
    pub description: &'static str,
}

/// What a model chose.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolCall {
    /// The tool.
    pub name: String,
    /// Its arguments, as text.
    pub arguments: HashMap<String, String>,
}

impl ToolCall {
    /// An argument, if it came and is not blank.
    pub fn argument(&self, name: &str) -> Option<&str> {
        self.arguments.get(name).map(|value| value.trim()).filter(|value| !value.is_empty())
    }
}

/// Instructions that make a model answer with one tool. Pass them as the system
/// text of [`LocalAssistant::write`](crate::LocalAssistant::write), with the person's
/// words as the question.
pub fn tool_rules(intro: &str, tools: &[Tool]) -> String {
    let list: String = tools.iter().map(|tool| format!("- {}: {}\n", tool.name, tool.description)).collect();
    format!("{intro} Answer only with JSON in the form {{\"tool\":\"name\",\"args\":{{}}}}.\nTools:\n{list}")
}

/// Reads what a model answered. Anything but a known tool, however it is written,
/// is no choice at all, so a model cannot ask for what the app did not offer.
pub fn read_tool_call(answer: &str, tools: &[Tool]) -> Option<ToolCall> {
    let (start, end) = (answer.find('{')?, answer.rfind('}')?);
    let value: Value = serde_json::from_str(answer.get(start..=end)?).ok()?;
    let name = value.get("tool")?.as_str()?;
    tools.iter().find(|tool| tool.name == name)?;
    let arguments = value
        .get("args")
        .and_then(Value::as_object)
        .map(|args| {
            args.iter()
                .filter_map(|(key, value)| {
                    let text = match value {
                        Value::String(text) => text.clone(),
                        Value::Number(number) => number.to_string(),
                        _ => return None,
                    };
                    Some((key.clone(), text))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(ToolCall { name: name.to_owned(), arguments })
}

#[cfg(test)]
mod tests {
    use super::{Tool, read_tool_call, tool_rules};

    const TOOLS: &[Tool] = &[
        Tool { name: "balance", description: "the balance" },
        Tool { name: "spend", description: "adds a spending" },
    ];

    #[test]
    fn it_reads_a_choice_and_its_arguments() {
        let call = read_tool_call(r#"{"tool":"spend","args":{"what":"bread","amount":12.5}}"#, TOOLS);
        let call = call.expect("a choice");
        assert_eq!(call.name, "spend");
        assert_eq!(call.argument("what"), Some("bread"));
        assert_eq!(call.argument("amount"), Some("12.5"));
    }

    #[test]
    fn it_finds_the_json_among_other_words() {
        let call = read_tool_call("Sure:\n{\"tool\":\"balance\",\"args\":{}}\nDone", TOOLS);
        assert_eq!(call.expect("a choice").name, "balance");
    }

    #[test]
    fn anything_else_is_no_choice() {
        assert_eq!(read_tool_call(r#"{"tool":"delete_everything","args":{}}"#, TOOLS), None);
        assert_eq!(read_tool_call("not json", TOOLS), None);
        assert_eq!(read_tool_call(r#"{"tool":3}"#, TOOLS), None);
        assert_eq!(read_tool_call("", TOOLS), None);
    }

    #[test]
    fn a_blank_argument_is_no_argument() {
        let call = read_tool_call(r#"{"tool":"spend","args":{"what":"  "}}"#, TOOLS).expect("a choice");
        assert_eq!(call.argument("what"), None);
    }

    #[test]
    fn the_rules_list_the_tools() {
        let rules = tool_rules("Choose.", TOOLS);
        assert!(rules.contains("- balance: the balance"));
        assert!(rules.contains("\"tool\""));
    }
}
