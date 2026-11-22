//! The stories of `Lottie`. Written by hand: the gallery's example keeps its animation in a
//! constant the scaffold does not carry.

use ravenrs_lottie::Animation;
use ravenrs_widgets_base::book::{ArgType, Args, ComponentBook, Meta};
use ravenrs_widgets_base::{AnyView, Text};

use crate::Lottie;

/// A ring that draws itself, on and on: the gallery's Lottie file.
const LOADING_RING: &str = r#"{"v":"5.7.0","fr":30,"ip":0,"op":60,"w":100,"h":100,"layers":[{"ty":4,"ks":{},
"ip":0,"op":60,"st":0,"shapes":[{"ty":"gr","it":[{"ty":"el","p":{"a":0,"k":[50,50]},"s":{"a":0,"k":[70,70]}},
{"ty":"tm","s":{"a":1,"k":[{"t":0,"s":[0],"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":30,"s":[0],
"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":60,"s":[100]}]},"e":{"a":1,"k":[{"t":0,"s":[0],
"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":30,"s":[100]}]},"o":{"a":0,"k":0},"m":1},
{"ty":"st","c":{"a":0,"k":[0.25,0.34,0.86,1]},"o":{"a":0,"k":100},"w":{"a":0,"k":9},"lc":2,"lj":2},
{"ty":"tr","a":{"a":0,"k":[0,0]},"p":{"a":0,"k":[0,0]},"s":{"a":0,"k":[100,100]},"r":{"a":0,"k":0},"o":{"a":0,"k":100}}]}]}]}"#;

fn render(args: &Args) -> AnyView {
    match Animation::from_json(LOADING_RING.as_bytes()) {
        Ok(animation) => {
            let size = args.number("size") as f32;
            Box::new(
                Lottie::new(animation)
                    .size(size, size)
                    .looping(args.bool("looping"))
                    .speed(args.number("speed"))
                    .alt(args.text("alt")),
            )
        }
        Err(_) => Box::new(Text::new("No animation")),
    }
}

impl ComponentBook for Lottie {
    fn book() -> Meta {
        Meta::new("Media/Lottie", render)
            .summary("Plays a Lottie animation, as LottieFiles keeps them, drawn by RavenRS.")
            .arg(ArgType::number("size", 96.0, 24.0, 320.0, 4.0))
            .arg(ArgType::bool("looping", true))
            .arg(ArgType::number("speed", 1.0, 0.25, 3.0, 0.25))
            .arg(ArgType::text("alt", "A ring that draws itself").description("What a screen reader says."))
            .story("Loading ring", Args::new())
            .story("Large", Args::new().with("size", 192.0))
            .story("Once", Args::new().with("looping", false))
    }
}
