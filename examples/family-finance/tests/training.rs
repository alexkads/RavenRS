//! Records the training films from the real app, and checks the ones in the app
//! still open. Recording runs when asked, after screens change:
//!
//! ```text
//! FF_RECORD_TRAINING=1 cargo test -p family-finance --test training
//! ```
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use family_finance::db::Database;
use family_finance::ui::App;
use family_finance::update::Updater;
use family_finance::{seed, themes};
use ravenrs::core::{AccessAction, AccessNode, AccessTree, Key};
use ravenrs::movie::{
    CardLook, HotspotKind, Movie, MovieLibrary, Narration, OverlayKind, RecordError, Recorder, Region,
};
use ravenrs::prelude::*;

const SIZE: Size = Size::new(1280.0, 800.0);
/// What the camera shows of the app around a step, and for how long around it.
const ZOOM_WIDTH: f32 = 800.0;
const ZOOM_HEIGHT: f32 = 500.0;
const ZOOM_BEFORE_MS: u64 = 1_100;
const ZOOM_AFTER_MS: u64 = 1_400;

type Script = fn(&mut Recorder) -> Result<(), RecordError>;

/// What the films say, recorded: each box lasts as long as its recording.
const NARRATION: &[u8] = include_bytes!("../assets/treinamentos/narracao.voz");
/// The line under the title of every opening.
const OPENING_LINE: &str = "Treinamento do Raven Finanças";
/// How long the opening and the farewell stay at least, in milliseconds.
const OPENING_MS: u64 = 4_200;
const FAREWELL_MS: u64 = 5_200;
/// The breath after the voice of a card, before what comes next.
const AFTER_CARD_MS: u64 = 900;

/// How each film says goodbye, by its file: the title and the line under it.
const FAREWELLS: [(&str, &str, &str); 6] = [
    ("visao-geral", "A casa é sua!", "Agora cada real tem endereço. Que venham meses leves."),
    ("lancamentos", "Anotou, controlou!", "Cada lançamento é um passo, e o seu dinheiro agradece a caminhada."),
    ("contas", "Surpresa, só as boas!", "Com os vencimentos em dia, a cabeça fica livre para o resto do mês."),
    ("agenda", "O calendário joga no seu time", "Com tudo marcado, sobra tempo para o que importa de verdade."),
    ("planejamento", "O futuro já tem um plano", "Sonho com meta vira conquista. A próxima é sua!"),
    ("cadastros", "Tudo no lugar, tudo seguro", "Casa arrumada e backup feito: pode seguir tranquilo. Até a próxima!"),
];

/// The films: the name shown in the app, the file, and how each one goes.
const FILMS: [(&str, &str, Script); 6] = [
    ("Visão geral", "visao-geral", overview),
    ("Lançamentos", "lancamentos", ledger),
    ("Contas a pagar", "contas", bills),
    ("Agenda", "agenda", agenda),
    ("Orçamentos e relatórios", "planejamento", planning),
    ("Cadastros e backup", "cadastros", records),
];

fn overview(take: &mut Recorder) -> Result<(), RecordError> {
    take.intro(
        "Conhecendo o Painel",
        "O Painel é o resumo das suas finanças. Ele junta o saldo, o que entrou e saiu no mês e as contas que estão chegando.",
    );
    take.explain(
        "SALDO DAS CONTAS",
        "Saldo das contas",
        "É a soma do dinheiro em todas as suas contas, com o que já foi lançado até hoje. O gráfico abaixo mostra se a sua reserva está crescendo ou diminuindo nos últimos 12 meses.",
    )?;
    take.explain(
        "RECEITAS DO MÊS",
        "Receitas e despesas do mês",
        "Mostram o que entrou e o que saiu neste mês, comparados com o mesmo período do mês anterior.",
    )?;
    take.explain(
        "A PAGAR EM 30 DIAS",
        "A pagar em 30 dias",
        "É o que vai sair nos próximos 30 dias, somando as contas em aberto e os pagamentos programados na Agenda.",
    )?;

    take.intro(
        "Encontrar o que precisa",
        "A barra lateral leva a cada tela: o dia a dia em cima, depois as consultas, a configuração e a ajuda.",
    );
    take.explain(
        "Contas a pagar",
        "O número ao lado da tela",
        "Em Contas a pagar, o número avisa quantas contas estão vencidas, mesmo quando você está em outra tela.",
    )?;
    take.guide(
        "Novo",
        "O menu Novo",
        "Clique em Novo para lançar algo de qualquer tela, sem precisar abrir a tela certa antes.",
    )?;
    take.click("Novo")?;
    take.wait(1.6);
    take.press(Key::Escape);
    take.wait(0.4);
    take.guide(
        "Procurar comando",
        "Procurar um comando",
        "Clique aqui, ou aperte Command K no Mac e Control K no Windows, para ir a qualquer tela digitando o nome dela.",
    )?;
    take.click("Procurar comando")?;
    take.wait(0.6);
    take.type_into("Paleta de comandos", "Orça")?;
    take.wait(1.6);
    take.press(Key::Escape);
    take.wait(0.5);

    take.intro(
        "Deixar com a sua cara",
        "O tema muda só a aparência do aplicativo. Os seus dados continuam iguais.",
    );
    take.guide("Ajustes", "Abra os Ajustes", "O tema fica nos Ajustes.")?;
    take.click("Ajustes")?;
    take.wait(0.8);
    take.guide("Escuro", "Tema escuro", "Clique em Escuro para descansar a vista à noite.")?;
    take.click("Escuro")?;
    take.wait(1.2);
    take.guide(
        "Claro",
        "Tema claro",
        "Clique em Claro para voltar. Vidro é a terceira opção, com um fundo translúcido.",
    )?;
    take.click("Claro")?;
    take.wait(1.0);
    Ok(())
}

