//! The mode `componentes` (#587): the pages of [`crate::widgets::PAGES`] with no sign-in and no
//! application framework, for the site and for whoever only wants to see a component. A sidebar
//! by group on a wide window, a list to pick from on a narrow one, and only the page when embedded.

use ravenrs::prelude::*;

use crate::widgets::{PAGES, Page};

/// Below this width the sidebar gives way to a list at the top.
const NARROW: f32 = 720.0;

/// The pages of the components, opening on `first` when there is a page with that id.
pub struct ComponentsShell {
    current: State<usize>,
    embedded: bool,
}

impl std::fmt::Debug for ComponentsShell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentsShell")
            .field("page", &self.current.peek())
            .field("embedded", &self.embedded)
            .finish()
    }
}

impl ComponentsShell {
    /// Opens on the page with id `first`, or on the first page.
    pub fn new(first: Option<&str>, embedded: bool) -> Self {
        let at = first.and_then(|id| PAGES.iter().position(|page| page.id == id)).unwrap_or(0);
        Self { current: State::new(at), embedded }
    }
}

fn shown(page: &Page) -> AnyView {
    Box::new(ScrollView::new(((page.make)(),)).padding((24.0, 24.0)))
}

impl Component for ComponentsShell {
    fn body(&self) -> impl View {
        let (current, embedded) = (self.current.clone(), self.embedded);
        responsive(move |size: Size| -> AnyView {
            let Some(page) = PAGES.get(current.get()) else {
                return Box::new(Alert::message("Nenhuma página de componente ainda."));
            };
            if embedded {
                return shown(page);
            }
            if size.width < NARROW {
                let picker = Select::new(
                    current.binding(),
                    PAGES.iter().map(|page| format!("{} · {}", page.group, page.caption)),
                )
                .label("Componente");
                return Box::new(
                    VStack::new((Box::new(picker) as AnyView, shown(page)))
                        .spacing(12.0)
                        .padding((12.0, 16.0)),
                );
            }
            let mut items: Vec<AnyView> = Vec::new();
            let mut group = "";
            for (index, page) in PAGES.iter().enumerate() {
                if page.group != group {
                    group = page.group;
                    items.push(Box::new(SidebarSection::new(group)));
                }
                let current = current.clone();
                items.push(Box::new(
                    SidebarItem::new(page.icon, page.caption)
                        .selected(index == current.get())
                        .on_click(move || current.set(index)),
                ));
            }
            Box::new(
                HStack::new((Box::new(Sidebar::new(items)) as AnyView, shown(page)))
                    .spacing(0.0)
                    .align(Align::Stretch)
                    .grow(1.0),
            )
        })
    }
}
