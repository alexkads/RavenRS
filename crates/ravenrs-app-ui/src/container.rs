//! Where actions show.

use ravenrs_app::{
    Action, ActionCategory, ActionInput, ActionKind, ChoiceItem, FieldKind, FrameContext, Value,
};
use ravenrs_app_a11y::AccessibleText;
use ravenrs_core::Key;
use ravenrs_reactive::{Binding, State};
use ravenrs_widgets::{
    BuildCx, Button, ButtonSize, ButtonStyle, DropdownMenu, Hotkey, IconName, MenuItem, NodeId, TextField,
    Toolbar, ToolbarAction, View,
};

use crate::session::Session;

/// An icon by the name an action gives it.
pub(crate) fn icon_named(name: &str) -> Option<IconName> {
    IconName::ALL.iter().copied().find(|icon| icon.name() == name)
}

/// Reads keys such as `command+shift+s`.
fn hotkey_of(text: &str) -> Option<Hotkey> {
    let mut command = false;
    let mut shift = false;
    let mut key = None;
    for part in text.split('+') {
        match part.trim().to_lowercase().as_str() {
            "command" | "cmd" | "ctrl" => command = true,
            "shift" => shift = true,
            "delete" => key = Some(Key::Delete),
            "enter" => key = Some(Key::Enter),
            other => key = Some(Key::Character(other.to_owned())),
        }
    }
    let key = key?;
    let hotkey = match (&key, command) {
        (Key::Character(letter), true) => Hotkey::command(letter),
        _ => Hotkey::key(key),
    };
    Some(if shift { hotkey.shift() } else { hotkey })
}

/// What was typed, as what the action asks for, or the words of a person who cannot read it.
fn parameter_of(kind: &FieldKind, text: &str) -> Result<Value, String> {
    let typed = text.trim();
    let bad = || format!("\u{201c}{typed}\u{201d}?");
    // Both ways of writing a number people use: 1.234,56 and 1234.56.
    let number = || {
        let plain =
            if typed.contains(',') { typed.replace('.', "").replace(',', ".") } else { typed.to_owned() };
        plain.parse::<f64>().map_err(|_| bad())
    };
    match kind {
        _ if typed.is_empty() => Ok(Value::Null),
        FieldKind::Integer => number().map(|n| Value::Number(n.trunc())),
        FieldKind::Decimal(_) => number().map(Value::Number),
        FieldKind::Money => number().map(|n| Value::Number((n * 100.0).round())),
        FieldKind::Date | FieldKind::DateTime => {
            ravenrs_criteria::DateTime::parse(typed).map(Value::Date).ok_or_else(bad)
        }
        FieldKind::Boolean => match typed.to_lowercase().as_str() {
            "true" | "yes" | "sim" | "1" => Ok(Value::Bool(true)),
            "false" | "no" | "não" | "nao" | "0" => Ok(Value::Bool(false)),
            _ => Err(bad()),
        },
        FieldKind::Enum(options) => options
            .iter()
            .find(|option| option.code == typed || option.caption.eq_ignore_ascii_case(typed))
            .map(|option| Value::Text(option.code.clone()))
            .ok_or_else(bad),
        _ => Ok(Value::Text(typed.to_owned())),
    }
}

fn add_choice(
    menu: DropdownMenu,
    session: &Session,
    action: &Action,
    items: &[ChoiceItem],
    prefix: &str,
) -> DropdownMenu {
    items.iter().fold(menu, |menu, item| {
        let caption = format!("{prefix}{}", item.caption);
        if item.items.is_empty() {
            let (session, action, id) = (session.clone(), action.clone(), item.id.clone());
            menu.item(MenuItem::new(caption).on_select(move || {
                session.run_action(&action, ActionInput::Choice(id.clone()));
            }))
        } else {
            add_choice(menu, session, action, &item.items, &format!("{caption} / "))
        }
    })
}

/// A place on the screen for actions of some categories: what is in place shows, what is
/// off shows faded, and what does not fit folds into a menu. Keys work while it shows.
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ActionContainer {
    session: Session,
    categories: Vec<ActionCategory>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ActionContainer {
    /// A place for these categories, in the order the actions came; for all of them, with none.
    pub fn new(session: Session, categories: impl IntoIterator<Item = ActionCategory>) -> Self {
        Self { session, categories: categories.into_iter().collect() }
    }

    fn entry(
        &self,
        toolbar: Toolbar,
        action: &Action,
        primary: bool,
        drafts: &State<std::collections::BTreeMap<String, String>>,
    ) -> Toolbar {
        let session = self.session.clone();
        match action.kind() {
            ActionKind::SingleChoice(items) => {
                if !action.is_enabled() {
                    return toolbar.action(ToolbarAction::new(action.caption().to_owned()).disabled(true));
                }
                let menu = DropdownMenu::new(action.caption().to_owned()).size(ButtonSize::Small);
                // Declared with no options, it offers what the model has for the view it is on.
                let part = if action.id() == crate::ids::FILTERS { "Filters" } else { "Variants" };
                let found: Vec<ChoiceItem> = if items.is_empty() {
                    self.session
                        .frame()
                        .view()
                        .map(|view| self.session.choices(&view.id, part))
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(id, caption)| ChoiceItem::new(id, caption))
                        .collect()
                } else {
                    items.clone()
                };
                toolbar.view(120.0, add_choice(menu, &session, action, &found, ""))
            }
            ActionKind::Parametrized(kind) => {
                let id = action.id().to_owned();
                let (read, write) = (drafts.clone(), drafts.clone());
                let (read_id, write_id) = (id.clone(), id);
                let draft = Binding::new(
                    move || read.with(|all| all.get(&read_id).cloned().unwrap_or_default()),
                    move |text: String| {
                        write.update(|all| {
                            all.insert(write_id.clone(), text);
                        })
                    },
                );
                let (run, kind) = (action.clone(), kind.clone());
                let field = TextField::new(draft)
                    .placeholder(action.caption().to_owned())
                    .compact()
                    .disabled(!action.is_enabled())
                    .on_submit(move |text| match parameter_of(&kind, &text) {
                        Ok(value) => session.run_action(&run, ActionInput::Parameter(value)),
                        Err(problem) => session.set_error(Some(problem)),
                    });
                toolbar.view(200.0, field)
            }
            _ => {
                let mut entry =
                    ToolbarAction::new(action.caption().to_owned()).disabled(!action.is_enabled());
                let spoken = AccessibleText::of_action(action);
                if spoken.renamed {
                    entry = entry.accessible_name(spoken.name);
                }
                if let Some(description) = spoken.description {
                    entry = entry.accessible_description(description);
                }
                if primary {
                    entry = entry.style(ButtonStyle::Primary);
                }
                if let Some(icon) = action.icon_name().and_then(icon_named) {
                    entry = entry.icon(icon);
                    // Moving around is known by its arrows, and leaves room for the rest.
                    if *action.action_category() == ActionCategory::Navigation {
                        entry = entry.icon_only();
                    }
                }
                if let Some(hotkey) = action.hotkey_text().and_then(hotkey_of) {
                    entry = entry.hotkey(hotkey);
                }
                let run = action.clone();
                toolbar.action(entry.on_click(move || session.run_action(&run, ActionInput::None)))
            }
        }
    }
}

