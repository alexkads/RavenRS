//! How the pointer and the keyboard change a selection of many, wherever it is shown.

use std::collections::BTreeSet;

use ravenrs_core::Modifiers;

use crate::selection_mode::SelectionMode;

/// The modifier that adds to a selection, by the habit of each system.
pub(crate) fn adds(modifiers: Modifiers) -> bool {
    modifiers.ctrl || modifiers.meta
}

/// Applies a pick of `index` to a selection: alone by default, toggled with Ctrl or Cmd, and
/// from `anchor` to it with Shift. Returns the new anchor. `key` says what the selection keeps
/// for each position, which may be the position itself.
pub(crate) fn pick<K: Ord>(
    set: &mut BTreeSet<K>,
    mode: SelectionMode,
    anchor: usize,
    index: usize,
    modifiers: Modifiers,
    key: impl Fn(usize) -> K,
) -> usize {
    match mode {
        SelectionMode::None => anchor,
        SelectionMode::Multiple if modifiers.shift => {
            if !adds(modifiers) {
                set.clear();
            }
            set.extend((anchor.min(index)..=anchor.max(index)).map(&key));
            anchor
        }
        SelectionMode::Multiple if adds(modifiers) => {
            let k = key(index);
            if !set.remove(&k) {
                set.insert(k);
            }
            index
        }
        _ => {
            set.clear();
            set.insert(key(index));
            index
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(shift: bool, ctrl: bool) -> Modifiers {
        Modifiers { shift, ctrl, ..Modifiers::default() }
    }

    #[test]
    fn alone_toggled_and_range() {
        let mut set = BTreeSet::new();
        let m = SelectionMode::Multiple;
        let a = pick(&mut set, m, 0, 3, with(false, false), |i| i);
        assert_eq!((a, set.iter().copied().collect::<Vec<_>>()), (3, vec![3]));
        let a = pick(&mut set, m, a, 5, with(false, true), |i| i);
        assert_eq!((a, set.len()), (5, 2));
        let a = pick(&mut set, m, a, 5, with(false, true), |i| i);
        assert_eq!((a, set.len()), (5, 1));
        let a = pick(&mut set, m, a, 2, with(true, false), |i| i);
        assert_eq!((a, set.into_iter().collect::<Vec<_>>()), (5, vec![2, 3, 4, 5]));
    }

    #[test]
    fn single_ignores_modifiers() {
        let mut set = BTreeSet::from([1_usize]);
        pick(&mut set, SelectionMode::Single, 1, 4, with(true, true), |i| i);
        assert_eq!(set.into_iter().collect::<Vec<_>>(), vec![4]);
    }
}
