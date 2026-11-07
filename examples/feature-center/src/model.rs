//! The demonstrations, one small entity each, grouped by what they show.

use ravenrs_app::{
    AppearanceRule, AppearanceTarget, DataAccess, EntityInfo, EnumOption, FieldInfo, FieldKind, ListFilter,
    Reminder, Rule, StateMachine, Tint, ViewVariant,
};
use ravenrs_app_ui::{CALENDAR_EDITOR, CHART_EDITOR, SCHEDULER_EDITOR, TREE_EDITOR};

/// The groups of the navigation, in the order they show.
const VALIDATION: &str = "Validação";
const APPEARANCE: &str = "Aparência condicional";
const ACTIONS: &str = "Actions";
const EDITORS: &str = "Editores de campo";
const LISTS: &str = "Listas";
const RELATIONS: &str = "Relações";
const PROCESSES: &str = "Estados e lembretes";
const SECURITY: &str = "Segurança";
const AUDIT: &str = "Auditoria";

/// What tells a demonstration apart in the navigation.
fn icon_of(name: &str) -> &'static str {
    match name {
        "ValidationRules" => "circle-check",
        "ValidationSoft" => "triangle-alert",
        "ValidationOnLeave" => "text-cursor",
        "AppearanceRules" => "palette",
        "ActionDemo" => "zap",
        "EditorsDemo" => "pencil",
        "CustomFieldsDemo" => "sliders-horizontal",
        "TextsDemo" => "languages",
        "ListEditorsDemo" => "chart-column",
        "VariantsDemo" => "filter",
        "EditableList" => "table",
        "ManyRows" => "rows-3",
        "RelationOwner" => "link",
        "StateDemo" => "git-branch",
        "ReminderDemo" => "bell",
        "SecuredDocument" => "lock",
        "AuditFull" => "history",
        "AuditLight" => "list",
        "AuditCreation" => "file-text",
        "AuditFields" => "shapes",
        "AuditOwner" => "layers",
        _ => "file",
    }
}

/// An entity of the center: where it goes, and what it is there to show.
fn demo(name: &str, caption: &str, group: &str, description: &str) -> EntityInfo {
    EntityInfo::new(name)
        .caption(caption)
        .icon(icon_of(name))
        .navigation(group)
        .description(description)
        .field(FieldInfo::new("id", FieldKind::Integer).key())
}

fn text(name: &str, caption: &str) -> FieldInfo {
    FieldInfo::new(name, FieldKind::Text).caption(caption)
}

fn title() -> FieldInfo {
    text("name", "Nome").default_property()
}

fn options(pairs: &[(&str, &str)]) -> FieldKind {
    FieldKind::Enum(pairs.iter().map(|(code, caption)| EnumOption::new(*code, *caption)).collect())
}

fn rule_kinds() -> EntityInfo {
    demo(
        "ValidationRules",
        "Regras",
        VALIDATION,
        "Cada campo tem uma regra: obrigatório, único, intervalo, padrão e critério entre campos. \
         Abra um registro, apague ou troque os valores e salve: a gravação não acontece e cada campo diz o que está errado.",
    )
    .field(title())
    .field(text("code", "Código, único").hint("Ao menos um igual ao de outro registro é recusado"))
    .field(FieldInfo::new("age", FieldKind::Integer).caption("Idade, de 18 a 65"))
    .field(text("email", "E-mail"))
    .field(FieldInfo::new("start_day", FieldKind::Date).caption("Início"))
    .field(FieldInfo::new("end_day", FieldKind::Date).caption("Fim, depois do início"))
    .rule(Rule::required("name").message("Dê um nome ao registro"))
    .rule(Rule::unique("code"))
    .rule(Rule::range("age", Some(18.0), Some(65.0)))
    .rule(Rule::matches("email", "^[^@ ]+@[^@ ]+$").message("{name}: este e-mail não parece certo"))
    .rule(Rule::criteria("ordered", "IsNull([start_day]) Or IsNull([end_day]) Or [end_day] >= [start_day]").message("O fim vem antes do início"))
}

fn soft_rules() -> EntityInfo {
    demo(
        "ValidationSoft",
        "Avisos e informações",
        VALIDATION,
        "Regras que não impedem a gravação. Com desconto acima de 20 o salvar pergunta e deixa seguir. \
         Com valor abaixo de 100 ele salva e deixa uma nota no campo.",
    )
    .field(title())
    .field(FieldInfo::new("discount", FieldKind::Integer).caption("Desconto"))
    .field(FieldInfo::new("amount", FieldKind::Integer).caption("Valor"))
    .rule(Rule::range("discount", None, Some(20.0)).warning().message("Desconto alto para {name}"))
    .rule(
        Rule::range("amount", Some(100.0), None)
            .information()
            .message("Valor baixo; confira se é isso mesmo"),
    )
}

