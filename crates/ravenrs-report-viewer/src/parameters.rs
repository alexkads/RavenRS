//! Asks for a report's parameters. Values are applied together, so a report is
//! not rendered again at each keystroke.

use std::collections::HashMap;

use ravenrs_core::Insets;
use ravenrs_layout::Align;
use ravenrs_reactive::{Binding, State};
use ravenrs_report::{Parameter, ParameterKind};
use ravenrs_style::{SPACING, Semantic};
use ravenrs_widgets::{
    AnyView, BuildCx, Button, ButtonSize, ButtonStyle, Checkbox, Date, DatePicker, FormField, HStack, NodeId,
    TextField, View, ViewExt,
};
use serde_json::{Value as Json, json};

/// Parameter values by name, as [`ravenrs_report::RenderOptions`] takes them.
pub type ParameterValues = HashMap<String, Json>;

/// The text a parameter panel shows.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ParameterLabels {
    /// Applies the values.
    pub apply: String,
    /// Goes back to the defaults.
    pub reset: String,
}

impl Default for ParameterLabels {
    fn default() -> Self {
        Self { apply: "Apply".to_owned(), reset: "Reset".to_owned() }
    }
}

impl ParameterLabels {
    /// The labels in Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self { apply: "Aplicar".to_owned(), reset: "Redefinir".to_owned() }
    }
}

/// Asks for values of parameters, with a control for each one's kind.
#[derive(Debug)]
pub struct ParameterPanel {
    parameters: Vec<Parameter>,
    applied: State<ParameterValues>,
    labels: ParameterLabels,
    today: Date,
}

impl ParameterPanel {
    /// A panel that writes to `applied` when the user applies the values.
    pub fn new(parameters: Vec<Parameter>, applied: State<ParameterValues>, today: Date) -> Self {
        Self { parameters, applied, labels: ParameterLabels::default(), today }
    }

    /// Sets the text the panel shows.
    pub fn labels(mut self, labels: ParameterLabels) -> Self {
        self.labels = labels;
        self
    }
}

/// A date as parameters take it.
fn date_text(date: Date) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

/// The date in a value such as `2026-11-01` or `2026-11-01T10:00:00`.
fn parse_date(value: &Json) -> Option<Date> {
    let text = value.as_str()?;
    let year = text.get(0..4)?.parse().ok()?;
    let month = text.get(5..7)?.parse().ok()?;
    let day = text.get(8..10)?.parse().ok()?;
    Date::new(year, month, day)
}

/// What a person typed as a number, with either decimal separator; `None` while it is not one.
pub(crate) fn parse_number(text: &str) -> Option<f64> {
    let text = text.trim();
    // "1.234,50" is thousands and decimals; "1,5" and "1.5" are decimals.
    let plain = if text.contains(',') { text.replace('.', "").replace(',', ".") } else { text.to_owned() };
    plain.parse().ok().filter(|n: &f64| n.is_finite())
}

/// A value as a text field shows it.
fn value_text(value: Option<&Json>) -> String {
    match value {
        Some(Json::String(text)) => text.clone(),
        Some(Json::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

impl View for ParameterPanel {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        // What is being edited; it becomes the applied values on Apply.
        let draft: State<ParameterValues> = cx.use_state(|| self.applied.get());
        let mut items: Vec<AnyView> = Vec::with_capacity(self.parameters.len() + 2);
        for parameter in &self.parameters {
            let name = parameter.name.clone();
            let caption = parameter.label.clone().unwrap_or_else(|| name.clone());
            let default = parameter.default.clone();
            let current = {
                let (draft, name, default) = (draft.clone(), name.clone(), default.clone());
                move || draft.with(|values| values.get(&name).cloned()).or_else(|| default.clone())
            };
            let store = {
                let (draft, name) = (draft.clone(), name.clone());
                move |value: Json| {
                    draft.update(|values| {
                        values.insert(name.clone(), value);
                    });
                }
            };
            let control: AnyView = match parameter.kind {
                ParameterKind::Boolean => {
                    let read = current.clone();
                    Box::new(Checkbox::new(
                        caption.clone(),
                        Binding::new(
                            move || read().is_some_and(|v| v.as_bool().unwrap_or(false)),
                            move |checked: bool| store(json!(checked)),
                        ),
                    ))
                }
                ParameterKind::Date => {
                    let read = current.clone();
                    let month = current().as_ref().and_then(parse_date).unwrap_or(self.today);
                    Box::new(
                        DatePicker::new(
                            Binding::new(
                                move || read().as_ref().and_then(parse_date),
                                move |date: Option<Date>| {
                                    store(date.map_or(Json::Null, |d| json!(date_text(d))));
                                },
                            ),
                            month,
                        )
                        .today(self.today)
                        .width(150.0)
                        .label(caption.clone()),
                    )
                }
                _ => {
                    // The text as typed is kept apart, so "1," can become "1,5".
                    let typed: State<Option<String>> = cx.use_state(|| None);
                    let is_number = parameter.kind == ParameterKind::Number;
                    let (read, read_typed, write_typed) = (current.clone(), typed.clone(), typed);
                    Box::new(
                        TextField::new(Binding::new(
                            move || read_typed.get().unwrap_or_else(|| value_text(read().as_ref())),
                            move |text: String| {
                                let value = if is_number {
                                    parse_number(&text).map_or(Json::Null, |n| json!(n))
                                } else {
                                    json!(text)
                                };
                                store(value);
                                write_typed.set(Some(text));
                            },
                        ))
                        .compact()
                        .width(if is_number { 110.0 } else { 180.0 }),
                    )
                }
            };
            items.push(if parameter.kind == ParameterKind::Boolean {
                control
            } else {
                Box::new(FormField::new(caption, ControlBox(control)))
            });
        }
        let (write, source) = (self.applied.clone(), draft.clone());
        items.push(Box::new(
            Button::new(self.labels.apply.clone())
                .size(ButtonSize::Small)
                .on_click(move || write.set(source.get())),
        ));
        let (write, source) = (self.applied.clone(), draft);
        items.push(Box::new(
            Button::new(self.labels.reset.clone())
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Small)
                .on_click(move || {
                    source.set(HashMap::new());
                    write.set(HashMap::new());
                }),
        ));
        cx.child(
            &HStack::new(items)
                .spacing(SPACING.s3)
                .align(Align::End)
                .wrap()
                .padding(Insets::symmetric(SPACING.s2, SPACING.s4))
                .background(Semantic::Surface),
        )
    }
}

/// Lets a boxed control sit in a form field.
struct ControlBox(AnyView);

impl View for ControlBox {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.child(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_with_either_separator() {
        assert_eq!(parse_number("1,5"), Some(1.5));
        assert_eq!(parse_number("1.5"), Some(1.5));
        assert_eq!(parse_number("1.234,50"), Some(1234.5));
        assert_eq!(parse_number(" 10 "), Some(10.0));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("inf"), None);
    }

    #[test]
    fn dates_read_with_or_without_a_time() {
        let date = parse_date(&json!("2026-11-01T10:00:00")).map(date_text);
        assert_eq!(date.as_deref(), Some("2026-11-01"));
        assert_eq!(parse_date(&json!("2026-02-30")), None);
        assert_eq!(parse_date(&json!("ontem")), None);
        assert_eq!(parse_date(&json!(5)), None);
    }
}
