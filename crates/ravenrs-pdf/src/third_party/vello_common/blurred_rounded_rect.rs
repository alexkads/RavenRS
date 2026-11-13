// Copyright 2025 the Vello Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Blurred, rounded rectangles.
use crate::third_party::vello_common::color::{AlphaColor, Srgb};
use crate::third_party::vello_common::kurbo::Rect;

/// A blurred, rounded rectangle.
#[derive(Debug)]
pub struct BlurredRoundedRectangle {
    /// The base rectangle to use for the blur effect.
    pub rect: Rect,
    /// The color of the blurred rectangle.
    pub color: AlphaColor<Srgb>,
    /// The radius of the rounded rectangle's corners.
    pub radius: f32,
    /// The standard deviation of the blur effect.
    pub std_dev: f32,
}
