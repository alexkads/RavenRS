#![doc = include_str!("../README.md")]

mod draw;
mod mov;
mod text;

use std::cell::RefCell;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::rc::Rc;

use openh264::OpenH264API;
use openh264::encoder::{
    BitRate, Complexity, Encoder, EncoderConfig, FrameRate, IntraFramePeriod, Profile, RateControlMode,
    UsageType, VuiConfig,
};
use openh264::formats::{RgbaSliceU8, YUVBuffer};
use ravenrs_core::Size;
use ravenrs_movie::{Movie, MovieFrame, MovieLabels, Narration, SocialFrame, Soundtrack};
use ravenrs_style::Theme;
use ravenrs_widgets::{BuildCx, NodeId, View};

pub use text::{chapters, subtitles};

/// The film being drawn and where in it, shared with the view that draws it.
type Playing = Rc<RefCell<(Movie, u64)>>;

/// How many samples of sound a second the video carries.
const SOUND_RATE: u32 = 48_000;

/// How a film fills the picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Layout {
    /// The film fills the picture, with its boxes over it, as in the player.
    Full,
    /// A headline, the film and the words under it, for tall and square screens.
    Social,
}

/// The size, pace and layout of a video, with ready ones for the usual places.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct VideoFormat {
    /// Pixels across.
    pub width: u32,
    /// Pixels down.
    pub height: u32,
    /// Pictures a second.
    pub fps: u32,
    /// How the film fills the picture.
    pub layout: Layout,
    /// The longest video the place takes, in seconds.
    pub longest_seconds: Option<u32>,
}

impl VideoFormat {
    /// 1920 × 1080: video sites and presentations.
    pub const WIDE: Self =
        Self { width: 1920, height: 1080, fps: 30, layout: Layout::Full, longest_seconds: None };
    /// 1080 × 1920: short vertical videos (YouTube Shorts, Instagram Reels, TikTok).
    pub const VERTICAL: Self =
        Self { width: 1080, height: 1920, fps: 30, layout: Layout::Social, longest_seconds: Some(180) };
    /// 1080 × 1350: a post in a feed.
    pub const PORTRAIT: Self =
        Self { width: 1080, height: 1350, fps: 30, layout: Layout::Social, longest_seconds: Some(60) };
    /// 1080 × 1080: a square post.
    pub const SQUARE: Self =
        Self { width: 1080, height: 1080, fps: 30, layout: Layout::Social, longest_seconds: Some(60) };

    /// A format of any size; even sizes, since the picture is coded in 2 × 2 blocks.
    pub fn new(width: u32, height: u32, layout: Layout) -> Self {
        Self { width: width & !1, height: height & !1, fps: 30, layout, longest_seconds: None }
    }

    /// Another pace, in pictures a second.
    pub fn fps(mut self, fps: u32) -> Self {
        self.fps = fps.clamp(1, 120);
        self
    }
}

/// Why a video could not be made.
#[derive(Debug)]
#[non_exhaustive]
pub enum VideoError {
    /// There is no GPU to draw on.
    NoGpu,
    /// The picture coder failed.
    Coder(String),
    /// Writing the file failed.
    Io(std::io::Error),
    /// There is nothing to show.
    Empty,
}

impl std::fmt::Display for VideoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoGpu => write!(f, "there is no GPU to draw the video on"),
            Self::Coder(why) => write!(f, "the picture coder failed: {why}"),
            Self::Io(error) => write!(f, "the video could not be written: {error}"),
            Self::Empty => write!(f, "there is no film to show"),
        }
    }
}

impl std::error::Error for VideoError {}

impl From<std::io::Error> for VideoError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// What came out.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct VideoReport {
    /// How many pictures.
    pub frames: u64,
    /// How long, in milliseconds.
    pub duration_ms: u64,
    /// How big the file is, in bytes.
    pub bytes: u64,
    /// What the boxes say that had no recording, so it was shown and not said.
    pub unrecorded: Vec<String>,
    /// Whether it is longer than the format's place takes.
    pub too_long: bool,
}