fn ledger(take: &mut Recorder) -> Result<(), RecordError> {
    take.intro(
        "Lançar uma despesa",
        "Lançamentos guarda tudo o que entrou e saiu. Vamos registrar um gasto do começo ao fim.",
    );
    take.guide(
        "Lançamentos",
        "Abra os Lançamentos",
        "Clique em Lançamentos, onde fica tudo o que entrou e saiu.",
    )?;
    take.click("Lançamentos")?;
    take.wait(0.8);
    take.guide("Nova despesa", "Comece uma despesa", "Clique em Nova despesa.")?;
    take.click("Nova despesa")?;
    take.wait(0.8);
    take.explain(
        "Descrição",
        "Descrição",
        "Escreva algo que você vá reconhecer depois. Só a descrição e o valor são obrigatórios.",
    )?;
    take.type_into("Descrição", "Mercado da semana")?;
    take.type_into("Valor", "187,50")?;
    take.explain(
        "Conta",
        "De onde saiu o dinheiro",
        "Escolha a conta, a categoria e de quem é a despesa. Ela alimenta os orçamentos e os relatórios, então vale a pena acertar.",
    )?;
    take.guide(
        "Salvar lançamento",
        "Salve",
        "Confira e clique em Salvar lançamento. O saldo da conta muda na hora.",
    )?;
    take.click("Salvar lançamento")?;
    take.wait(1.5);

    take.intro(
        "Lançar uma receita",
        "Salário, venda ou trabalho extra entram do mesmo jeito. O formulário só troca as categorias pelas de receita.",
    );
    take.guide("Nova receita", "Comece uma receita", "Clique em Nova receita.")?;
    take.click("Nova receita")?;
    take.wait(0.8);
    take.type_into("Descrição", "Venda da bicicleta")?;
    take.type_into("Valor", "650,00")?;
    take.guide("Salvar lançamento", "Salve a receita", "Clique em Salvar lançamento.")?;
    take.click("Salvar lançamento")?;
    take.wait(1.5);

    take.intro(
        "Navegar pelos meses e ver só uma parte",
        "A tela mostra um mês por vez, e o resumo no alto diz quanto entrou, quanto saiu e quanto sobrou.",
    );
    take.guide(
        "Mês anterior",
        "Veja outro mês",
        "Use as setas para ir a outro mês. Este mês traz você de volta.",
    )?;
    take.click("Mês anterior")?;
    take.wait(1.0);
    take.click("Este mês")?;
    take.wait(0.6);
    take.guide(
        "Despesas",
        "Só as despesas",
        "Escolha Todos, Despesas ou Receitas para ver só o que interessa.",
    )?;
    take.click("Despesas")?;
    take.wait(1.2);
    take.click("Todos")?;
    take.wait(0.6);

    take.intro(
        "Filtrar os lançamentos",
        "Quando a lista é grande, os filtros encontram exatamente o que você procura. Comece pelos filtros prontos.",
    );
    take.guide("Filtrar", "Filtrar", "Clique em Filtrar e escolha um filtro. Ele é aplicado na hora.")?;
    take.click("Filtrar")?;
    take.wait(0.8);
    take.guide(
        "Acima de R$ 500",
        "Acima de quinhentos reais",
        "Este filtro mostra só os lançamentos maiores que quinhentos reais. No mesmo menu, Filtro avançado deixa escrever a sua própria regra.",
    )?;
    take.click("Acima de R$ 500")?;
    take.wait(1.2);
    take.guide("Limpar", "Tire o filtro", "Clique em Limpar para voltar a ver tudo.")?;
    take.click("Limpar")?;
    take.wait(1.0);

    take.intro(
        "Organizar a lista e levar para fora",
        "A lista funciona como uma planilha: dá para buscar, ordenar, agrupar e exportar.",
    );
    take.explain(
        "Buscar em todas as colunas",
        "Buscar",
        "Digite qualquer palavra para encontrá-la em todas as colunas, sem precisar saber onde ela está.",
    )?;
    take.type_into("Buscar em todas as colunas", "Mercado")?;
    take.wait(1.2);
    for _ in 0.."Mercado".len() {
        take.press(Key::Backspace);
    }
    take.wait(0.5);
    take.explain(
        "Agrupar",
        "Agrupar",
        "Agrupar junta as linhas por categoria, conta ou pessoa, com o total de cada grupo. Clicar no título de uma coluna ordena a lista por ela.",
    )?;
    take.explain(
        "Exportar CSV",
        "Exportar",
        "Exportar CSV salva a lista como você está vendo, para abrir em uma planilha.",
    )?;
    take.explain(
        "Excluir",
        "Corrigir ou excluir",
        "Selecione uma linha para Alterar ou Excluir. Atenção: excluir um lançamento que pagou uma conta faz essa conta voltar a ficar em aberto.",
    )?;
    Ok(())
}

