//! The arithmetic and the drag of a reorder, shared by whatever lets its pieces be moved: rows,
//! tabs, and later docked panels.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{Modifiers, Point};
use ravenrs_layout::LayoutStyle;
use ravenrs_reactive::State;
use ravenrs_style::Semantic;

use crate::context::{BuildCx, DragHandler, DragPhase, NodeId, Paint, QuadPaint, Semantics};

/// How far the pointer goes before a press becomes a drag, so a click stays a click.
#[doc(hidden)]
pub const DRAG_THRESHOLD: f32 = 4.0;
/// How thick the line that shows the target is.
const LINE: f32 = 2.0;

/// A drag in progress.
#[derive(Clone, Copy, Debug, PartialEq)]
#[doc(hidden)]
pub struct Reorder {
    /// The piece being moved.
    pub from: usize,
    /// The gap it would land in: 0 is before the first piece, and `n` after the last.
    pub gap: usize,
    /// It went past the threshold; until then there is nothing to show.
    pub moving: bool,
    pressed_at: Point,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Reorder {
    /// The gap to mark, once there is something to show and it would change anything.
    pub fn marked_gap(&self) -> Option<usize> {
        (self.moving && landing(self.from, self.gap).is_some()).then_some(self.gap)
    }
}

/// How long each piece is along the strip, written by the pieces as they are laid out.
#[doc(hidden)]
pub type Extents = Rc<RefCell<Vec<f32>>>;

/// The gap nearest to `at`, measured from the start of the first piece.
#[doc(hidden)]
pub fn gap_at(extents: &[f32], spacing: f32, at: f32) -> usize {
    let mut start = 0.0;
    for (i, extent) in extents.iter().enumerate() {
        if at < start + extent / 2.0 {
            return i;
        }
        start += extent + spacing;
    }
    extents.len()
}

/// Where a piece taken from `from` lands when dropped in `gap`, if it moves at all. The piece
/// itself leaves the row: a gap after it is one position less.
#[doc(hidden)]
pub fn landing(from: usize, gap: usize) -> Option<usize> {
    let to = if gap > from { gap - 1 } else { gap };
    (to != from).then_some(to)
}

/// Where a key moves a piece: Alt with an arrow one place, and Alt with Home or End to the ends.
#[doc(hidden)]
pub fn key_target(
    key: &ravenrs_core::Key,
    modifiers: Modifiers,
    index: usize,
    count: usize,
    vertical: bool,
) -> Option<usize> {
    use ravenrs_core::Key;
    if !modifiers.alt || count == 0 {
        return None;
    }
    let to = match key {
        Key::ArrowUp if vertical => index.checked_sub(1)?,
        Key::ArrowLeft if !vertical => index.checked_sub(1)?,
        Key::ArrowDown if vertical => index + 1,
        Key::ArrowRight if !vertical => index + 1,
        Key::Home => 0,
        Key::End => count - 1,
        _ => return None,
    };
    (to < count && to != index).then_some(to)
}

/// Whether this is a key that moves pieces, even where it has nowhere left to move one to: it
/// must not go on to walk between them instead.
#[doc(hidden)]
pub fn is_move_key(key: &ravenrs_core::Key, modifiers: Modifiers) -> bool {
    use ravenrs_core::Key;
    modifiers.alt
        && matches!(
            key,
            Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End
        )
}

/// The handler for dragging piece `index`, which goes on the piece. Positions are read from the
/// piece's own frame, which moves with any scroll around it.
#[doc(hidden)]
pub fn drag_handler(
    state: State<Option<Reorder>>,
    extents: Extents,
    index: usize,
    spacing: f32,
    vertical: bool,
    on_reorder: Rc<dyn Fn(usize, usize)>,
) -> DragHandler {
    Rc::new(move |drag| match drag.phase {
        DragPhase::Start => {
            state.set(Some(Reorder { from: index, gap: index, moving: false, pressed_at: drag.position }))
        }
        DragPhase::Move => {
            // Nothing here when it was called off with Escape.
            let Some(mut now) = state.get() else { return };
            let distance = if vertical {
                (drag.position.y - now.pressed_at.y).abs()
            } else {
                (drag.position.x - now.pressed_at.x).abs()
            };
            now.moving |= distance > DRAG_THRESHOLD;
            let sizes = extents.borrow();
            let before: f32 = sizes.iter().take(index).map(|extent| extent + spacing).sum();
            let (pointer, own) =
                if vertical { (drag.position.y, drag.frame.y()) } else { (drag.position.x, drag.frame.x()) };
            now.gap = gap_at(&sizes, spacing, pointer - (own - before));
            drop(sizes);
            state.set_if_changed(Some(now));
        }
        DragPhase::End => {
            if let Some(done) = state.get()
                && done.moving
                && let Some(to) = landing(done.from, done.gap)
            {
                on_reorder(done.from, to);
            }
            state.set(None);
        }
    })
}

/// The line that marks the target, for a piece to carry at its start or at its end. It stays
/// out of the flow, so showing it moves nothing.
#[doc(hidden)]
pub fn marker(cx: &mut BuildCx<'_>, vertical: bool, across: f32, position: f32) -> NodeId {
    let accent = cx.theme().color(Semantic::Accent);
    let (x, y, width, height) = if vertical {
        (0.0, position - LINE / 2.0, across, LINE)
    } else {
        (position - LINE / 2.0, 0.0, LINE, across)
    };
    let node = cx.container(
        LayoutStyle {
            absolute: Some((x, y)),
            width: Some(width),
            height: Some(height),
            ..LayoutStyle::leaf()
        },
        &[],
        Paint::Quad(QuadPaint { corner_radius: LINE / 2.0, ..QuadPaint::new(accent) }),
    );
    cx.semantics(node, Semantics::hidden());
    node
}

/// A node of no size that screen readers announce when its text changes.
#[doc(hidden)]
pub fn announcer(cx: &mut BuildCx<'_>, text: String) -> NodeId {
    let node = cx.container(
        LayoutStyle {
            absolute: Some((0.0, 0.0)),
            width: Some(0.0),
            height: Some(0.0),
            ..LayoutStyle::leaf()
        },
        &[],
        Paint::None,
    );
    cx.semantics(node, Semantics::new(ravenrs_core::Role::Status).name(text));
    node
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_gap_goes_by_the_middle_of_each_piece() {
        let sizes = [10.0, 30.0, 10.0];
        assert_eq!(gap_at(&sizes, 0.0, -5.0), 0);
        assert_eq!(gap_at(&sizes, 0.0, 4.0), 0);
        assert_eq!(gap_at(&sizes, 0.0, 6.0), 1);
        assert_eq!(gap_at(&sizes, 0.0, 26.0), 2);
        assert_eq!(gap_at(&sizes, 0.0, 46.0), 3);
        assert_eq!(gap_at(&sizes, 10.0, 200.0), 3);
        assert_eq!(gap_at(&[], 0.0, 10.0), 0);
    }

    #[test]
    fn a_gap_next_to_the_piece_moves_nothing() {
        assert_eq!(landing(2, 2), None);
        assert_eq!(landing(2, 3), None);
        assert_eq!(landing(2, 0), Some(0));
        assert_eq!(landing(2, 5), Some(4));
    }

    #[test]
    fn keys_stop_at_the_ends_and_need_alt() {
        use ravenrs_core::Key;
        let alt = Modifiers { alt: true, ..Modifiers::default() };
        assert_eq!(key_target(&Key::ArrowUp, alt, 0, 3, true), None);
        assert_eq!(key_target(&Key::ArrowUp, alt, 1, 3, true), Some(0));
        assert_eq!(key_target(&Key::ArrowDown, alt, 2, 3, true), None);
        assert_eq!(key_target(&Key::ArrowDown, Modifiers::default(), 0, 3, true), None);
        assert_eq!(key_target(&Key::ArrowRight, alt, 0, 3, false), Some(1));
        assert_eq!(key_target(&Key::End, alt, 0, 3, false), Some(2));
    }
}
