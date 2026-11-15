//! The steps of the family `ravenrs-widgets-imaging`: the pages Ver imagens, Recortar e girar,
//! Seleção e medida, Cor e tons and Animação.

use super::*;
use ravenrs_core::{Event, Key, Modifiers, Point, PointerButton};

/// Gives the keyboard to what has this role and this name.
fn focus(c: &mut Center, role: Role, name: &str) {
    c.settle();
    let tree = c.ui.access_tree();
    let found = tree.nodes.iter().find(|n| n.role == role && n.name.as_deref() == Some(name));
    let id = found.unwrap_or_else(|| panic!("no {role:?} {name}: {:?}", c.texts())).id;
    c.ui.perform_action(id, AccessAction::Focus);
    c.settle();
}

/// Presses a key with these modifiers held.
fn key_with(c: &mut Center, key: Key, modifiers: Modifiers) {
    c.ui.handle_event(&Event::KeyDown { key, modifiers });
    c.settle();
}

/// Types one character as a key.
fn character(c: &mut Center, text: &str) {
    c.key(Key::Character(text.to_string()));
}

/// Clicks the middle of what has this role and this name.
fn click_on(c: &mut Center, role: Role, name: &str) {
    c.settle();
    let tree = c.ui.access_tree();
    let node = tree.nodes.iter().find(|n| n.role == role && n.name.as_deref() == Some(name));
    let b = node.unwrap_or_else(|| panic!("no {role:?} {name}: {:?}", c.texts())).bounds;
    let at = Point::new(b.x() + b.width() / 2.0, b.y() + b.height() / 4.0);
    c.ui.handle_event(&Event::PointerMove { position: at });
    c.ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    c.ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    c.settle();
}

/// The value a screen reader hears for what has this role and this name.
fn value_of(c: &mut Center, role: Role, name: &str) -> Option<String> {
    c.settle();
    let tree = c.ui.access_tree();
    tree.nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref() == Some(name))
        .and_then(|n| n.value.clone())
}

fn shift() -> Modifiers {
    Modifiers { shift: true, ..Modifiers::default() }
}

fn alt() -> Modifiers {
    Modifiers { alt: true, ..Modifiers::default() }
}

#[test]
fn the_photo_zooms_and_moves_with_its_map_the_comparison_divides_and_the_gallery_turns_pages() {
    use feature_center::widgets::imagem::ver::MODES;
    let mut c = Center::memory("ana");
    c.press("Ver imagens");
    assert!(c.says("Zoom: ajustado à tela"), "{:?}", c.texts());
    // From outside, and then from the keyboard of the view.
    c.press("Ver em 200%");
    assert!(c.says("Zoom: 200%"), "{:?}", c.texts());
    focus(&mut c, Role::Image, "Foto ampliável");
    character(&mut c, "+");
    assert!(!c.says("Zoom: 200%") && c.says("Zoom: "), "the + key zooms in: {:?}", c.texts());
    // The map moves the view: the center goes right.
    focus(&mut c, Role::Slider, "Mapa da foto");
    c.key(Key::ArrowRight);
    assert!(!c.says("centro em 50%, 50%"), "the map moves the view: {:?}", c.texts());
    c.press("Ajustar à tela");
    assert!(c.says("Zoom: ajustado à tela"));
    // The plan drawn in code zooms to 100% with its key.
    assert!(c.says("Planta inteira"));
    focus(&mut c, Role::Image, "Planta da loja");
    character(&mut c, "1");
    assert!(c.says("Planta em 100%"), "{:?}", c.texts());
    // Side by side, the arrows choose the other picture.
    assert!(c.says("Escolhida: Antes"));
    focus(&mut c, Role::Group, "Comparação da foto");
    c.key(Key::ArrowRight);
    assert!(c.says("Escolhida: Depois"), "{:?}", c.texts());
    assert!(c.has_name("Antes") && c.has_name("Depois"), "the captions show");
    c.activate("Legendas sobre as fotos");
    assert!(!c.has_name("Antes"), "no captions: {:?}", c.texts());
    // With a divider, the arrows and End move it.
    c.activate(MODES[1]);
    assert!(c.says("Divisória em 50%"), "{:?}", c.texts());
    focus(&mut c, Role::Slider, "Comparação da foto");
    key_with(&mut c, Key::ArrowRight, shift());
    assert!(!c.says("Divisória em 50%"), "the divider moved: {:?}", c.texts());
    c.key(Key::End);
    assert!(c.says("Divisória em 100%"), "{:?}", c.texts());
    // The gallery opens on the first, goes to the next and stops at the last.
    c.press("Abrir a galeria");
    // Over the whole app, the gallery is all a screen reader hears: its name, caption and counter.
    assert!(c.has_name("Fotos da viagem") && c.has_name("Manhã na serra"), "{:?}", c.texts());
    assert!(c.says("1 de 3") && !c.says("Galeria fechada"), "modal: {:?}", c.texts());
    c.key(Key::ArrowRight);
    assert!(c.has_name("Fim de tarde") && c.says("2 de 3"), "{:?}", c.texts());
    c.key(Key::End);
    c.key(Key::ArrowRight);
    assert!(c.has_name("Pôr do sol") && c.says("3 de 3"), "no wrap, it stays: {:?}", c.texts());
    c.key(Key::Escape);
    assert!(c.says("Galeria fechada"), "{:?}", c.texts());
    // With wrap, past the last comes the first.
    c.activate("Voltar à primeira depois da última");
    c.press("Abrir a galeria");
    c.key(Key::End);
    c.key(Key::ArrowRight);
    assert!(c.has_name("Manhã na serra") && c.says("1 de 3"), "it wraps: {:?}", c.texts());
    c.key(Key::Escape);
}

