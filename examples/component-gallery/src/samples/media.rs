//! Pictures, films and conversations.

use std::collections::BTreeSet;

use ravenrs::prelude::*;

use super::picture;

samples! {
    "MoviePlayer" => movie_player, "MovieAssistant" => movie_assistant, "Chat" => chat,
    "ModelPicker" => model_picker, "AttachmentStrip" => attachment_strip,
    "AudioPlayerButton" => audio_player_button, "CompareView" => compare_view,
    "ImageLightbox" => image_lightbox, "Filmstrip" => filmstrip, "Image" => image, "Lottie" => lottie,
    "Carousel" => carousel, "Canvas" => canvas,
}

// sample: MoviePlayer, 520
fn movie_player() -> impl View {
    use ravenrs::core::Primitive;
    use ravenrs::movie::{FilmBuilder, MoviePlayer};
    // Real films come from `Recorder`, which runs the app from a script.
    // ravenrs-lint: allow(raw_color) the film is a recording, and its white page is part of the picture
    let mut film = FilmBuilder::new(Size::new(320.0, 200.0), Color::WHITE);
    for second in 0..=6_u64 {
        let square =
            // ravenrs-lint: allow(raw_color) the square is drawn into the film, not into the interface
            Quad::new(Rect::new(20.0 + second as f32 * 40.0, 80.0, 40.0, 40.0), Color::hex(0x4f46e5));
        film.frame(second * 1000, &[Primitive::Quad(square.radius(8.0))]);
    }
    film.chapter("A square", "It goes across the screen.", 0);
    film.finish(6000).map(|film| MoviePlayer::new(film).width(640.0)).ok()
}
// end

// sample: MovieAssistant, 760
fn movie_assistant() -> impl View {
    use ravenrs::core::Primitive;
    use ravenrs::movie::{FilmBuilder, MovieAssistant, MovieLibrary};
    // ravenrs-lint: allow(raw_color) the film is a recording, and its white page is part of the picture
    let mut film = FilmBuilder::new(Size::new(320.0, 200.0), Color::WHITE);
    for second in 0..=9_u64 {
        let square =
            // ravenrs-lint: allow(raw_color) the square is drawn into the film, not into the interface
            Quad::new(Rect::new(20.0 + second as f32 * 25.0, 80.0, 40.0, 40.0), Color::hex(0x4f46e5));
        film.frame(second * 1000, &[Primitive::Quad(square.radius(8.0))]);
    }
    film.chapter("Move a square", "How it goes across the screen.", 0);
    film.chapter("Save your work", "Keep a copy of what you did.", 5000);
    // Accents and case do not matter, and near words count: try "saving".
    film.finish(9000)
        .map(|film| MovieAssistant::new(MovieLibrary::new().film("Manual", film)).width(640.0))
        .ok()
}
// end

// sample: AttachmentStrip, 120
fn attachment_strip() -> impl View {
    // The app holds the list, and changes it when told.
    AttachmentStrip::new([
        Attachment::new("report.pdf").size(120_000),
        Attachment::new("beach.jpg").size(2_400_000).icon(IconName::Image).send_as(
            ["photo", "document"],
            0,
            |_how| {},
        ),
    ])
    .on_remove(|_index| {})
    .width(420.0)
}
// end

// sample: AudioPlayerButton, 150
fn audio_player_button() -> impl View {
    // A second of a low tone, computed here. An app delivers its own recording.
    let tone: Vec<f32> =
        (0..44_100).map(|i| (i as f32 / 44_100.0 * 220.0 * std::f32::consts::TAU).sin() * 0.2).collect();
    AudioPlayerButton::new()
        .clip(SoundClip::new(tone, 44_100))
        .transcript("What was said goes here, to open when wanted.")
        .width(320.0)
}
// end

// sample: Chat, 420
fn chat() -> impl View {
    /// Keeps the messages, here while the page rebuilds. A real app keeps them in its state.
    struct Conversation;

    impl View for Conversation {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let messages = cx.use_state(Vec::<ChatMessage>::new);
            let answers = messages.clone();
            let chat = Chat::new(messages.binding())
                .placeholder("Ask something")
                .intro("It does not know who answers: yours adds to the messages, now or later.")
                .examples(["How do I start?", "Where are my files?"])
                .on_send(move |text| {
                    // Marks, links and a file in the bubble are asked for message by message.
                    let reply = ChatMessage::assistant(format!("*You asked:* {text}"))
                        .formatted()
                        .media(ChatMedia::document("answer.pdf").size(48_000).on_open(|| {}))
                        .card(|| Box::new(Button::new("Open it").size(ButtonSize::Small)));
                    answers.update(|all| all.push(reply));
                });
            cx.child(&VStack::new((chat,)).height(380.0).width(420.0))
        }
    }
    Conversation
}
// end

// sample: ModelPicker, 360
fn model_picker() -> impl View {
    let choice = |name: &str, status| ModelChoice {
        name: name.into(),
        details: "1.1 GB · License: Apache 2.0".into(),
        status,
        in_use: false,
        result: None,
        error: None,
    };
    // It shows what it is given. `ravenrs_ai::ModelManager` does the work on the desktop.
    ModelPicker::new(vec![
        choice("To download", ModelStatus::Available),
        choice("Arriving", ModelStatus::Downloading(0.4)),
        ModelChoice {
            result: Some("About 20 words per second.".into()),
            ..choice("On this machine", ModelStatus::Installed)
        },
    ])
    .on_test(|_| {})
    .on_use(|_| {})
}
// end

