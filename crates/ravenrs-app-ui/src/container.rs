//! Where actions show.

use ravenrs_app::{Action, ActionCategory, ActionInput, ActionKind, ChoiceItem};
use ravenrs_core::Key;
use ravenrs_reactive::{Binding, State};
use ravenrs_widgets::{
    BuildCx, ButtonSize, DropdownMenu, Hotkey, IconName, MenuItem, NodeId, TextField, Toolbar, ToolbarAction,
    View,
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
pub struct ActionContainer {
    session: Session,
    categories: Vec<ActionCategory>,
}

impl ActionContainer {
    /// A place for these categories, in the order the actions came; for all of them, with none.
    pub fn new(session: Session, categories: impl IntoIterator<Item = ActionCategory>) -> Self {
        Self { session, categories: categories.into_iter().collect() }
    }

    fn entry(
        &self,
        toolbar: Toolbar,
        action: &Action,
        drafts: &State<std::collections::BTreeMap<String, String>>,
    ) -> Toolbar {
        let session = self.session.clone();
        match action.kind() {
            ActionKind::SingleChoice(items) => {
                if !action.is_enabled() {
                    return toolbar.action(ToolbarAction::new(action.caption().to_owned()).disabled(true));
                }
                let menu = DropdownMenu::new(action.caption().to_owned()).size(ButtonSize::Small);
                toolbar.view(120.0, add_choice(menu, &session, action, items, ""))
            }
            ActionKind::Parametrized(_) => {
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
                let run = action.clone();
                let field = TextField::new(draft)
                    .placeholder(action.caption().to_owned())
                    .compact()
                    .disabled(!action.is_enabled())
                    .on_submit(move |text| {
                        session.run_action(&run, ActionInput::Parameter(text.into()));
                    });
                toolbar.view(200.0, field)
            }
            _ => {
                let mut entry =
                    ToolbarAction::new(action.caption().to_owned()).disabled(!action.is_enabled());
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
            action.is_active()
                && (self.categories.is_empty() || self.categories.contains(action.action_category()))
        });
        let mut toolbar = Toolbar::new();
        let mut last: Option<&ActionCategory> = None;
        for action in shown {
            if last.is_some_and(|category| category != action.action_category()) {
                toolbar = toolbar.separator();
            }
            last = Some(action.action_category());
            toolbar = self.entry(toolbar, action, &drafts);
        }
        cx.child(&toolbar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
