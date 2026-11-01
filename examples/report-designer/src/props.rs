//! The property grid: edits the selected control, band or the report.

use ravenrs::prelude::*;
use ravenrs::style::SPACING;
use ravenrs_report::{ControlKind, PageSettings, Style, TextAlign};

use crate::bands::BandRef;
use crate::editor::{Editor, Selection};

/// Formats offered in the format picker, with the format string they use.
pub const FORMATS: [(&str, &str); 8] = [
    ("Nenhum", ""),
    ("Moeda (R$ 1.234,50)", "C2"),
    ("Número (1.234,50)", "N2"),
    ("Inteiro (1.235)", "N0"),
    ("Porcentagem (12,3%)", "P1"),
    ("Data (31/12/2026)", "dd/MM/yyyy"),
    ("Data longa", "D"),
    ("Mês e ano", "MMMM 'de' yyyy"),
];

/// Text colors and fills offered as swatches: Raven Light tokens, since they are
/// printed on white paper.
pub const PALETTE: [(&str, &str); 8] = [
    ("#12151c", "Tinta"),
    ("#5a6375", "Tinta secundária"),
    ("#ffffff", "Branco"),
    ("#3f57db", "Destaque"),
    ("#1f8a5b", "Sucesso"),
    ("#b26a00", "Atenção"),
    ("#c93434", "Perigo"),
    ("#f4f5f8", "Fundo suave"),
];

fn section(title: &str) -> Text {
    Text::new(title.to_uppercase()).style(TextStyle::Caption).foreground(Semantic::InkMuted)
}

fn number_field(
    e: &Editor,
    kind: &'static str,
    get: fn(&ravenrs_report::Control) -> f32,
    set: fn(&mut ravenrs_report::Control, f32),
) -> TextField {
    let (read, write) = (e.clone(), e.clone());
    TextField::new(Binding::new(
        move || read.selected_control().map(|(_, _, c)| format!("{:.0}", get(&c))).unwrap_or_default(),
        move |s: String| {
            if let Ok(v) = s.trim().replace(',', ".").parse::<f32>() {
                write.edit_control(kind, |c| set(c, v.max(0.0)));
            }
        },
    ))
    .compact()
}

struct Swatches {
    editor: Editor,
    background: bool,
}

impl View for Swatches {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let current = self
            .editor
            .selected_control()
            .and_then(|(_, _, c)| if self.background { c.style.background } else { c.style.color });
        let pick = |hex: Option<&'static str>| {
            let (e, background) = (self.editor.clone(), self.background);
            move || {
                let value = hex.map(str::to_owned);
                e.edit_control(if background { "bg" } else { "fg" }, |c| {
                    if background {
                        c.style.background = value;
                    } else {
                        c.style.color = value;
                    }
                });
            }
        };
        let mut items: Vec<AnyView> =
            vec![Box::new(ColorSwatch::none("Padrão").selected(current.is_none()).on_click(pick(None)))];
        for (hex, name) in PALETTE {
            let color = crate::surface::color(Some(hex)).unwrap_or(Color::TRANSPARENT);
            items.push(Box::new(
                ColorSwatch::new(color, name)
                    .selected(current.as_deref() == Some(hex))
                    .on_click(pick(Some(hex))),
            ));
        }
        cx.child(&HStack::new(items).spacing(0.0).wrap())
    }
}

/// The properties of whatever is selected.
pub struct Properties(pub Editor);

impl View for Properties {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let e = &self.0;
        let content: AnyView = match e.selection.get() {
            Selection::Control(..) => Box::new(control_props(e)),
            Selection::Band(b) => Box::new(band_props(e, b)),
            Selection::None => Box::new(report_props(e)),
        };
        cx.child(
            &ScrollView::new((VStack::new((content,)).padding(SPACING.s4).spacing(SPACING.s4),)).grow(1.0),
        )
    }
}