#[test]
fn the_crop_moves_and_straightens_the_guides_show_and_the_layer_turns() {
    use feature_center::widgets::imagem::recorte::{ASPECTS, LIMITS};
    let mut c = Center::memory("ana");
    c.press("Recortar e girar");
    assert!(c.says("Recorte: 240 × 140 a 0,0°"), "{:?}", c.texts());
    // The keys move and resize the crop, and each change is a commit.
    focus(&mut c, Role::Group, "Recorte da foto");
    key_with(&mut c, Key::ArrowRight, alt());
    assert!(!c.says("Recorte: 240 × 140"), "Alt and an arrow resize it: {:?}", c.texts());
    assert!(!c.says("Arraste uma alça do recorte"), "the commit wrote its line: {:?}", c.texts());
    c.key(Key::Character("]".to_string()));
    assert!(!c.says("a 0,0°"), "] turns it: {:?}", c.texts());
    // The protractor shares the crop's angle.
    focus(&mut c, Role::Slider, "Endireitar");
    c.key(Key::Delete);
    assert!(c.says("Endireitado em 0,0°") && c.says("a 0,0°"), "back to zero: {:?}", c.texts());
    c.key(Key::ArrowRight);
    assert!(c.says("Endireitado em 0,5°"), "a step of 0.5: {:?}", c.texts());
    c.key(Key::End);
    assert!(c.says("Endireitado em 45,0°"), "the end of the range: {:?}", c.texts());
    // A square crop cuts it down to its proportions.
    c.activate(ASPECTS[2]);
    let value = value_of(&mut c, Role::Group, "Recorte da foto");
    assert!(value.is_some(), "the crop says its size");
    // Without turning, the protractor is off.
    c.activate(LIMITS[2]);
    assert!(c.says("Recorte: "), "{:?}", c.texts());
    // A locked crop takes no keys.
    c.activate("Travar o recorte");
    let crop_line = |c: &mut Center| c.texts().into_iter().find(|t| t.starts_with("Recorte:"));
    let before = crop_line(&mut c);
    focus(&mut c, Role::Group, "Recorte da foto");
    key_with(&mut c, Key::ArrowLeft, shift());
    assert_eq!(crop_line(&mut c), before, "locked, the keys do nothing");
    // The guides are only lines: nothing in them for a screen reader, but their pictures are named.
    for caption in ["Terços", "Proporção áurea", "Quatro partes", "Três quadrados", "Terços do recorte"]
    {
        assert!(c.has_name(caption), "{caption}: {:?}", c.texts());
    }
    // The layer: the keys move and turn it, and the protractor turns it by 15 degrees.
    assert!(c.says("Camada: 120 × 80 em (90, 50) a 0°"), "{:?}", c.texts());
    focus(&mut c, Role::Group, "Camada do logotipo");
    c.key(Key::ArrowRight);
    assert!(c.says("Camada solta: 120 × 80"), "the commit: {:?}", c.texts());
    assert!(c.says("em (91, 50)"), "it moved: {:?}", c.texts());
    focus(&mut c, Role::Slider, "Giro da camada");
    c.key(Key::ArrowRight);
    assert!(c.says("Camada girada para 15°") && c.says("a 15°"), "{:?}", c.texts());
    // Locked, the layer leaves the keys alone.
    c.activate("Travar a camada");
    focus(&mut c, Role::Group, "Camada do logotipo");
    c.key(Key::ArrowRight);
    assert!(c.says("em (91, 50)"), "locked: {:?}", c.texts());
}

