//! Text with expressions inside it: `Olá {{ item.nome }}, o total é {{ [preço] * [qtd] }}`.
//!
//! Each `{{ … }}` holds an expression of the criteria language; a field path alone, such as
//! `item.nome`, needs no brackets. `\{{` writes two braces. Parsing reports every problem at
//! once, each with where it is in the text, so an editor can mark it. Rendering stops at the
//! first expression that fails, saying which one and why, so a failure never reads as an
//! empty value; and it stops when the text grows past a limit, so a preview cannot run away.

use std::ops::Range;

use crate::ast::Criteria;
use crate::eval::{EvalError, Record, Scope};

/// A text with expressions inside it.
#[derive(Clone, Debug, PartialEq)]
pub struct Template {
    parts: Vec<Part>,
}

/// A piece of a [`Template`].
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    /// Text written as it is.
    Text(String),
    /// An expression, and where its source is in the template's text.
    Expression(Criteria, Range<usize>),
}

/// A problem found in a template's text, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateError {
    /// The bytes of the template's text it is about.
    pub at: Range<usize>,
    /// What is wrong, in English.
    pub message: String,
}

/// Why a template could not be rendered.
#[derive(Clone, Debug, PartialEq)]
pub enum RenderError {
    /// The expression at these bytes failed.
    Failed(Range<usize>, EvalError),
    /// The text grew past this many bytes.
    TooLong(usize),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed(_, error) => write!(f, "{error}"),
            Self::TooLong(limit) => write!(f, "the text passed {limit} bytes"),
        }
    }
}

/// Whether `text` is a field path alone: words joined by dots.
fn bare_path(text: &str) -> bool {
    !text.is_empty()
        && text.split('.').all(|word| {
            let mut chars = word.chars();
            chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
                && chars.all(|c| c.is_alphanumeric() || c == '_')
        })
}

impl Template {
    /// Reads `text`, finding every problem at once.
    pub fn parse(text: &str) -> Result<Self, Vec<TemplateError>> {
        let mut parts = Vec::new();
        let mut errors = Vec::new();
        let mut plain = String::new();
        let mut i = 0;
        while i < text.len() {
            let rest = &text[i..];
            if rest.starts_with("\\{{") {
                plain.push_str("{{");
                i += 3;
            } else if let Some(open) = rest.strip_prefix("{{") {
                let Some(end) = open.find("}}") else {
                    errors.push(TemplateError {
                        at: i..text.len(),
                        message: "`{{` is never closed with `}}`".into(),
                    });
                    break;
                };
                if !plain.is_empty() {
                    parts.push(Part::Text(std::mem::take(&mut plain)));
                }
                let inner = &open[..end];
                let at = i..i + 2 + end + 2;
                let source = inner.trim();
                let code = if bare_path(source) { format!("[{source}]") } else { source.to_owned() };
                if source.is_empty() {
                    errors.push(TemplateError {
                        at: at.clone(),
                        message: "an expression is missing between `{{` and `}}`".into(),
                    });
                } else {
                    match Criteria::parse(&code) {
                        Ok(criteria) => parts.push(Part::Expression(criteria, at.clone())),
                        Err(error) => errors.push(TemplateError { at: at.clone(), message: error.message }),
                    }
                }
                i = at.end;
            } else {
                let c = rest.chars().next().unwrap_or_default();
                plain.push(c);
                i += c.len_utf8();
            }
        }
        if !plain.is_empty() {
            parts.push(Part::Text(plain));
        }
        if errors.is_empty() { Ok(Self { parts }) } else { Err(errors) }
    }

    /// Its pieces, in order.
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// Whether it has no expression: the text is what it renders.
    pub fn is_plain(&self) -> bool {
        self.parts.iter().all(|p| matches!(p, Part::Text(_)))
    }

    /// Every field path its expressions read, with where each expression is.
    pub fn references(&self) -> Vec<(Vec<String>, Range<usize>)> {
        let mut out = Vec::new();
        for part in &self.parts {
            if let Part::Expression(criteria, at) = part {
                let mut paths = Vec::new();
                properties(criteria, &mut paths);
                out.extend(paths.into_iter().map(|p| (p, at.clone())));
            }
        }
        out
    }

    /// The text with each expression's value in its place; an empty value writes nothing.
    pub fn render<R: Record>(&self, scope: &Scope<'_, R>) -> Result<String, RenderError> {
        self.render_within(scope, usize::MAX)
    }

