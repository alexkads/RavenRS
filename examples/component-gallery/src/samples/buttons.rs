//! Buttons, toggles and choices.

use ravenrs::prelude::*;

samples! {
    "Button" => button, "IconButton" => icon_button, "ToggleButton" => toggle_button,
    "ToggleGroup" => toggle_group, "SegmentedControl" => segmented_control, "Toggle" => toggle,
    "Checkbox" => checkbox, "RadioGroup" => radio_group, "Slider" => slider, "Stepper" => stepper,
    "ThemeSwitcher" => theme_switcher, "ThemePicker" => theme_picker, "ThemeEditor" => theme_editor,
    "Toolbar" => toolbar, "SplitButton" => split_button, "Rating" => rating,
    "FloatingToolbar" => floating_toolbar, "ValueSlider" => value_slider, "ButtonGroup" => button_group,
    "FavoriteToggle" => favorite_toggle,
}

// sample: Button, 200
fn button() -> impl View {
    let clicks = State::new(0);
    let add = clicks.clone();
    VStack::new((
        HStack::new((
            Button::new("Save").on_click(move || add.update(|n| *n += 1)),
            Button::new("Cancel").style(ButtonStyle::Secondary),
            Button::new("Show more").style(ButtonStyle::Ghost),
            Button::new("Delete").style(ButtonStyle::Danger).icon(IconName::Trash2),
        ))
        .spacing(8.0)
        .wrap(),
        HStack::new((
            Button::new("Upload").style(ButtonStyle::Secondary).loading(true),
            Button::new("Show grid").style(ButtonStyle::Secondary).selected(true),
            Button::new("Open").style(ButtonStyle::Ghost).shortcut("F8"),
        ))
        .spacing(8.0)
        .wrap(),
        Text::bound(&clicks, |n| format!("Saved {n} times")).foreground(Semantic::InkMuted),
    ))
    .spacing(12.0)
}
// end

// sample: IconButton, 90
fn icon_button() -> impl View {
    HStack::new((
        IconButton::new(IconName::Pencil).label("Edit"),
        IconButton::new(IconName::Copy).label("Duplicate"),
        IconButton::new(IconName::Settings).label("Settings").style(ButtonStyle::Secondary),
        IconButton::new(IconName::Plus).label("New").style(ButtonStyle::Primary),
        IconButton::new(IconName::Bold).label("Bold").selected(true),
    ))
    .spacing(8.0)
}
// end

// sample: ToggleButton, 90
fn toggle_button() -> impl View {
    let bold = State::new(true);
    let italic = State::new(false);
    let open = State::new(true);
    HStack::new((
        ToggleButton::new(bold.binding(), "Bold"),
        ToggleButton::new(italic.binding(), "Italic"),
        ToggleButton::new(open.binding(), "Open").count(12).pill(),
    ))
    .spacing(8.0)
}
// end

// sample: ToggleGroup, 90
fn toggle_group() -> impl View {
    let align = State::new(0);
    ToggleGroup::single(
        align.binding(),
        [
            (Some(IconName::AlignLeft), ""),
            (Some(IconName::AlignCenter), ""),
            (Some(IconName::AlignRight), ""),
        ],
    )
}
// end

// sample: SegmentedControl, 90
fn segmented_control() -> impl View {
    let period = State::new(1);
    SegmentedControl::new(period.binding(), ["Day", "Week", "Month"]).label("Period")
}
// end

// sample: Toggle, 110
fn toggle() -> impl View {
    let push = State::new(true);
    let email = State::new(false);
    VStack::new((
        Toggle::new(push.binding()).label("Push notifications"),
        Toggle::new(email.binding()).label("Email digest").small(),
    ))
    .spacing(12.0)
}
// end

// sample: Checkbox, 110
fn checkbox() -> impl View {
    let terms = State::new(true);
    let news = State::new(false);
    VStack::new((
        Checkbox::new("I accept the terms", terms.binding()),
        Checkbox::new("Send me product news", news.binding()),
    ))
    .spacing(12.0)
}
// end

// sample: RadioGroup, 150
fn radio_group() -> impl View {
    let plan = State::new(1);
    RadioGroup::new(plan.binding(), ["Starter", "Team", "Enterprise"]).label("Plan")
}
// end