fn bills(take: &mut Recorder) -> Result<(), RecordError> {
    take.intro(
        "Entender as contas",
        "Contas a pagar mostra o que você deve e quando vence. Cada conta tem uma situação: Vencida, Vence hoje, A vencer ou Paga.",
    );
    take.guide("Contas a pagar", "Abra as contas", "Clique em Contas a pagar.")?;
    take.click("Contas a pagar")?;
    take.wait(0.8);
    take.guide("Vencidas", "Veja o que atrasou", "Clique em Vencidas para ver só o que já passou da data.")?;
    take.click("Vencidas")?;
    take.wait(1.4);
    take.explain("Pagas", "Contas pagas", "Em Pagas fica o histórico, e Todas mostra a lista inteira.")?;
    take.guide(
        "Em aberto",
        "Volte às contas em aberto",
        "Clique em Em aberto para ver tudo o que ainda falta pagar.",
    )?;
    take.click("Em aberto")?;
    take.wait(1.0);

    take.intro(
        "Cadastrar uma conta nova",
        "Cadastre a conta assim que ela chegar. Ela passa a aparecer no Painel e na Agenda, e o aplicativo avisa quando atrasar.",
    );
    take.guide("Nova conta", "Comece uma conta", "Clique em Nova conta.")?;
    take.click("Nova conta")?;
    take.wait(0.8);
    take.type_into("Descrição", "Sofá da sala")?;
    take.type_into("Valor", "1.800,00")?;
    take.explain(
        "Repetição",
        "Repetição",
        "Use Todo mês ou Todo ano para contas fixas, como aluguel e internet. Quando você paga, o próximo vencimento é criado sozinho.",
    )?;
    take.explain(
        "Parcelas",
        "Parcelas",
        "Para uma compra parcelada, aumente aqui. O valor digitado passa a ser o total e o aplicativo cria uma conta para cada parcela, até sessenta.",
    )?;
    take.click("Aumentar")?;
    take.wait(0.5);
    take.click("Aumentar")?;
    take.wait(1.2);
    take.explain(
        "Código de barras do boleto",
        "Código de barras",
        "É opcional. Guarde aqui o código do boleto para ter sempre à mão na hora de pagar.",
    )?;
    take.guide("Salvar conta", "Salve", "Clique em Salvar conta.")?;
    take.click("Salvar conta")?;
    take.wait(1.5);

    take.intro(
        "Pagar uma conta",
        "Pagar dá baixa na conta e também cria o lançamento da despesa, então você não precisa lançar de novo.",
    );
    take.guide(
        "Plano de saúde",
        "Selecione a conta",
        "Clique na conta que você pagou, para selecionar a linha.",
    )?;
    take.click("Plano de saúde")?;
    take.wait(0.6);
    take.guide("Pagar", "Pague", "Clique em Pagar. Dois cliques na linha fazem o mesmo.")?;
    take.click("Pagar")?;
    take.wait(0.8);
    take.explain(
        "Valor pago",
        "Juros ou desconto",
        "O valor já vem preenchido. Mude se houve juros ou desconto, e escolha a conta de onde saiu o dinheiro e o dia do pagamento.",
    )?;
    take.guide(
        "Confirmar pagamento",
        "Confirme",
        "Confira o valor e a conta, e clique em Confirmar pagamento.",
    )?;
    take.click("Confirmar pagamento")?;
    take.wait(1.5);

    take.intro(
        "Desfazer um pagamento",
        "Errou? Desfazer devolve a conta para em aberto e apaga o lançamento que o pagamento criou.",
    );
    take.guide("Pagas", "Encontre a conta paga", "Clique em Pagas.")?;
    take.click("Pagas")?;
    take.wait(1.0);
    take.guide("Plano de saúde", "Selecione a conta", "Clique na linha da conta.")?;
    take.click("Plano de saúde")?;
    take.wait(0.6);
    take.explain(
        "Desfazer pagamento",
        "Desfazer pagamento",
        "O botão aparece quando uma conta paga está selecionada. O aplicativo pergunta antes de desfazer.",
    )?;

    take.intro(
        "Pagar pelo Painel",
        "As contas mais próximas também aparecem no Painel, para você pagar sem trocar de tela.",
    );
    take.guide("Painel", "Volte ao Painel", "Clique em Painel.")?;
    take.click("Painel")?;
    take.wait(0.8);
    take.guide("Pagar", "Pague uma conta", "Clique em Pagar na conta que você quitou.")?;
    take.click("Pagar")?;
    take.wait(1.0);
    take.guide(
        "Confirmar pagamento",
        "Confirme",
        "É o mesmo pagamento de antes: confira e clique em Confirmar pagamento.",
    )?;
    take.click("Confirmar pagamento")?;
    take.wait(1.5);
    Ok(())
}

fn agenda(take: &mut Recorder) -> Result<(), RecordError> {
    take.intro(
        "Ver o que vem pela frente",
        "A Agenda junta no calendário as contas e tudo o que você programou, para saber quanto vai entrar e sair em cada dia.",
    );
    take.guide("Agenda", "Abra a Agenda", "Clique em Agenda.")?;
    take.click("Agenda")?;
    take.wait(1.0);
    take.explain(
        "Linha do tempo",
        "Linha do tempo",
        "Aqui os dias correm na horizontal, uma linha por pessoa. Arrastar uma conta para outro dia remarca o vencimento, e arrastar em um espaço vazio cria uma programação.",
    )?;
    take.guide("Mês", "Veja o mês", "Clique em Mês para ver o calendário tradicional.")?;
    take.click("Mês")?;
    take.wait(1.2);
    take.guide(
        "16 novembro de 2026",
        "Escolha um dia",
        "Clique em um dia. Ao lado aparece tudo o que está previsto para ele.",
    )?;
    take.click("16 novembro de 2026")?;
    take.wait(1.0);

    take.intro(
        "Confirmar o que aconteceu",
        "Uma programação não mexe no saldo até você confirmar. Assim o saldo mostra só o que aconteceu de verdade.",
    );
    take.explain(
        "Pular Aula de natação",
        "Pular um dia",
        "Se neste dia não houve o pagamento, Pular tira só esse dia e mantém os próximos.",
    )?;
    take.guide("Confirmar Aula de natação", "Confirme", "Clique em Confirmar quando o pagamento acontecer.")?;
    take.click("Confirmar Aula de natação")?;
    take.wait(0.8);
    take.explain(
        "Valor",
        "Ajuste se precisar",
        "Este valor vale só para este dia. Os próximos continuam como programados.",
    )?;
    take.guide(
        "Confirmar pagamento",
        "Confirme o pagamento",
        "Clique em Confirmar pagamento. Agora ele vira um lançamento.",
    )?;
    take.click("Confirmar pagamento")?;
    take.wait(1.5);

    take.intro(
        "Programar um pagamento ou recebimento",
        "Programe o que se repete e não é conta com boleto, como mesada, aula ou aluguel que você recebe.",
    );
    take.guide("Programar", "Comece", "Clique em Programar.")?;
    take.click("Programar")?;
    take.wait(0.8);
    take.explain("Recebimento", "Pagamento ou recebimento", "Escolha se o dinheiro sai ou entra.")?;
    take.type_into("Descrição", "Mesada da Lia")?;
    take.type_into("Valor", "150,00")?;
    take.guide(
        "Semanal",
        "Escolha quando repete",
        "Escolha entre uma vez, todo dia, toda semana, todo mês ou todo ano.",
    )?;
    take.click("Semanal")?;
    take.wait(1.0);
    take.explain(
        "Nunca",
        "Até quando repete",
        "Pode repetir para sempre, até uma data ou um número de vezes. Logo abaixo, o aplicativo escreve a regra em palavras e mostra as próximas datas.",
    )?;
    take.guide("Salvar programação", "Salve", "Clique em Salvar programação.")?;
    take.click("Salvar programação")?;
    take.wait(1.5);
    Ok(())
}

