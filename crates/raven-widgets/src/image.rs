use std::fmt;
use std::rc::Rc;

use raven_core::{ImageData, ImageQuad, Rect};
use raven_layout::{Align, LayoutStyle};

use crate::View;
use crate::context::{BuildCx, NodeId, Paint};

/// How an image fills its frame when their aspect ratios differ.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContentMode {
    /// Fills the frame, cropping the overflowing part (CSS `object-fit: cover`).
    #[default]
    Cover,
    /// Shows the whole image, letterboxed inside the frame (`contain`).
    Fit,
    /// Stretches to the frame, ignoring the aspect ratio.
    Fill,
}

/// Why image bytes could not be decoded.
#[derive(Debug)]
pub struct DecodeError(String);

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the image could not be decoded: {}", self.0)
    }
}

impl std::error::Error for DecodeError {}

/// Decodes PNG or JPEG bytes into RGBA pixels.
#[cfg(feature = "decode")]
pub fn decode_image(bytes: &[u8]) -> Result<ImageData, DecodeError> {
    let decoded = image::load_from_memory(bytes).map_err(|e| DecodeError(e.to_string()))?.into_rgba8();
    let (w, h) = decoded.dimensions();
    ImageData::from_rgba8(w, h, decoded.into_raw()).ok_or_else(|| DecodeError("empty image".into()))
}

/// Shows an image. Without a size it takes the image's own size in logical
/// pixels; with only a width (or height) it keeps the aspect ratio.
#[derive(Clone, Debug)]
pub struct Image {
    data: ImageData,
    mode: ContentMode,
    radius: f32,
    width: Option<f32>,
    height: Option<f32>,
    stretch: bool,
    opacity: f32,
}

impl Image {
    pub fn new(data: ImageData) -> Self {
        Self {
            data,
            mode: ContentMode::Cover,
            radius: 0.0,
            width: None,
            height: None,
            stretch: false,
            opacity: 1.0,
        }
    }

    /// Decodes PNG or JPEG bytes (e.g. from `include_bytes!`).
    #[cfg(feature = "decode")]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        decode_image(bytes).map(Self::new)
    }

    pub fn content_mode(mut self, mode: ContentMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn size(self, width: f32, height: f32) -> Self {
        self.width(width).height(height)
    }

    /// Stretches across the parent stack (height follows the aspect ratio unless set).
    pub fn stretch(mut self) -> Self {
        self.stretch = true;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }
}

/// The destination rectangle and source UVs for drawing `aspect` into `frame`.
pub(crate) fn placement(mode: ContentMode, frame: Rect, aspect: f32) -> (Rect, [f32; 4]) {
    let frame_aspect = frame.width() / frame.height().max(f32::EPSILON);
    match mode {
        ContentMode::Fill => (frame, [0.0, 0.0, 1.0, 1.0]),
        ContentMode::Cover => {
            if frame_aspect > aspect {
                let visible = aspect / frame_aspect;
                let v0 = (1.0 - visible) / 2.0;
                (frame, [0.0, v0, 1.0, v0 + visible])
            } else {
                let visible = frame_aspect / aspect;
                let u0 = (1.0 - visible) / 2.0;
                (frame, [u0, 0.0, u0 + visible, 1.0])
            }
        }
        ContentMode::Fit => {
            let (w, h) = if frame_aspect > aspect {
                (frame.height() * aspect, frame.height())
            } else {
                (frame.width(), frame.width() / aspect)
            };
            let x = frame.x() + (frame.width() - w) / 2.0;
            let y = frame.y() + (frame.height() - h) / 2.0;
            (Rect::new(x, y, w, h), [0.0, 0.0, 1.0, 1.0])
        }
    }
}

impl View for Image {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let aspect = self.data.aspect_ratio();
        let (width, height) = match (self.width, self.height, self.stretch) {
            (None, None, false) => (Some(self.data.width() as f32), Some(self.data.height() as f32)),
            (w, h, _) => (w, h),
        };
        let style = LayoutStyle {
            width,
            height,
            aspect_ratio: (width.is_none() || height.is_none()).then_some(aspect),
            align_self: self.stretch.then_some(Align::Stretch),
            ..LayoutStyle::leaf()
        };
        let (data, mode, radius, opacity) = (self.data.clone(), self.mode, self.radius, self.opacity);
        cx.container(
            style,
            &[],
            Paint::Custom(Rc::new(move |frame: Rect, scene| {
                if frame.is_empty() {
                    return;
                }
                let (rect, uv) = placement(mode, frame, aspect);
                let mut quad = ImageQuad::new(rect, data.clone()).radius(radius);
                quad.uv = uv;
                quad.opacity = opacity;
                scene.push_image(quad);
            })),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_crops_and_fit_letterboxes() {
        let frame = Rect::new(0.0, 0.0, 200.0, 100.0);
        // A square image in a 2:1 frame.
        let (r, uv) = placement(ContentMode::Cover, frame, 1.0);
        assert_eq!(r, frame);
        assert_eq!(uv, [0.0, 0.25, 1.0, 0.75]);
        let (r, uv) = placement(ContentMode::Fit, frame, 1.0);
        assert_eq!(r, Rect::new(50.0, 0.0, 100.0, 100.0));
        assert_eq!(uv, [0.0, 0.0, 1.0, 1.0]);
    }

    #[cfg(feature = "decode")]
    #[test]
    fn decodes_png() {
        // A 1x1 opaque red PNG.
        let png: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
            0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
            0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xdd, 0x8d, 0xb0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
            0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        let image = decode_image(png).expect("valid png");
        assert_eq!((image.width(), image.height()), (1, 1));
        assert_eq!(image.pixels(), &[255, 0, 0, 255]);
        assert!(decode_image(b"not an image").is_err());
    }
}
