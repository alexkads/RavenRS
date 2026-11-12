//! Where rows and columns sit, in logical pixels, with their custom sizes.

use ravenrs_sheet::model::Sheet;

/// Height of the column header strip.
pub(crate) const HEADER_HEIGHT: f32 = 24.0;
/// Width of the row header strip.
pub(crate) const ROW_HEADER_WIDTH: f32 = 48.0;

/// One direction of the grid: every line has the default size except the listed ones.
#[derive(Clone, Debug)]
pub(crate) struct Axis {
    default: f32,
    /// Lines with their own size (zero when hidden), in order.
    custom: Vec<(u32, f32)>,
    /// Lines past the last one do not exist.
    count: u32,
}

impl Axis {
    /// The columns of `sheet`.
    pub(crate) fn columns(sheet: &Sheet, zoom: f32) -> Self {
        let custom = sheet.cols.keys().map(|&c| (c, sheet.col_width(c) * zoom)).collect();
        Self { default: sheet.default_col_width * zoom, custom, count: ravenrs_sheet::base::MAX_COLS }
    }

    /// The rows of `sheet`.
    pub(crate) fn rows(sheet: &Sheet, zoom: f32) -> Self {
        let custom = sheet.rows.keys().map(|&r| (r, sheet.row_height(r) * zoom)).collect();
        Self { default: sheet.default_row_height * zoom, custom, count: ravenrs_sheet::base::MAX_ROWS }
    }

    /// Lines in this direction.
    pub(crate) fn count(&self) -> u32 {
        self.count
    }

    /// The size of line `i`.
    pub(crate) fn size(&self, i: u32) -> f32 {
        match self.custom.binary_search_by_key(&i, |&(k, _)| k) {
            Ok(at) => self.custom[at].1,
            Err(_) => self.default,
        }
    }

    /// Where line `i` starts.
    pub(crate) fn start(&self, i: u32) -> f32 {
        let mut at = i as f32 * self.default;
        for &(k, size) in &self.custom {
            if k >= i {
                break;
            }
            at += size - self.default;
        }
        at
    }

    /// The line under `position`, the last one past the end.
    pub(crate) fn at(&self, position: f32) -> u32 {
        let position = position.max(0.0);
        let (mut line, mut start) = (0u32, 0.0f32);
        for &(k, size) in &self.custom {
            let k_start = start + (k - line) as f32 * self.default;
            if position < k_start {
                break;
            }
            if position < k_start + size {
                return k;
            }
            line = k + 1;
            start = k_start + size;
        }
        if self.default <= 0.0 {
            return line.min(self.count - 1);
        }
        let more = ((position - start) / self.default).floor();
        (line as f32 + more).min((self.count - 1) as f32) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::Axis;

    fn axis() -> Axis {
        Axis { default: 20.0, custom: vec![(2, 50.0), (4, 0.0)], count: 100 }
    }

    #[test]
    fn custom_sizes_move_the_lines_after_them() {
        let a = axis();
        assert_eq!(a.start(0), 0.0);
        assert_eq!(a.start(2), 40.0);
        assert_eq!(a.start(3), 90.0);
        assert_eq!(a.start(5), 110.0, "a hidden line takes no room");
        assert_eq!(a.size(2), 50.0);
        assert_eq!(a.size(7), 20.0);
    }

    #[test]
    fn a_position_finds_its_line_and_skips_hidden_ones() {
        let a = axis();
        assert_eq!(a.at(0.0), 0);
        assert_eq!(a.at(39.9), 1);
        assert_eq!(a.at(40.0), 2);
        assert_eq!(a.at(89.0), 2);
        assert_eq!(a.at(90.0), 3);
        assert_eq!(a.at(110.0), 5);
        assert_eq!(a.at(1.0e9), 99, "stops at the last line");
        for i in [0, 1, 2, 3, 5, 6, 40] {
            assert_eq!(a.at(a.start(i)), i);
        }
    }
}
