//! The stories of `DataGrid`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #759: it is no longer written over.

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

/// The gallery's example of `DataGrid`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_height = args.number("height") as f32;
        let arg_detail_height = args.number("detail_height") as f32;
        let arg_empty_text = args.text("empty_text");
        let arg_export_title = args.text("export_title");
        let arg_filter_builder = args.bool("filter_builder");
        let arg_filter_row = args.bool("filter_row");
        let arg_footer = args.bool("footer");
        let arg_horizontal_lines = args.bool("horizontal_lines");
        let arg_row_height = args.number("row_height") as f32;
        let arg_search_panel = args.bool("search_panel");
        let arg_selection_mode = match args.choice("selection_mode") {
            "None" => SelectionMode::None,
            "Multiple" => SelectionMode::Multiple,
            _ => SelectionMode::Single,
        };
        let arg_striped = args.bool("striped");
        let arg_toolbar = args.bool("toolbar");
        let arg_vertical_lines = args.bool("vertical_lines");

        let rows = (0..1000)
            .map(|i| {
                vec![
                    Value::Number(f64::from(i)),
                    Value::Text(format!("Customer {i}")),
                    Value::Number(f64::from(i % 90) * 13.5),
                ]
            })
            .collect();
        {
            let mut it = DataGrid::new(
                GridData::new(rows),
                [
                    GridColumn::number("ID").width(90.0),
                    GridColumn::text("Customer").width(220.0).editable(),
                    GridColumn::currency("MRR", "R$ ", 2).width(150.0).summary(Summary::Sum),
                ],
            );
            it = it.height(arg_height);
            it = it.detail_height(arg_detail_height);
            if !arg_empty_text.is_empty() {
                it = it.empty_text(arg_empty_text.clone());
            }
            if !arg_export_title.is_empty() {
                it = it.export_title(arg_export_title.clone());
            }
            it = it.filter_builder(arg_filter_builder);
            it = it.filter_row(arg_filter_row);
            it = it.footer(arg_footer);
            it = it.horizontal_lines(arg_horizontal_lines);
            it = it.row_height(arg_row_height);
            it = it.search_panel(arg_search_panel);
            it = it.selection_mode(arg_selection_mode.clone());
            it = it.striped(arg_striped);
            it = it.toolbar(arg_toolbar);
            it = it.vertical_lines(arg_vertical_lines);
            it
        }
    })
}

impl ComponentBook for DataGrid {
    fn book() -> Meta {
        Meta::new("Data/DataGrid", render)
            .summary("Virtualized grid for 100,000 rows: sorting, filters, grouping and export.")
            .arg(ArgType::number("height", 380.0, 0.0, 1520.0, 1.0))
            .arg(ArgType::number("detail_height", 240.0, 0.0, 960.0, 1.0))
            .arg(ArgType::text("empty_text", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::text("export_title", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::bool("filter_builder", false))
            .arg(ArgType::bool("filter_row", true))
            .arg(ArgType::bool("footer", true))
            .arg(ArgType::bool("horizontal_lines", true))
            .arg(ArgType::number("row_height", 36.0, 0.0, 144.0, 1.0))
            .arg(ArgType::bool("search_panel", true))
            .arg(ArgType::choice("selection_mode", &["None", "Single", "Multiple"], "Single"))
            .arg(ArgType::bool("striped", false))
            .arg(ArgType::bool("toolbar", true))
            .arg(ArgType::bool("vertical_lines", false))
            .story("Example", Args::new())
            .story("Striped", Args::new().with("striped", true))
            .story("Compact", Args::new().with("row_height", 28.0).with("striped", true))
            .story(
                "Bare",
                Args::new()
                    .with("toolbar", false)
                    .with("search_panel", false)
                    .with("filter_row", false)
                    .with("footer", false),
            )
            .story("Vertical lines", Args::new().with("vertical_lines", true))
            .story("Selection mode: None", Args::new().with("selection_mode", ArgValue::choice("None")))
            .story(
                "Selection mode: Multiple",
                Args::new().with("selection_mode", ArgValue::choice("Multiple")),
            )
    }
}