fn leave_rules() -> EntityInfo {
    demo(
        "ValidationOnLeave",
        "Ao sair do campo",
        VALIDATION,
        "Aqui o erro aparece assim que o foco sai do campo, sem esperar pelo salvar. Abra um registro, apague o nome e saia dele.",
    )
    .field(title())
    .field(text("city", "Cidade"))
    .checks_on_leave()
    .rule(Rule::required("name"))
}

fn appearance() -> EntityInfo {
    demo(
        "AppearanceRules",
        "Regras de aparência",
        APPEARANCE,
        "A situação muda como tudo aparece. Atrasado pinta o nome na lista e no formulário, bloqueado impede editar o valor e tira o Excluir, \
         e sigiloso esconde o campo de observação.",
    )
    .field(title())
    .field(FieldInfo::new("state", options(&[("ok", "Em dia"), ("late", "Atrasado"), ("locked", "Bloqueado"), ("secret", "Sigiloso")])).caption("Situação"))
    .field(FieldInfo::new("amount", FieldKind::Money).caption("Valor"))
    .field(text("notes", "Observação"))
    .appearance(AppearanceRule::new("late_name", ["name"]).criteria("[state] = 'late'").font(Tint::Danger).bold())
    .appearance(AppearanceRule::new("locked_amount", ["amount"]).criteria("[state] = 'locked'").enabled(false))
    .appearance(AppearanceRule::new("secret_notes", ["notes"]).criteria("[state] = 'secret'").visible(false))
    .appearance(
        AppearanceRule::new("locked_delete", ["Delete"])
            .target(AppearanceTarget::Action)
            .criteria("[state] = 'locked'")
            .enabled(false),
    )
}

fn actions() -> EntityInfo {
    demo(
        "ActionDemo",
        "Tipos de Action",
        ACTIONS,
        "Selecione um registro e use a barra: Action simples, com confirmação, com valor digitado, com escolha, com janela e abrindo um endereço. \
         Só para abertos só acende quando todos os selecionados estão abertos, e dentro do formulário há uma Action no próprio layout.",
    )
    .field(title())
    .field(FieldInfo::new("state", options(&[("open", "Aberto"), ("closed", "Fechado")])).caption("Situação"))
    .field(FieldInfo::new("amount", FieldKind::Integer).caption("Valor"))
    .field(FieldInfo::new("priority", options(&[("low", "Baixa"), ("high", "Alta")])).caption("Prioridade"))
    .form_actions("Tools")
}

fn all_editors() -> EntityInfo {
    demo(
        "EditorsDemo",
        "Todos os tipos",
        EDITORS,
        "Um campo de cada tipo, cada um com o seu controle: texto, texto longo, número, decimal, dinheiro, data, data e hora, booleano, opções, \
         várias opções, cor, imagem, arquivo, critério, máscara, valores prontos, barra de progresso e formato próprio, com grupos com título.",
    )
    .field(title())
    .field(FieldInfo::new("long_text", FieldKind::LongText).caption("Texto longo"))
    .field(FieldInfo::new("whole", FieldKind::Integer).caption("Inteiro"))
    .field(FieldInfo::new("decimal", FieldKind::Decimal(3)).caption("Decimal"))
    .field(FieldInfo::new("money", FieldKind::Money).caption("Dinheiro"))
    .field(FieldInfo::new("day", FieldKind::Date).caption("Data"))
    .field(FieldInfo::new("moment", FieldKind::DateTime).caption("Data e hora"))
    .field(FieldInfo::new("active", FieldKind::Boolean).caption("Ativo").default_value(true))
    .field(FieldInfo::new("kind", options(&[("a", "Primeira"), ("b", "Segunda"), ("c", "Terceira")])).caption("Opções"))
    .field(FieldInfo::new("side", options(&[("l", "Esquerda"), ("r", "Direita")])).caption("Lado a lado").editor("Segmented"))
    .field(FieldInfo::new("tags", FieldKind::Tags(vec![EnumOption::new("x", "Urgente"), EnumOption::new("y", "Revisar"), EnumOption::new("z", "Cliente")])).caption("Várias opções"))
    .field(FieldInfo::new("color", FieldKind::Color).caption("Cor"))
    .field(FieldInfo::new("picture", FieldKind::Image).caption("Imagem"))
    .field(FieldInfo::new("document", FieldKind::File).caption("Arquivo"))
    .field(FieldInfo::new("rule", FieldKind::Criteria).caption("Critério").criteria_of("EditorsDemo"))
    .field(text("phone", "Máscara").edit_mask("(00) 00000-0000").group("Formatos"))
    .field(text("size", "Valores prontos").predefined(["Pequeno", "Médio", "Grande"]).group("Formatos"))
    .field(FieldInfo::new("progress", FieldKind::Integer).caption("Progresso").editor("Progress").display_format("{} %").group("Formatos"))
    .field(FieldInfo::new("rate", FieldKind::Decimal(2)).caption("Formato próprio").display_format("{:.2} %").group("Formatos"))
    .field(FieldInfo::new("twice", FieldKind::Integer).caption("Calculado, o dobro").calculated("[whole] * 2").group("Formatos"))
    .field(FieldInfo::new("code", FieldKind::Guid).caption("GUID").read_only())
}