    /// Like [`Self::render`], stopping when the text passes `limit` bytes.
    pub fn render_within<R: Record>(
        &self,
        scope: &Scope<'_, R>,
        limit: usize,
    ) -> Result<String, RenderError> {
        let mut out = String::new();
        for part in &self.parts {
            match part {
                Part::Text(text) => out.push_str(text),
                Part::Expression(criteria, at) => {
                    let value = scope.evaluate(criteria).map_err(|e| RenderError::Failed(at.clone(), e))?;
                    out.push_str(&value.to_text());
                }
            }
            if out.len() > limit {
                return Err(RenderError::TooLong(limit));
            }
        }
        Ok(out)
    }
}

/// The field paths `criteria` reads.
fn properties(criteria: &Criteria, out: &mut Vec<Vec<String>>) {
    let all = |list: &[Criteria], out: &mut Vec<Vec<String>>| list.iter().for_each(|c| properties(c, out));
    match criteria {
        Criteria::Property(path) => out.push(path.clone()),
        Criteria::Unary(_, a) | Criteria::IsNull(a) => properties(a, out),
        Criteria::Binary(_, a, b) => {
            properties(a, out);
            properties(b, out);
        }
        Criteria::Between(a, b, c) => {
            properties(a, out);
            properties(b, out);
            properties(c, out);
        }
        Criteria::In(a, list) => {
            properties(a, out);
            all(list, out);
        }
        Criteria::Group(_, list) | Criteria::Function(_, list) | Criteria::Custom(_, list) => all(list, out),
        Criteria::Aggregate(call) => {
            if let Some(path) = &call.collection {
                out.push(path.clone());
            }
            if let Some(condition) = &call.condition {
                properties(condition, out);
            }
            all(&call.args, out);
        }
        Criteria::Value(_) | Criteria::Parameter(_) => {}
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use super::*;
    use crate::Environment;
    use serde_json::json;

    #[test]
    fn a_field_path_alone_needs_no_brackets_and_text_stays_as_it_is() {
        let env = Environment::new();
        let item = json!({ "item": { "nome": "Ana" }, "qtd": 2, "preço": 3.5 });
        let t = Template::parse("Olá {{ item.nome }}, são {{ [qtd] * [preço] }} reais \\{{não}}")
            .expect("parses");
        assert_eq!(t.render(&Scope::new(&env).row(&item)).as_deref(), Ok("Olá Ana, são 7 reais {{não}}"));
        assert_eq!(t.references().len(), 3);
        assert!(Template::parse("só texto").expect("parses").is_plain());
    }

    #[test]
    fn every_problem_is_found_with_where_it_is() {
        let errors = Template::parse("a {{ [x] + }} b {{ }} c {{ [y]").expect_err("three problems");
        assert_eq!(errors.len(), 3);
        assert_eq!(errors[0].at, 2..13);
        assert!(errors[2].message.contains("never closed"));
    }

    #[test]
    fn a_failure_is_not_an_empty_value_and_a_runaway_text_stops() {
        let env = Environment::new();
        let item = json!({ "a": null });
        let scope = Scope::new(&env).row(&item);
        assert_eq!(
            Template::parse("[{{ a }}]").expect("parses").render(&scope).as_deref(),
            Ok("[]"),
            "null writes nothing"
        );
        let failed = Template::parse("x {{ Nope(1) }}").expect("parses").render(&scope);
        assert!(matches!(failed, Err(RenderError::Failed(ref at, _)) if *at == (2..15)), "{failed:?}");
        let long = Template::parse("{{ PadLeft('a', 5000, 'b') }}").expect("parses");
        assert_eq!(long.render_within(&scope, 100), Err(RenderError::TooLong(100)));
    }

    #[test]
    fn two_threads_each_render_with_their_own_environment() {
        let template = Template::parse("{{ [n] * 2 }}").expect("parses");
        let handles: Vec<_> = (1..=2)
            .map(|n| {
                let t = template.clone();
                std::thread::spawn(move || {
                    let env = Environment::new();
                    let item = json!({ "n": n });
                    t.render(&Scope::new(&env).row(&item))
                })
            })
            .collect();
        let got: Vec<_> = handles.into_iter().map(|h| h.join().expect("joins")).collect();
        assert_eq!(got, [Ok("2".to_owned()), Ok("4".to_owned())]);
    }
}
