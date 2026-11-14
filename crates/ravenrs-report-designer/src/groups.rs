//! The Group and sort panel: every level of grouping of the report and its sorting, in one place.
//! It edits the same model as the properties of a group band, so undo and redo cover it.

use crate::prelude::*;
use ravenrs_report::{Band, Group, Report, SortField};
use ravenrs_style::SPACING;

use crate::editor::{Editor, Selection};

/// The height of a band a new group gets for its header.
const NEW_HEADER: f32 = 24.0;

/// The panel: the groups, outermost first, then the sorting of the rows inside them.
pub(crate) struct GroupPanel(pub(crate) Editor);

impl View for GroupPanel {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.0;
        let report = e.report.get();
        let mut rows: Vec<AnyView> = vec![Box::new(Text::new("Grupos").font_weight(Weight::SEMIBOLD))];
        if report.groups.is_empty() {
            rows.push(Box::new(
                Text::new("Nenhum grupo: as linhas saem na ordem abaixo.")
                    .style(TextStyle::Caption)
                    .foreground(Semantic::InkMuted),
            ));
        }
        for (i, group) in report.groups.iter().enumerate() {
            rows.push(Box::new(group_row(e, i, group, report.groups.len())));
        }
        let add = e.clone();
        rows.push(Box::new(
            Button::new("Adicionar grupo")
                .icon(IconName::Plus)
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .on_click(move || add_group(&add)),
        ));
        rows.push(Box::new(Divider::horizontal()));
        rows.push(Box::new(Text::new("Ordenação").font_weight(Weight::SEMIBOLD)));
        for (i, sort) in report.sort.iter().enumerate() {
            rows.push(Box::new(sort_row(e, i, sort, report.sort.len())));
        }
        let add = e.clone();
        rows.push(Box::new(
            Button::new("Adicionar ordenação")
                .icon(IconName::Plus)
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .on_click(move || {
                    add.edit(|r| r.sort.push(SortField { expression: first_field(r), descending: false }));
                }),
        ));
        cx.child(&VStack::new(rows).spacing(SPACING.s2).align(Align::Stretch))
    }
}

/// The first field of the data, as `[Field]`, for a new level to start from.
fn first_field(report: &Report) -> String {
    let data = report.embedded_data().cloned().unwrap_or_default();
    ravenrs_report::wizard::fields(&data, None)
        .first()
        .map_or_else(String::new, |field| format!("[{}]", field.path))
}

fn add_group(e: &Editor) {
    e.edit(|r| {
        let expression = first_field(r);
        r.groups.push(Group::new(&expression).header(Band::new(NEW_HEADER)));
    });
    // Bands are counted by place, and a new level moves them.
    e.selection.set(Selection::None);
}

/// Moves item `i` of a list one place, `-1` up or `1` down, if it can.
fn shift<T>(items: &mut [T], i: usize, by: isize) {
    let to = i as isize + by;
    if to >= 0 && (to as usize) < items.len() {
        items.swap(i, to as usize);
    }
}

/// Up, down and remove, for one level of a list `len` long.
fn level_buttons(
    i: usize,
    len: usize,
    what: &'static str,
    on_shift: impl Fn(isize) + Clone + 'static,
    on_remove: impl Fn() + 'static,
) -> impl View {
    let (up, down) = (on_shift.clone(), on_shift);
    HStack::new((
        IconButton::new(IconName::ArrowUp)
            .label(format!("Subir {what} {}", i + 1))
            .size(ButtonSize::Small)
            .disabled(i == 0)
            .on_click(move || up(-1)),
        IconButton::new(IconName::ArrowDown)
            .label(format!("Descer {what} {}", i + 1))
            .size(ButtonSize::Small)
            .disabled(i + 1 >= len)
            .on_click(move || down(1)),
        IconButton::new(IconName::Trash2)
            .label(format!("Remover {what} {}", i + 1))
            .size(ButtonSize::Small)
            .on_click(on_remove),
    ))
    .spacing(SPACING.s1)
}