// sample: Slider, 200
fn slider() -> impl View {
    let volume = State::new(0.6_f32);
    let (exposure, warmth) = (State::new(1.5_f32), State::new(0.4_f32));
    let shown = volume.clone();
    VStack::new((
        Slider::new(volume.binding()).label("Volume"),
        Text::dynamic(move || format!("{:.0}%", shown.get() * 100.0)).foreground(Semantic::InkMuted),
        // Fills from zero, a double click goes back to it, and the value sits at the right.
        Slider::new(exposure.binding())
            .range(-5.0, 5.0)
            .step(0.1)
            .neutral(0.0)
            .label("Exposure")
            .show_value(),
        Slider::new(warmth.binding())
            // ravenrs-lint: allow(raw_color) the colors on the track say what the value means, cold to warm
            .track_colors([Color::hex(0x3b82f6), Color::hex(0xfacc15)])
            .label("Temperature"),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
// end

// sample: Toolbar, 90
fn toolbar() -> impl View {
    Toolbar::new()
        .label("Records")
        .action(
            ToolbarAction::new("New")
                .icon(IconName::Plus)
                .style(ButtonStyle::Primary)
                .hotkey(Hotkey::command("n"))
                .on_click(|| {}),
        )
        .action(ToolbarAction::new("Refresh").icon(IconName::RefreshCw).on_click(|| {}))
        .separator()
        .action(ToolbarAction::new("Export").icon(IconName::Download).on_click(|| {}))
        .action(ToolbarAction::new("Delete").icon(IconName::Trash2).style(ButtonStyle::Danger).disabled(true))
}
// end

// sample: SplitButton, 90
fn split_button() -> impl View {
    SplitButton::new("Save")
        .icon(IconName::Check)
        .on_click(|| {})
        .item(MenuItem::new("Save and close").on_select(|| {}))
        .item(MenuItem::new("Save and new").on_select(|| {}))
}
// end

// sample: Stepper, 90
fn stepper() -> impl View {
    let seats = State::new(3);
    Stepper::new(seats.binding()).range(1, 50).label("Seats")
}
// end

// sample: ThemeSwitcher, 90
fn theme_switcher() -> impl View {
    ThemeSwitcher::new()
}
// end

// sample: ThemePicker, 90
fn theme_picker() -> impl View {
    ThemePicker::new()
}
// end

// sample: ThemeEditor, 900
fn theme_editor() -> impl View {
    ThemeEditor::new()
}
// end

// sample: FloatingToolbar, 230
fn floating_toolbar() -> impl View {
    let (place, level) = (State::new(None::<Point>), State::new(100.0_f32));
    FloatingToolbar::new(
        Spacer::new().frame(None, Some(200.0)).background(Semantic::Control).corner_radius(8.0),
    )
    .label("Zoom")
    .movable(place.binding())
    .action(ToolbarAction::new("Fit"))
    .action(ToolbarAction::new("Fill"))
    .separator()
    .action(ToolbarAction::new("Zoom out").icon(IconName::Minus).icon_only())
    .view(ValueSlider::new(level.binding()).range(10.0, 400.0).step(10.0).unit("%").label("Zoom level"))
    .action(ToolbarAction::new("Zoom in").icon(IconName::Plus).icon_only())
}
// end

// sample: ValueSlider, 70
fn value_slider() -> impl View {
    let (opacity, size) = (State::new(80.0_f32), State::new(24.0_f32));
    HStack::new((
        ValueSlider::new(opacity.binding())
            .range(0.0, 100.0)
            .step(1.0)
            .unit("%")
            .label("Opacity")
            .show_label(),
        ValueSlider::new(size.binding()).range(1.0, 200.0).step(1.0).unit(" px").label("Size").field(),
    ))
    .spacing(24.0)
    .align(Align::Center)
}
// end

// sample: Rating, 100
fn rating() -> impl View {
    let score = State::new(3u32);
    VStack::new((
        Rating::new(score.binding()).label("Photo rating").clearable(),
        Rating::read_only(4).size(14.0),
    ))
    .spacing(12.0)
}
// end

// sample: ButtonGroup, 80
fn button_group() -> impl View {
    let period = State::new(0);
    view_fn(move || {
        let chosen = period.get();
        let item = |index, label: &str| {
            let period = period.clone();
            let style = if chosen == index { ButtonStyle::Primary } else { ButtonStyle::Secondary };
            Button::new(label).style(style).on_click(move || period.set(index))
        };
        ButtonGroup::new([item(0, "Day"), item(1, "Week"), item(2, "Month")]).label("Period")
    })
}
// end

// sample: FavoriteToggle, 70
fn favorite_toggle() -> impl View {
    let (heart, star) = (State::new(true), State::new(false));
    HStack::new((
        FavoriteToggle::new(heart.binding()).label("Warm portrait"),
        FavoriteToggle::new(star.binding()).label("Pinned").star(),
    ))
    .spacing(8.0)
}
// end