fn plan() -> EntityInfo {
    demo(
        "ListEditorsDemo",
        "Uma lista, cinco visões",
        LISTS,
        "A mesma lista de cinco formas: use o menu Visão. Na linha do tempo, arraste uma barra para outro responsável ou outro dia e confira na lista. \
         Nada muda no código entre uma e outra, só o modelo.",
    )
    .field(title())
    .field(FieldInfo::new("owner", FieldKind::Reference("Person".into())).caption("Responsável"))
    .field(FieldInfo::new("parent", FieldKind::Reference("ListEditorsDemo".into())).caption("Dentro de"))
    .field(FieldInfo::new("starts", FieldKind::DateTime).caption("Início"))
    .field(FieldInfo::new("ends", FieldKind::DateTime).caption("Fim"))
    .field(FieldInfo::new("cost", FieldKind::Money).caption("Custo"))
    .field(FieldInfo::new("state", options(&[("open", "Aberta"), ("late", "Atrasada")])).caption("Situação"))
    .editor_setting("ChartSettings", "Argument", "owner")
    .editor_setting("ChartSettings", "Series", "cost:Sum")
    .editor_setting("CalendarSettings", "Tone", "state")
    .editor_setting("SchedulerSettings", "Tone", "state")
    .variant(ViewVariant::new("grid", "Lista"))
    .variant(ViewVariant::new("chart", "Gráfico").editor(CHART_EDITOR))
    .variant(ViewVariant::new("month", "Mês").editor(CALENDAR_EDITOR))
    .variant(ViewVariant::new("timeline", "Linha do tempo").editor(SCHEDULER_EDITOR))
    .variant(ViewVariant::new("tree", "Árvore").editor(TREE_EDITOR))
}

fn variants() -> EntityInfo {
    demo(
        "VariantsDemo",
        "Variantes e filtros",
        LISTS,
        "O menu Visão muda colunas e registros, e o menu Filtro aplica filtros prontos, um deles com uma função da própria aplicação. \
         A escolha fica guardada na sua camada do modelo.",
    )
    .field(title())
    .field(text("city", "Cidade"))
    .field(FieldInfo::new("amount", FieldKind::Integer).caption("Valor"))
    .field(FieldInfo::new("active", FieldKind::Boolean).caption("Ativo").default_value(true))
    .variant(ViewVariant::new("all", "Todos"))
    .variant(ViewVariant::new("short", "Só nome e cidade").columns(["name", "city"]))
    .variant(ViewVariant::new("active", "Só ativos").filter("[active] = True"))
    .filter(ListFilter::new("all", "Sem filtro"))
    .filter(ListFilter::new("big", "Valor alto").criteria("[amount] >= 100"))
    .filter(ListFilter::new("even", "Valor par, por função própria").criteria("IsEven([amount])"))
}

fn editable() -> EntityInfo {
    demo(
        "EditableList",
        "Edição na lista",
        LISTS,
        "Dê dois cliques numa célula e edite ali mesmo. As regras valem: um valor negativo é recusado, a célula fica marcada e diz o motivo.",
    )
    .field(title())
    .field(FieldInfo::new("amount", FieldKind::Integer).caption("Valor"))
    .field(FieldInfo::new("state", options(&[("a", "Aberto"), ("b", "Fechado")])).caption("Situação"))
    .list_editable()
    .rule(Rule::range("amount", Some(0.0), None).message("O valor não pode ser negativo"))
}