fn planning(take: &mut Recorder) -> Result<(), RecordError> {
    take.intro(
        "Definir limites por categoria",
        "O orçamento é o quanto você quer gastar por mês em cada categoria. O aplicativo acompanha sozinho, a partir dos lançamentos.",
    );
    take.guide("Orçamentos", "Abra os Orçamentos", "Clique em Orçamentos.")?;
    take.click("Orçamentos")?;
    take.wait(1.2);
    take.explain(
        "Definir limite de Mercado",
        "As cores da barra",
        "Até oitenta por cento do limite a barra fica verde. Até cem por cento, amarela. Passou do limite, fica vermelha e um aviso aparece no alto.",
    )?;
    take.guide("Definir limite de Mercado", "Mude um limite", "Clique para definir o limite de Mercado.")?;
    take.click("Definir limite de Mercado")?;
    take.wait(0.8);
    take.explain("Limite mensal", "Limite mensal", "Deixe vazio para acompanhar a categoria sem limite.")?;
    take.click("Limite mensal")?;
    for _ in 0.."1500,00".len() {
        take.press(Key::Backspace);
        take.wait(0.05);
    }
    take.type_into("Limite mensal", "900,00")?;
    take.guide("Salvar limite", "Salve", "Clique em Salvar limite. Como o gasto de Mercado já passou de novecentos reais, a barra fica vermelha.")?;
    take.click("Salvar limite")?;
    take.wait(3.0);

    take.intro(
        "Ver para onde foi o dinheiro",
        "Os relatórios mostram os mesmos dados prontos para ler, imprimir ou guardar.",
    );
    take.guide("Relatórios", "Abra os Relatórios", "Clique em Relatórios.")?;
    take.click("Relatórios")?;
    take.wait(1.8);
    take.explain(
        "Extrato do mês",
        "Extrato do mês",
        "Lista tudo o que entrou e saiu no mês, separado por categoria.",
    )?;
    take.guide(
        "Orçamento",
        "Compare com o planejado",
        "O relatório Orçamento compara o que saiu com o planejado para cada categoria.",
    )?;
    take.click("Orçamento")?;
    take.wait(1.8);
    take.guide(
        "Resumo de 12 meses",
        "Veja o ano",
        "O Resumo de 12 meses mostra como cada mês terminou, para enxergar a tendência.",
    )?;
    take.click("Resumo de 12 meses")?;
    take.wait(1.8);
    take.explain(
        "Exportar",
        "Guardar ou imprimir",
        "Exportar salva o relatório em PDF, Excel ou CSV, e Imprimir manda para a impressora. Também é possível buscar palavras dentro do relatório.",
    )?;
    Ok(())
}

fn records(take: &mut Recorder) -> Result<(), RecordError> {
    take.intro(
        "Cadastrar o que o aplicativo usa",
        "Em Cadastros ficam as contas do banco, as categorias e as pessoas da família, que aparecem nos formulários.",
    );
    take.guide("Cadastros", "Abra os Cadastros", "Clique em Cadastros.")?;
    take.click("Cadastros")?;
    take.wait(1.0);
    take.explain(
        "Saldo inicial",
        "Uma conta nova",
        "Dê um nome, o tipo, que pode ser conta corrente, poupança, dinheiro ou cartão de crédito, e o saldo que ela tem hoje.",
    )?;
    take.guide("Categorias", "Categorias", "Clique em Categorias.")?;
    take.click("Categorias")?;
    take.wait(1.0);
    take.explain(
        "Limite mensal",
        "Uma categoria nova",
        "Cada categoria é de despesa ou de receita, e pode ter um limite mensal, o mesmo que aparece em Orçamentos.",
    )?;
    take.guide("Família", "Família", "Clique em Família para dizer quem são as pessoas da casa.")?;
    take.click("Família")?;
    take.wait(1.0);
    take.explain(
        "Adicionar pessoa",
        "Excluir tem regra",
        "Uma conta ou categoria em uso não pode ser excluída, para não perder a história. Ao remover uma pessoa, os lançamentos dela ficam sem responsável.",
    )?;

    take.intro(
        "Guardar seus dados",
        "Os seus dados ficam só neste computador. Se ele quebrar, somente um backup guardado em outro lugar traz tudo de volta.",
    );
    take.guide("Ajustes", "Abra os Ajustes", "Clique em Ajustes.")?;
    take.click("Ajustes")?;
    take.wait(1.0);
    take.move_to("Fazer backup")?;
    take.explain(
        "Fazer backup",
        "Fazer backup",
        "Salva um arquivo com todos os seus dados. Guarde em um pen drive ou na nuvem, e repita de vez em quando.",
    )?;
    take.explain(
        "Restaurar backup",
        "Restaurar backup",
        "Traz os dados de volta, aqui mesmo ou ao trocar de computador. Antes, o aplicativo mostra o que tem no arquivo e pede confirmação.",
    )?;
    take.intro(
        "Cópias automáticas e recomeçar",
        "O aplicativo também guarda uma cópia por dia e outra antes de substituir os dados. Ela ajuda em um engano, mas não substitui o backup.",
    );
    take.scroll(500.0);
    take.explain(
        "Começar do zero",
        "Começar do zero",
        "Apaga os dados de exemplo para você começar a usar de verdade. O aplicativo pede confirmação antes de apagar.",
    )?;
    Ok(())
}

/// How long a card stays: at least `least`, and long enough for its recording to be
/// said from when the player starts saying it.
fn card_ms(look: CardLook, title: &str, line: &str, least: u64) -> u64 {
    let said = format!("{title}. {line}");
    let voice = Narration::new(NARRATION).seconds(&said).map_or(0, |seconds| (seconds * 1000.0) as u64);
    least.max(look.speaks_from_ms() + voice + AFTER_CARD_MS)
}

/// The farewell of the film in `file`.
fn farewell_of(file: &str) -> (&'static str, &'static str) {
    FAREWELLS
        .iter()
        .find(|(name, _, _)| *name == file)
        .map(|(_, title, line)| (*title, *line))
        .expect("every film says goodbye")
}

