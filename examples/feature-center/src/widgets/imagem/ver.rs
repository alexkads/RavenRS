//! `ZoomPanView`, `Minimap`, `CompareView` and `ImageLightbox`, every feature: a photo to zoom
//! and move with its map, a floor plan drawn in code that zooms the same way, before and after
//! side by side or under a divider, and a gallery over the whole app. Lines say the zoom, the
//! divider, the picture chosen and the one open.

use ravenrs::prelude::*;

use super::{PHOTO, decimal, heading, note, photo, shown_size};

/// The control's name, as the session knows it.
pub const ID: &str = "ImageViewingDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Ver imagens";

/// How the comparison shows its pictures, in the order of the switch.
pub const MODES: [&str; 2] = ["Lado a lado", "Com divisória"];

/// The size of the floor plan, in its own units (centimeters).
const PLAN: Size = Size::new(1200.0, 700.0);

/// The pictures of the gallery, with their captions.
pub const GALLERY: [(&str, f32); 3] = [("Manhã na serra", 0.0), ("Fim de tarde", 0.5), ("Pôr do sol", 1.0)];

/// Draws the floor plan of a small shop where the view puts it: rooms, walls and a grid of meters.
fn floor_plan(at: ZoomPanPaint, scene: &mut Scene) {
    let unit = at.scale;
    let place = |x: f32, y: f32, w: f32, h: f32| {
        Rect::new(at.placed.x() + x * unit, at.placed.y() + y * unit, w * unit, h * unit)
    };
    scene.push_quad(Quad::new(at.placed, Color::hex(0xF4EFE6)));
    for meter in 1..12 {
        let x = meter as f32 * 100.0;
        scene.push_quad(Quad::new(place(x, 0.0, 1.0 / unit.max(0.01), PLAN.height), Color::hex(0xDDD5C7)));
    }
    for meter in 1..7 {
        let y = meter as f32 * 100.0;
        scene.push_quad(Quad::new(place(0.0, y, PLAN.width, 1.0 / unit.max(0.01)), Color::hex(0xDDD5C7)));
    }
    let rooms = [
        (20.0, 20.0, 700.0, 660.0, 0xCFE3F5),
        (740.0, 20.0, 440.0, 320.0, 0xD8EFD3),
        (740.0, 360.0, 440.0, 320.0, 0xF6DCC8),
    ];
    for (x, y, w, h, color) in rooms {
        scene.push_quad(Quad::new(place(x, y, w, h), Color::hex(color)).border(2.0, Color::hex(0x4B5563)));
    }
    // The counter and the shelves, so that zooming in shows detail.
    scene.push_quad(Quad::new(place(80.0, 520.0, 300.0, 60.0), Color::hex(0x8B5E34)));
    for shelf in 0..5 {
        let x = 100.0 + shelf as f32 * 110.0;
        scene.push_quad(Quad::new(place(x, 80.0, 40.0, 340.0), Color::hex(0x6B7280)));
    }
}

/// The page: the photo and its map, the plan, the comparison and the gallery.
#[derive(Debug, Default)]
pub struct ImageViewingPage;

