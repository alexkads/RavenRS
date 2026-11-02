//! Records the training films from the real app, and checks the ones in the app
//! still open. Recording runs when asked, after screens change:
//!
//! ```text
//! FF_RECORD_TRAINING=1 cargo test -p family-finance --test training
//! ```
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use family_finance::db::Database;
use family_finance::seed;
use family_finance::ui::App;
use family_finance::update::Updater;
use ravenrs::core::{AccessAction, AccessNode, AccessTree, Key};
use ravenrs::movie::{Movie, MovieLibrary, OverlayKind, RecordError, Recorder};
use ravenrs::prelude::*;

const SIZE: Size = Size::new(1280.0, 800.0);

type Script = fn(&mut Recorder) -> Result<(), RecordError>;

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
        "É a soma do dinheiro em todas as suas contas, com o que já foi lançado até hoje.",
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
    take.explain(
        "Saldo nos últimos 12 meses",
        "Saldo nos últimos 12 meses",
        "O gráfico mostra se a sua reserva está crescendo ou diminuindo com o tempo. O outro gráfico separa as despesas do mês por categoria.",
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
    take.guide(
        "Filtros prontos",
        "Filtros prontos",
        "Clique em Filtros prontos e escolha um. Ele é aplicado na hora.",
    )?;
    take.click("Filtros prontos")?;
    take.wait(0.8);
    take.guide(
        "Acima de R$ 500",
        "Acima de quinhentos reais",
        "Este filtro mostra só os lançamentos maiores que quinhentos reais.",
    )?;
    take.click("Acima de R$ 500")?;
    take.wait(1.2);
    take.explain(
        "Filtro avançado",
        "Filtro avançado",
        "Aqui você escreve a sua própria regra, com o nome da coluna entre colchetes, como Valor maior que cem e Categoria igual a Mercado. Se a regra tiver erro, o aplicativo avisa e mostra tudo sem filtrar.",
    )?;
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
    take.move_to("Saldo nos últimos 12 meses")?;
    take.scroll(420.0);
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

/// The finishing touches, the same ones the editor offers.
fn edit(film: Movie, title: &str) -> Movie {
    let mut project = film.project().clone();
    let total = project.duration_ms();
    project.add_overlay(OverlayKind::FadeOut, total.saturating_sub(800), 800);
    project.add_overlay(OverlayKind::PointerHighlight, 0, total);
    project.insert_title(0, title, "Treinamento do Raven Finanças", 2500);
    project.add_overlay(OverlayKind::FadeIn, 0, 600);
    film.with_project(project)
}

fn record(title: &str, script: Script) -> Movie {
    let today = Date::new(2026, 11, 15).expect("valid date");
    let db = Database::in_memory().expect("database");
    seed::load_demo(&db, today).expect("seed");
    let app = App::new(db, today, None, Updater::idle());
    let mut take = Recorder::new(app, Theme::raven_light(), SIZE);
    script(&mut take).unwrap_or_else(|e| panic!("the script of {title} no longer fits the app: {e}"));
    edit(take.finish().expect("film"), title)
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

#[test]
fn the_films_in_the_app_open_and_teach_each_part() {
    if std::env::var_os("FF_RECORD_TRAINING").is_some() {
        for (title, file, script) in FILMS {
            std::fs::write(path_of(file), record(title, script).to_bytes()).expect("written");
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
        ("filtro avançado", "Filtro avançado"),
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
            let mut take = Recorder::new(app, Theme::raven_light(), SIZE);
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
    let mut ui = Ui::new(App::new(db, today, None, Updater::idle()), Theme::raven_light());
    ui.set_viewport(SIZE, 1.0);
    ui.update();

    let click = |ui: &mut Ui, name: &str| {
        let id = named(&ui.access_tree(), name).unwrap_or_else(|| panic!("no {name}")).id;
        ui.perform_action(id, AccessAction::Click);
        ui.update();
    };
    click(&mut ui, "Assistente");
    click(&mut ui, "Como pagar uma conta?");
    assert!(named(&ui.access_tree(), "Estes momentos").is_some());
    click(&mut ui, "Pagar uma conta");
    let tree = ui.access_tree();
    let clock = tree.nodes.iter().filter_map(|n| n.name.as_deref()).find(|name| name.contains(" / "));
    let names: Vec<_> = tree.nodes.iter().filter_map(|n| n.name.clone()).collect();
    let clock = clock.unwrap_or_else(|| panic!("no film: {names:?}"));
    assert!(!clock.starts_with("0:00"), "the film opened at that moment, not at the start: {clock}");
    assert!(named(&tree, "Estes momentos").is_none(), "and the panel closed");
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
    let mut ui = Ui::new(App::new(db, today, Some(path), Updater::idle()), Theme::raven_light());
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
