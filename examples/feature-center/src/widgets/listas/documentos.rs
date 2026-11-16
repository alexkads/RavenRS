//! `DocumentTabs`, `PagedDocumentView` and `Filmstrip`, every feature: open documents as tabs
//! that close, move, are renamed, carry a color and a dot for what was not saved and have a menu
//! of their own; a document of a hundred and twenty pages that builds only what shows, goes to a
//! page and zooms; and two thousand photos in a strip that selects one or many.

use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "DocumentTabsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Documentos e miniaturas";

/// How many pages the document has.
pub const PAGES_IN_DOCUMENT: usize = 120;
/// How many photos the strip has.
pub const PHOTOS: usize = 2_000;

/// An open document: its id, its title, and whether it has changes not saved.
#[derive(Clone, Debug, PartialEq)]
struct Open {
    id: u64,
    title: String,
    unsaved: bool,
}

fn first_tabs() -> Vec<Open> {
    [("Contrato de aluguel", false), ("Proposta comercial", true), ("Ata da reunião", false)]
        .into_iter()
        .zip(1_u64..)
        .map(|((title, unsaved), id)| Open { id, title: title.to_string(), unsaved })
        .collect()
}

/// The colors that tell the tabs apart, by id.
const COLORS: [u32; 4] = [0x3B82F6, 0x10B981, 0xF59E0B, 0xEF4444];

/// The page: tabs over the document they open, and a strip of photos.
#[derive(Debug, Default)]
pub struct DocumentTabsPage;

