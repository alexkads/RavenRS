//! Choosing a theme, and changing its tokens.

use ravenrs_core::Color;
use ravenrs_layout::Align;
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Theme};

use crate::context::{BuildCx, NodeId};
use crate::{
    Alert, Button, ButtonSize, ButtonStyle, ColorPicker, DropdownMenu, FormField, FormGroup, FormLayout,
    HStack, IconName, MenuItem, Spacer, Stepper, Text, Tone, VStack, View,
};

/// The contrast text has to reach on its ground.
const LEGIBLE: f32 = 4.5;
const RADIUS_MOST: i32 = 64;
const SIZE_LEAST: i32 = 8;
const SIZE_MOST: i32 = 96;
const COLUMNS: usize = 3;

/// A menu of themes that says which one is on, small enough for a sidebar's footer or a
/// toolbar. For the three of them side by side, in a settings page, see
/// [`ThemeSwitcher`](crate::ThemeSwitcher).
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ThemePicker {
    themes: Vec<Theme>,
}

impl Default for ThemePicker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ThemePicker {
    /// A menu of the themes that come with RavenRS.
    pub fn new() -> Self {
        Self { themes: Theme::builtin().to_vec() }
    }

    /// Adds a theme of the application's own to the menu, under its name.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.themes.push(theme);
        self
    }
}

impl View for ThemePicker {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let state = cx.theme_state();
        let system = cx.system_appearance();
        let following = system.follows();
        let on = state.with(|theme| theme.name);
        // Those that come with RavenRS have words in each language and an icon of their own.
        let look = |theme: &Theme| match theme.name {
            "Raven Dark" => (labels.theme_dark.clone(), IconName::Moon),
            "Raven Light" => (labels.theme_light.clone(), IconName::Sun),
            "Raven Glass" => (labels.theme_glass.clone(), IconName::Layers),
            other => (other.to_owned(), IconName::Palette),
        };
        let current = if following {
            IconName::Monitor
        } else {
            self.themes.iter().find(|theme| theme.name == on).map_or(IconName::Palette, |t| look(t).1)
        };
        let mut menu = DropdownMenu::new(labels.theme.clone()).icon(current).size(ButtonSize::Small);
        // Following the system is offered only where the platform says what the system is.
        if system.dark().is_some() {
            let (system, state) = (system.clone(), state.clone());
            let shown = if following { IconName::Check } else { IconName::Monitor };
            menu = menu.item(
                MenuItem::new(labels.theme_system.clone())
                    .icon(shown)
                    .on_select(move || system.set_follow(true, &state)),
            );
        }
        for theme in &self.themes {
            let (caption, icon) = look(theme);
            let (state, theme, system) = (state.clone(), *theme, system.clone());
            let shown = if theme.name == on && !following { IconName::Check } else { icon };
            menu = menu.item(MenuItem::new(caption).icon(shown).on_select(move || {
                system.set_follow(false, &state);
                state.set(theme);
            }));
        }
        cx.child(&menu)
    }
}

/// A color as it looks on the canvas, whatever it lets through.
fn flattened(color: Color, canvas: Color) -> Color {
    canvas.lerp(color.with_alpha(1.0), color.a)
}

/// Changes the tokens of the theme on screen, which follows at once: colors, the radii and the
/// sizes of text. It warns when text stops being legible on its ground, and copies what changed
/// as JSON, or takes it back, so an application can keep it wherever it wants.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ThemeEditor {
    picker: ThemePicker,
}

impl Default for ThemeEditor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ThemeEditor {
    /// An editor over the theme of the application.
    pub fn new() -> Self {
        Self { picker: ThemePicker::new() }
    }

    /// Offers a theme of the application's own to start from.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.picker = self.picker.theme(theme);
        self
    }
}

fn color_field(
    state: &State<Theme>,
    role: Semantic,
    over: Option<Semantic>,
    cx: &BuildCx<'_>,
) -> FormField<ColorPicker> {
    let (read, write) = (state.clone(), state.clone());
    let binding = Binding::new(
        move || Some(read.with(|theme| theme.palette.get(role))),
        move |color: Option<Color>| {
            if let Some(color) = color {
                write.update(|theme| theme.palette.set(role, color));
            }
        },
    );
    let picker = ColorPicker::new(binding).alpha(true).label(role.token());
    let field = FormField::new(role.token(), picker);
    let Some(ground) = over else { return field };
    let theme = state.get();
    let canvas = theme.palette.bg_000;
    let ratio = flattened(theme.palette.get(role), canvas)
        .contrast_ratio(flattened(theme.palette.get(ground), canvas));
    let tone = if ratio < LEGIBLE { Tone::Warning } else { Tone::Neutral };
    field.note_text(Some(cx.labels().theme_contrast(ratio, ground.token())), tone)
}

