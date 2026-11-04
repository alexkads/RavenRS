//! What a file asks for reaches the curves.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{Color, PathElement, PathFill, PathPaint};
use ravenrs_lottie::Animation;

fn file(layers: &str, assets: &str) -> Vec<PathFill> {
    let json = format!(
        r#"{{"v":"5.7.0","fr":10,"ip":0,"op":10,"w":100,"h":100,"assets":[{assets}],"layers":[{layers}]}}"#
    );
    let animation = Animation::from_json(json.as_bytes()).expect("a Lottie");
    assert_eq!(animation.unsupported(), Vec::<String>::new());
    animation.paths_at(0.0, 1.0)
}

const SOLID_RED_SHAPE: &str = r#"{"ty":"fl","c":{"a":0,"k":[1,0,0,1]},"o":{"a":0,"k":100}}"#;

/// The middle of the first curve of a shape's outline, from its center.
fn star_with(outer_round: f32) -> Vec<PathFill> {
    let star = format!(
        r#"{{"ty":"sr","sy":1,"p":{{"a":0,"k":[50,50]}},"r":{{"a":0,"k":0}},"pt":{{"a":0,"k":5}},
        "ir":{{"a":0,"k":20}},"or":{{"a":0,"k":40}},"is":{{"a":0,"k":0}},"os":{{"a":0,"k":{outer_round}}}}}"#
    );
    file(&format!(r#"{{"ty":4,"ks":{{}},"ip":0,"op":10,"st":0,"shapes":[{star},{SOLID_RED_SHAPE}]}}"#), "")
}

#[test]
fn a_star_bends_its_corners_when_asked() {
    let handles = |paths: &[PathFill]| -> Option<(f32, f32, f32, f32)> {
        paths[0].elements.iter().find_map(|e| match e {
            PathElement::CubicTo(c1, _, p) => Some((c1.x, c1.y, p.x, p.y)),
            _ => None,
        })
    };
    let sharp = star_with(0.0);
    let round = star_with(100.0);
    let Some((x, y, ..)) = handles(&round) else { panic!("curves") };
    // The first corner is at the top; a round one leaves it sideways, along its circle.
    assert!((y - 10.0).abs() < 1e-3, "leaves level: {y}");
    assert!(x > 55.0, "and to the right: {x}");
    if let Some((x, y, px, py)) = handles(&sharp) {
        let straight = (x - 50.0) * (py - 10.0) - (y - 10.0) * (px - 50.0);
        assert!(straight.abs() < 1e-2, "a sharp corner leaves along its side");
    }
}

#[test]
fn the_fill_effect_paints_everything_in_the_layer() {
    let effect = r#"{"ty":21,"ef":[{"ty":3,"ix":1,"v":{"k":0}},{"ty":2,"ix":3,"v":{"k":[0,1,0,1]}},{"ty":0,"ix":7,"v":{"k":0.5}}]}"#;
    let paths = file(
        &format!(
            r#"{{"ty":4,"ks":{{}},"ip":0,"op":10,"st":0,"ef":[{effect}],"shapes":[{{"ty":"rc","p":{{"a":0,"k":[50,50]}},
            "s":{{"a":0,"k":[20,20]}},"r":{{"a":0,"k":0}}}},{SOLID_RED_SHAPE}]}}"#
        ),
        "",
    );
    assert_eq!(paths[0].paint, PathPaint::Solid(Color::rgba(0.0, 1.0, 0.0, 1.0)));
    assert!((paths[0].opacity - 0.5).abs() < 1e-3);
}

#[test]
fn an_image_in_the_file_shows_where_its_layer_puts_it() {
    // One red pixel, as a PNG.
    let png =
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
    let paths = file(
        r#"{"ty":2,"refId":"pic","ks":{"p":{"a":0,"k":[10,20]},"a":{"a":0,"k":[0,0]},"s":{"a":0,"k":[200,100]}},"ip":0,"op":10,"st":0}"#,
        &format!(r#"{{"id":"pic","w":30,"h":40,"e":1,"u":"","p":"data:image/png;base64,{png}"}}"#),
    );
    let PathPaint::Image { image, placement } = &paths[0].paint else { panic!("an image") };
    assert_eq!((image.width(), image.height()), (1, 1));
    assert_eq!(*placement, [60.0, 0.0, 0.0, 40.0, 10.0, 20.0]);
}

#[test]
fn an_image_kept_outside_the_file_is_said_and_not_guessed() {
    let json = br#"{"v":"5.7.0","fr":10,"ip":0,"op":10,"w":100,"h":100,
        "assets":[{"id":"pic","w":30,"h":40,"u":"images/","p":"a.png"}],
        "layers":[{"ty":2,"refId":"pic","ks":{},"ip":0,"op":10,"st":0}]}"#;
    let animation = Animation::from_json(json).expect("a Lottie");
    assert_eq!(animation.unsupported(), ["imagens fora do arquivo"]);
    assert!(animation.paths_at(0.0, 1.0).is_empty());
}