#[test]
fn the_band_selects_from_the_keys_the_outlines_follow_and_the_rulers_zoom() {
    let mut c = Center::memory("ana");
    c.press("Seleção e medida");
    assert!(c.says("Nada selecionado"), "{:?}", c.texts());
    // An arrow starts from the middle quarter, and Enter selects it.
    focus(&mut c, Role::Group, "Seleção na foto");
    c.key(Key::ArrowRight);
    assert!(c.says("Seleção: "), "{:?}", c.texts());
    c.key(Key::Enter);
    assert!(c.says("Selecionado: "), "on_select: {:?}", c.texts());
    // Escape clears it.
    c.key(Key::Escape);
    assert!(c.says("Nada selecionado"), "{:?}", c.texts());
    // The three outlines are named for screen readers.
    for name in ["Contorno da seleção", "Laço em volta do sol", "Borda da máscara"] {
        assert!(c.has_name(name), "{name}: {:?}", c.texts());
    }
    c.activate("Formigas andando");
    assert!(c.has_name("Contorno da seleção"), "still shown, standing");
    // The rulers follow the zoom and the pan, and mark where the selection starts.
    assert!(c.says("Escala: 1,50 px por pixel da foto · começa em 0"), "{:?}", c.texts());
    assert_eq!(value_of(&mut c, Role::Image, "Régua de cima").as_deref(), Some("60"), "the marker");
    c.press("Aproximar");
    assert!(c.says("Escala: 3,00"), "{:?}", c.texts());
    c.press("Mover para a direita");
    assert!(c.says("começa em 50"), "{:?}", c.texts());
    c.press("Voltar ao início");
    c.press("Afastar");
    c.press("Afastar");
    assert!(c.says("Escala: 0,75 px por pixel da foto · começa em 0"), "{:?}", c.texts());
    assert!(c.has_name("Régua do lado"));
    // A band turned off takes no keys.
    c.activate("Desligar a seleção");
    assert!(c.says("Nada selecionado"));
}

#[test]
fn the_wheels_tint_the_curve_bends_the_histogram_scales_and_the_eyedropper_picks() {
    use feature_center::widgets::imagem::tons::{CHANNELS, SCALES};
    let mut c = Center::memory("ana");
    c.press("Cor e tons");
    // The eyedropper: armed by its button, it picks the color under the click and disarms.
    assert!(c.says("Desarmado · última cor: nenhuma"), "{:?}", c.texts());
    c.press("Conta-gotas");
    assert!(c.says("Armado: clique na foto"), "{:?}", c.texts());
    click_on(&mut c, Role::Group, "Foto para pegar a cor");
    assert!(c.says("Cor pegada: #"), "picked: {:?}", c.texts());
    assert!(c.says("Desarmado · última cor: #"), "{:?}", c.texts());
    // Escape disarms it as well.
    c.press("Conta-gotas pequeno");
    assert!(c.says("Armado"));
    c.key(Key::Escape);
    assert!(c.says("Desarmado"), "{:?}", c.texts());
    // The wheels: the arrows turn the hue, Home takes the color out.
    assert!(c.says("Sombras: 220°, 25%"), "{:?}", c.texts());
    focus(&mut c, Role::Slider, "Sombras");
    c.key(Key::ArrowUp);
    assert!(!c.says("Sombras: 220°, 25%"), "up adds saturation: {:?}", c.texts());
    c.key(Key::Home);
    assert!(c.says("Sombras: 220°, 0%"), "Home keeps the hue: {:?}", c.texts());
    c.activate("Travar as rodas");
    focus(&mut c, Role::Slider, "Realces");
    c.key(Key::ArrowUp);
    assert!(c.says("Realces: 40°, 30%"), "locked: {:?}", c.texts());
    c.activate("Travar as rodas");
    // The curve: a preset, the value in the middle, and a point added from the keyboard.
    assert!(c.says("Luz · 3 pontos · o meio sai em 0,50"), "{:?}", c.texts());
    c.press("Contraste em S");
    assert!(c.says("4 pontos"), "{:?}", c.texts());
    c.press("Linha reta");
    assert!(c.says("2 pontos · o meio sai em 0,50"), "{:?}", c.texts());
    focus(&mut c, Role::Slider, "Curva de tons");
    c.key(Key::Enter);
    assert!(c.says("3 pontos"), "Enter adds a point: {:?}", c.texts());
    c.key(Key::ArrowUp);
    assert!(!c.says("o meio sai em 0,50"), "the point moved up: {:?}", c.texts());
    c.activate(CHANNELS[1]);
    assert!(c.says("Vermelho · 3 pontos"));
    // The histogram says its channels, in either scale.
    assert!(c.has_name("Histograma da foto") && c.has_name("Só o vermelho"));
    c.activate(SCALES[1]);
    assert!(c.has_name("Histograma da foto"));
    c.activate("Zonas");
    assert!(c.has_name("Histograma da foto"));
}

#[test]
fn the_animations_are_named_and_play_pause_and_change_speed() {
    use feature_center::widgets::imagem::animacao::SPEEDS;
    let mut c = Center::memory("ana");
    c.press("Animação");
    assert!(!c.says("Animação inválida"), "both animations read: {:?}", c.texts());
    for size in ["pequeno", "médio", "grande"] {
        assert!(c.has_name(&format!("Anel que se desenha, {size}")), "{size}: {:?}", c.texts());
        assert!(c.has_name(&format!("Ponto pulsando, {size}")));
    }
    assert!(c.says("Tocando · velocidade normal · repete sem parar"), "{:?}", c.texts());
    c.activate("Tocando");
    assert!(c.says("Pausada"), "{:?}", c.texts());
    c.activate("Repetir");
    assert!(c.says("para no último quadro"));
    c.activate(SPEEDS[2]);
    assert!(c.says("velocidade dobro"), "{:?}", c.texts());
    c.activate("Tocando");
    assert!(c.says("Tocando · velocidade dobro · para no último quadro"), "{:?}", c.texts());
}