fn control_props(e: &Editor) -> impl View + use<> {
    let Some((band, _, control)) = e.selected_control() else { return Box::new(Text::new("")) as AnyView };
    let report = e.report.get();
    let kind = match &control.kind {
        ControlKind::Label(l) if l.expression.is_some() => "Campo",
        ControlKind::Label(_) => "Rótulo",
        ControlKind::Table(_) => "Tabela",
        ControlKind::CheckBox(_) => "Caixa de seleção",
        ControlKind::Line(_) => "Linha",
        ControlKind::Shape(_) => "Forma",
        ControlKind::Panel(_) => "Painel",
        _ => "Controle",
    };
    let mut rows: Vec<AnyView> = vec![Box::new(
        HStack::new((
            Text::new(kind).style(TextStyle::Headline),
            Spacer::new(),
            Badge::new(band.title(&report)).tone(Tone::Accent),
        ))
        .align(Align::Center),
    )];
    if let ControlKind::Label(label) = &control.kind {
        let is_field = label.expression.is_some();
        let (read, write) = (e.clone(), e.clone());
        let text = TextField::new(Binding::new(
            move || {
                read.selected_control()
                    .map(|(_, _, c)| match c.kind {
                        ControlKind::Label(l) => l.expression.unwrap_or(l.text),
                        _ => String::new(),
                    })
                    .unwrap_or_default()
            },
            move |s: String| {
                write.edit_control("text", |c| {
                    if let ControlKind::Label(l) = &mut c.kind {
                        if l.expression.is_some() {
                            l.expression = Some(s);
                        } else {
                            l.text = s;
                        }
                    }
                })
            },
        ))
        .compact();
        rows.push(Box::new(FormField::new(if is_field { "Expressão" } else { "Texto" }, text).hint(
            if is_field {
                "Ex.: [Preco] * [Qtd], Sum([Total]), Iif([Pago], 'Sim', 'Não')"
            } else {
                "Texto fixo exibido no relatório"
            },
        )));
        let current = label.format.clone().unwrap_or_default();
        let index = FORMATS.iter().position(|(_, f)| *f == current).unwrap_or(0);
        let write = e.clone();
        let picker = Select::new(
            Binding::new(
                move || index,
                move |i: usize| {
                    let f = FORMATS[i.min(FORMATS.len() - 1)].1;
                    write.edit_control("format", |c| {
                        if let ControlKind::Label(l) = &mut c.kind {
                            l.format = (!f.is_empty()).then(|| f.to_owned());
                        }
                    });
                },
            ),
            FORMATS.iter().map(|(name, _)| *name),
        )
        .width(268.0)
        .label("Formato");
        rows.push(Box::new(FormField::new("Formato", picker)));
    }
    rows.push(Box::new(section("Posição e tamanho (pt)")));
    rows.push(Box::new(
        Grid::new(
            2,
            (
                FormField::new("X", number_field(e, "x", |c| c.x, |c, v| c.x = v)),
                FormField::new("Y", number_field(e, "y", |c| c.y, |c, v| c.y = v)),
                FormField::new("Largura", number_field(e, "w", |c| c.width, |c, v| c.width = v.max(4.0))),
                FormField::new("Altura", number_field(e, "h", |c| c.height, |c, v| c.height = v.max(2.0))),
            ),
        )
        .spacing(SPACING.s3),
    ));
    rows.push(Box::new(section("Aparência")));
    let size_e = e.clone();
    let size = State::new(control.style.font_size.unwrap_or(10.0).round() as i32);
    let size_binding = {
        let (read, write) = (e.clone(), size_e);
        Binding::new(
            move || {
                read.selected_control()
                    .map_or(10, |(_, _, c)| c.style.font_size.unwrap_or(10.0).round() as i32)
            },
            move |v: i32| write.edit_control("size", |c| c.style.font_size = Some(v.clamp(5, 72) as f32)),
        )
    };
    let _ = size;
    let (bread, bwrite) = (e.clone(), e.clone());
    let bold = ToggleButton::new(
        Binding::new(
            move || bread.selected_control().is_some_and(|(_, _, c)| c.style.bold == Some(true)),
            move |v: bool| bwrite.edit_control("bold", |c| c.style.bold = Some(v)),
        ),
        "Negrito",
    )
    .icon(IconName::Bold);
    rows.push(Box::new(
        HStack::new((
            FormField::new("Fonte", Stepper::new(size_binding).range(5, 72).label("Tamanho da fonte")),
            Spacer::new(),
            bold,
        ))
        .align(Align::End),
    ));
    let (aread, awrite) = (e.clone(), e.clone());
    let align = SegmentedControl::with_icons(
        Binding::new(
            move || {
                aread.selected_control().map_or(0, |(_, _, c)| match c.style.text_align {
                    Some(TextAlign::Center) => 1,
                    Some(TextAlign::Right) => 2,
                    _ => 0,
                })
            },
            move |i: usize| {
                awrite.edit_control("align", |c| {
                    c.style.text_align =
                        Some([TextAlign::Left, TextAlign::Center, TextAlign::Right][i.min(2)]);
                })
            },
        ),
        [(IconName::AlignLeft, "Esq."), (IconName::AlignCenter, "Centro"), (IconName::AlignRight, "Dir.")],
    )
    .label("Alinhamento");
    rows.push(Box::new(FormField::new("Alinhamento", align)));
    rows.push(Box::new(FormField::new("Cor do texto", Swatches { editor: e.clone(), background: false })));
    rows.push(Box::new(FormField::new("Fundo", Swatches { editor: e.clone(), background: true })));
    let (bread, bwrite) = (e.clone(), e.clone());
    rows.push(Box::new(
        Toggle::new(Binding::new(
            move || {
                bread.selected_control().is_some_and(|(_, _, c)| c.style.border_width.unwrap_or(0.0) > 0.0)
            },
            move |on: bool| {
                bwrite.edit_control("border", |c| {
                    if on {
                        c.style = c.style.clone().merged(&Style::new().border("all", 0.75, "#dadde5"));
                    } else {
                        c.style.border_width = Some(0.0);
                    }
                })
            },
        ))
        .label("Borda"),
    ));
    let del = e.clone();
    rows.push(Box::new(
        Button::new("Excluir controle")
            .icon(IconName::Trash2)
            .style(ButtonStyle::Danger)
            .full_width()
            .on_click(move || del.delete_selected()),
    ));
    Box::new(VStack::new(rows).spacing(SPACING.s4).align(Align::Stretch)) as AnyView
}