impl View for ActionContainer {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let drafts = cx.use_state(Default::default);
        let actions = self.session.frame().actions();
        let shown = actions.iter().filter(|action| {
            // What goes inside a form's layout does not show above it as well.
            let in_form = matches!(action.action_category(), ActionCategory::InForm(_));
            action.is_active()
                && (if self.categories.is_empty() {
                    !in_form
                } else {
                    self.categories.contains(action.action_category())
                })
        });
        let shown: Vec<&Action> = shown.collect();
        // A window has one action that matters most: saving what is open, or else adding a record.
        let primary = (self.session.frame().context() == FrameContext::Main)
            .then(|| {
                [crate::ids::SAVE, crate::ids::NEW]
                    .into_iter()
                    .find(|id| shown.iter().any(|action| action.id() == *id && action.is_enabled()))
            })
            .flatten();
        let mut toolbar = Toolbar::new();
        let mut last: Option<&ActionCategory> = None;
        for action in shown {
            if last.is_some_and(|category| category != action.action_category()) {
                toolbar = toolbar.separator();
            }
            last = Some(action.action_category());
            toolbar = self.entry(toolbar, action, primary == Some(action.id()), &drafts);
        }
        cx.child(&toolbar)
    }
}

/// One action as a button of its own, for a screen that places its commands itself.
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ActionButton {
    session: Session,
    id: String,
    style: ButtonStyle,
    size: ButtonSize,
    hide_when_off: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ActionButton {
    /// The button of the action with this id.
    pub fn new(session: Session, id: impl Into<String>) -> Self {
        Self {
            session,
            id: id.into(),
            style: ButtonStyle::Primary,
            size: ButtonSize::Small,
            hide_when_off: false,
        }
    }

    /// How it looks.
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// How big it is. Small by default.
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Takes it away while it cannot be used, instead of fading it. Only for a button at the end
    /// of a row of them, so nothing moves under the pointer.
    pub fn hide_when_off(mut self) -> Self {
        self.hide_when_off = true;
        self
    }
}

impl View for ActionButton {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let Some(action) = self.session.frame().action(&self.id) else { return cx.empty() };
        if !action.is_active() || (self.hide_when_off && !action.is_enabled()) {
            return cx.empty();
        }
        let mut button = Button::new(action.caption().to_owned())
            .size(self.size)
            .style(self.style)
            .disabled(!action.is_enabled());
        if let Some(icon) = action.icon_name().and_then(icon_named) {
            button = button.icon(icon);
        }
        let session = self.session.clone();
        cx.child(&button.on_click(move || session.run_action(&action, ActionInput::None)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_typed_becomes_what_the_action_asks_for() {
        assert_eq!(parameter_of(&FieldKind::Money, "1.234,56"), Ok(Value::Number(123_456.0)));
        assert_eq!(parameter_of(&FieldKind::Integer, "12"), Ok(Value::Number(12.0)));
        assert_eq!(parameter_of(&FieldKind::Boolean, "sim"), Ok(Value::Bool(true)));
        assert_eq!(parameter_of(&FieldKind::Text, "  a "), Ok(Value::Text("a".into())));
        assert_eq!(parameter_of(&FieldKind::Integer, ""), Ok(Value::Null));
        assert!(parameter_of(&FieldKind::Integer, "doze").is_err());
        assert!(parameter_of(&FieldKind::Date, "ontem").is_err());
        let kinds = FieldKind::Enum(vec![ravenrs_app::EnumOption::new("o", "Open")]);
        assert_eq!(parameter_of(&kinds, "open"), Ok(Value::Text("o".into())));
    }

    #[test]
    fn keys_are_read_from_their_words() {
        let saved = hotkey_of("command+shift+s");
        assert_eq!(saved, Some(Hotkey::command("s").shift()));
        assert_eq!(hotkey_of("Delete"), Some(Hotkey::key(Key::Delete)));
        assert_eq!(hotkey_of("command+s"), Some(Hotkey::command("s")));
    }

    #[test]
    fn icons_are_found_by_their_name() {
        assert_eq!(icon_named("trash-2"), Some(IconName::Trash2));
        assert_eq!(icon_named("no such thing"), None);
    }
}