/// One level of grouping: what it groups by, the order, its header and footer, and moving it.
fn group_row(e: &Editor, i: usize, group: &Group, len: usize) -> impl View + use<> {
    let (read, write) = (e.clone(), e.clone());
    let expression = TextField::new(Binding::new(
        move || read.report.with(|r| r.groups.get(i).map(|g| g.expression.clone()).unwrap_or_default()),
        move |s: String| {
            write.edit_as("group", |r| {
                if let Some(g) = r.groups.get_mut(i) {
                    g.expression = s;
                }
            });
        },
    ))
    .compact();
    let toggle = |label: &str, get: fn(&Group) -> bool, set: fn(&mut Group, bool)| {
        let (read, write) = (e.clone(), e.clone());
        Toggle::new(Binding::new(
            move || read.report.with(|r| r.groups.get(i).is_some_and(get)),
            move |on: bool| {
                write.edit(|r| {
                    if let Some(g) = r.groups.get_mut(i) {
                        set(g, on);
                    }
                });
            },
        ))
        .label(format!("{label} do grupo {}", i + 1))
    };
    let descending = toggle("Decrescente", |g| g.descending, |g, on| g.descending = on);
    let header = toggle(
        "Cabeçalho",
        |g| g.header.is_some(),
        |g, on| g.header = on.then(|| g.header.take().unwrap_or_else(|| Band::new(NEW_HEADER))),
    );
    let footer = toggle(
        "Rodapé",
        |g| g.footer.is_some(),
        |g, on| g.footer = on.then(|| g.footer.take().unwrap_or_else(|| Band::new(NEW_HEADER))),
    );
    let (moved, removed) = (e.clone(), e.clone());
    let buttons = level_buttons(
        i,
        len,
        "o grupo",
        move |by| {
            moved.edit(|r| shift(&mut r.groups, i, by));
            moved.selection.set(Selection::None);
        },
        move || {
            removed.edit(|r| {
                if i < r.groups.len() {
                    r.groups.remove(i);
                }
            });
            removed.selection.set(Selection::None);
        },
    );
    let title =
        format!("{}. {}", i + 1, if group.expression.is_empty() { "(sem campo)" } else { &group.expression });
    Card::new(
        VStack::new((
            HStack::new((Text::new(title).font_weight(Weight::MEDIUM).grow(1.0), buttons))
                .align(Align::Center),
            FormField::new("Agrupar por", expression),
            descending,
            header,
            footer,
        ))
        .spacing(SPACING.s2)
        .align(Align::Stretch),
    )
    .padding(SPACING.s2)
}

/// One key of the sorting: its expression, its order, and moving it.
fn sort_row(e: &Editor, i: usize, _sort: &SortField, len: usize) -> impl View + use<> {
    let (read, write) = (e.clone(), e.clone());
    let expression = TextField::new(Binding::new(
        move || read.report.with(|r| r.sort.get(i).map(|s| s.expression.clone()).unwrap_or_default()),
        move |s: String| {
            write.edit_as("sort", |r| {
                if let Some(key) = r.sort.get_mut(i) {
                    key.expression = s;
                }
            });
        },
    ))
    .compact();
    let (read, write) = (e.clone(), e.clone());
    let descending = Toggle::new(Binding::new(
        move || read.report.with(|r| r.sort.get(i).is_some_and(|s| s.descending)),
        move |on: bool| {
            write.edit(|r| {
                if let Some(key) = r.sort.get_mut(i) {
                    key.descending = on;
                }
            });
        },
    ))
    .label(format!("Decrescente da ordenação {}", i + 1));
    let (moved, removed) = (e.clone(), e.clone());
    let buttons = level_buttons(
        i,
        len,
        "a ordenação",
        move |by| moved.edit(|r| shift(&mut r.sort, i, by)),
        move || {
            removed.edit(|r| {
                if i < r.sort.len() {
                    r.sort.remove(i);
                }
            });
        },
    );
    VStack::new((
        HStack::new((FormField::new(format!("Ordenar por ({})", i + 1), expression).grow(1.0), buttons))
            .spacing(SPACING.s2)
            .align(Align::End),
        descending,
    ))
    .spacing(SPACING.s1)
    .align(Align::Stretch)
}
