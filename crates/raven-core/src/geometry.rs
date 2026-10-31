//! Geometry in logical pixels. Conversion to physical pixels happens only in the
//! renderer and platform layers, using the window's scale factor.

use std::ops::{Add, Sub};

/// A point in logical pixels, origin at the top-left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

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
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

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
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const ZERO: Self = Self {
        origin: Point::ZERO,
        size: Size::ZERO,
    };

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    pub const fn from_origin_size(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    pub fn x(&self) -> f32 {
        self.origin.x
    }

    pub fn y(&self) -> f32 {
        self.origin.y
    }

    pub fn width(&self) -> f32 {
        self.size.width
    }

    pub fn height(&self) -> f32 {
        self.size.height
    }

    pub fn max_x(&self) -> f32 {
        self.origin.x + self.size.width
    }

    pub fn max_y(&self) -> f32 {
        self.origin.y + self.size.height
    }

    pub fn center(&self) -> Point {
        Point::new(
            self.origin.x + self.size.width * 0.5,
            self.origin.y + self.size.height * 0.5,
        )
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

    pub fn translate(&self, offset: Point) -> Self {
        Self::from_origin_size(self.origin + offset, self.size)
    }
}

/// Distances from each edge, used for padding.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Insets {
    pub const ZERO: Self = Self::uniform(0.0);

    pub const fn uniform(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

impl From<f32> for Insets {
    fn from(v: f32) -> Self {
        Self::uniform(v)
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
    fn inflate_never_goes_negative() {
        let r = Rect::new(0.0, 0.0, 4.0, 4.0).inflate(-5.0);
        assert_eq!(r.size, Size::ZERO);
        let g = Rect::new(10.0, 10.0, 4.0, 4.0).inflate(3.0);
        assert_eq!(g, Rect::new(7.0, 7.0, 10.0, 10.0));
    }
}
