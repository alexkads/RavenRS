//! What each part of the app is for, in the panels of its "?" buttons.

use ravenrs::prelude::*;

/// An explanation: what it is about, what it is, and how to use it.
pub(crate) type Help = (&'static str, &'static str, &'static str);

/// The "?" for an explanation.
pub(crate) fn tip(help: Help) -> HelpTip {
    let (title, text, how) = help;
    let tip = HelpTip::new(title, text);
    if how.is_empty() { tip } else { tip.how(how) }
}

pub(crate) const PAINEL: Help = (
    "Painel",
    "O resumo das finanças da família: quanto há nas contas, o que entrou, o que saiu e o que está para vencer.",
    "Comece pelas próximas contas. Dá para pagar direto daqui.",
);
pub(crate) const SALDO: Help = (
    "Saldo das contas",
    "A soma do dinheiro em todas as contas, com o que já foi lançado até hoje. O gráfico mostra os últimos 12 meses.",
    "",
);
pub(crate) const PROXIMAS: Help = (
    "Próximas contas",
    "O que vence nos próximos 30 dias, a começar pelo que está atrasado.",
    "Pagar dá baixa na conta e cria o lançamento da despesa.",
);
pub(crate) const RECEITAS: Help =
    ("Receitas do mês", "Tudo o que entrou neste mês, comparado com o mesmo período do mês passado.", "");
pub(crate) const DESPESAS: Help =
    ("Despesas do mês", "Tudo o que saiu neste mês, comparado com o mesmo período do mês passado.", "");
pub(crate) const A_PAGAR: Help = (
    "A pagar em 30 dias",
    "O que vai sair nos próximos 30 dias: as contas em aberto e os pagamentos programados na Agenda.",
    "",
);
pub(crate) const POR_CATEGORIA: Help = (
    "Despesas por categoria",
    "Para onde foi o dinheiro neste mês, da categoria que mais gastou para a que menos gastou.",
    "",
);
pub(crate) const POR_CONTA: Help = (
    "Saldo por conta",
    "Quanto há em cada conta, carteira ou poupança. Elas são cadastradas em Cadastros.",
    "",
);
pub(crate) const LANCAMENTOS: Help = (
    "Lançamentos",
    "Tudo o que entrou e saiu no mês, um por linha.",
    "Clique duas vezes em uma linha para alterar. Em Colunas, dá para mostrar o que estiver escondido.",
);
pub(crate) const FILTROS: Help = (
    "Filtrar",
    "Mostra só o que interessa: use um filtro pronto ou escreva a sua regra em Filtro avançado.",
    "Exemplo de regra: [Valor] > 100 And [Categoria] = 'Mercado'.",
);
pub(crate) const CONTAS: Help = (
    "Contas a pagar",
    "O que a família tem a pagar, e até quando. As recorrentes voltam sozinhas depois de pagas.",
    "Selecione uma conta e clique em Pagar, ou clique duas vezes nela.",
);
pub(crate) const AGENDA: Help = (
    "Agenda",
    "O que está previsto, por pessoa e por dia: contas e recebimentos ou pagamentos programados.",
    "Arraste para remarcar e arraste em um espaço vazio para programar. O botão direito mostra o que dá para fazer, e ⌘Z desfaz.",
);
pub(crate) const ORCAMENTOS: Help = (
    "Orçamentos",
    "Quanto a família quer gastar por categoria em cada mês, e quanto já gastou.",
    "O lápis define o limite. A barra muda de cor perto do limite e quando passa dele.",
);
pub(crate) const RELATORIOS: Help = (
    "Relatórios",
    "Os documentos prontos: extrato do mês, contas a pagar, orçamento e resumo de 12 meses.",
    "Escolha um relatório e, em Exportar, salve em PDF, Excel ou CSV.",
);
pub(crate) const CADASTROS: Help = (
    "Cadastros",
    "As contas, categorias e pessoas que os lançamentos usam.",
    "Só dá para excluir o que nada estiver usando.",
);
pub(crate) const AJUSTES: Help =
    ("Ajustes", "A aparência do app, os backups dos seus dados, as atualizações e o assistente.", "");
pub(crate) const BACKUP: Help = (
    "Backup",
    "Um arquivo com todos os seus dados, para guardar fora deste computador.",
    "Para trazer os dados de volta, aqui ou em outro computador, use Restaurar backup.",
);
pub(crate) const APARENCIA: Help = ("Aparência", "Muda só o visual do app. Seus dados continuam iguais.", "");
pub(crate) const DESEMPENHO: Help = (
    "Desempenho",
    "Mede se este computador desenha o app com folga.",
    "O teste abre o Painel, redesenha a janela por 5 segundos e diz o resultado.",
);
pub(crate) const ASSISTENTE: Help = (
    "Assistente com IA",
    "Opcional. Com um modelo, o assistente responde sobre saldo, gastos e contas. Tudo roda neste computador.",
    "Baixe, teste e remova se ficar lento.",
);
pub(crate) const VOZ: Help = (
    "Voz do assistente",
    "Faz o assistente ler as respostas em voz alta. A fala é gerada neste computador, sem enviar nada para fora.",
    "Baixe o modelo, escolha idioma e voz, e ouça um exemplo.",
);
pub(crate) const COPIAS: Help = (
    "Cópias automáticas",
    "Uma cópia por dia e outra antes de trocar seus dados. Ficam neste computador, então não substituem um backup.",
    "Clique em uma para voltar a ela.",
);
pub(crate) const DADOS: Help = (
    "Onde seus dados ficam",
    "Seus dados ficam em um arquivo neste computador. Nada é enviado para fora.",
    "",
);
pub(crate) const ATUALIZACAO: Help = (
    "Atualização",
    "O app baixa o código e compila neste computador.",
    "A versão atual só é trocada depois de a nova provar que abre.",
);
pub(crate) const RECOMECAR: Help = (
    "Recomeçar",
    "Troca tudo o que está no app, por dados de exemplo ou por um app vazio.",
    "Antes, o app guarda uma cópia, que aparece em Cópias automáticas.",
);
pub(crate) const TREINAMENTOS: Help = (
    "Treinamentos",
    "Filmes curtos que mostram cada parte do app.",
    "Pergunte o que quer aprender. O filme explica cada ação e espera você clicar no ponto marcado.",
);
