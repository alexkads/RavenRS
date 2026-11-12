//! The motion page, with the views it animates.

use ravenrs::layout::{Axis, LayoutStyle};
use ravenrs::prelude::*;

use crate::{Gallery, columns, demo, page_header, samples};

/// A puck that crosses a rail on a spring when `0` flips.
struct MotionTrack(State<bool>);

impl View for MotionTrack {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let puck = cx.container(
            LayoutStyle { width: Some(44.0), height: Some(28.0), ..LayoutStyle::leaf() },
            &[],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                ..QuadPaint::new(theme.color(Semantic::Accent))
            }),
        );
        cx.motion(puck, Motion::new().layout(Transition::Spring(Spring::from_response(0.45, 0.8))));
        let rail = cx.container(
            LayoutStyle {
                padding: Insets::uniform(4.0),
                justify: if self.0.get() { Justify::End } else { Justify::Start },
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[puck],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                ..QuadPaint::new(theme.color(Semantic::Control))
            }),
        );
        cx.clip(rail);
        rail
    }
}

/// Turns the theme's reduced-motion token on and off.
struct ReducedMotionSwitch;

impl View for ReducedMotionSwitch {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = cx.theme_state();
        let on = cx.reduced_motion();
        cx.child(
            &Button::new(if on { "Reduced motion is on" } else { "Try reduced motion" })
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Small)
                .on_click(move || {
                    theme.update(|t| t.interaction.reduced_motion = !t.interaction.reduced_motion)
                }),
        )
    }
}

impl Gallery {
    pub(crate) fn motion(&self, width: f32) -> impl View + use<> {
        let (sprint, progress) = (self.motion_side.clone(), self.motion_progress.clone());
        let flip = sprint.clone();
        let advance = progress.clone();
        let toasts = self.toasts.clone();
        let show = move || toasts.show("Saved", "Toasts slide in and make room.", Tone::Success);
        VStack::new((
            page_header(
                "Motion",
                "Springs that keep their speed when interrupted, colors mixed in OKLab, and exits that outlive the view.",
            ),
            Grid::new(columns(width, 2), (
                demo(
                    "Interruptible springs",
                    "Click again before it lands: it turns around with no jump in speed.",
                    VStack::new((
                        MotionTrack(sprint),
                        Button::new("Send to the other side")
                            .style(ButtonStyle::Secondary)
                            .on_click(move || flip.update(|s| *s = !*s)),
                    ))
                    .spacing(16.0)
                    .align(Align::Stretch),
                ),
                demo(
                    "Scroll-driven scene",
                    "The section holds while you scroll, and each step fades in and out with the scroll, not a clock.",
                    samples::sample("ScrollScene").unwrap_or_else(|| Box::new(Text::new("ScrollScene"))),
                ),
                demo(
                    "Shared selection",
                    "One marker that travels between items, under the labels.",
                    VStack::new((
                        SegmentedControl::new(self.motion_segment.binding(), ["Day", "Week", "Month", "Year"]),
                        Tabs::new(self.motion_tab.binding())
                            .tab("Summary", Text::new("Summary").foreground(Semantic::InkMuted))
                            .tab("Activity log", Text::new("Activity").foreground(Semantic::InkMuted))
                            .tab("Billing", Text::new("Billing").foreground(Semantic::InkMuted)),
                    ))
                    .spacing(20.0)
                    .align(Align::Stretch),
                ),
                demo(
                    "Press and toggle",
                    "Colors ease in OKLab, buttons give under the pointer, and the thumb stretches while held.",
                    VStack::new((
                        HStack::new((
                            Button::new("Save"),
                            Button::new("Edit").style(ButtonStyle::Secondary),
                            Button::new("Skip").style(ButtonStyle::Ghost),
                        ))
                        .spacing(12.0),
                        Toggle::new(self.motion_toggle.binding()).label("Wi-Fi"),
                        Checkbox::new("Send a copy to me", self.motion_check.binding()),
                    ))
                    .spacing(16.0),
                ),
                demo(
                    "Height to fit",
                    "The hard one on the web: content of unknown height opens smoothly.",
                    Accordion::new(vec![
                        Disclosure::new(
                            "What is animated?",
                            Text::new("Fill, border, shadow, radius, text color, position, size and opacity.")
                                .foreground(Semantic::InkMuted),
                        ),
                        Disclosure::new(
                            "What does it cost?",
                            Text::new("Paint only: views are not rebuilt and layout does not run again.")
                                .foreground(Semantic::InkMuted),
                        ),
                    ]),
                ),
                demo(
                    "Enter and exit",
                    "A closed view stays on screen as a picture until its exit ends. It takes no input.",
                    HStack::new((
                        Button::new("Open dialog").style(ButtonStyle::Secondary).on_click({
                            let open = self.motion_dialog.clone();
                            move || open.set(true)
                        }),
                        Button::new("Show toast").style(ButtonStyle::Secondary).on_click(show),
                    ))
                    .spacing(12.0),
                ),
                demo(
                    "Values and loaders",
                    "Progress eases to each new value. Loaders paint without rebuilding the UI.",
                    VStack::new((
                        ProgressBar::new(*progress.peek()),
                        HStack::new((
                            Button::new("Move ahead")
                                .style(ButtonStyle::Secondary)
                                .size(ButtonSize::Small)
                                .on_click(move || advance.update(|p| *p = if *p >= 1.0 { 0.1 } else { *p + 0.3 })),
                            Spinner::new(),
                            Skeleton::line(14.0).grow(1.0),
                        ))
                        .spacing(16.0)
                        .align(Align::Center),
                        ReducedMotionSwitch,
                    ))
                    .spacing(16.0)
                    .align(Align::Stretch),
                ),
            ))
            .spacing(16.0),
            Dialog::new(self.motion_dialog.binding(), "Enter and exit")
                .description("The card springs in. Close it and it fades out as a picture of itself."),
        ))
        .spacing(20.0)
        .align(Align::Stretch)
    }
}