impl View for ImageViewingPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let shown = shown_size(cx, 480.0);
        // Made once: a picture is a texture, and making it again would upload it again.
        let large = cx.use_state(|| photo(960, 600, 0.0)).get();
        let thumbnail = cx.use_state(|| photo(96, 60, 0.0)).get();
        let before = cx.use_state(|| photo(PHOTO.width as u32, PHOTO.height as u32, 0.0)).get();
        let after = cx.use_state(|| photo(PHOTO.width as u32, PHOTO.height as u32, 0.8)).get();
        let gallery = cx.use_state(|| {
            GALLERY
                .iter()
                .filter_map(|(caption, warmth)| {
                    Some(LightboxItem::new(photo(640, 400, *warmth)?).caption(*caption))
                })
                .collect::<Vec<_>>()
        });

        let view = cx.use_state(ZoomView::fit);
        let plan_view = cx.use_state(ZoomView::fit);
        let coast = cx.use_state(|| true);
        let mode = cx.use_state(|| 0_usize);
        let divider = cx.use_state(|| 0.5_f32);
        let chosen = cx.use_state(|| CompareSide::First);
        let captions = cx.use_state(|| true);
        let open = cx.use_state(|| None::<usize>);
        let wrap = cx.use_state(|| false);

        // Zoom and pan, with the map and the buttons that zoom from outside.
        let current = view.get();
        let zoom_line = if current.fit {
            "Zoom: ajustado à tela".to_string()
        } else {
            format!(
                "Zoom: {}% · centro em {}%, {}%",
                (current.scale * 100.0).round(),
                (current.center.x * 100.0).round(),
                (current.center.y * 100.0).round()
            )
        };
        let (to_double, to_fit) = (view.clone(), view.clone());
        let photo_row: AnyView = match (large, thumbnail) {
            (Some(large), Some(thumbnail)) => Box::new(
                HStack::new((
                    ZoomPanView::image(view.binding(), large)
                        .size(shown.width, shown.height)
                        .range(0.1, 8.0)
                        .inertia(coast.get())
                        .label("Foto ampliável"),
                    VStack::new((
                        Minimap::image(view.binding(), thumbnail)
                            .content_size(Size::new(960.0, 600.0))
                            .max_size(if narrow { 120.0 } else { 160.0 }, 120.0)
                            .press_zoom(2.0)
                            .label("Mapa da foto"),
                        ZoomControl::new(view.binding()).range(0.1, 8.0).label("Zoom da foto"),
                    ))
                    .spacing(8.0)
                    .align(Align::Start),
                ))
                .spacing(12.0)
                .wrap(),
            ),
            _ => Box::new(Text::new("Sem imagem")),
        };
        let zoom = VStack::new((
            heading("Ampliar e mover"),
            note(
                "Pinça ou Ctrl com a roda amplia, arrastar move, clique duplo alterna entre ajustar e 100%.",
            ),
            photo_row,
            HStack::new((
                Button::new("Ver em 200%").on_click(move || to_double.set(ZoomView::at(2.0))),
                Button::new("Ajustar à tela").on_click(move || to_fit.set(ZoomView::fit())),
                Toggle::new(coast.binding()).label("Deslizar ao soltar"),
            ))
            .spacing(8.0)
            .wrap(),
            Text::new(zoom_line),
        ))
        .spacing(8.0)
        .align(Align::Start);

        // Content drawn in code zooms the same way, and its map draws it smaller.
        let plan_now = plan_view.get();
        let plan = VStack::new((
            heading("Planta desenhada em código"),
            note("Sem imagem: o desenho vem de uma função, na escala que a vista pede."),
            HStack::new((
                ZoomPanView::custom(plan_view.binding(), PLAN, floor_plan)
                    .size(if narrow { shown.width } else { 320.0 }, if narrow { 160.0 } else { 220.0 })
                    .grow(1.0)
                    .label("Planta da loja"),
                Minimap::custom(plan_view.binding(), PLAN, floor_plan)
                    .max_size(140.0, 90.0)
                    .label("Mapa da planta"),
            ))
            .spacing(12.0)
            .wrap(),
            Text::new(if plan_now.fit {
                "Planta inteira".to_string()
            } else {
                format!("Planta em {}%", (plan_now.scale * 100.0).round())
            }),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        // Before and after: side by side with a choice, or one over the other with a divider.
        let modes: AnyView = if narrow {
            Box::new(Select::new(mode.binding(), MODES).label("Comparar"))
        } else {
            Box::new(SegmentedControl::new(mode.binding(), MODES))
        };
        let compare_mode = if mode.get() == 0 { CompareMode::SideBySide } else { CompareMode::Divider };
        let compare: AnyView = match (before, after) {
            (Some(before), Some(after)) => {
                let view = CompareView::new(before, after)
                    .mode(compare_mode)
                    .divider(divider.binding())
                    .chosen(chosen.binding())
                    .size(shown.width, shown.height * 0.8)
                    .grow(if narrow { 0.0 } else { 1.0 })
                    .label("Comparação da foto");
                Box::new(if captions.get() { view.captions("Antes", "Depois") } else { view.no_captions() })
            }
            _ => Box::new(Text::new("Sem imagem")),
        };
        let picked = if chosen.get() == CompareSide::First { "Antes" } else { "Depois" };
        let compare_line = if mode.get() == 0 {
            format!("Escolhida: {picked}")
        } else {
            format!("Divisória em {}%", decimal(divider.get() * 100.0, 0))
        };
        let compared = VStack::new((
            heading("Antes e depois"),
            modes,
            HStack::new((compare,)).align(Align::Start),
            Toggle::new(captions.binding()).label("Legendas sobre as fotos"),
            Text::new(compare_line),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        // The gallery: over the whole app, one picture at a time.
        let opener = open.clone();
        let items = gallery.get();
        let count = items.len();
        let open_line = match open.get() {
            Some(index) => format!(
                "Aberta: {} ({} de {count})",
                GALLERY.get(index).map_or("?", |(caption, _)| *caption),
                index + 1
            ),
            None => "Galeria fechada".to_string(),
        };
        let lightbox = VStack::new((
            heading("Galeria"),
            note("As setas passam as fotos, + e - ampliam, Esc fecha."),
            HStack::new((
                Button::new("Abrir a galeria").on_click(move || opener.set(Some(0))),
                Toggle::new(wrap.binding()).label("Voltar à primeira depois da última"),
            ))
            .spacing(8.0)
            .wrap(),
            Text::new(open_line),
            ImageLightbox::new(open.binding(), items).wrap(wrap.get()).label("Fotos da viagem"),
        ))
        .spacing(8.0)
        .align(Align::Start);

        cx.child(
            &VStack::new((zoom, plan, compared, lightbox))
                .spacing(24.0)
                .padding(if narrow { 0.0 } else { 16.0 })
                .align(Align::Stretch),
        )
    }
}
