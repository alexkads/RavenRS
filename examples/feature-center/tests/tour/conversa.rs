//! The steps of the family `ravenrs-widgets-chat`: the pages Conversa, Editor e playground and
//! Texto rico.

use super::*;

#[test]
fn the_chat_sends_answers_takes_files_out_plays_late_audio_and_acts_on_a_selection() {
    use feature_center::widgets::conversa::mensagens::{EXAMPLES, ORDERS, TRANSCRIPT};
    let mut c = Center::memory("ana");
    c.press("Conversa");
    assert!(c.says("A assistente da loja responde na hora"), "the intro: {:?}", c.texts());
    // An example is sent as if typed, and the assistant answers with a link.
    c.press(EXAMPLES[0]);
    assert!(c.says(&format!("Enviado: {}", EXAMPLES[0])), "{:?}", c.texts());
    assert!(c.says("Recebi:"), "the answer: {:?}", c.texts());
    assert!(!c.has_name(EXAMPLES[1]), "the examples go once there is a message");
    c.press("Ouvir");
    assert!(c.says("Ouvir: ") && c.says("Recebi:"), "on_listen gets the answer: {:?}", c.texts());
    c.activate("https://ravenrs.dev/pedidos");
    assert!(c.says("Link: https://ravenrs.dev/pedidos"), "{:?}", c.texts());
    // While an answer is on its way, nothing is sent.
    c.activate("Resposta a caminho");
    c.type_into("Pergunte sobre o pedido", "E o frete?");
    c.press("Enviar");
    assert!(!c.says("Enviado: E o frete?"), "busy holds it");
    c.activate("Resposta a caminho");
    c.press("Enviar");
    assert!(c.says("Enviado: E o frete?"), "{:?}", c.texts());
    // A file taken out, and the photo sent as a document.
    c.press("Tirar nota-fiscal.pdf");
    assert!(c.says("Tirou nota-fiscal.pdf"), "{:?}", c.texts());
    c.activate("Enviar como foto");
    c.activate("documento");
    assert!(c.says("A foto vai como: documento"), "{:?}", c.texts());
    // A recording asked for when played, and a transcript asked for and delivered.
    c.press("Tocar o áudio");
    assert!(c.says("Pediu o áudio"), "{:?}", c.texts());
    c.press("Transcrever");
    assert!(c.says("Pediu a transcrição"));
    assert!(c.says("Transcrevendo…"), "the button holds: {:?}", c.texts());
    c.press("Entregar a transcrição");
    assert!(c.says(TRANSCRIPT), "{:?}", c.texts());
    // The bar counts what is picked and leaves the selection.
    assert!(c.says("Marque pedidos para agir sobre eles"));
    c.activate(ORDERS[0]);
    c.activate(ORDERS[2]);
    assert!(c.says("2 selecionados"), "{:?}", c.texts());
    c.press("Arquivar");
    assert!(c.says("Arquivou 2 pedidos"), "{:?}", c.texts());
    c.press("Sair da seleção");
    assert!(c.says("Saiu da seleção") && c.says("Marque pedidos"), "{:?}", c.texts());
}

#[test]
fn the_code_editor_goes_to_a_line_undoes_and_the_playground_runs_its_examples() {
    use feature_center::widgets::conversa::codigo::EXAMPLES;
    let mut c = Center::memory("ana");
    c.press("Editor e playground");
    assert!(c.says("Ln 1, Col 1 · Linhas: 8"), "{:?}", c.texts());
    c.press("Ir para a linha 6");
    assert!(c.says("Ln 6, Col 4"), "the caret is the app's: {:?}", c.texts());
    c.press("Inserir um comentário");
    assert!(c.says("Linhas: 9"));
    c.press("Desfazer");
    assert!(c.says("Linhas: 8"), "the history is the app's: {:?}", c.texts());
    for option in ["Números das linhas", "Marcar problemas", "Só leitura", "Texto puro", "Recuo 2"] {
        c.activate(option);
        assert!(c.says("Linhas: 8"), "{option}");
    }
    c.press("Imprimir uma linha");
    assert!(c.says("Linha 1 impressa"), "the console: {:?}", c.texts());
    // The playground runs its first example, then the one with a problem.
    assert!(c.says("Exemplo escolhido: Saudação"));
    c.press("Executar");
    assert!(c.says("Olá, Raven"), "{:?}", c.texts());
    c.press("Contar as linhas");
    assert!(c.says("O roteiro tem 2 linhas"), "{:?}", c.texts());
    c.activate("Exemplo");
    c.activate(EXAMPLES[1].0);
    assert!(c.says("Exemplo escolhido: Com erro"), "{:?}", c.texts());
    c.press("Executar");
    assert!(c.says("Comando desconhecido"), "the problem is listed: {:?}", c.texts());
}

#[test]
fn the_rich_text_opens_its_links_and_the_panel_opens_and_reduces() {
    use feature_center::widgets::conversa::texto::PANEL;
    let mut c = Center::memory("ana");
    c.press("Texto rico");
    assert!(c.says("Pedido 102 pronto."), "{:?}", c.texts());
    assert!(c.says("Prévia: Pedido 102 pronto."), "plain_text: {:?}", c.texts());
    c.activate("https://ravenrs.dev/pedidos/102");
    assert!(c.says("Abriu https://ravenrs.dev/pedidos/102"), "{:?}", c.texts());
    assert!(c.says("*não*"), "links_only keeps the marks");
    // The panel opens from its sphere, reduces to its summary and closes from inside.
    assert!(c.says("Painel: fechado"));
    c.activate(PANEL);
    assert!(c.says("Painel: aberto"), "{:?}", c.texts());
    assert!(c.says("Posso ajudar"), "{:?}", c.texts());
    c.press("Minimizar");
    assert!(c.says("minimizado") && c.says("2 mensagens novas"), "{:?}", c.texts());
    c.press("Restaurar");
    c.press("Fechar o assistente");
    assert!(c.says("Painel: fechado"));
    // Loose, it has no sphere: the app opens it.
    c.activate("Painel solto");
    c.press("Abrir o assistente");
    assert!(c.says("Painel: aberto") && c.says("Posso ajudar"), "{:?}", c.texts());
}