// sample: Lottie, 140
/// A Lottie file: a ring that draws itself, on and on. Any file from LottieFiles goes here.
const LOADING_RING: &str = r#"{"v":"5.7.0","fr":30,"ip":0,"op":60,"w":100,"h":100,"layers":[{"ty":4,"ks":{},
"ip":0,"op":60,"st":0,"shapes":[{"ty":"gr","it":[{"ty":"el","p":{"a":0,"k":[50,50]},"s":{"a":0,"k":[70,70]}},
{"ty":"tm","s":{"a":1,"k":[{"t":0,"s":[0],"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":30,"s":[0],
"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":60,"s":[100]}]},"e":{"a":1,"k":[{"t":0,"s":[0],
"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":30,"s":[100]}]},"o":{"a":0,"k":0},"m":1},
{"ty":"st","c":{"a":0,"k":[0.25,0.34,0.86,1]},"o":{"a":0,"k":100},"w":{"a":0,"k":9},"lc":2,"lj":2},
{"ty":"tr","a":{"a":0,"k":[0,0]},"p":{"a":0,"k":[0,0]},"s":{"a":0,"k":[100,100]},"r":{"a":0,"k":0},"o":{"a":0,"k":100}}]}]}]}"#;

fn lottie() -> impl View {
    match LottieAnimation::from_json(LOADING_RING.as_bytes()) {
        Ok(animation) => Some(Lottie::new(animation).size(96.0, 96.0).alt("A ring that draws itself")),
        Err(_) => None,
    }
}
// end

// sample: CompareView, 210
fn compare_view() -> AnyView {
    let tone = |warm: f32| {
        ImageData::from_fn(240, 160, move |x, y| {
            let light = 0.25 + 0.5 * x as f32 / 240.0;
            Color::rgb(light + warm, light, light - warm + 0.2 * y as f32 / 160.0)
        })
    };
    match (tone(0.0), tone(0.2)) {
        (Some(before), Some(after)) => {
            Box::new(CompareView::new(before, after).mode(CompareMode::Divider).size(420.0, 200.0))
        }
        _ => Box::new(Text::new("No pictures")),
    }
}
// end

// sample: ImageLightbox, 60
fn image_lightbox() -> impl View {
    let open = State::new(None::<usize>);
    let opener = open.clone();
    let shades = [0.25_f32, 0.45, 0.65];
    let pictures = shades.into_iter().filter_map(|shade| {
        let image = ImageData::from_fn(320, 200, move |x, _| Color::rgb(shade, 0.3, x as f32 / 320.0))?;
        Some(LightboxItem::new(image).caption(format!("Shade {shade}")))
    });
    VStack::new((
        Button::new("Open pictures").on_click(move || opener.set(Some(0))),
        ImageLightbox::new(open.binding(), pictures),
    ))
}
// end

// sample: Filmstrip, 130
fn filmstrip() -> impl View {
    let selected = State::new(BTreeSet::from([2_usize]));
    Filmstrip::new(5_000, |cell: FilmstripCell| {
        // The cell belongs to the app: its picture, its stars, its badges.
        let shade = (cell.index % 10) as f32 / 10.0;
        let picture = ImageData::from_fn(64, 64, move |x, y| {
            Color::rgb(0.2 + shade * 0.5, 0.3 + y as f32 / 200.0, 0.7 - x as f32 / 200.0)
        });
        let picture: AnyView = match picture {
            Some(picture) => Box::new(Image::new(picture).height(72.0).stretch()),
            None => Box::new(Spacer::new()),
        };
        VStack::new((picture, Rating::read_only((cell.index % 6) as u32).size(11.0)))
            .spacing(4.0)
            .align(Align::Center)
    })
    .cell_name(|index| format!("Photo {}", index + 1))
    .selection(selected.binding())
    .mode(SelectionMode::Multiple)
    .cell_width(120.0)
    .height(112.0)
    .fade(Semantic::SurfaceElevated)
}
// end

// sample: Image, 150
fn image() -> impl View {
    HStack::new((
        // `.crossfade` dissolves into the next image the app hands it.
        Image::new(picture(1.0)).size(160.0, 90.0).corner_radius(12.0).crossfade(0.2),
        Image::new(picture(0.3)).size(90.0, 90.0).corner_radius(45.0),
    ))
    .spacing(12.0)
}
// end

// sample: Carousel, 280
fn carousel() -> impl View {
    let slides: Vec<AnyView> = [1.0, 0.6, 0.2]
        .into_iter()
        .map(|hue| Box::new(Image::new(picture(hue)).stretch().height(220.0)) as AnyView)
        .collect();
    Carousel::new(slides).height(220.0)
}
// end

// sample: Canvas, 170
fn canvas() -> impl View {
    Canvas::new(|frame, scene| {
        for i in 0..8 {
            let x = frame.x() + i as f32 * 28.0;
            let height = 20.0 + i as f32 * 12.0;
            let bar = Rect::new(x, frame.max_y() - height, 22.0, height);
            // ravenrs-lint: allow(raw_color) a Canvas paints plain colors: it is drawing, with no theme to read
            scene.push_quad(Quad::new(bar, Color::rgb(0.3, 0.4, 0.93)).radius(4.0));
        }
    })
    .size(240.0, 130.0)
}
// end