fn band_props(e: &Editor, band: BandRef) -> impl View + use<> {
    let report = e.report.get();
    let mut rows: Vec<AnyView> = vec![Box::new(Text::new(band.title(&report)).style(TextStyle::Headline))];
    let (read, write) = (e.clone(), e.clone());
    rows.push(Box::new(FormField::new(
        "Altura (pt)",
        TextField::new(Binding::new(
            move || read.report.with(|r| band.get(r).map(|b| format!("{:.0}", b.height)).unwrap_or_default()),
            move |s: String| {
                if let Ok(v) = s.trim().parse::<f32>() {
                    write.edit_as("band-height", |r| {
                        if let Some(b) = band.get_mut(r) {
                            b.height = v.max(4.0);
                        }
                    });
                }
            },
        ))
        .compact(),
    )));
    if let BandRef::GroupHeader(i) | BandRef::GroupFooter(i) = band {
        let (read, write) = (e.clone(), e.clone());
        rows.push(Box::new(
            FormField::new(
                "Agrupar por",
                TextField::new(Binding::new(
                    move || {
                        read.report
                            .with(|r| r.groups.get(i).map(|g| g.expression.clone()).unwrap_or_default())
                    },
                    move |s: String| {
                        write.edit_as("group", |r| {
                            if let Some(g) = r.groups.get_mut(i) {
                                g.expression = s;
                            }
                        })
                    },
                ))
                .compact(),
            )
            .hint("Uma expressão, como [Regiao]"),
        ));
        let (read, write) = (e.clone(), e.clone());
        rows.push(Box::new(
            Toggle::new(Binding::new(
                move || read.report.with(|r| r.groups.get(i).is_some_and(|g| g.page_break_before)),
                move |v: bool| {
                    write.edit(|r| {
                        if let Some(g) = r.groups.get_mut(i) {
                            g.page_break_before = v;
                        }
                    })
                },
            ))
            .label("Cada grupo em uma nova página"),
        ));
    }
    if band == BandRef::Detail {
        let (read, write) = (e.clone(), e.clone());
        rows.push(Box::new(
            Toggle::new(Binding::new(
                move || read.report.with(|r| r.detail.odd_style.is_some()),
                move |on: bool| {
                    write.edit(|r| {
                        if on {
                            r.styles
                                .entry("odd".into())
                                .or_insert_with(|| Style::new().background("#ffffff"));
                            r.styles
                                .entry("even".into())
                                .or_insert_with(|| Style::new().background("#f4f5f8"));
                            r.detail.odd_style = Some("odd".into());
                            r.detail.even_style = Some("even".into());
                        } else {
                            r.detail.odd_style = None;
                            r.detail.even_style = None;
                        }
                    })
                },
            ))
            .label("Linhas zebradas"),
        ));
    }
    rows.push(Box::new(
        Text::new("Arraste campos da lista para esta banda, ou use Inserir na faixa de opções.")
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted),
    ));
    VStack::new(rows).spacing(SPACING.s4).align(Align::Stretch)
}

