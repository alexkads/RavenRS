//! The stories of `Chart`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #754: it carries the target line (`ChartReference`), a part of a chart, as a
//! story, and is no longer written over.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use ravenrs_core::*;
#[allow(unused_imports)]
use ravenrs_layout::*;
#[allow(unused_imports)]
use ravenrs_reactive::*;
#[allow(unused_imports)]
use ravenrs_style::*;
#[allow(unused_imports)]
use ravenrs_widgets_base::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
#[allow(unused_imports)]
use ravenrs_widgets_base::*;
#[allow(unused_imports)]
use ravenrs_widgets_kit::*;
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `Chart`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_title = args.text("title");
        let arg_height = args.number("height") as f32;
        let arg_align_axis_ends = args.bool("align_axis_ends");
        let arg_alt = args.text("alt");
        let arg_animate_changes = args.bool("animate_changes");
        let arg_concentric = args.bool("concentric");
        let arg_dots = args.bool("dots");
        let arg_gradient = args.bool("gradient");
        let arg_grid_dashed = args.bool("grid_dashed");
        let arg_hide_category_axis = args.bool("hide_category_axis");
        let arg_hide_legend = args.bool("hide_legend");
        let arg_hide_tooltip = args.bool("hide_tooltip");
        let arg_horizontal = args.bool("horizontal");
        let arg_label_guides = args.bool("label_guides");
        let arg_labels_at_zero = args.bool("labels_at_zero");
        let arg_legend_at = match args.choice("legend_at") {
            "Top" => ChartLegend::Top,
            _ => ChartLegend::Bottom,
        };
        let arg_lines_only = args.bool("lines_only");
        let arg_no_appear = args.bool("no_appear");
        let arg_no_hover_focus = args.bool("no_hover_focus");
        let arg_no_hover_motion = args.bool("no_hover_motion");
        let arg_no_separator = args.bool("no_separator");
        let arg_no_spokes = args.bool("no_spokes");
        let arg_polar_fill = args.bool("polar_fill");
        let arg_radius_axis = args.bool("radius_axis");
        let arg_show_value_axis = args.bool("show_value_axis");
        let arg_tight_scale = args.bool("tight_scale");
        let arg_tracks = args.bool("tracks");
        let arg_vary_colors = args.bool("vary_colors");

        {
            let mut it = Chart::new(ChartKind::Line)
                .categories(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
                .series("2025", [12.0, 18.0, 15.0, 21.0, 19.0, 24.0])
                .series("2026", [14.0, 21.0, 26.0, 24.0, 31.0, 35.0]);
            it = it.title(arg_title.clone());
            it = it.height(arg_height);
            if arg_align_axis_ends {
                it = it.align_axis_ends();
            }
            if !arg_alt.is_empty() {
                it = it.alt(arg_alt.clone());
            }
            if arg_animate_changes {
                it = it.animate_changes();
            }
            if arg_concentric {
                it = it.concentric();
            }
            if arg_dots {
                it = it.dots();
            }
            if arg_gradient {
                it = it.gradient();
            }
            if arg_grid_dashed {
                it = it.grid_dashed();
            }
            if arg_hide_category_axis {
                it = it.hide_category_axis();
            }
            if arg_hide_legend {
                it = it.hide_legend();
            }
            if arg_hide_tooltip {
                it = it.hide_tooltip();
            }
            if arg_horizontal {
                it = it.horizontal();
            }
            if arg_label_guides {
                it = it.label_guides();
            }
            if arg_labels_at_zero {
                it = it.labels_at_zero();
            }
            it = it.legend_at(arg_legend_at.clone());
            if arg_lines_only {
                it = it.lines_only();
            }
            if arg_no_appear {
                it = it.no_appear();
            }
            if arg_no_hover_focus {
                it = it.no_hover_focus();
            }
            if arg_no_hover_motion {
                it = it.no_hover_motion();
            }
            if arg_no_separator {
                it = it.no_separator();
            }
            if arg_no_spokes {
                it = it.no_spokes();
            }
            if arg_polar_fill {
                it = it.polar_fill();
            }
            if arg_radius_axis {
                it = it.radius_axis();
            }
            if arg_show_value_axis {
                it = it.show_value_axis();
            }
            if arg_tight_scale {
                it = it.tight_scale();
            }
            if arg_tracks {
                it = it.tracks();
            }
            if arg_vary_colors {
                it = it.vary_colors();
            }
            it
        }
    })
}

/// A line in a cell, as a grid shows the durations of the last builds: no axis, no legend.
fn sparkline(_: &Args) -> AnyView {
    Box::new(
        Chart::new(ChartKind::Sparkline)
            .series("Duration (s)", [62.0, 58.0, 71.0, 64.0, 66.0, 59.0, 80.0, 72.0, 69.0, 74.0])
            .size(120.0, 24.0)
            .hide_legend()
            .alt("Duration of the last builds"),
    )
}

/// A chart with a target line, the gallery's example of `ChartReference`.
fn with_target(_: &Args) -> AnyView {
    Box::new(
        Chart::new(ChartKind::Bar)
            .categories(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
            .series("Sales", [12.0, 18.0, 15.0, 21.0, 19.0, 24.0])
            .show_value_axis()
            .grid_dashed()
            .value_domain(0.0, 25.0)
            .tooltip(ChartTooltip::new().value_format(|value| format!("{value:.0} units")))
            .reference(ChartReference::new(20.0).label("Target").color(Semantic::Warning))
            .height(260.0),
    )
}

impl ComponentBook for Chart {
    fn book() -> Meta {
        Meta::new("Data/Chart", render)
            .summary("Area, bar, line, pie, doughnut, radar and radial charts in the patterns of the shadcn/ui catalog, with tooltips, keyboard, a focus that glides between categories, lines of reference and a fixed axis; scatter, sparklines and candlesticks too.")
            .arg(ArgType::text("title", "Sales"))
            .arg(ArgType::number("height", 260.0, 0.0, 1040.0, 1.0))
            .arg(ArgType::bool("align_axis_ends", false))
            .arg(ArgType::text("alt", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::bool("animate_changes", false))
            .arg(ArgType::bool("concentric", false))
            .arg(ArgType::bool("dots", false))
            .arg(ArgType::bool("gradient", false))
            .arg(ArgType::bool("grid_dashed", false))
            .arg(ArgType::bool("hide_category_axis", false))
            .arg(ArgType::bool("hide_legend", false))
            .arg(ArgType::bool("hide_tooltip", false))
            .arg(ArgType::bool("horizontal", false))
            .arg(ArgType::bool("label_guides", false))
            .arg(ArgType::bool("labels_at_zero", false))
            .arg(ArgType::choice("legend_at", &["Bottom", "Top"], "Bottom"))
            .arg(ArgType::bool("lines_only", false))
            .arg(ArgType::bool("no_appear", false))
            .arg(ArgType::bool("no_hover_focus", false))
            .arg(ArgType::bool("no_hover_motion", false))
            .arg(ArgType::bool("no_separator", false))
            .arg(ArgType::bool("no_spokes", false))
            .arg(ArgType::bool("polar_fill", false))
            .arg(ArgType::bool("radius_axis", false))
            .arg(ArgType::bool("show_value_axis", false))
            .arg(ArgType::bool("tight_scale", false))
            .arg(ArgType::bool("tracks", false))
            .arg(ArgType::bool("vary_colors", false))
            .story("Example", Args::new())
            .story_with("With target line", Args::new(), with_target)
            .story("Legend at: Top", Args::new().with("legend_at", ArgValue::choice("Top")))
            .story_with("Sparkline", Args::new(), sparkline)
    }
}
