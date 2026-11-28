//! The stories of `Checkbox`, `Toggle`, `Slider` and `SegmentedControl`.

use ravenrs_reactive::State;

use super::{ArgType, Args, ComponentBook, Meta};
use crate::{AnyView, Checkbox, SegmentedControl, Slider, Toggle};

fn checkbox(args: &Args) -> AnyView {
    let checkbox = if args.bool("optional") {
        let answer = State::new(None::<bool>);
        Checkbox::optional(args.text("label"), answer.binding())
    } else {
        let on = State::new(args.bool("checked"));
        Checkbox::new(args.text("label"), on.binding()).mixed(args.bool("mixed"))
    };
    Box::new(checkbox.disabled(args.bool("disabled")).read_only(args.bool("read_only")))
}

fn toggle(args: &Args) -> AnyView {
    let on = State::new(args.bool("on"));
    let mut toggle =
        Toggle::new(on.binding()).disabled(args.bool("disabled")).read_only(args.bool("read_only"));
    if !args.text("label").is_empty() {
        toggle = toggle.label(args.text("label"));
    }
    if !args.text("accessible_name").is_empty() {
        toggle = toggle.accessible_name(args.text("accessible_name"));
    }
    if args.bool("small") {
        toggle = toggle.small();
    }
    Box::new(toggle)
}

fn slider(args: &Args) -> AnyView {
    let value = State::new(args.number("value") as f32);
    let mut slider = Slider::new(value.binding())
        .label(args.text("label"))
        .range(args.number("min") as f32, args.number("max") as f32)
        .step(args.number("step") as f32);
    if args.bool("show_value") {
        slider = slider.show_value();
    }
    Box::new(slider)
}

fn segmented(args: &Args) -> AnyView {
    let options: Vec<String> = args.text("options").split(',').map(|s| s.trim().to_owned()).collect();
    let chosen = State::new((args.number("selected") as usize).min(options.len().saturating_sub(1)));
    Box::new(SegmentedControl::new(chosen.binding(), options).label(args.text("label")))
}

impl ComponentBook for Checkbox {
    fn book() -> Meta {
        Meta::new("Actions and inputs/Checkbox", checkbox)
            .summary("Check box with its label; clicking the label toggles it.")
            .arg(ArgType::text("label", "I accept the terms"))
            .arg(ArgType::bool("checked", true))
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::bool("read_only", false))
            .arg(
                ArgType::bool("mixed", false)
                    .description("A select all whose children are partly checked; a click checks it."),
            )
            .arg(
                ArgType::bool("optional", false)
                    .description("An answer that may be unknown, shown mixed; clicks go unknown, on, off."),
            )
            .story("Checked", Args::new())
            .story("Unchecked", Args::new().with("checked", false).with("label", "Send me product news"))
            .story("Disabled", Args::new().with("disabled", true))
            .story("Read only", Args::new().with("read_only", true))
            .story("Mixed", Args::new().with("mixed", true).with("label", "Select all"))
            .story("Unknown answer", Args::new().with("optional", true).with("label", "Has a car"))
    }
}

impl ComponentBook for Toggle {
    fn book() -> Meta {
        Meta::new("Actions and inputs/Toggle", toggle)
            .summary("On and off switch for a setting that applies at once.")
            .arg(ArgType::text("label", "Push notifications"))
            .arg(ArgType::bool("on", true))
            .arg(ArgType::bool("small", false).description("The compact switch, for lists and toolbars."))
            .arg(
                ArgType::text("accessible_name", "")
                    .description("What a screen reader calls it, with no text shown."),
            )
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::bool("read_only", false))
            .story("On", Args::new())
            .story("Off", Args::new().with("on", false).with("label", "Email digest"))
            .story("Small", Args::new().with("small", true))
            .story(
                "Accessible name only",
                Args::new().with("label", "").with("small", true).with("accessible_name", "Turn docker on"),
            )
            .story("Disabled", Args::new().with("disabled", true))
            .story("Read only", Args::new().with("read_only", true))
    }
}

impl ComponentBook for Slider {
    fn book() -> Meta {
        Meta::new("Actions and inputs/Slider", slider)
            .summary("Picks a number in a range by dragging, with keys and an optional value at the side.")
            .arg(ArgType::text("label", "Volume"))
            .arg(ArgType::number("value", 0.6, -5.0, 5.0, 0.1))
            .arg(ArgType::number("min", 0.0, -10.0, 0.0, 1.0))
            .arg(ArgType::number("max", 1.0, 1.0, 10.0, 1.0))
            .arg(ArgType::number("step", 0.01, 0.01, 1.0, 0.01))
            .arg(ArgType::bool("show_value", false).description("The value at the right of the label."))
            .story("Volume", Args::new())
            .story(
                "Exposure",
                Args::new()
                    .with("label", "Exposure")
                    .with("value", 1.5)
                    .with("min", -5.0)
                    .with("max", 5.0)
                    .with("step", 0.1)
                    .with("show_value", true),
            )
    }
}

impl ComponentBook for SegmentedControl {
    fn book() -> Meta {
        Meta::new("Actions and inputs/SegmentedControl", segmented)
            .summary("Picks one of a few options laid side by side on a track.")
            .arg(ArgType::text("label", "Period"))
            .arg(
                ArgType::text("options", "Day, Week, Month").description("The options, separated by commas."),
            )
            .arg(ArgType::number("selected", 1.0, 0.0, 5.0, 1.0))
            .story("Three options", Args::new())
            .story(
                "Two options",
                Args::new().with("options", "List, Board").with("selected", 0.0).with("label", "View"),
            )
    }
}
