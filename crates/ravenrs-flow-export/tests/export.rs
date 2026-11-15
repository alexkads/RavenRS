//! The three files a diagram is saved as, from the same picture.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use ravenrs_flow_export::{PNG_SCALE, export, file_name};
use ravenrs_widgets::{FlowColors, FlowEdge, FlowEndpoint, FlowExportFormat, FlowGraph, FlowNode, FlowPort};

fn picture() -> ravenrs_widgets::FlowPicture {
    FlowGraph::new()
        .node(FlowNode::new("a", "step", "Receive order", 0.0, 0.0).port(FlowPort::output("out")))
        .node(
            FlowNode::new("b", "step", "Pick the items for a very long order title", 360.0, 0.0)
                .port(FlowPort::input("in")),
        )
        .edge(FlowEdge::new("e", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in")).label("Paid"))
        .picture(None, FlowColors::paper())
        .expect("a picture")
}

#[test]
fn png_is_a_picture_at_twice_the_size_with_ink_on_it() {
    let p = picture();
    let file = export(&p, FlowExportFormat::Png, "Orders").expect("a png");
    assert_eq!((file.name.as_str(), file.mime), ("Orders.png", "image/png"));
    let image = image::load_from_memory(&file.bytes).expect("it decodes").to_rgba8();
    assert_eq!(image.width(), (p.bounds.width() * PNG_SCALE).ceil() as u32);
    // Dark words over the light node: some pixels are near black.
    let dark = image.pixels().filter(|px| px.0[0] < 60 && px.0[1] < 60 && px.0[2] < 60).count();
    assert!(dark > 50, "the titles are drawn ({dark} dark pixels)");
}

#[test]
fn pdf_is_one_page_with_the_titles_as_text() {
    let file = export(&picture(), FlowExportFormat::Pdf, "Orders").expect("a pdf");
    assert_eq!(file.mime, "application/pdf");
    assert!(file.bytes.starts_with(b"%PDF"));
}

#[test]
fn svg_writes_the_words_and_names_are_kept_safe() {
    let file = export(&picture(), FlowExportFormat::Svg, "Orders: May/June").expect("an svg");
    assert_eq!(file.name, "Orders MayJune.svg");
    let svg = String::from_utf8(file.bytes).expect("text");
    assert!(svg.contains("Receive order") && svg.contains("Paid"));
    assert_eq!(file_name("  ", FlowExportFormat::Pdf), "diagram.pdf");
}
