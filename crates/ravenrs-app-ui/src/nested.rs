//! The records of another entity that point to the one on the form.

use std::rc::Rc;

use ravenrs_app::{Criteria, EntityInfo, FieldKind, FrameContext, Value, ViewInfo};
use ravenrs_layout::Align;
use ravenrs_reactive::State;
use ravenrs_widgets::{BuildCx, DataGrid, NodeId, Text, VStack, View, ViewExt};

use crate::shell::PopupHost;
use crate::{ActionContainer, ListView, Session};

/// How tall a list inside a form is.
const NESTED_HEIGHT: f32 = 240.0;

/// Which record a list inside a form belongs to, for the actions that link and unlink.
pub(crate) struct NestedOwner {
    /// The entity of the records listed.
    pub target: String,
    /// Their field that points to the owner.
    pub field: String,
    /// The owner's key.
    pub key: Value,
    /// What the list to pick from has to meet.
    pub link_filter: Option<Criteria>,
}

/// The field of `target` that points to `owner`: the one named, or the first that does.
pub(crate) fn back_reference(
    session: &Session,
    owner: &EntityInfo,
    target: &str,
    named: Option<&str>,
) -> Option<String> {
    let target = session.entity(target)?;
    let points_back = |kind: &FieldKind| matches!(kind, FieldKind::Reference(to) if *to == owner.name);
    target
        .fields
        .iter()
        .find(|field| named.map_or(points_back(&field.kind), |name| field.name == name))
        .map(|field| field.name.clone())
}

/// A list inside a form, in a frame of its own. What links and unlinks is kept right away.
pub(crate) struct NestedList {
    parent: Session,
    owner: Option<Value>,
    target: String,
    field: Option<String>,
    link_filter: Option<String>,
}

impl NestedList {
    pub fn new(
        parent: Session,
        owner: &EntityInfo,
        key: Option<Value>,
        target: &str,
        named: Option<&str>,
        link_filter: Option<&str>,
    ) -> Self {
        let field = back_reference(&parent, owner, target, named);
        Self {
            parent,
            owner: key,
            target: target.to_owned(),
            field,
            link_filter: link_filter.map(str::to_owned),
        }
    }
}

impl View for NestedList {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let words = self.parent.words();
        let (Some(key), Some(field)) = (self.owner.clone(), self.field.clone()) else {
            return cx.child(&Text::new(words.save_first.clone()));
        };
        let id = format!("{}_ListView", self.target);
        let Ok(view) = ViewInfo::from_model(&self.parent.model(), &id) else {
            return cx.child(&Text::new(id));
        };
        let held: State<Option<Session>> = cx.use_state(|| None);
        let kept = held.peek().clone();
        let child = kept.unwrap_or_else(|| {
            let child = self.parent.child(FrameContext::Nested);
            child.frame().set_view(Some(view.clone()));
            held.set(Some(child.clone()));
            child
        });
        child.frame().set_extension(Rc::new(NestedOwner {
            target: self.target.clone(),
            field: field.clone(),
            key: key.clone(),
            link_filter: self
                .link_filter
                .as_deref()
                .and_then(|text| crate::editors::filter_for(text, self.parent.form_record().as_ref())),
        }));
        let opener = self.parent.clone();
        let target = self.target.clone();
        let list = ListView::new(child.clone(), child.frame(), view)
            .filter(Some(Criteria::property(&field).equals(Criteria::value(key))))
            .plain_toolbar()
            .on_open(Rc::new(move |key: &Value| opener.open_detail(&target, key)))
            .grid_shape(|grid: DataGrid| grid.toolbar(false).filter_row(false).footer(false));
        cx.child(
            &VStack::new((
                ActionContainer::new(child.clone(), []),
                list.height(NESTED_HEIGHT),
                child.confirmations(),
                PopupHost::new(child),
            ))
            .spacing(8.0)
            .align(Align::Stretch),
        )
    }
}