fn many_rows() -> EntityInfo {
    demo(
        "ManyRows",
        "Muitas linhas",
        LISTS,
        "Mil e quinhentas linhas lidas aos poucos, conforme a rolagem, em vez de todas de uma vez. Role a lista e use a busca.",
    )
    .field(title())
    .field(FieldInfo::new("position", FieldKind::Integer).caption("Posição"))
    .data_access(DataAccess::Server)
}

fn owner_with_lists() -> EntityInfo {
    demo(
        "RelationOwner",
        "Coleções e vínculos",
        RELATIONS,
        "Abra um pedido: Itens pertencem a ele, são criados ali e vão junto se o pedido for clonado. Etiquetas existem por conta própria e são \
         ligadas com Vincular e desligadas com Desvincular. O campo Categoria pega só as categorias do tipo escolhido.",
    )
    .field(title())
    .field(FieldInfo::new("kind", options(&[("x", "Tipo X"), ("y", "Tipo Y")])).caption("Tipo"))
    .field(
        FieldInfo::new("category", FieldKind::Reference("RelationCategory".into()))
            .caption("Categoria")
            .lookup_filter("[kind] = ?kind")
            .none_caption("Sem categoria"),
    )
    .field(FieldInfo::new("items", FieldKind::Collection("RelationItem".into())).caption("Itens").back_reference("owner_id").aggregated())
    .field(FieldInfo::new("labels", FieldKind::Collection("RelationLabel".into())).caption("Etiquetas").back_reference("owner_id"))
    .cloneable()
}

fn relation_parts() -> Vec<EntityInfo> {
    let plain = |name: &str, caption: &str| {
        EntityInfo::new(name)
            .caption(caption)
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(title())
    };
    vec![
        plain("RelationItem", "Itens")
            .field(FieldInfo::new("owner_id", FieldKind::Reference("RelationOwner".into())).caption("Pedido"))
            .field(FieldInfo::new("quantity", FieldKind::Integer).caption("Quantidade")),
        plain("RelationLabel", "Etiquetas").field(
            FieldInfo::new("owner_id", FieldKind::Reference("RelationOwner".into())).caption("Pedido"),
        ),
        plain("RelationCategory", "Categorias")
            .field(FieldInfo::new("kind", options(&[("x", "Tipo X"), ("y", "Tipo Y")])).caption("Tipo")),
    ]
}

fn simulation() -> EntityInfo {
    EntityInfo::new("Simulation")
        .caption("Simulação de parcelas")
        .transient()
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(text("name", "Nome da simulação").default_property())
        .field(FieldInfo::new("amount", FieldKind::Integer).caption("Valor"))
        .field(FieldInfo::new("months", FieldKind::Integer).caption("Meses"))
        .field(
            FieldInfo::new("installment", FieldKind::Decimal(2))
                .caption("Parcela")
                .calculated("[amount] / Max([months], 1)"),
        )
}

fn states() -> EntityInfo {
    demo(
        "StateDemo",
        "Máquina de estados",
        PROCESSES,
        "Rascunho vai para Em análise, que volta ou vai para Aprovado, e Aprovado não sai mais. Só acende a Action que vale para o estado do \
         selecionado, e aprovar pede custo abaixo de mil. Qualquer outra mudança é recusada ao gravar, mesmo sem tela.",
    )
    .field(title())
    .field(FieldInfo::new("state", options(&[("draft", "Rascunho"), ("review", "Em análise"), ("done", "Aprovado")])).caption("Estado"))
    .field(FieldInfo::new("cost", FieldKind::Integer).caption("Custo").default_value(0.0))
    .state_machine(
        StateMachine::new("state")
            .state("draft", "Rascunho")
            .state("review", "Em análise")
            .state("done", "Aprovado")
            .start("draft")
            .transition("draft", "review", "Enviar para análise")
            .transition("review", "draft", "Devolver")
            .transition("review", "done", "Aprovar")
            .when("[cost] < 1000"),
    )
    .state_appearance("state", "done", AppearanceRule::new("done_locked", ["cost"]).enabled(false))
}

fn reminders() -> EntityInfo {
    demo(
        "ReminderDemo",
        "Lembretes",
        PROCESSES,
        "Um lembrete vence meio minuto depois de abrir, e a janela aparece sozinha, sem nada rodando enquanto espera. Abra, adie 15 minutos ou dispense. \
         Com a janela fechada, ele fica no rodapé da navegação.",
    )
    .field(title())
    .field(FieldInfo::new("remind_at", FieldKind::DateTime).caption("Lembrar em"))
    .reminder(Reminder::new("remind_at"))
}

