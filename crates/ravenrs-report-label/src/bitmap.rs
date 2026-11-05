//! A picture of one label: every dot the printer can burn, black or white.

use image::ImageEncoder as _;
use image::codecs::png::PngEncoder;

/// A 1-bit picture. Rows start on whole bytes, the leftmost dot in the highest bit, and `1` is black.
/// Each language turns this into its own polarity.
#[derive(Clone, PartialEq, Eq)]
pub struct Bitmap {
    width: u32,
    height: u32,
    bits: Vec<u8>,
}

impl std::fmt::Debug for Bitmap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bitmap").field("width", &self.width).field("height", &self.height).finish()
    }
}

impl Bitmap {
    /// An all-white picture.
    pub fn new(width: u32, height: u32) -> Self {
        let bits = vec![0; width.div_ceil(8) as usize * height as usize];
        Self { width, height, bits }
    }

    /// Width in dots.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in dots.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// How many bytes a row takes.
    pub fn bytes_per_row(&self) -> usize {
        self.width.div_ceil(8) as usize
    }

    /// All the rows, top first.
    pub fn bits(&self) -> &[u8] {
        &self.bits
    }

    /// The rows from the top down.
    pub fn rows(&self) -> impl DoubleEndedIterator<Item = &[u8]> {
        self.bits.chunks_exact(self.bytes_per_row().max(1))
    }

    /// Whether a dot is black. Outside the picture it is white.
    pub fn get(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let at = y as usize * self.bytes_per_row() + (x / 8) as usize;
        self.bits[at] & (0x80 >> (x % 8)) != 0
    }

    /// Paints a dot. Outside the picture nothing happens.
    pub fn set(&mut self, x: u32, y: u32, black: bool) {
        if x >= self.width || y >= self.height {
            return;
        }
        let at = y as usize * self.bytes_per_row() + (x / 8) as usize;
        let mask = 0x80 >> (x % 8);
        if black {
            self.bits[at] |= mask;
        } else {
            self.bits[at] &= !mask;
        }
    }

    /// Paints the dots from `(x0, y0)` up to, and not including, `(x1, y1)`.
    pub fn fill(&mut self, x0: u32, y0: u32, x1: u32, y1: u32, black: bool) {
        for y in y0..y1.min(self.height) {
            for x in x0..x1.min(self.width) {
                self.set(x, y, black);
            }
        }
    }
}

/// The picture as a grayscale PNG, one pixel per dot, to see before printing.
pub fn bitmap_to_png(bitmap: &Bitmap) -> Vec<u8> {
    let mut gray = Vec::with_capacity(bitmap.width as usize * bitmap.height as usize);
    for y in 0..bitmap.height {
        for x in 0..bitmap.width {
            gray.push(if bitmap.get(x, y) { 0 } else { 255 });
        }
    }
    let mut png = Vec::new();
    // Writing to memory fails only on a picture with no dots, and that has no preview to give.
    let written = PngEncoder::new(&mut png).write_image(
        &gray,
        bitmap.width,
        bitmap.height,
        image::ExtendedColorType::L8,
    );
    if written.is_err() {
        png.clear();
    }
    png
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dots_land_in_the_right_bit_and_padding_stays_white() {
        let mut bitmap = Bitmap::new(10, 2);
        bitmap.set(0, 0, true);
        bitmap.set(9, 1, true);
        assert_eq!(bitmap.bytes_per_row(), 2);
        assert_eq!(bitmap.bits(), [0x80, 0, 0, 0x40]);
        assert!(bitmap.get(0, 0) && bitmap.get(9, 1) && !bitmap.get(1, 0));
        assert!(!bitmap.get(10, 0), "outside is white");
        bitmap.set(99, 99, true);
        bitmap.set(0, 0, false);
        assert_eq!(bitmap.bits(), [0, 0, 0, 0x40]);
    }

    #[test]
    fn the_preview_is_a_png() {
        let png = bitmap_to_png(&Bitmap::new(8, 8));
        assert_eq!(png.get(1..4), Some(&b"PNG"[..]));
    }
}