/// Turns films into a video file (QuickTime, H.264 with plain sound), the films one
/// after the other, with their boxes, clicks and narration. See the
/// [crate documentation](crate).
#[derive(Clone, Debug)]
pub struct Video {
    films: Vec<Movie>,
    format: VideoFormat,
    narration: Option<Narration>,
    labels: MovieLabels,
    theme: Theme,
    brand: String,
    quality: u32,
}

/// The moment the video is at, read by the view while drawing.
#[derive(Clone)]
struct Moment {
    film: Playing,
    format: VideoFormat,
    logical: Size,
    labels: MovieLabels,
    brand: String,
}

impl View for Moment {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        // Woken every time the clock moves, so each picture is built anew.
        cx.wake_at(cx.now());
        let (movie, at) = self.film.borrow().clone();
        match self.format.layout {
            Layout::Social => cx.child(&SocialFrame::new(movie, at, self.logical).brand(self.brand.clone())),
            Layout::Full => {
                cx.child(&MovieFrame::new(movie, at).width(self.logical.width).labels(self.labels.clone()))
            }
        }
    }
}

impl Video {
    /// A video of `film` in `format`.
    pub fn new(film: Movie, format: VideoFormat) -> Self {
        Self {
            films: vec![film],
            format,
            narration: None,
            labels: MovieLabels::default(),
            theme: Theme::raven_light(),
            brand: String::new(),
            quality: 8_000_000,
        }
    }

    /// Another film after the ones it has.
    pub fn then(mut self, film: Movie) -> Self {
        self.films.push(film);
        self
    }

    /// The voice of the boxes, recorded ahead.
    pub fn narration(mut self, narration: Narration) -> Self {
        self.narration = Some(narration);
        self
    }

    /// The words of the boxes.
    pub fn labels(mut self, labels: MovieLabels) -> Self {
        self.labels = labels;
        self
    }

    /// The theme of what is drawn around the film.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// A name at the foot of the social layout.
    pub fn brand(mut self, brand: impl Into<String>) -> Self {
        self.brand = brand.into();
        self
    }

    /// Bits a second of picture (8 million by default).
    pub fn bitrate(mut self, bits: u32) -> Self {
        self.quality = bits;
        self
    }

