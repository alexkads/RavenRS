//! Barcodes as shapes. What a code is comes from `ravenrs-barcode`, and it is all here too, so
//! everything that draws finds both in one place.

use ravenrs_core::{Color, Rect};

pub use ravenrs_barcode::*;

use crate::vector::{Shape, append_rectangle};

/// The shape of a pattern inside `rect`, centered and with its quiet zone.
pub fn shape(pattern: &Pattern, rect: Rect, color: Color) -> Shape {
    let mut path = Vec::new();
    match pattern {
        Pattern::Linear { modules, .. } => {
            let module = rect.width() / (modules.len() as f32 + 2.0 * QUIET_LINEAR as f32);
            let left = rect.x() + QUIET_LINEAR as f32 * module;
            for (start, length) in runs(modules) {
                let x = left + start as f32 * module;
                append_rectangle(&mut path, x, rect.y(), x + length as f32 * module, rect.max_y(), 0.0);
            }
        }
        Pattern::Matrix { size, cells } => {
            let module = rect.width().min(rect.height()) / (*size as f32 + 2.0 * QUIET_MATRIX as f32);
            let side = *size as f32 * module;
            let (left, top) =
                (rect.x() + (rect.width() - side) / 2.0, rect.y() + (rect.height() - side) / 2.0);
            for (row, cells) in cells.chunks((*size).max(1)).enumerate() {
                let y = top + row as f32 * module;
                for (start, length) in runs(cells) {
                    let x = left + start as f32 * module;
                    append_rectangle(&mut path, x, y, x + length as f32 * module, y + module, 0.0);
                }
            }
        }
    }
    Shape::filled(path, color)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::PathSegment;

    #[test]
    fn shapes_stay_inside_their_box_and_keep_the_quiet_zone() {
        let rect = Rect::new(10.0, 20.0, 200.0, 50.0);
        for (symbology, data) in
            [(Symbology::Code128, "ABC123"), (Symbology::Qr, "x"), (Symbology::Ean13, "400638133393")]
        {
            let pattern = encode(symbology, data).map_err(|e| e.to_string());
            let drawn = shape(&pattern.unwrap_or_else(|e| panic!("{e}")), rect, Color::BLACK);
            assert!(!drawn.path.is_empty());
            for segment in &drawn.path {
                if let PathSegment::Move(p) | PathSegment::Line(p) = segment {
                    assert!(p.x > rect.x() && p.x < rect.max_x() + 0.001, "{p:?}");
                    assert!(p.y >= rect.y() - 0.001 && p.y <= rect.max_y() + 0.001, "{p:?}");
                }
            }
        }
    }
}
