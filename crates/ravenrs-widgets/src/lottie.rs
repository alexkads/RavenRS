//! Plays a Lottie animation.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{PathFill, Rect, Role};
use ravenrs_layout::LayoutStyle;
use ravenrs_lottie::Animation;

use crate::context::{BuildCx, NodeId, Paint, Semantics};
use crate::view::View;

/// Plays a Lottie animation, drawn with curves on the GPU, sharp at any size. It
/// sleeps when it ends or is paused, so that an idle window stays idle. Where motion
/// is reduced it shows its last frame, still.
///
/// ```
/// use ravenrs_widgets::{Lottie, LottieAnimation, View};
///
/// let json = br#"{"v":"5.5.7","fr":30,"ip":0,"op":30,"w":100,"h":100,"layers":[]}"#;
/// let animation = LottieAnimation::from_json(json).expect("a Lottie");
/// let view = Lottie::new(animation).size(96.0, 96.0).looping(false).alt("Our logo");
/// ```
#[derive(Clone)]
pub struct Lottie {
    animation: Animation,
    width: f32,
    height: f32,
    looping: bool,
    speed: f64,
    playing: Option<ravenrs_reactive::Binding<bool>>,
    alt: Option<String>,
}

impl fmt::Debug for Lottie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lottie")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("looping", &self.looping)
            .finish_non_exhaustive()
    }
}

impl Lottie {
    /// Plays `animation` at its own size, over and over.
    pub fn new(animation: Animation) -> Self {
        Self {
            width: animation.width(),
            height: animation.height(),
            animation,
            looping: true,
            speed: 1.0,
            playing: None,
            alt: None,
        }
    }

    /// The size it takes, in logical pixels. It keeps its proportions inside it.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Whether it starts over at the end, or stays on its last frame.
    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// How fast it plays, 1.0 being as authored.
    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = speed;
        self
    }

    /// Pauses it while this is false. Without one, it plays from when it first shows.
    pub fn playing(mut self, playing: ravenrs_reactive::Binding<bool>) -> Self {
        self.playing = Some(playing);
        self
    }

    /// What screen readers say it shows. Without it, it is decoration they skip.
    pub fn alt(mut self, alt: impl Into<String>) -> Self {
        self.alt = Some(alt.into());
        self
    }
}

/// Where a moment is, given how long it has been playing.
fn moment(played: f64, duration: f64, looping: bool) -> f64 {
    if duration <= 0.0 {
        0.0
    } else if looping {
        played.rem_euclid(duration)
    } else {
        played.min(duration)
    }
}

impl View for Lottie {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let now = cx.now();
        let reduced = cx.reduced_motion();
        let duration = self.animation.duration();
        // How far it has played, the clock when that was true, and whether it was
        // playing then: nothing rebuilds it during a pause, which then does not count.
        // Not reactive: writing it here must not ask for another build.
        let state = cx.use_state(|| Rc::new(Cell::new((0.0_f64, now, true)))).peek().clone();
        let (played_before, since, was_on) = state.get();
        let on = self.playing.as_ref().is_none_or(|playing| playing.get());
        // Frames between builds only paint, so a build that pauses it still counts up to now.
        let ran = if was_on { (now - since).max(0.0) } else { 0.0 };
        let played = played_before + ran * self.speed;
        state.set((played, now, on));
        let moves = on && !reduced && (self.looping || played < duration);
        // Only until it ends, so that it can show its last frame and rest.
        if on && !reduced && !self.looping && played < duration {
            cx.wake_at(now + (duration - played) / self.speed.max(f64::EPSILON));
        }

        let (animation, speed, looping) = (self.animation.clone(), self.speed, self.looping);
        let (comp_w, comp_h) = (animation.width().max(1.0), animation.height().max(1.0));
        let draw = move |frame: Rect, at: f64, scene: &mut ravenrs_core::Scene| {
            let scale = (frame.width() / comp_w).min(frame.height() / comp_h);
            let x = frame.x() + (frame.width() - comp_w * scale) / 2.0;
            let y = frame.y() + (frame.height() - comp_h * scale) / 2.0;
            // What is past the edges of the composition is not part of the picture.
            scene.push_clip(Rect::new(x, y, comp_w * scale, comp_h * scale));
            // Twice for a dense display: only how finely strokes are cut depends on it.
            for path in animation.paths_at(at, scale * 2.0) {
                scene.push_path(PathFill { transform: [scale, 0.0, 0.0, scale, x, y], ..path });
            }
            scene.pop_clip();
        };
        let paint = if moves {
            // Each frame repeats only the painting: the rest of the screen did not change.
            Paint::Live(Rc::new(move |frame, time, scene| {
                let played = played + (time - now).max(0.0) * speed;
                draw(frame, moment(played, duration, looping), scene);
            }))
        } else {
            let at = if reduced { duration } else { moment(played, duration, self.looping) };
            Paint::Custom(Rc::new(move |frame, scene| draw(frame, at, scene)))
        };
        let node = cx.container(
            LayoutStyle { width: Some(self.width), height: Some(self.height), ..LayoutStyle::leaf() },
            &[],
            paint,
        );
        match &self.alt {
            Some(alt) => cx.semantics(node, Semantics::new(Role::Image).name(alt.clone())),
            None => cx.semantics(node, Semantics::hidden()),
        }
        node
    }
}