/// The finishing touches, the same ones the editor offers, and the opening and the farewell.
fn edit(film: Movie, title: &str, file: &str) -> Movie {
    let mut project = film.project().clone();
    let total = project.duration_ms();
    // The camera goes to where each step happens, so that it can be read. A step close
    // after another stays on what is already in view.
    let mut zoomed_until = 0;
    for step in film.hotspots().iter().filter(|spot| spot.kind == HotspotKind::Stop) {
        let start = step.from_ms.saturating_sub(ZOOM_BEFORE_MS);
        if start < zoomed_until || step.is_general() {
            continue;
        }
        let (x, y) = (step.x + step.width / 2.0, step.y + step.height / 2.0);
        let region = Region::new(x - ZOOM_WIDTH / 2.0, y - ZOOM_HEIGHT / 2.0, ZOOM_WIDTH, ZOOM_HEIGHT);
        project.add_overlay(OverlayKind::Zoom { region }, start, ZOOM_BEFORE_MS + ZOOM_AFTER_MS);
        zoomed_until = step.from_ms + ZOOM_AFTER_MS;
    }
    project.add_overlay(OverlayKind::PointerHighlight, 0, total);
    let (goodbye, line) = farewell_of(file);
    let farewell = card_ms(CardLook::Closing, goodbye, line, FAREWELL_MS);
    project.insert_card(total, goodbye, line, farewell, CardLook::Closing);
    let opening = card_ms(CardLook::Opening, title, OPENING_LINE, OPENING_MS);
    project.insert_card(0, title, OPENING_LINE, opening, CardLook::Opening);
    let end = project.duration_ms();
    project.add_overlay(OverlayKind::FadeOut, end.saturating_sub(800), 800);
    project.add_overlay(OverlayKind::FadeIn, 0, 600);
    film.with_project(project)
}

fn record(title: &str, file: &str, script: Script) -> Movie {
    let today = Date::new(2026, 11, 15).expect("valid date");
    let db = Database::in_memory().expect("database");
    seed::load_demo(&db, today).expect("seed");
    let app = App::new(db, today, None, Updater::idle());
    let mut take = Recorder::new(app, themes::light(), SIZE).narration(Narration::new(NARRATION));
    script(&mut take).unwrap_or_else(|e| panic!("the script of {title} no longer fits the app: {e}"));
    edit(take.finish().expect("film"), title, file)
}

fn path_of(file: &str) -> String {
    format!("{}/assets/treinamentos/{file}.ravenmovie", env!("CARGO_MANIFEST_DIR"))
}

fn films_in_the_app() -> MovieLibrary {
    FILMS.iter().fold(MovieLibrary::new(), |library, (title, file, _)| {
        let bytes = std::fs::read(path_of(file)).expect("the film is there");
        library.film(*title, Movie::from_bytes(&bytes).expect("the film opens"))
    })
}

/// Each box stays up for as long as its voice speaks, so the next one never cuts it,
/// and each film opens with its title, ends with its farewell and outlasts its voice.
#[test]
fn the_voice_and_the_boxes_go_together_from_the_opening_to_the_farewell() {
    let narration = Narration::new(NARRATION);
    let library = films_in_the_app();
    for (index, (title, file, _)) in FILMS.iter().enumerate() {
        let (_, film) = library.get(index).expect("film");
        for spot in film.hotspots() {
            let said = format!("{}. {}", spot.title, spot.text);
            if let Some(seconds) = narration.seconds(&said) {
                let voice = (seconds * 1000.0) as u64;
                assert!(spot.until_ms - spot.from_ms >= voice, "{file}: the box goes before its voice: {said}");
            }
        }
        let clips = &film.project().clips;
        let card = |clip: Option<&ravenrs::movie::Clip>| match clip.map(|clip| &clip.kind) {
            Some(ravenrs::movie::ClipKind::Title { title, subtitle, look, .. }) => {
                Some((*look, title.clone(), subtitle.clone()))
            }
            _ => None,
        };
        let (look, opening, _) = card(clips.first()).expect("an opening");
        assert_eq!((look, opening.as_str()), (CardLook::Opening, *title), "{file}");
        let (look, goodbye, line) = card(clips.last()).expect("a farewell");
        assert_eq!((look, (goodbye.as_str(), line.as_str())), (CardLook::Closing, farewell_of(file)), "{file}");
        let last = film.hotspots().iter().map(|spot| spot.until_ms).max().unwrap_or(0);
        assert!(film.duration_ms() >= last + FAREWELL_MS, "{file}: it ends before its last box");
    }
}

#[test]
fn the_films_in_the_app_open_and_teach_each_part() {
    if std::env::var_os("FF_RECORD_TRAINING").is_some() {
        for (title, file, script) in FILMS {
            std::fs::write(path_of(file), record(title, file, script).to_bytes()).expect("written");
        }
    }
    let library = films_in_the_app();
    assert_eq!(library.len(), FILMS.len());
    let mut seconds = 0;
    let mut bytes = 0;
    for (index, (_, file, _)) in FILMS.iter().enumerate() {
        let (_, film) = library.get(index).expect("film");
        assert!(film.chapters().len() >= 2, "{file}");
        assert!(film.hotspots().len() >= 5, "{file}");
        seconds += film.duration_ms() / 1000;
        bytes += std::fs::metadata(path_of(file)).expect("file").len();
    }
    assert!(seconds > 8 * 60, "it teaches for {seconds} seconds");
    assert!(bytes < 4_000_000, "vectors should stay small: {bytes} bytes");
}

#[test]
fn questions_in_plain_words_find_the_right_moment_of_the_film() {
    let library = films_in_the_app();
    let best = |question: &str| library.ask(question, 4).first().map(|a| a.moment.title.clone());
    let cases = [
        ("como lançar uma despesa", "Lançar uma despesa"),
        ("lancar gasto", "Lançar uma despesa"),
        ("receber salário", "Lançar uma receita"),
        ("como pago uma conta?", "Pagar uma conta"),
        ("quitei", "Pague uma conta"),
        ("desfazer pagamento", "Desfazer um pagamento"),
        ("compra parcelada", "Parcelas"),
        ("juros", "Juros ou desconto"),
        ("filtro avançado", "Acima de quinhentos reais"),
        ("exportar para planilha", "Exportar"),
        ("programar mesada", "Programar um pagamento ou recebimento"),
        ("limite de gastos", "Definir limites por categoria"),
        ("trocar de computador", "Restaurar backup"),
        ("fazer backup", "Fazer backup"),
        ("restaurar meus dados", "Restaurar backup"),
        ("ver os relatórios", "Abra os Relatórios"),
        ("tema escuro", "Tema escuro"),
    ];
    let wrong: Vec<String> = cases
        .iter()
        .filter(|(question, moment)| best(question).as_deref() != Some(*moment))
        .map(|(question, moment)| format!("{question}: {:?}, not {moment}", best(question)))
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert_eq!(best("empréstimo consignado"), None);
    assert_eq!(best("investir em ações"), None);
}

