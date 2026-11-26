//! The receipt of a sale on the screen, as it will come out of the printer.

use ravenrs_core::Role;
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{AnyView, Divider, HStack, Text, TextAlign, VStack, View, ViewExt};

/// One line of a [`Receipt`]: what was sold, what it comes to and, under it, how many at what
/// price.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ReceiptLine {
    description: String,
    amount: String,
    detail: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ReceiptLine {
    /// A line for `description` that comes to `amount`, both as the receipt prints them
    /// ("FOTO 10X15", "30,00").
    pub fn new(description: impl Into<String>, amount: impl Into<String>) -> Self {
        Self { description: description.into(), amount: amount.into(), detail: None }
    }

    /// What goes under it, such as how many at what price ("2 x 15,00").
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// The receipt of a sale on the screen while it is made, as it will come out of the printer: a
/// title, the lines in the monospaced font, rows such as the subtotal, and the total in a band
/// with the amount large. With no lines it says where they will show.
///
/// The amounts come as text, formatted by the app as it prints them; a screen fed by a cart of
/// `ravenrs-pos` gets it from `ravenrs_app_pos::cupom_na_tela`. Screen readers hear the lines
/// in order, as a list, and the total by its name and amount.
///
/// ```
/// use ravenrs_widgets::{Receipt, ReceiptLine, Ui};
///
/// let receipt = Receipt::new()
///     .title("Cupom")
///     .line(ReceiptLine::new("FOTO 10X15", "30,00").detail("2 x 15,00"))
///     .summary("SUBTOTAL (1 ITEM)", "30,00")
///     .total("TOTAL A RECEBER", "R$ 30,00");
/// let mut ui = Ui::new(receipt, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Receipt {
    title: Option<String>,
    subtitle: Option<String>,
    lines: Vec<ReceiptLine>,
    summary: Vec<(String, String)>,
    total: Option<(String, String)>,
    empty_text: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Receipt {
    /// An empty receipt, with no title and no total.
    pub fn new() -> Self {
        Self::default()
    }

    /// The title above it ("Cupom").
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// What goes under the title, such as the session or who is buying.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Adds a line of the sale, after those already there.
    pub fn line(mut self, line: ReceiptLine) -> Self {
        self.lines.push(line);
        self
    }

    /// Adds a row under the lines, such as the subtotal or a discount, with its value at the
    /// end; rows show in the order they are added.
    pub fn summary(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.summary.push((label.into(), value.into()));
        self
    }

    /// The total, in a band at the foot with the amount large ("TOTAL A RECEBER", "R$ 30,00").
    pub fn total(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.total = Some((label.into(), value.into()));
        self
    }

    /// What shows while there are no lines (from [`Labels`](crate::Labels) by default: "The
    /// receipt shows here.").
    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.empty_text = Some(text.into());
        self
    }
}

/// The lines of a [`Receipt`], a list screen readers hear in order.
struct ReceiptLines {
    lines: Vec<ReceiptLine>,
    name: String,
}

impl View for ReceiptLines {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let rows: Vec<NodeId> = self
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let mut parts: Vec<AnyView> = vec![Box::new(
                    HStack::new((
                        Text::new(line.description.clone()).mono().truncate().grow(1.0),
                        Text::new(line.amount.clone()).mono(),
                    ))
                    .spacing(12.0)
                    .align(Align::Start),
                )];
                if let Some(detail) = &line.detail {
                    parts.push(Box::new(
                        Text::new(detail.clone())
                            .style(TextStyle::Caption)
                            .mono()
                            .foreground(Semantic::InkMuted),
                    ));
                }
                let content =
                    cx.keyed_child(index as u64, &VStack::new(parts).spacing(2.0).align(Align::Stretch));
                let row = cx.container(
                    LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
                    &[content],
                    Paint::None,
                );
                let mut semantics =
                    Semantics::new(Role::ListItem).name(line.description.clone()).value(line.amount.clone());
                if let Some(detail) = &line.detail {
                    semantics = semantics.description(detail.clone());
                }
                cx.semantics(row, semantics);
                row
            })
            .collect();
        let node = cx.container(
            LayoutStyle { spacing: 8.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &rows,
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::List).name(self.name.clone()));
        node
    }
}

/// The band with the total, which says its name and amount, and says them again when the total
/// changes.
struct ReceiptTotal {
    label: String,
    value: String,
}

impl View for ReceiptTotal {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let content = cx.child(
            &HStack::new((
                Text::new(self.label.clone())
                    .style(TextStyle::Callout)
                    .font_weight(Weight::SEMIBOLD)
                    .mono()
                    .grow(1.0),
                Text::new(self.value.clone()).style(TextStyle::Title).font_weight(Weight::SEMIBOLD).mono(),
            ))
            .spacing(12.0)
            .align(Align::Center),
        );
        let node = cx.container(
            LayoutStyle {
                padding: ravenrs_core::Insets::symmetric(theme.spacing.s3, theme.spacing.s4),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                ..QuadPaint::new(theme.color(Semantic::Ink).with_alpha(0.08))
            }),
        );
        cx.semantics(node, Semantics::new(Role::Status).name(self.label.clone()).value(self.value.clone()));
        node
    }
}

impl View for Receipt {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let mut parts: Vec<AnyView> = Vec::new();
        if self.title.is_some() || self.subtitle.is_some() {
            let mut head: Vec<AnyView> = Vec::new();
            if let Some(title) = &self.title {
                head.push(Box::new(Text::new(title.clone()).style(TextStyle::Headline)));
            }
            if let Some(subtitle) = &self.subtitle {
                head.push(Box::new(
                    Text::new(subtitle.clone()).style(TextStyle::Callout).foreground(Semantic::InkMuted),
                ));
            }
            parts.push(Box::new(VStack::new(head).spacing(2.0).align(Align::Start)));
        }
        if self.lines.is_empty() {
            let empty = self.empty_text.clone().unwrap_or_else(|| labels.receipt_empty.clone());
            parts.push(Box::new(
                Text::new(empty).foreground(Semantic::InkMuted).align(TextAlign::Center).padding(24.0),
            ));
        } else {
            parts.push(Box::new(ReceiptLines {
                lines: self.lines.clone(),
                name: labels.receipt_lines.clone(),
            }));
        }
        if !self.summary.is_empty() || self.total.is_some() {
            parts.push(Box::new(Divider::horizontal()));
        }
        for (label, value) in &self.summary {
            parts.push(Box::new(
                HStack::new((
                    Text::new(label.clone())
                        .style(TextStyle::Caption)
                        .mono()
                        .foreground(Semantic::InkMuted)
                        .grow(1.0),
                    Text::new(value.clone()).style(TextStyle::Caption).mono(),
                ))
                .spacing(12.0)
                .align(Align::Center),
            ));
        }
        if let Some((label, value)) = &self.total {
            parts.push(Box::new(ReceiptTotal { label: label.clone(), value: value.clone() }));
        }
        cx.child(&VStack::new(parts).spacing(12.0).align(Align::Stretch))
    }
}
