//! DropdownMenu: a button, or any view, that opens a list of actions: items with an icon, a key, a
//! check mark, a view at their end or a submenu, one that cannot be chosen and says why, a
//! destructive one, separators, titled sections and a header; four sizes and styles, only an icon,
//! where it opens and the open state shared with the app.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "DropdownMenuDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Menus";

const TEMAS: [&str; 3] = ["Claro", "Escuro", "Sistema"];

fn quem() -> HStack<(Avatar, VStack<(Text, Text)>)> {
    HStack::new((
        Avatar::new("Ana Lima").shape(AvatarShape::Rounded),
        VStack::new((Text::new("Ana Lima"), Text::new("ana@exemplo.com").foreground(Semantic::InkMuted)))
            .spacing(2.0),
    ))
    .spacing(12.0)
    .align(Align::Center)
}

/// Every public feature of DropdownMenu on one page.
#[derive(Debug, Default)]
pub struct MenusPage;

impl View for MenusPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let escolha = cx.use_state(|| String::from("nada"));
        let grade = cx.use_state(|| true);
        let tema = cx.use_state(|| 2_usize);
        let aberto = cx.use_state(|| false);
        let (agora, grade_agora, tema_agora) = (escolha.get(), grade.get(), tema.get());
        let marca = |nome: &'static str| {
            let escolha = escolha.clone();
            move || escolha.set(nome.to_owned())
        };
        let alterna = grade.clone();
        let acoes = DropdownMenu::new("Ações")
            .item(MenuItem::new("Editar").icon(IconName::Pencil).shortcut("⌘E").on_select(marca("Editar")))
            .item(MenuItem::new("Compartilhar").icon(IconName::Share2).on_select(marca("Compartilhar")))
            .item(
                MenuItem::new("Exportar")
                    .item(MenuItem::new("Como PDF").on_select(marca("Como PDF")))
                    .item(MenuItem::new("Como CSV").on_select(marca("Como CSV"))),
            )
            .item(
                MenuItem::new("Mostrar grade")
                    .checked(grade_agora)
                    .on_select(move || alterna.update(|g| *g = !*g)),
            )
            .item(MenuItem::new("Etiqueta").element(Badge::new("Família").tone(Tone::Accent)))
            .item(
                MenuItem::new("Gravar vídeo").icon(IconName::Video).disabled("Precisa de uma sessão aberta"),
            )
            .separator()
            .item(MenuItem::new("Apagar").icon(IconName::Trash2).destructive().on_select(marca("Apagar")));
        let mut conta = DropdownMenu::new("Conta")
            .trigger(
                HStack::new((
                    quem(),
                    Icon::new(IconName::ChevronsUpDown).size(16.0).color(Semantic::InkMuted),
                ))
                .spacing(12.0)
                .align(Align::Center),
            )
            .placement(Placement::Right)
            .header(quem())
            .item(MenuItem::new("Configurações").icon(IconName::Settings).on_select(marca("Configurações")))
            .label("Tema");
        for (i, nome) in TEMAS.iter().enumerate() {
            let tema = tema.clone();
            conta = conta.item(MenuItem::new(*nome).checked(tema_agora == i).on_select(move || tema.set(i)));
        }
        conta = conta
            .separator()
            .item(MenuItem::new("Sair").icon(IconName::LogOut).destructive().on_select(marca("Sair")));
        let abrir = aberto.clone();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Uma lista de ações que abre de um botão ou de qualquer vista.")
                    .foreground(Semantic::InkMuted),
                VStack::new((
                    Text::new("Itens").style(TextStyle::Headline),
                    acoes,
                    Text::new(format!("Último escolhido: {agora}")),
                    Text::new(format!("Grade: {}", if grade_agora { "ligada" } else { "desligada" })),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Aberto por uma vista, com cabeçalho e seções").style(TextStyle::Headline),
                    conta,
                    Text::new(format!("Tema: {}", TEMAS[tema_agora])),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Estilos e tamanhos").style(TextStyle::Headline),
                    HStack::new((
                        DropdownMenu::new("Novo")
                            .style(ButtonStyle::Primary)
                            .icon(IconName::Plus)
                            .item(MenuItem::new("Sessão")),
                        DropdownMenu::new("Mais").style(ButtonStyle::Ghost).item(MenuItem::new("Ajuda")),
                        DropdownMenu::new("Apagar")
                            .style(ButtonStyle::Danger)
                            .item(MenuItem::new("Esta foto")),
                        DropdownMenu::new("Pequeno").size(ButtonSize::Small).item(MenuItem::new("Um")),
                        DropdownMenu::new("Mini").size(ButtonSize::Mini).item(MenuItem::new("Um")),
                        DropdownMenu::new("Grande").size(ButtonSize::Large).item(MenuItem::new("Um")),
                        DropdownMenu::new("Opções")
                            .icon(IconName::Settings)
                            .icon_only()
                            .item(MenuItem::new("Preferências")),
                    ))
                    .spacing(12.0)
                    .align(Align::Center)
                    .wrap(),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Aberto pelo app").style(TextStyle::Headline),
                    HStack::new((
                        Button::new("Abrir pelo atalho")
                            .style(ButtonStyle::Secondary)
                            .on_click(move || abrir.set(true)),
                        DropdownMenu::new("Atalhos").open(aberto.binding()).item(MenuItem::new("Copiar")),
                    ))
                    .spacing(12.0),
                ))
                .spacing(8.0)
                .align(Align::Start),
            ))
            .spacing(16.0)
            .padding(16.0)
            .max_width(960.0)
            .align(Align::Start),
        )
    }
}
