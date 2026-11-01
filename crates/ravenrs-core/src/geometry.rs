//! Geometry in logical pixels. Conversion to physical pixels happens only in the
//! renderer and platform layers, using the window's scale factor.

use std::ops::{Add, Sub};

/// A point in logical pixels, origin at the top-left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal coordinate in logical pixels.
    pub x: f32,
    /// Vertical coordinate in logical pixels, growing downward.
    pub y: f32,
}

impl Point {
    /// The origin, `(0, 0)`.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Creates a point from logical-pixel coordinates.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl Add for Point {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Point {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

/// A size in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    /// Width in logical pixels.
    pub width: f32,
    /// Height in logical pixels.
    pub height: f32,
}

impl Size {
    /// A zero-area size.
    pub const ZERO: Self = Self { width: 0.0, height: 0.0 };

    /// Creates a size from logical-pixel dimensions.
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Scales both dimensions, e.g. logical to physical pixels.
    pub fn scale(self, factor: f32) -> Self {
        Self::new(self.width * factor, self.height * factor)
    }
}

/// An axis-aligned rectangle in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Top-left corner.
    pub origin: Point,
    /// Width and height.
    pub size: Size,
}

impl Rect {
    /// An empty rectangle at the origin.
    pub const ZERO: Self = Self { origin: Point::ZERO, size: Size::ZERO };

    /// Creates a rectangle from its top-left corner and dimensions, in logical pixels.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { origin: Point::new(x, y), size: Size::new(width, height) }
    }

    /// Creates a rectangle from a top-left corner and a size.
    pub const fn from_origin_size(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    /// Left edge.
    pub fn x(&self) -> f32 {
        self.origin.x
    }

    /// Top edge.
    pub fn y(&self) -> f32 {
        self.origin.y
    }

    /// Horizontal extent.
    pub fn width(&self) -> f32 {
        self.size.width
    }

    /// Vertical extent.
    pub fn height(&self) -> f32 {
        self.size.height
    }

    /// Right edge (`x + width`).
    pub fn max_x(&self) -> f32 {
        self.origin.x + self.size.width
    }

    /// Bottom edge (`y + height`).
    pub fn max_y(&self) -> f32 {
        self.origin.y + self.size.height
    }

    /// Center point.
    pub fn center(&self) -> Point {
        Point::new(self.origin.x + self.size.width * 0.5, self.origin.y + self.size.height * 0.5)
    }

    /// Half-open containment: the left/top edges are inside, the right/bottom edges are not.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.origin.x && p.x < self.max_x() && p.y >= self.origin.y && p.y < self.max_y()
    }

    /// Grows the rectangle by `amount` on every side (shrinks for negative values).
    pub fn inflate(&self, amount: f32) -> Self {
        Self::new(
            self.origin.x - amount,
            self.origin.y - amount,
            (self.size.width + amount * 2.0).max(0.0),
            (self.size.height + amount * 2.0).max(0.0),
        )
    }

    /// The overlapping area, or an empty rectangle at the origin of `self` when
    /// the two do not overlap.
    pub fn intersect(&self, other: &Rect) -> Self {
        let x0 = self.x().max(other.x());
        let y0 = self.y().max(other.y());
        let x1 = self.max_x().min(other.max_x());
        let y1 = self.max_y().min(other.max_y());
        if x1 <= x0 || y1 <= y0 {
            return Self::from_origin_size(self.origin, Size::ZERO);
        }
        Self::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// Returns `true` when the width or height is zero or negative.
    pub fn is_empty(&self) -> bool {
        self.size.width <= 0.0 || self.size.height <= 0.0
    }

    /// Returns the rectangle moved by `offset`, keeping its size.
    pub fn translate(&self, offset: Point) -> Self {
        Self::from_origin_size(self.origin + offset, self.size)
    }
}

/// Distances from each edge, used for padding.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    /// Top inset in logical pixels.
    pub top: f32,
    /// Right inset in logical pixels.
    pub right: f32,
    /// Bottom inset in logical pixels.
    pub bottom: f32,
    /// Left inset in logical pixels.
    pub left: f32,
}

impl Insets {
    /// No inset on any side.
    pub const ZERO: Self = Self::uniform(0.0);

    /// Same inset `v` on all four sides.
    pub const fn uniform(v: f32) -> Self {
        Self { top: v, right: v, bottom: v, left: v }
    }

    /// `vertical` on top and bottom, `horizontal` on left and right.
    pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self { top: vertical, right: horizontal, bottom: vertical, left: horizontal }
    }
}

impl From<f32> for Insets {
    fn from(v: f32) -> Self {
        Self::uniform(v)
    }
}

/// `(vertical, horizontal)`, like the two-value CSS `padding`.
impl From<(f32, f32)> for Insets {
    fn from((vertical, horizontal): (f32, f32)) -> Self {
        Self::symmetric(vertical, horizontal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_is_half_open() {
        let r = Rect::new(10.0, 10.0, 20.0, 10.0);
        assert!(r.contains(Point::new(10.0, 10.0)));
        assert!(r.contains(Point::new(29.9, 19.9)));
        assert!(!r.contains(Point::new(30.0, 15.0)));
        assert!(!r.contains(Point::new(15.0, 20.0)));
    }

    #[test]
    fn intersection() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert_eq!(a.intersect(&Rect::new(5.0, 5.0, 10.0, 10.0)), Rect::new(5.0, 5.0, 5.0, 5.0));
        assert!(a.intersect(&Rect::new(20.0, 0.0, 5.0, 5.0)).is_empty());
    }

    #[test]
    fn inflate_never_goes_negative() {
        let r = Rect::new(0.0, 0.0, 4.0, 4.0).inflate(-5.0);
        assert_eq!(r.size, Size::ZERO);
        let g = Rect::new(10.0, 10.0, 4.0, 4.0).inflate(3.0);
        assert_eq!(g, Rect::new(7.0, 7.0, 10.0, 10.0));
    }
}