#[test]
fn the_scripts_still_fit_the_app() {
    let broken: Vec<String> = FILMS
        .iter()
        .filter_map(|(title, _, script)| {
            let today = Date::new(2026, 11, 15).expect("valid date");
            let db = Database::in_memory().expect("database");
            seed::load_demo(&db, today).expect("seed");
            let app = App::new(db, today, None, Updater::idle());
            let mut take = Recorder::new(app, themes::light(), SIZE);
            script(&mut take).err().map(|e| format!("{title}: {e}"))
        })
        .collect();
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

fn named<'a>(tree: &'a AccessTree, name: &str) -> Option<&'a AccessNode> {
    tree.nodes.iter().find(|n| n.name.as_deref().is_some_and(|t| t.starts_with(name)))
}

#[test]
fn the_assistant_opens_the_film_from_any_screen() {
    let today = Date::new(2026, 11, 15).expect("valid date");
    let db = Database::in_memory().expect("database");
    seed::load_demo(&db, today).expect("seed");
    let mut ui = Ui::new(App::new(db, today, None, Updater::idle()), themes::light());
    ui.set_viewport(SIZE, 1.0);
    ui.update();

    let click = |ui: &mut Ui, name: &str| {
        let id = named(&ui.access_tree(), name).unwrap_or_else(|| panic!("no {name}")).id;
        ui.perform_action(id, AccessAction::Click);
        ui.update();
    };
    click(&mut ui, "Assistente");
    click(&mut ui, "Como pagar uma conta?");
    assert!(named(&ui.access_tree(), "Pagar dá baixa").is_some(), "the answer, in words");
    click(&mut ui, "Pagar uma conta");
    let tree = ui.access_tree();
    let clock = tree.nodes.iter().filter_map(|n| n.name.as_deref()).find(|name| name.contains(" / "));
    let names: Vec<_> = tree.nodes.iter().filter_map(|n| n.name.clone()).collect();
    let clock = clock.unwrap_or_else(|| panic!("no film: {names:?}"));
    assert!(!clock.starts_with("0:00"), "the film opened at that moment, not at the start: {clock}");
    let assistant = tree.nodes.iter().filter(|n| n.name.as_deref() == Some("Assistente")).count();
    assert_eq!(assistant, 1, "and the panel closed, with only its sphere left");
}

/// Runs only where the real model is, as it is more than a gigabyte:
/// `FF_AI_MODELS=folder cargo test --release -p family-finance --test training real -- --ignored --nocapture`.
#[test]
#[ignore = "needs the real model"]
fn real_model_choices() {
    use family_finance::ui::{ASSISTANT_INTRO, ASSISTANT_TOOLS};
    use ravenrs_ai::{LocalEngine, ModelFolder, model_by_id, read_tool_call, tool_rules};
    const CASES: &[(&str, &str)] = &[
        ("qual é o meu saldo?", "saldo_das_contas"),
        ("quanto dinheiro eu tenho nas contas?", "saldo_das_contas"),
        ("quanto gastei este mês?", "gastos_por_categoria"),
        ("quanto gastei com mercado?", "gastos_por_categoria"),
        ("onde estou gastando mais?", "gastos_por_categoria"),
        ("tenho alguma conta vencida?", "contas_a_pagar"),
        ("o que tenho para pagar essa semana?", "contas_a_pagar"),
        ("quais contas vencem em breve?", "contas_a_pagar"),
        ("abra os relatórios", "abrir_tela"),
        ("me leve para a agenda", "abrir_tela"),
        ("quero ver os orçamentos", "abrir_tela"),
        ("ir para os ajustes", "abrir_tela"),
        ("gastei 50 reais na padaria", "lancar_despesa"),
        ("lança uma despesa de 120 de farmácia", "lancar_despesa"),
        ("paguei 80 de gasolina hoje", "lancar_despesa"),
        ("criar uma despesa de 30 de lanche", "lancar_despesa"),
        ("anota uma despesa de 200 no cartão", "lancar_despesa"),
        ("agendar pagamento de luz para o dia 3", "criar_conta"),
        ("criar um pagamento para o dia 3 no valor de 100", "criar_conta"),
        ("tenho que pagar o aluguel dia 10", "criar_conta"),
        ("nova conta de internet de 120 para o dia 15", "criar_conta"),
        ("agendar pagamento de luz para o dia 3. mas eu quero criar", "criar_conta"),
        ("criar conta de luz de 330 para o dia 3, da Ana, todo mês", "criar_conta"),
        ("nova conta do carro de 12000 em 10x", "criar_conta"),
        ("gastei 200 no mercado ontem com o cartão", "lancar_despesa"),
        ("marca a conta de luz como paga", "pagar_conta"),
        ("paguei o streaming", "pagar_conta"),
        ("como faço um backup?", "ajuda"),
        ("como funciona o orçamento?", "ajuda"),
        ("como restauro meus dados em outro computador?", "ajuda"),
        ("qual a capital da França?", "nenhuma"),
        ("me conta uma piada", "nenhuma"),
    ];
    let root = std::env::var("FF_AI_MODELS").expect("FF_AI_MODELS");
    // `FF_AI_MODEL` names which one, by its id.
    let wanted = std::env::var("FF_AI_MODEL").unwrap_or_else(|_| "qwen3-1.7b-q4".to_owned());
    let model = model_by_id(&wanted).expect("a known model");
    let mut engine = LocalEngine::load(&ModelFolder::new(root), &model).expect("the model");
    let rules = format!(
        "{}Categorias de gasto: Assinaturas, Contas da casa, Educação, Lazer, Mercado, Moradia, Saúde, Transporte.",
        tool_rules(ASSISTANT_INTRO, ASSISTANT_TOOLS)
    );
    let mut right = 0;
    for (question, wanted) in CASES {
        let mut answer = String::new();
        engine
            .generate(&rules, question, 50, &mut |piece| {
                answer.push_str(piece);
                true
            })
            .expect("an answer");
        let chosen = read_tool_call(&answer, ASSISTANT_TOOLS).map_or("?".to_owned(), |call| call.name);
        right += usize::from(chosen == *wanted);
        println!(
            "{} {question:?} -> {}",
            if chosen == *wanted { "OK " } else { "ERR" },
            answer.trim().replace('\n', " ")
        );
    }
    println!("{right}/{} right", CASES.len());
    assert!(right * 10 >= CASES.len() * 6, "too many wrong: {right}");
}