    /// How long the whole video is, in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.films.iter().map(|film| self.frames_of(film) * 1000 / u64::from(self.format.fps)).sum()
    }

    fn frames_of(&self, film: &Movie) -> u64 {
        (film.duration_ms() * u64::from(self.format.fps)).div_ceil(1000)
    }

    /// The logical size the views are laid out at, and the scale to the pixels.
    fn logical(&self) -> (Size, f32) {
        let (width, height) = (self.format.width as f32, self.format.height as f32);
        match self.format.layout {
            Layout::Full => {
                let film = self.films.first().map_or(Size::new(width, height), Movie::logical_size);
                let scale = width / film.width.max(1.0);
                (Size::new(film.width, height / scale), scale)
            }
            // Laid out at half size: the words come out big for a phone.
            Layout::Social => (Size::new(width / 2.0, height / 2.0), 2.0),
        }
    }

    fn ui(&self) -> Option<(ravenrs_widgets::Ui, Playing, f32)> {
        let first = self.films.first()?.clone();
        let (logical, scale) = self.logical();
        let film = Rc::new(RefCell::new((first, 0)));
        let moment = Moment {
            film: film.clone(),
            format: self.format,
            logical,
            labels: self.labels.clone(),
            brand: self.brand.clone(),
        };
        Some((draw::ui_for(moment, self.theme, logical, scale), film, scale))
    }

    /// The picture at `at_ms` of the first film, as a PNG, for a cover.
    pub fn thumbnail(&self, at_ms: u64) -> Result<Vec<u8>, VideoError> {
        let (mut ui, film, scale) = self.ui().ok_or(VideoError::Empty)?;
        let mut canvas =
            draw::Canvas::new((self.format.width, self.format.height)).ok_or(VideoError::NoGpu)?;
        film.borrow_mut().1 = at_ms;
        ui.set_time(at_ms as f64 / 1000.0);
        ui.update();
        let pixels = canvas.draw(&ui, scale);
        let mut png = Vec::new();
        image::RgbaImage::from_raw(self.format.width, self.format.height, pixels)
            .ok_or(VideoError::Empty)?
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|error| VideoError::Io(std::io::Error::other(error)))?;
        Ok(png)
    }

    /// Writes the video to `path`. `progress` hears each picture done, out of all.
    pub fn write(
        &self,
        path: impl AsRef<Path>,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<VideoReport, VideoError> {
        let (mut ui, film, scale) = self.ui().ok_or(VideoError::Empty)?;
        let (width, height, fps) = (self.format.width, self.format.height, self.format.fps);
        let mut canvas = draw::Canvas::new((width, height)).ok_or(VideoError::NoGpu)?;
        let config = EncoderConfig::new()
            .bitrate(BitRate::from_bps(self.quality))
            .max_frame_rate(FrameRate::from_hz(fps as f32))
            .rate_control_mode(RateControlMode::Bitrate)
            .usage_type(UsageType::ScreenContentRealTime)
            .profile(Profile::High)
            .complexity(Complexity::High)
            .skip_frames(false)
            .adaptive_quantization(false)
            .background_detection(false)
            .intra_frame_period(IntraFramePeriod::from_num_frames(fps * 2))
            .vui(VuiConfig::bt601());
        let mut encoder = Encoder::with_api_config(OpenH264API::from_source(), config)
            .map_err(|e| VideoError::Coder(e.to_string()))?;
        let mut writer = mov::MovWriter::new(BufWriter::new(File::create(path.as_ref())?))?;
        let total: u64 = self.films.iter().map(|f| self.frames_of(f)).sum();
        let mut report = VideoReport::default();
        let mut done = 0;
        let mut clock = 0.0;
        for movie in &self.films {
            let sound = Soundtrack::of(movie, self.narration.as_ref(), SOUND_RATE);
            report.unrecorded.extend(sound.unrecorded().iter().cloned());
            let frames = self.frames_of(movie);
            let per_frame = f64::from(SOUND_RATE) / f64::from(fps);
            for frame in 0..frames {
                let at = frame * 1000 / u64::from(fps);
                *film.borrow_mut() = (movie.clone(), at);
                clock += 1.0 / f64::from(fps);
                ui.set_time(clock);
                ui.update();
                let pixels = canvas.draw(&ui, scale);
                let yuv =
                    YUVBuffer::from_rgb8_source(RgbaSliceU8::new(&pixels, (width as usize, height as usize)));
                let coded = encoder.encode(&yuv).map_err(|e| VideoError::Coder(e.to_string()))?;
                let mut nals: Vec<Vec<u8>> = Vec::new();
                for l in 0..coded.num_layers() {
                    let Some(layer) = coded.layer(l) else { continue };
                    for n in 0..layer.nal_count() {
                        if let Some(nal) = layer.nal_unit(n) {
                            nals.push(without_start_code(nal).to_vec());
                        }
                    }
                }
                let parts: Vec<&[u8]> = nals.iter().map(Vec::as_slice).collect();
                writer.picture(&parts)?;
                let from = (frame as f64 * per_frame) as usize;
                let to = ((frame + 1) as f64 * per_frame) as usize;
                let piece: Vec<i16> = (from..to)
                    .map(|i| (sound.samples().get(i).copied().unwrap_or(0.0) * 32_767.0) as i16)
                    .collect();
                writer.sound(&piece)?;
                done += 1;
                progress(done, total);
            }
        }
        let out = writer.finish((width, height), fps, SOUND_RATE)?;
        let file = out.into_inner().map_err(|e| VideoError::Io(e.into_error()))?;
        report.frames = done;
        report.duration_ms = done * 1000 / u64::from(fps);
        report.bytes = file.metadata()?.len();
        report.too_long =
            self.format.longest_seconds.is_some_and(|s| report.duration_ms > u64::from(s) * 1000);
        Ok(report)
    }
}

fn without_start_code(nal: &[u8]) -> &[u8] {
    let zeros = nal.iter().take_while(|b| **b == 0).count();
    if zeros >= 2 && nal.get(zeros) == Some(&1) { &nal[zeros + 1..] } else { nal }
}