impl View for DocumentTabsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Abra, feche, arraste ou renomeie uma aba.".to_string());
        let tabs = cx.use_state(first_tabs);
        let active = cx.use_state(|| 1_u64);
        let next = cx.use_state(|| 4_u64);
        let page = cx.use_state(|| 0_usize);
        let zoom = cx.use_state(ZoomView::fit);
        let photos = cx.use_state(|| BTreeSet::from([0_usize]));
        let many = cx.use_state(|| 1_usize);
        // The pictures of the pages, made once: the odd pages have none, and show their placeholder.
        let pictures = cx
            .use_state(|| {
                Rc::new(
                    (0..PAGES_IN_DOCUMENT)
                        .map(|i| {
                            (i % 2 == 0)
                                .then(|| {
                                    let tint = (i % 10) as f32 / 10.0;
                                    ImageData::from_fn(60, 85, move |x, y| {
                                        if y < 12 || !(6..=54).contains(&x) {
                                            Color::rgb(1.0, 1.0, 1.0)
                                        } else if y % 8 < 2 {
                                            Color::rgb(0.55, 0.6 + tint * 0.2, 0.7)
                                        } else {
                                            Color::rgb(0.97, 0.97, 0.98)
                                        }
                                    })
                                })
                                .flatten()
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .get();

        let open = tabs.get();
        let strip_tabs: Vec<DocumentTab> = open
            .iter()
            .map(|tab| {
                DocumentTab::new(tab.id, tab.title.clone())
                    .color(Color::hex(COLORS[(tab.id as usize - 1) % COLORS.len()]))
                    .unsaved(tab.unsaved)
                    .icon(IconName::FileText)
                    // The minutes are always there: they cannot be closed.
                    .closable(tab.title != "Ata da reunião")
            })
            .collect();
        let (closing, close_line, picked) = (tabs.clone(), said.clone(), active.clone());
        let (moving, move_line) = (tabs.clone(), said.clone());
        let (renaming, rename_line) = (tabs.clone(), said.clone());
        let (adding, add_line, counter, focus) = (tabs.clone(), said.clone(), next.clone(), active.clone());
        let (saving, save_line) = (tabs.clone(), said.clone());
        let document_tabs = DocumentTabs::new(strip_tabs, active.binding())
            .label("Documentos abertos")
            .fade(Semantic::Canvas)
            .on_close(move |id| {
                let title =
                    closing.peek().iter().find(|t| t.id == id).map(|t| t.title.clone()).unwrap_or_default();
                closing.update(|all| all.retain(|t| t.id != id));
                if *picked.peek() == id
                    && let Some(first) = closing.peek().first()
                {
                    picked.set(first.id);
                }
                close_line.set(format!("Fechou {title}"));
            })
            .on_reorder(move |from, to| {
                moving.update(|all| {
                    let tab = all.remove(from);
                    all.insert(to, tab);
                });
                move_line.set(format!("Aba da posição {} foi para a {}", from + 1, to + 1));
            })
            .on_rename(move |id, name| {
                renaming.update(|all| {
                    if let Some(tab) = all.iter_mut().find(|t| t.id == id) {
                        tab.title = name.clone();
                    }
                });
                rename_line.set(format!("Renomeou para {name}"));
            })
            .on_new(move || {
                let id = counter.get();
                counter.set(id + 1);
                adding.update(|all| all.push(Open { id, title: format!("Documento {id}"), unsaved: true }));
                focus.set(id);
                add_line.set(format!("Abriu Documento {id}"));
            })
            .menu(move |id| {
                let (keep, line) = (saving.clone(), save_line.clone());
                vec![MenuItem::new("Salvar").icon(IconName::Save).on_select(move || {
                    keep.update(|all| {
                        if let Some(tab) = all.iter_mut().find(|t| t.id == id) {
                            tab.unsaved = false;
                        }
                    });
                    line.set(format!("Salvou o documento {id}"));
                })]
            });
        let current =
            open.iter().find(|t| t.id == active.get()).map_or("nenhum", |t| t.title.as_str()).to_string();
        let unsaved = open.iter().filter(|t| t.unsaved).count();

        let shown = pictures.clone();
        let document = PagedDocumentView::new(
            PAGES_IN_DOCUMENT,
            // Every tenth page lies down, as a table would.
            |i| if i % 10 == 9 { Size::new(842.0, 595.0) } else { Size::new(595.0, 842.0) },
            move |i| shown.get(i).cloned().flatten(),
        )
        .label("Documento paginado")
        .page(page.binding())
        .zoom(zoom.binding())
        .height(if narrow { 360.0 } else { 460.0 });
        let (to_page, to_full, to_fit) = (page.clone(), zoom.clone(), zoom.clone());
        let scale = zoom.get();

        let (activate_line, chosen) = (said.clone(), photos.clone());
        let film = Filmstrip::new(PHOTOS, |cell: FilmstripCell| {
            VStack::new((
                Text::new(format!("{}", cell.index + 1)).style(TextStyle::Headline),
                Text::new(if cell.selected { "★ escolhida" } else { "foto" }).style(TextStyle::Caption),
            ))
            .spacing(2.0)
            .align(Align::Center)
        })
        .label("Fotos do ensaio")
        .key(|index| index as u64 * 7 + 1)
        .selection(photos.binding())
        .mode(if many.get() == 1 { SelectionMode::Multiple } else { SelectionMode::Single })
        .cell_name(|index| format!("Foto {}", index + 1))
        .cell_width(if narrow { 72.0 } else { 96.0 })
        .height(if narrow { 80.0 } else { 104.0 })
        .spacing(8.0)
        .fade(Semantic::Canvas)
        .on_activate(move |index| activate_line.set(format!("Abriu a foto {}", index + 1)));
        let modes = ["Uma", "Várias"];
        let mode_picker: AnyView = if narrow {
            Box::new(Select::new(many.binding(), modes).label("Seleção de fotos"))
        } else {
            Box::new(SegmentedControl::new(many.binding(), modes))
        };
        let (to_last, clear) = (chosen.clone(), chosen.clone());

        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                VStack::new((
                    Text::new("Abas de documentos").style(TextStyle::Headline),
                    document_tabs,
                    Text::new(format!("Aberto: {current} · {} abas, {unsaved} sem salvar", open.len())),
                    Text::new("Documento paginado").style(TextStyle::Headline),
                    HStack::new((
                        Button::new("Ir para a página 60").on_click(move || to_page.set(59)),
                        Button::new("Zoom 100%")
                            .style(ButtonStyle::Secondary)
                            .on_click(move || to_full.set(ZoomView::at(1.0))),
                        Button::new("Ajustar à largura")
                            .style(ButtonStyle::Secondary)
                            .on_click(move || to_fit.set(ZoomView::fit())),
                    ))
                    .spacing(8.0)
                    .wrap(),
                    document,
                    Text::new(format!(
                        "Página {} de {PAGES_IN_DOCUMENT} · zoom {:.0}%{}",
                        page.get() + 1,
                        scale.scale * 100.0,
                        if scale.fit { " (ajustado)" } else { "" }
                    )),
                ))
                .spacing(12.0)
                .align(Align::Stretch),
                VStack::new((
                    Text::new("Tira de miniaturas").style(TextStyle::Headline),
                    mode_picker,
                    film,
                    HStack::new((
                        Button::new("Escolher a última foto")
                            .on_click(move || to_last.set(BTreeSet::from([PHOTOS - 1]))),
                        Button::new("Limpar a escolha")
                            .style(ButtonStyle::Secondary)
                            .on_click(move || clear.set(BTreeSet::new())),
                    ))
                    .spacing(8.0)
                    .wrap(),
                    Text::new(format!("{} foto(s) escolhida(s)", photos.get().len())),
                ))
                .spacing(12.0)
                .align(Align::Stretch),
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
