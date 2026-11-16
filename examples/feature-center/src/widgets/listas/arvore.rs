//! `TreeView` and `FileBrowser`, every feature: the menu of a system as a tree that filters,
//! checks, opens all at once, brings a branch only when it opens and has a menu on each item; a
//! thousand lines in a tree that builds only what shows; and an explorer of folders that sorts,
//! selects one or many and goes back by its breadcrumb. A line says what happened last.

use std::collections::BTreeSet;

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "TreeViewDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Árvores e arquivos";

/// The branches of the menu "Abrir tudo" opens: not the reports, whose items come when a person
/// opens them.
pub const BRANCHES: [&str; 2] = ["vendas", "estoque"];

/// The menu of a system: two branches with items, and one whose items come when it opens.
fn menu(reports: &[String]) -> Vec<TreeNode> {
    let mut later = TreeNode::new("relatorios", "Relatórios").icon(IconName::FileText).lazy();
    if !reports.is_empty() {
        later = later.children(reports.iter().map(|name| TreeNode::new(name.clone(), name.clone())));
    }
    vec![
        TreeNode::new("vendas", "Vendas").icon(IconName::ShoppingCart).children([
            TreeNode::new("pedidos", "Pedidos"),
            TreeNode::new("clientes", "Clientes"),
            TreeNode::new("comissoes", "Comissões").disabled(true),
        ]),
        TreeNode::new("estoque", "Estoque")
            .icon(IconName::Package)
            .child(TreeNode::new("produtos", "Produtos"))
            .child(TreeNode::new("depositos", "Depósitos")),
        later,
    ]
}

/// A thousand items in ten branches, for the tree that builds only what shows.
fn thousand() -> Vec<TreeNode> {
    (1..=10)
        .map(|group| {
            TreeNode::new(format!("g{group}"), format!("Grupo {group}")).children(
                (1..=100).map(move |n| TreeNode::new(format!("g{group}-{n}"), format!("Item {group}.{n}"))),
            )
        })
        .collect()
}

