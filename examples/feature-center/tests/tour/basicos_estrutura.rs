//! The steps of the pages in src/widgets/basicos/estrutura/.

use super::*;
use feature_center::widgets::basicos::estrutura::{divisoes, janela, rolagem, texto};

impl Center {
    /// Moves the focus to the node with this name and role, then presses `key` there.
    fn key_on(&mut self, name: &str, role: Role, key: ravenrs_core::Key) {
        self.settle();
        let tree = self.ui.access_tree();
        let found = tree.nodes.iter().find(|n| n.role == role && n.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no {role:?} {name}: {:?}", self.texts())).id;
        self.ui.perform_action(id, AccessAction::Focus);
        self.key(key);
    }
}

#[test]
fn the_text_follows_its_state_the_images_fade_and_bad_bytes_are_refused() {
    let mut c = Center::memory("ana");
    c.press("Texto e ícones");
    assert!(c.says("Título de exibição"), "the type scale: {:?}", c.texts());
    assert!(c.says("Ninguém curtiu ainda"), "dynamic: {:?}", c.texts());
    c.press("Curtir");
    c.press("Curtir");
    assert!(c.says("Curtidas: 2"), "bound: {:?}", c.texts());
    assert!(c.says("2 pessoas curtiram"), "dynamic reads the same state: {:?}", c.texts());
    assert!(c.says("Evento: Curtida somada"));
    c.press("Zerar");
    assert!(c.says("Curtidas: 0") && c.says("Curtidas zeradas"), "{:?}", c.texts());
    assert!(c.has_name("Gradiente em Cobrir") && c.has_name("Gradiente em Caber"), "alt names the images");
    assert!(c.says("PNG lido dos bytes"), "from_bytes decodes a PNG: {:?}", c.texts());
    assert!(c.says("recusados com erro"), "from_bytes refuses what is not an image: {:?}", c.texts());
    assert!(c.has_name(&format!("Foto: {}", texto::PICTURES[0])));
    c.press("Trocar a foto");
    assert!(c.has_name(&format!("Foto: {}", texto::PICTURES[1])), "the crossfade changes the picture");
    assert!(c.says("Foto trocada com dissolução"));
    assert!(c.says("16:9") && c.says("1:1"), "the proportions: {:?}", c.texts());
}

#[test]
fn the_cards_are_picked_the_header_acts_and_the_footer_links_answer() {
    let mut c = Center::memory("ana");
    c.press("Cartões e seções");
    assert!(c.says("Sombra alta") && c.says("Vidro justo"), "the surfaces: {:?}", c.texts());
    c.press("Empresa R$ 199 por mês");
    assert!(c.says("Plano escolhido: Empresa"), "a card with on_click: {:?}", c.texts());
    assert!(c.says("Empresa"), "the row of the section follows the chosen card");
    c.press("Novo pedido");
    assert!(c.says("Novo pedido aberto"), "the action of the header: {:?}", c.texts());
    assert!(c.says("12 novos") && c.says("3 em falta"), "the badges of both headers");
    c.press("Editar");
    assert!(c.says("Editar a conta"), "the action of the section: {:?}", c.texts());
    assert!(c.says("ana@exemplo.com.br") && c.says("Em dia"), "label and value rows");
    c.press("Privacidade");
    assert!(c.says("Link do rodapé: Privacidade"), "{:?}", c.texts());
    c.press("Termos");
    assert!(c.says("Link do rodapé: Termos"));
    assert!(c.says("© 2026 Raven") && c.says("Painel estreito"), "both footers");
}

#[test]
fn the_grid_changes_its_columns_and_the_canvas_draws_what_the_state_holds() {
    let mut c = Center::memory("ana");
    c.press("Grade e camadas");
    assert!(c.says("Item 8"), "the cells: {:?}", c.texts());
    c.press("4 colunas");
    assert!(c.says("Grade com 4 colunas"), "{:?}", c.texts());
    assert!(c.says("Início") && c.says("Centro") && c.says("Fim"), "layers aligned each way");
    assert!(c.has_name("Exportar") && c.has_name("Imprimir"), "the spacer keeps the buttons in the row");
    assert!(c.says("Barras no desenho: 3"), "{:?}", c.texts());
    c.press("Mais uma barra");
    assert!(c.says("Barras no desenho: 4") && c.says("Barra desenhada"), "{:?}", c.texts());
    c.press("Limpar");
    assert!(c.says("Barras no desenho: 0") && c.says("Desenho limpo"), "{:?}", c.texts());
}

#[test]
fn the_panes_hide_the_bar_moves_by_key_and_saves_and_the_details_open() {
    let mut c = Center::memory("ana");
    c.press("Divisões");
    assert!(c.says("Larguras: 160 · o resto · 160"), "the sizes kept in a state: {:?}", c.texts());
    assert!(c.says("Explorador") && c.says("Terminal") && c.says("Resultado"), "every split");
    c.press("Esconder a atividade");
    assert!(c.says("Atividade escondida"), "{:?}", c.texts());
    assert!(!c.says("Pode sumir"), "the hidden pane goes: {:?}", c.texts());
    c.press("Mostrar a atividade");
    assert!(c.says("Pode sumir"), "and comes back");
    assert!(c.says("Console com 96 de altura"), "{:?}", c.texts());
    c.key_on(divisoes::HANDLE, Role::Slider, ravenrs_core::Key::End);
    assert!(c.says("Console com 200 de altura"), "the range of the bar: {:?}", c.texts());
    assert!(c.says("Altura do console salva"), "on_end after a key: {:?}", c.texts());
    c.key_on(divisoes::HANDLE, Role::Slider, ravenrs_core::Key::Home);
    assert!(c.says("Console com 48 de altura"), "{:?}", c.texts());
    assert!(!c.says("frete grátis acima"), "closed at first");
    c.press("Mostrar os detalhes");
    assert!(c.says("frete grátis acima de R$ 200"), "Collapse::new opens: {:?}", c.texts());
    c.press("Notas da versão");
    assert!(c.says("primeira página das divisões"), "Collapse::boxed opens: {:?}", c.texts());
}

#[test]
fn the_strip_picks_a_category_and_the_carousel_turns() {
    let mut c = Center::memory("ana");
    c.press("Rolagem");
    assert!(c.says("Linha 30") && c.says("Coluna 16"), "both scroll views: {:?}", c.texts());
    assert!(c.says("Cada cartão aparece") && c.says("2. Layout"), "linked and the scene: {:?}", c.texts());
    c.press(rolagem::CATEGORIES[3]);
    assert!(c.says("Categoria: Hortifrúti"), "{:?}", c.texts());
    assert!(c.says("Mensagem 12") && c.says("Sala 1"), "the vertical strip and the sticky column");
    assert!(c.says(rolagem::SLIDES[0]), "{:?}", c.texts());
    c.press("Próximo slide");
    assert!(c.says(rolagem::SLIDES[1]), "the arrow turns the slide: {:?}", c.texts());
    c.press("Slide 3 de 3");
    assert!(c.says(rolagem::SLIDES[2]), "a dot goes straight there: {:?}", c.texts());
    c.press("Slide anterior");
    assert!(c.says(rolagem::SLIDES[1]));
}

#[test]
fn the_title_bar_maximizes_the_menus_run_and_the_status_bar_saves() {
    let mut c = Center::memory("ana");
    c.press("Janela e barras");
    c.press("Maximizar");
    assert!(c.says("Janela maximizada"), "{:?}", c.texts());
    c.press("Restaurar");
    assert!(c.says("Janela restaurada"), "maximized turns the button into restore: {:?}", c.texts());
    c.press("Minimizar");
    assert!(c.says("Janela minimizada"));
    c.press("Compartilhar");
    assert!(c.says("Evento: Compartilhar"), "the action of the bar");
    assert!(c.says("Outra janela, sem o teclado"), "the quiet bar");
    c.activate("Arquivo");
    c.activate("Salvar");
    assert!(c.says("Menu: Salvar"), "an item of the menubar: {:?}", c.texts());
    assert!(c.says("Ferramenta: Selecionar"), "{:?}", c.texts());
    c.activate(janela::TOOLS[4]);
    assert!(c.says("Ferramenta: Régua"), "the rail picks a tool: {:?}", c.texts());
    assert!(c.says("Tudo salvo"), "{:?}", c.texts());
    c.press("Marcar como alterado");
    assert!(c.says("Há mudanças não salvas") && c.says("Arquivo alterado"), "{:?}", c.texts());
    c.press("Salvar agora");
    assert!(c.says("Tudo salvo") && c.says("Arquivo salvo"));
}