fn secured() -> EntityInfo {
    demo(
        "SecuredDocument",
        "Documentos por papel",
        SECURITY,
        "Entre como ana, bia ou caio e compare. Ana administra tudo. Bia vê todos, não vê o custo e só altera os próprios. Caio só lê, e nem vê \
         o que é de ana. Tente: Administração some para quem não é administrador, e a recusa vem do armazenamento, não da tela.",
    )
    .field(title())
    .field(text("owner", "Dona").predefined(["ana", "bia", "caio"]))
    .field(FieldInfo::new("cost", FieldKind::Money).caption("Custo"))
    .field(text("subject", "Assunto"))
}

fn audit_demo(name: &str, caption: &str, description: &str) -> EntityInfo {
    demo(name, caption, AUDIT, description)
        .field(title())
        .field(FieldInfo::new("amount", FieldKind::Integer).caption("Valor"))
        .field(text("ignored", "Fora da auditoria"))
}

/// What the trail covers, one mode to a demonstration, so they can be compared.
fn audits() -> Vec<EntityInfo> {
    vec![
        audit_demo("AuditFull", "Completa", "Tudo vai para a trilha: quem, quando, o campo, o valor anterior e o novo. Altere um registro e veja em Administração, Trilha de auditoria."),
        audit_demo("AuditLight", "Leve", "Só que o registro foi criado, alterado ou excluído, sem campos nem valores."),
        audit_demo("AuditCreation", "Só criação", "Só a criação de registros, como numa importação. Alterar ou excluir aqui não deixa rastro."),
        audit_demo("AuditFields", "Campos de fora", "Como a completa, mas o campo Fora da auditoria não entra na trilha."),
        audit_demo("AuditOwner", "Coleções", "O que muda nos itens aparece no dono: acrescentar, retirar ou alterar um item fica registrado no pedido.")
            .field(FieldInfo::new("parts", FieldKind::Collection("AuditPart".into())).caption("Itens").back_reference("owner_id")),
        EntityInfo::new("AuditPart")
            .caption("Itens")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(title())
            .field(FieldInfo::new("owner_id", FieldKind::Reference("AuditOwner".into())).caption("Dono")),
    ]
}

fn custom_fields() -> EntityInfo {
    demo(
        "CustomFieldsDemo",
        "Campos personalizados",
        EDITORS,
        "Segmento e Valor dobrado não existem no código: vêm de uma diferença do modelo, com coluna na lista, lugar no formulário e espaço no banco.",
    )
    .field(title())
    .field(FieldInfo::new("amount", FieldKind::Integer).caption("Valor"))
}

fn texts() -> EntityInfo {
    demo(
        "TextsDemo",
        "Textos por chave",
        APPEARANCE,
        "Os textos das telas e dos componentes vêm de um catálogo por chave, com o inglês por trás. \
         O centro passa um JSON que troca Atualizar por Recarregar em toda a aplicação e acrescenta center.hello, \
         que a Action Dizer olá lê, sem nenhum código de tela.",
    )
    .field(title())
}

/// What the center adds to the texts and what it replaces, as an application would give them.
pub const TEXTS: &str = r#"{
    "ui.refresh": "Recarregar",
    "center.hello": "Olá, {0}! Este texto veio do catálogo."
}"#;

/// The difference that adds fields to [`custom_fields`], as an administrator would give it.
pub const CUSTOM_FIELDS: &str = r#"{ "Entities": { "CustomFieldsDemo": { "Members": {
    "segment": { "$new": true, "Kind": "Text", "Caption": "Segmento" },
    "twice": { "$new": true, "Kind": "Integer", "Caption": "Valor dobrado", "Expression": "[amount] * 2" }
} } } }"#;

/// Every entity of the center that is not from a module.
pub fn entities() -> Vec<EntityInfo> {
    let mut all = vec![
        rule_kinds(),
        soft_rules(),
        leave_rules(),
        appearance(),
        actions(),
        all_editors(),
        custom_fields(),
        texts(),
        plan(),
        variants(),
        editable(),
        many_rows(),
        owner_with_lists(),
    ];
    all.extend(relation_parts());
    all.push(simulation());
    all.push(states());
    all.push(reminders());
    all.push(secured());
    all.extend(audits());
    all
}