/// A folder of a made-up backup server, by its path from the root.
fn folder(path: &[String]) -> Vec<FileEntry> {
    match path.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [_] => vec![
            FileEntry::folder("Contratos").modified("10/11/2026", 3),
            FileEntry::folder("Fotos").modified("02/11/2026", 2),
            FileEntry::file("leia-me.txt", 2_048).modified("01/11/2026", 1),
            FileEntry::file("balanço.xlsx", 512_000).modified("12/11/2026", 4).icon(IconName::Table),
        ],
        [_, "Contratos"] => vec![
            FileEntry::file("aluguel.pdf", 120_000).modified("05/10/2026", 1),
            FileEntry::file("serviços.pdf", 88_000).modified("08/11/2026", 2),
        ],
        [_, "Fotos"] => (1..=40)
            .map(|n| {
                FileEntry::file(format!("foto-{n:03}.jpg"), 900_000 + n * 1_000)
                    .id(format!("fotos/{n}"))
                    .modified(format!("{n:02}/10/2026"), i64::try_from(n).unwrap_or(0))
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The page: the tree of the menu, the long tree and the explorer, with a line of events.
#[derive(Debug, Default)]
pub struct TreeViewPage;

impl View for TreeViewPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Escolha, marque ou abra um item.".to_string());
        let selected = cx.use_state(|| None::<String>);
        let expanded = cx.use_state(BTreeSet::<String>::new);
        let checked = cx.use_state(BTreeSet::<String>::new);
        let query = cx.use_state(String::new);
        let reports = cx.use_state(Vec::<String>::new);
        let long_selected = cx.use_state(|| None::<String>);
        let path = cx.use_state(|| vec!["Servidor".to_string()]);
        let files = cx.use_state(BTreeSet::<String>::new);
        let order = cx.use_state(FileSort::default);
        let many = cx.use_state(|| 1_usize);

        let (open_line, menu_line, loaded) = (said.clone(), said.clone(), reports.clone());
        let load_line = said.clone();
        let tree = TreeView::new(menu(&reports.get()))
            .label("Menu do sistema")
            .selected(selected.binding())
            .expanded(expanded.binding())
            .checked(checked.binding())
            .filter(query.get())
            .row_height(if narrow { 36.0 } else { 32.0 })
            .on_open(move |key| open_line.set(format!("Abriu {key}")))
            .on_expand(move |key| {
                if key == "relatorios" && loaded.peek().is_empty() {
                    loaded.set(vec!["Vendas do mês".into(), "Estoque baixo".into()]);
                    load_line.set("Relatórios carregados ao abrir".into());
                }
            })
            .context_menu(move |key| {
                let (rename, remove) = (menu_line.clone(), menu_line.clone());
                let (first, second) = (key.to_owned(), key.to_owned());
                vec![
                    MenuItem::new("Renomear")
                        .icon(IconName::Pencil)
                        .on_select(move || rename.set(format!("Renomear {first}"))),
                    MenuItem::new("Excluir")
                        .icon(IconName::Trash2)
                        .destructive()
                        .on_select(move || remove.set(format!("Excluir {second}"))),
                ]
            });
        let (open_all, close_all) = (expanded.clone(), expanded.clone());
        let tools = HStack::new((
            Button::new("Abrir tudo").on_click(move || {
                open_all.set(BRANCHES.iter().map(|b| (*b).to_string()).collect());
            }),
            Button::new("Fechar tudo")
                .style(ButtonStyle::Secondary)
                .on_click(move || close_all.set(BTreeSet::new())),
        ))
        .spacing(8.0)
        .wrap();
        let marked = checked.get();
        let tree_side = VStack::new((
            Text::new("Menu do sistema").style(TextStyle::Headline),
            FormField::new(
                "Filtrar o menu",
                TextField::new(query.binding()).placeholder("Pedidos, estoque…"),
            ),
            tools,
            tree,
            Text::new(format!("Selecionado: {}", selected.get().unwrap_or_else(|| "nenhum".into()))),
            Text::new(if marked.is_empty() {
                "Nada marcado".to_string()
            } else {
                format!("Marcados: {}", marked.iter().cloned().collect::<Vec<_>>().join(", "))
            }),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        let long_side = VStack::new((
            Text::new("Mil itens").style(TextStyle::Headline),
            Text::new("Com altura fixa, só as linhas à vista são montadas.")
                .style(TextStyle::Caption)
                .foreground(Semantic::InkMuted),
            TreeView::new(thousand())
                .label("Mil itens")
                .selected(long_selected.binding())
                .height(if narrow { 220.0 } else { 280.0 })
                .row_height(28.0),
            Text::new(format!("No longo: {}", long_selected.get().unwrap_or_else(|| "nenhum".into()))),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        let (into, back) = (path.clone(), path.clone());
        let (open_file, back_line) = (said.clone(), said.clone());
        let picked = files.clone();
        let sort = order.get();
        let browser = FileBrowser::new(path.get(), folder(&path.get()))
            .label("Arquivos do servidor")
            .selection(files.binding())
            .sort(order.binding())
            .mode(if many.get() == 1 { SelectionMode::Multiple } else { SelectionMode::Single })
            .height(if narrow { 300.0 } else { 340.0 })
            .on_open(move |entry| {
                if entry.is_folder {
                    into.update(|p| p.push(entry.name.clone()));
                    picked.set(BTreeSet::new());
                    open_file.set(format!("Entrou em {}", entry.name));
                } else {
                    open_file.set(format!("Abriu o arquivo {}", entry.name));
                }
            })
            .on_navigate(move |depth| {
                back.update(|p| p.truncate(depth + 1));
                back_line.set(format!("Voltou ao nível {depth}"));
            });
        let modes = ["Um de cada vez", "Vários"];
        let mode_picker: AnyView = if narrow {
            Box::new(Select::new(many.binding(), modes).label("Seleção"))
        } else {
            Box::new(SegmentedControl::new(many.binding(), modes))
        };
        let column = match sort.column {
            FileSortColumn::Size => "tamanho",
            FileSortColumn::Modified => "data",
            _ => "nome",
        };
        let browser_side = VStack::new((
            Text::new("Explorador de arquivos").style(TextStyle::Headline),
            mode_picker,
            browser,
            Text::new(format!(
                "Ordem: {column}, {} · {} selecionado(s)",
                if sort.ascending { "crescente" } else { "decrescente" },
                files.get().len()
            )),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        let trees: AnyView = if narrow {
            Box::new(VStack::new((tree_side, long_side)).spacing(16.0).align(Align::Stretch))
        } else {
            Box::new(
                HStack::new((
                    VStack::new((tree_side,)).width(360.0).align(Align::Stretch),
                    VStack::new((long_side,)).width(320.0).align(Align::Stretch),
                ))
                .spacing(24.0)
                .align(Align::Start),
            )
        };
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                trees,
                browser_side,
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
