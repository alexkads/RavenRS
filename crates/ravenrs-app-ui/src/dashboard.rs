//! Other views together.

use std::collections::BTreeMap;

use ravenrs_app::{FrameContext, ViewInfo, ViewKind};
use ravenrs_reactive::State;
use ravenrs_widgets::{AnyView, BuildCx, NodeId, Text, Tile, TileLayout, TileSpot, View, ViewExt};

use crate::{ListView, Session};

/// How many cells wide a piece starts, and how tall.
const ITEM_COLUMNS: u16 = 2;
const ITEM_ROWS: u16 = 2;

/// Several views together, each in a frame of its own, as the model says. A dashboard is bound to
/// no records, and people can move and resize its pieces.
#[derive(Debug)]
pub struct DashboardView {
    session: Session,
    view: ViewInfo,
}

impl DashboardView {
    /// The dashboard of this view in the model.
    pub fn new(session: Session, view: ViewInfo) -> Self {
        Self { session, view }
    }
}

impl View for DashboardView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let model = self.session.model();
        let Some(items) = model.at(&["Views", &self.view.id, "Items"]) else {
            return cx.child(&Text::new(self.view.id.clone()));
        };
        let order = items.in_order();
        let frames: State<BTreeMap<String, Session>> = cx.use_state(BTreeMap::new);
        let spots: State<Vec<TileSpot>> = cx.use_state(|| {
            order.iter().map(|item| TileSpot::new(item.id.clone(), ITEM_COLUMNS, ITEM_ROWS)).collect()
        });
        let mut tiles = TileLayout::new(spots.binding());
        for item in order {
            let caption = item.text("Caption").unwrap_or(&item.id).to_owned();
            let content: AnyView = if let Some(name) = item.text("Control") {
                match self.session.control(name) {
                    Some(control) => control(),
                    None => Box::new(Text::new(name.to_owned())),
                }
            } else if let Some(info) = item.text("View").and_then(|id| ViewInfo::from_model(&model, id).ok())
            {
                let kept = frames.peek().get(&item.id).cloned();
                let session = kept.unwrap_or_else(|| {
                    let session = self.session.child(FrameContext::Nested);
                    session.frame().set_view(Some(info.clone()));
                    frames.update(|all| {
                        all.insert(item.id.clone(), session.clone());
                    });
                    session
                });
                match info.kind {
                    ViewKind::List => {
                        let opener = self.session.clone();
                        let entity = info.entity.clone().unwrap_or_default();
                        Box::new(
                            ListView::new(session.clone(), session.frame(), info)
                                .plain_toolbar()
                                .on_open(std::rc::Rc::new(move |key: &ravenrs_app::Value| {
                                    opener.open_detail(&entity, key);
                                }))
                                .grid_shape(|grid| grid.toolbar(false).filter_row(false))
                                .stretch()
                                .grow(1.0),
                        )
                    }
                    _ => Box::new(Text::new(info.id)),
                }
            } else {
                Box::new(Text::new(item.id.clone()))
            };
            tiles = tiles.tile(Tile::new(item.id.clone(), caption, content));
        }
        cx.child(&tiles)
    }
}
