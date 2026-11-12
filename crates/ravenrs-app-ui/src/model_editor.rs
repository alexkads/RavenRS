//! The application's model as a tree, whose settings a person who can may change, in their own
//! layer. It is kept simple on purpose: a tree with its settings beside it, one at a time.

use std::collections::BTreeSet;

use ravenrs_app::ModelNode;
use ravenrs_layout::Align;
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, ButtonSize, ButtonStyle, FormField, HStack, NodeId, ScrollView, Text,
    TextField, Tone, TreeNode, TreeView, VStack, View, ViewExt,
};

use crate::Session;

const TREE_HEIGHT: f32 = 520.0;
const SEPARATOR: char = '/';

fn tree_of(node: &ModelNode, path: &str) -> TreeNode {
    let key = if path.is_empty() { node.id.clone() } else { format!("{path}{SEPARATOR}{}", node.id) };
    let children = node.children.iter().map(|child| tree_of(child, &key));
    TreeNode::new(key.clone(), node.id.clone()).children(children.collect::<Vec<_>>())
}

/// The application's model, for whoever may change it.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ModelEditor {
    session: Session,
    selected: State<Option<String>>,
    expanded: State<BTreeSet<String>>,
    search: State<String>,
    drafts: State<std::collections::BTreeMap<String, String>>,
    error: State<Option<String>>,
}

impl std::fmt::Debug for ModelEditor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelEditor").finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ModelEditor {
    /// Over the model of this session.
    pub fn new(session: Session) -> Self {
        Self {
            session,
            selected: State::new(None),
            expanded: State::new(BTreeSet::new()),
            search: State::new(String::new()),
            drafts: State::new(std::collections::BTreeMap::new()),
            error: State::new(None),
        }
    }

    fn settings(&self, path: &str) -> AnyView {
        let model = self.session.model();
        let ids: Vec<&str> = path.split(SEPARATOR).collect();
        let Some(node) = model.at(&ids) else { return Box::new(Text::new(path.to_owned())) };
        let words = self.session.words();
        let mut fields: Vec<AnyView> = Vec::new();
        for name in node.properties.keys() {
            let (slot, stored) = (format!("{path}{SEPARATOR}{name}"), node.typed(name).unwrap_or_default());
            let (read, write) = (self.drafts.clone(), self.drafts.clone());
            let (reading, writing, shown) = (slot.clone(), slot, stored);
            let binding = Binding::new(
                move || read.get().get(&reading).cloned().unwrap_or_else(|| shown.clone()),
                move |text: String| {
                    let mut drafts = write.peek().clone();
                    drafts.insert(writing.clone(), text);
                    write.set(drafts);
                },
            );
            fields.push(Box::new(FormField::new(name.clone(), TextField::new(binding))));
        }
        let (session, error, drafts) = (self.session.clone(), self.error.clone(), self.drafts.clone());
        let target = path.to_owned();
        let apply = Button::new(words.apply.clone()).on_click(move || {
            let prefix = format!("{target}{SEPARATOR}");
            let waiting = drafts.peek().clone();
            let ids: Vec<&str> = target.split(SEPARATOR).collect();
            let mut problem = None;
            for (slot, text) in &waiting {
                if let Some(name) = slot.strip_prefix(&prefix).filter(|name| !name.contains(SEPARATOR))
                    && let Err(why) = session.set_model_setting(&ids, name, text)
                {
                    problem = Some(why);
                }
            }
            error.set(problem);
            drafts.set(std::collections::BTreeMap::new());
        });
        let (session, error, drafts) = (self.session.clone(), self.error.clone(), self.drafts.clone());
        let restore = Button::new(words.restore_model.clone())
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Small)
            .on_click(move || {
                error.set(session.restore_model_settings().err());
                drafts.set(std::collections::BTreeMap::new());
            });
        Box::new(
            VStack::new((
                Text::new(path.to_owned()).style(TextStyle::Caption),
                VStack::new(fields).spacing(8.0).align(Align::Stretch),
                HStack::new(((!node.properties.is_empty()).then_some(apply), restore))
                    .spacing(8.0)
                    .align(Align::Center),
            ))
            .spacing(12.0)
            .align(Align::Stretch),
        )
    }
}

impl View for ModelEditor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let words = self.session.words();
        let allowed = self
            .session
            .frame()
            .security()
            .is_some_and(|security| security.with(ravenrs_app::Permissions::can_edit_model));
        if !allowed {
            return cx.child(
                &Alert::new(words.model_editor.clone(), words.access_denied.clone()).tone(Tone::Danger),
            );
        }
        let model = self.session.model();
        let nodes: Vec<TreeNode> = model.children.iter().map(|node| tree_of(node, "")).collect();
        let (read, write) = (self.selected.clone(), self.selected.clone());
        let selected = Binding::new(move || read.get(), move |key| write.set(key));
        let (read, write) = (self.expanded.clone(), self.expanded.clone());
        let expanded = Binding::new(move || read.get(), move |keys| write.set(keys));
        let (read, write) = (self.search.clone(), self.search.clone());
        let search = Binding::new(move || read.get(), move |text| write.set(text));
        let tree = TreeView::new(nodes)
            .selected(selected)
            .expanded(expanded)
            .filter(self.search.get())
            .height(TREE_HEIGHT)
            .label(words.model_editor.clone());
        let side: AnyView = match self.selected.get() {
            Some(path) => self.settings(&path),
            None => Box::new(
                Text::new(words.pick_model_node.clone())
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
            ),
        };
        let content = VStack::new((
            self.error.get().map(|why| Alert::new(words.model_editor.clone(), why).tone(Tone::Danger)),
            HStack::new((
                VStack::new((TextField::new(search).placeholder(words.search_model.clone()), tree))
                    .spacing(8.0)
                    .align(Align::Stretch)
                    .grow(1.0),
                ScrollView::new((side,)).grow(1.0),
            ))
            .spacing(16.0)
            .align(Align::Stretch),
        ))
        .spacing(12.0)
        .align(Align::Stretch);
        cx.child(&content)
    }
}
