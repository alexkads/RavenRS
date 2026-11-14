//! What an application adds to its navigation: a brand at the top, counts beside items and the
//! command palette, which opens with Cmd/Ctrl+K on whatever the person can open.

use std::collections::HashMap;
use std::rc::Rc;

use ravenrs_widgets::{AnyView, BuildCx, Command, NodeId, SidebarBrand, View};

use crate::container::icon_named;
use crate::session::Session;

/// A count beside an item of the navigation, such as the lessons waiting for attendance. See
/// [`App::navigation_count`](crate::App::navigation_count).
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct NavigationCount {
    pub(crate) shown: String,
    pub(crate) spoken: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl NavigationCount {
    /// Shows `count`, such as `3` or `9+`. Screen readers hear it after the item's name.
    pub fn new(count: impl Into<String>) -> Self {
        Self { shown: count.into(), spoken: None }
    }

    /// What screen readers hear instead of the bare count, such as "1 with attendance to take".
    pub fn spoken(mut self, words: impl Into<String>) -> Self {
        self.spoken = Some(words.into());
        self
    }
}

pub(crate) type CountOf = Rc<dyn Fn(&Session) -> Option<NavigationCount>>;

/// The counts of the navigation, by the id of their item.
pub(crate) type Counts = Rc<HashMap<String, CountOf>>;

/// The mark of the application and the line under its name, at the top of the navigation.
#[derive(Clone)]
pub(crate) struct Brand {
    pub(crate) mark: Rc<AnyView>,
    pub(crate) subtitle: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Brand {
    /// The brand of the application called `title`.
    pub(crate) fn view(&self, title: String) -> SidebarBrand {
        let brand = SidebarBrand::new(SharedMark(self.mark.clone()), title);
        match &self.subtitle {
            Some(subtitle) => brand.subtitle(subtitle.clone()),
            None => brand,
        }
    }
}

struct SharedMark(Rc<AnyView>);

impl View for SharedMark {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.child(&**self.0)
    }
}

/// A command for each place of the navigation this person can open, under its group.
pub(crate) fn commands(session: &Session) -> Vec<Command> {
    let model = session.model();
    let Some(root) = model.get("NavigationItems") else { return Vec::new() };
    let mut commands = Vec::new();
    let mut add = |group: Option<&str>, item: &ravenrs_app::ModelNode, view: &str| {
        let (opening, target) = (session.clone(), view.to_owned());
        let key = item.text("ObjectKey").map(str::to_owned);
        let mut command = Command::new(item.text("Caption").unwrap_or(&item.id).to_owned(), move || {
            opening.open_navigation(&target, key.as_deref());
        });
        if let Some(icon) = item.text("Icon").and_then(icon_named) {
            command = command.icon(icon);
        }
        if let Some(group) = group {
            command = command.group(group.to_owned());
        }
        commands.push(command);
    };
    for group in root.in_order() {
        if let Some(view) = group.text("View") {
            if session.can_open(None, &group.id, view) {
                add(None, group, view);
            }
            continue;
        }
        let caption = group.text("Caption").unwrap_or(&group.id).to_owned();
        for item in group.in_order() {
            if let Some(view) = item.text("View")
                && session.can_open(Some(&group.id), &item.id, view)
            {
                add(Some(&caption), item, view);
            }
        }
    }
    commands
}