/// The whole way, with the real model: the same command as above.
#[test]
#[ignore = "needs the real model"]
fn real_model_answers_in_the_chat_with_the_numbers_of_the_database() {
    let models = std::env::var("FF_AI_MODELS").expect("FF_AI_MODELS");
    let folder = std::env::temp_dir().join(format!("ff-assistant-{}", std::process::id()));
    std::fs::create_dir_all(folder.join("modelos")).expect("folder");
    let wanted = std::env::var("FF_AI_MODEL").unwrap_or_else(|_| "qwen3-1.7b-q4".to_owned());
    let model = ravenrs_ai::model_by_id(&wanted).expect("a known model");
    std::os::unix::fs::symlink(format!("{models}/{}", model.id), folder.join("modelos").join(model.id))
        .expect("link");
    std::fs::write(folder.join("modelos/in-use"), model.id).expect("in use");
    let today = Date::new(2026, 11, 15).expect("valid date");
    let path = folder.join("financas.db");
    let db = Database::open(&path).expect("database");
    seed::load_demo(&db, today).expect("seed");
    let total: i64 = db.accounts().expect("accounts").iter().map(|a| a.balance_cents).sum();
    let mut ui = Ui::new(App::new(db, today, Some(path), Updater::idle()), themes::light());
    ui.set_viewport(SIZE, 1.0);
    ui.update();
    let click = |ui: &mut Ui, name: &str| {
        let id = named(&ui.access_tree(), name).unwrap_or_else(|| panic!("no {name}")).id;
        ui.perform_action(id, AccessAction::Click);
        ui.update();
    };
    click(&mut ui, "Assistente");
    click(&mut ui, "Qual é o meu saldo?");
    let started = std::time::Instant::now();
    let wanted = format!("Você tem {} nas contas", family_finance::money::format_brl(total));
    while named(&ui.access_tree(), &wanted).is_none() {
        assert!(started.elapsed().as_secs() < 60, "no answer");
        std::thread::sleep(std::time::Duration::from_millis(50));
        ui.update();
    }
    let _ = std::fs::remove_dir_all(folder);
}

/// Records what the films say with Kokoro's `pm_santa`, into wave files for `pack_narration`
/// when `FF_NARRATION_WAVS` says where: `FF_AI_MODELS=folder cargo test --release -p family-finance --test training record_narration -- --ignored --nocapture`.
/// `FF_NARRATION_SPEAKER` picks another of its voices, and `FF_NARRATION_MODEL` another model.
#[test]
#[ignore = "needs the real voice model"]
fn record_narration() {
    use ravenrs::movie::Narration;
    use ravenrs_ai::{ModelFolder, SPEECH_RATE, SpeechEngine, SpeechSettings, model_by_id};
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/treinamentos");
    let mut said: Vec<String> = Vec::new();
    for file in std::fs::read_dir(&folder).expect("films") {
        let path = file.expect("film").path();
        if path.extension().is_some_and(|extension| extension == "ravenmovie") {
            let movie =
                ravenrs::movie::Movie::from_bytes(&std::fs::read(path).expect("film")).expect("a film");
            said.extend(movie.hotspots().iter().map(|spot| format!("{}. {}", spot.title, spot.text)));
            // The opening and the farewell are said too.
            said.extend(movie.project().clips.iter().filter_map(|clip| match &clip.kind {
                ravenrs::movie::ClipKind::Title { title, subtitle, look, .. } if *look != CardLook::Plain => {
                    Some(format!("{title}. {subtitle}"))
                }
                _ => None,
            }));
        }
    }
    said.sort();
    said.dedup();
    let limit = std::env::var("FF_NARRATION_LIMIT").ok().and_then(|limit| limit.parse().ok());
    let mut settings = if std::env::var("FF_NARRATION_STEADIER").is_ok() {
        SpeechSettings::steadier()
    } else {
        SpeechSettings::default()
    };
    let out = std::env::var("FF_NARRATION_OUT").map_or_else(|_| folder.join("narracao.voz"), Into::into);
    let root = std::env::var("FF_AI_MODELS").expect("FF_AI_MODELS");
    let wanted = std::env::var("FF_NARRATION_MODEL").unwrap_or_else(|_| "kokoro-82m".to_owned());
    let model = model_by_id(&wanted).expect("known");
    if model.family == ravenrs_ai::ModelFamily::Kokoro {
        // The training films speak with Santa, which suits them.
        settings.speaker =
            Some(std::env::var("FF_NARRATION_SPEAKER").unwrap_or_else(|_| "pm_santa".to_owned()));
    }
    let mut engine = SpeechEngine::load(&ModelFolder::new(root), &model).expect("the model");
    let silence = vec![0.0_f32; (SPEECH_RATE / 4) as usize];
    let mut recordings: Vec<(String, Vec<f32>)> = Vec::new();
    let started = std::time::Instant::now();
    let total: usize = said.iter().map(String::len).sum();
    println!("{} texts, {total} letters", said.len());
    // More takes of some texts, for choosing the best by ear or by a recognizer.
    if let Ok(only) = std::env::var("FF_NARRATION_TAKES") {
        let wavs = std::env::var("FF_NARRATION_WAVS").expect("FF_NARRATION_WAVS");
        for index in only.split(',').filter_map(|index| index.parse::<usize>().ok()) {
            let text = family_finance::ui::spoken(&said[index]);
            for take in 1..=6_u64 {
                let settings = SpeechSettings { seed: take * 7919, ..settings.clone() };
                let parts = if take % 2 == 0 { vec![text.clone()] } else { engine.pieces(&text) };
                let mut sound = Vec::new();
                for part in &parts {
                    sound.extend(engine.speak(part, &settings, &mut |_| true).expect("sound"));
                    sound.extend_from_slice(&silence);
                }
                std::fs::write(format!("{wavs}/{index:03}-{take}.wav"), wav_of(&sound)).expect("wav");
            }
        }
        return;
    }
    // Only what no recording in the folder has yet, after a film changed.
    if std::env::var("FF_NARRATION_MISSING").is_ok() {
        let wavs = std::env::var("FF_NARRATION_WAVS").expect("FF_NARRATION_WAVS");
        let mut have = std::collections::HashSet::new();
        for file in std::fs::read_dir(&wavs).expect("folder") {
            let path = file.expect("file").path();
            if path.extension().is_some_and(|extension| extension == "txt") {
                let text = std::fs::read_to_string(&path).expect("text");
                if !said.contains(&text) {
                    // It is no longer in any film.
                    std::fs::remove_file(path.with_extension("wav")).expect("old");
                    std::fs::remove_file(&path).expect("old");
                }
                have.insert(text);
            }
        }
        for (index, text) in said.iter().filter(|text| !have.contains(*text)).enumerate() {
            let mut sound = Vec::new();
            for piece in engine.pieces(&family_finance::ui::spoken(text)) {
                sound.extend(engine.speak(&piece, &settings, &mut |_| true).expect("sound"));
                sound.extend_from_slice(&silence);
            }
            println!("new: {text}");
            std::fs::write(format!("{wavs}/n{index:03}.wav"), wav_of(&sound)).expect("wav");
            std::fs::write(format!("{wavs}/n{index:03}.txt"), text).expect("text");
        }
        return;
    }
    for text in said.iter().take(limit.unwrap_or(usize::MAX)) {
        let mut sound = Vec::new();
        for piece in engine.pieces(&family_finance::ui::spoken(text)) {
            sound.extend(engine.speak(&piece, &settings, &mut |_| true).expect("sound"));
            sound.extend_from_slice(&silence);
        }
        let seconds = sound.len() as f32 / SPEECH_RATE as f32;
        let allowed = 0.8 + text.chars().count() as f32 * 0.075;
        println!("{seconds:.1}s of {allowed:.1}s | {:.0}Hz | {text}", pitch(&sound));
        recordings.push((text.clone(), sound));
    }
    std::fs::write(&out, Narration::pack(&recordings, SPEECH_RATE)).expect("the file");
    if let Ok(wavs) = std::env::var("FF_NARRATION_WAVS") {
        for (index, (text, sound)) in recordings.iter().enumerate() {
            std::fs::write(format!("{wavs}/{index:03}.wav"), wav_of(sound)).expect("wav");
            std::fs::write(format!("{wavs}/{index:03}.txt"), text).expect("text");
        }
    }
    println!("took {:.0}s", started.elapsed().as_secs_f32());
}