fn stepper_field(
    name: &str,
    least: i32,
    most: i32,
    read: impl Fn(&Theme) -> f32 + 'static,
    write: impl Fn(&mut Theme, f32) + 'static,
    state: &State<Theme>,
) -> FormField<Stepper> {
    let (reading, writing) = (state.clone(), state.clone());
    let binding = Binding::new(
        move || reading.with(|theme| read(theme).round() as i32),
        move |value: i32| writing.update(|theme| write(theme, value as f32)),
    );
    FormField::new(name, Stepper::new(binding).range(least, most).label(name))
}

impl View for ThemeEditor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let state = cx.theme_state();
        let notice: State<Option<(String, Tone)>> = cx.use_state(|| None);
        let changed = state.with(Theme::changed_tokens);
        let group = |title: &str, roles: &[(Semantic, Option<Semantic>)], cx: &BuildCx<'_>| {
            roles.iter().fold(FormGroup::titled(title).columns(COLUMNS), |group, (role, over)| {
                group.field(color_field(&state, *role, *over, cx))
            })
        };
        use Semantic as S;
        let surfaces = group(
            &labels.theme_surfaces,
            &[
                (S::Canvas, None),
                (S::Surface, None),
                (S::SurfaceElevated, None),
                (S::Control, None),
                (S::Border, None),
            ],
            cx,
        );
        let text = group(
            &labels.theme_text,
            &[(S::Ink, Some(S::SurfaceElevated)), (S::InkMuted, Some(S::SurfaceElevated))],
            cx,
        );
        let accent = group(
            &labels.theme_accent,
            &[
                (S::Accent, None),
                (S::AccentHover, None),
                (S::AccentPressed, None),
                (S::OnAccent, Some(S::Accent)),
                (S::FocusRing, None),
            ],
            cx,
        );
        let status = group(
            &labels.theme_status,
            &[
                (S::Success, Some(S::SurfaceElevated)),
                (S::Warning, Some(S::SurfaceElevated)),
                (S::Danger, Some(S::SurfaceElevated)),
            ],
            cx,
        );
        let shape = ravenrs_style::Radii::TOKENS.into_iter().fold(
            FormGroup::titled(&labels.theme_shape).columns(COLUMNS),
            |group, token| {
                group.field(stepper_field(
                    &format!("radius-{token}"),
                    0,
                    RADIUS_MOST,
                    move |theme| theme.radii.get(token).unwrap_or_default(),
                    move |theme, radius| theme.radii.set(token, radius),
                    &state,
                ))
            },
        );
        let sizes = TextStyle::TOKENS.into_iter().fold(
            FormGroup::titled(&labels.theme_sizes).columns(COLUMNS),
            |group, style| {
                group.field(stepper_field(
                    style.token(),
                    SIZE_LEAST,
                    SIZE_MOST,
                    move |theme| theme.typography.get(style).size,
                    move |theme, size| theme.typography.set_size(style, size),
                    &state,
                ))
            },
        );

        let (restoring, restored) = (state.clone(), notice.clone());
        let restore = Button::new(labels.theme_restore.clone())
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Small)
            .disabled(changed == 0)
            .on_click(move || {
                restoring.update(|theme| *theme = theme.base());
                restored.set(None);
            });
        let (clipboard, copying, copied) = (cx.clipboard(), state.clone(), notice.clone());
        let copied_words = labels.copied.clone();
        let copy = Button::new(labels.theme_copy.clone())
            .icon(IconName::Copy)
            .style(ButtonStyle::Secondary)
            .size(ButtonSize::Small)
            .on_click(move || {
                clipboard.write(&copying.with(Theme::to_tokens_json));
                copied.set(Some((copied_words.clone(), Tone::Success)));
            });
        let (clipboard, pasting, pasted) = (cx.clipboard(), state.clone(), notice.clone());
        let refused = labels.theme_invalid.clone();
        let paste = Button::new(labels.theme_paste.clone())
            .style(ButtonStyle::Secondary)
            .size(ButtonSize::Small)
            .on_click(move || match clipboard.read().map(|text| Theme::from_tokens_json(&text)) {
                Some(Ok(theme)) => {
                    pasting.set(theme);
                    pasted.set(None);
                }
                _ => pasted.set(Some((refused.clone(), Tone::Danger))),
            });
        let bar = HStack::new((
            self.picker.clone(),
            Text::new(labels.theme_changed(changed)).style(TextStyle::Callout).foreground(Semantic::InkMuted),
            Spacer::new(),
            restore,
            copy,
            paste,
        ))
        .spacing(8.0)
        .align(Align::Center);
        let form = FormLayout::new()
            .group(surfaces)
            .group(text)
            .group(accent)
            .group(status)
            .group(shape)
            .group(sizes);
        cx.child(
            &VStack::new((bar, notice.get().map(|(message, tone)| Alert::message(message).tone(tone)), form))
                .spacing(16.0)
                .align(Align::Stretch),
        )
    }
}