fn report_props(e: &Editor) -> impl View + use<> {
    let (read, write) = (e.clone(), e.clone());
    let name = TextField::new(Binding::new(
        move || read.report.with(|r| r.name.clone()),
        move |s: String| write.edit_as("name", |r| r.name = s),
    ))
    .compact();
    let (read, write) = (e.clone(), e.clone());
    let culture = SegmentedControl::new(
        Binding::new(
            move || read.report.with(|r| usize::from(!r.culture.to_lowercase().starts_with("pt"))),
            move |i: usize| write.edit(|r| r.culture = if i == 0 { "pt-BR".into() } else { "en-US".into() }),
        ),
        ["Português", "English"],
    )
    .label("Idioma dos formatos");
    let (read, write) = (e.clone(), e.clone());
    let paper = SegmentedControl::new(
        Binding::new(
            move || read.report.with(|r| usize::from(r.page.width > r.page.height)),
            move |i: usize| {
                write.edit(|r| {
                    let base = if (r.page.width.min(r.page.height) - 612.0).abs() < 1.0 {
                        PageSettings::letter()
                    } else {
                        PageSettings::a4()
                    };
                    r.page = if i == 1 { base.landscape() } else { base };
                })
            },
        ),
        ["Retrato", "Paisagem"],
    )
    .label("Orientação");
    let (read, write) = (e.clone(), e.clone());
    let filter = TextField::new(Binding::new(
        move || read.report.with(|r| r.filter.clone().unwrap_or_default()),
        move |s: String| write.edit_as("filter", |r| r.filter = (!s.trim().is_empty()).then_some(s)),
    ))
    .compact();
    VStack::new((
        Text::new("Relatório").style(TextStyle::Headline),
        FormField::new("Nome", name),
        FormField::new("Idioma dos formatos", culture),
        FormField::new("Orientação", paper),
        FormField::new("Filtro", filter)
            .hint("Linhas em que a expressão é falsa ficam de fora, ex.: [Total] >= ?MinTotal"),
        Text::new("Clique numa banda ou num controle para editar as propriedades dele.")
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted),
    ))
    .spacing(SPACING.s4)
    .align(Align::Stretch)
}