/// The median pitch of voiced stretches, to see whether the voice stays the same.
fn pitch(sound: &[f32]) -> f32 {
    let rate = ravenrs_ai::SPEECH_RATE as usize;
    let (least, most) = (rate / 400, rate / 70);
    let mut found: Vec<f32> = sound
        .chunks(rate / 10)
        .filter(|part| part.len() > most * 2)
        .filter_map(|part| {
            let energy: f32 = part.iter().map(|sample| sample * sample).sum();
            if energy / (part.len() as f32) < 1e-4 {
                return None;
            }
            let (lag, best) = (least..most)
                .map(|lag| {
                    let score: f32 = part.iter().zip(&part[lag..]).map(|(a, b)| a * b).sum();
                    (lag, score)
                })
                .max_by(|a, b| a.1.total_cmp(&b.1))?;
            (best > energy * 0.4).then_some(rate as f32 / lag as f32)
        })
        .collect();
    found.sort_by(f32::total_cmp);
    found.get(found.len() / 2).copied().unwrap_or(0.0)
}

fn wav_of(sound: &[f32]) -> Vec<u8> {
    let rate = ravenrs_ai::SPEECH_RATE;
    let mut wav = Vec::with_capacity(44 + sound.len() * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + sound.len() as u32 * 2).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(sound.len() as u32 * 2).to_le_bytes());
    for sample in sound {
        wav.extend_from_slice(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    wav
}

/// Packs the recordings chosen in a folder: `FF_NARRATION_WAVS=folder cargo test --release -p family-finance --test training pack_narration -- --ignored`.
#[test]
#[ignore = "needs the recordings"]
fn pack_narration() {
    let wavs = std::env::var("FF_NARRATION_WAVS").expect("FF_NARRATION_WAVS");
    let mut recordings = Vec::new();
    for file in std::fs::read_dir(&wavs).expect("folder") {
        let path = file.expect("file").path();
        if path.extension().is_some_and(|extension| extension == "txt") {
            let text = std::fs::read_to_string(&path).expect("text");
            let wav = std::fs::read(path.with_extension("wav")).expect("wav");
            let mut sound: Vec<f32> = wav[44..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|bytes| f32::from(i16::from_le_bytes(*bytes)) / 32768.0)
                .collect();
            // The pause after the last sentence only makes the next box wait.
            while sound.last().is_some_and(|sample| sample.abs() < 0.002) {
                sound.pop();
            }
            recordings.push((text, sound));
        }
    }
    let bytes = ravenrs::movie::Narration::pack(&recordings, ravenrs_ai::SPEECH_RATE);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/treinamentos/narracao.voz");
    std::fs::write(out, &bytes).expect("the file");
    println!("{} recordings, {} bytes", recordings.len(), bytes.len());
}

/// A film may be recorded again by someone without the voice model: what is new is said by
/// the system's voice, until the narration is recorded again.
#[test]
fn most_of_what_the_films_say_is_recorded() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/treinamentos");
    let bytes = std::fs::read(folder.join("narracao.voz")).expect("the narration");
    let narration = ravenrs::movie::Narration::new(Box::leak(bytes.into_boxed_slice()));
    let (mut all, mut recorded) = (0, 0);
    for file in std::fs::read_dir(&folder).expect("films") {
        let path = file.expect("film").path();
        if path.extension().is_some_and(|extension| extension == "ravenmovie") {
            let movie = Movie::from_bytes(&std::fs::read(path).expect("film")).expect("a film");
            for spot in movie.hotspots() {
                all += 1;
                recorded += usize::from(narration.clip(&format!("{}. {}", spot.title, spot.text)).is_some());
            }
        }
    }
    println!("{recorded} of {all} are recorded");
    assert!(all > 0 && recorded * 10 >= all * 9, "{recorded} of {all} are recorded");
}
