//! What can go wrong between a document and a printed label. The messages are for the person at the
//! counter, so they are in Portuguese and say what to change.

use std::fmt;

/// Why a label could not be made or sent.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum LabelError {
    /// A barcode cannot keep its minimum module in its box.
    BarcodeDoesNotFit {
        /// The format's name.
        symbology: &'static str,
        /// What it encodes.
        content: String,
        /// Where its box starts on the label, in millimeters.
        at_mm: (f32, f32),
        /// The size available across, in millimeters.
        available_mm: f32,
        /// The size the code needs across, quiet zone included, in millimeters.
        needed_mm: f32,
        /// The module that fits, in whole dots. It can be zero.
        module_dots: u32,
        /// The smallest module allowed at this resolution, in whole dots.
        minimum_dots: u32,
        /// The smallest module allowed for this format, in millimeters.
        minimum_mm: f32,
        /// The printer's resolution.
        dpi: u32,
    },
    /// The content cannot be encoded in the format.
    InvalidBarcode {
        /// The format's name.
        symbology: &'static str,
        /// What it should encode.
        content: String,
        /// What the encoder said.
        reason: String,
    },
    /// The document has no pages to print.
    EmptyDocument,
    /// The page cannot become a picture at this resolution.
    InvalidPage(String),
    /// A setting of the job is out of range for the language.
    InvalidJob(String),
    /// The printer's name or address is not acceptable.
    InvalidDestination(String),
    /// The printer or the file did not take the bytes.
    Output(String),
    /// What went wrong on one label of the document.
    OnLabel {
        /// The label, from 1.
        label: usize,
        /// What went wrong.
        source: Box<LabelError>,
    },
}

fn mm(value: f32, decimals: usize) -> String {
    format!("{value:.decimals$}").replace('.', ",")
}

impl fmt::Display for LabelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BarcodeDoesNotFit {
                symbology,
                content,
                at_mm,
                available_mm,
                needed_mm,
                module_dots,
                minimum_dots,
                minimum_mm,
                dpi,
            } => write!(
                f,
                "o código {symbology} \"{content}\", no controle em {} x {} mm, não cabe: tem {} mm, \
                 precisa de {} mm com a zona quieta. A {dpi} dpi o módulo daria {module_dots} ponto(s), \
                 e o mínimo é {minimum_dots} ({} mm). Aumente o controle, use uma etiqueta maior ou \
                 uma impressora de resolução maior.",
                mm(at_mm.0, 1),
                mm(at_mm.1, 1),
                mm(*available_mm, 1),
                mm(*needed_mm, 1),
                mm(*minimum_mm, 3),
            ),
            Self::InvalidBarcode { symbology, content, reason } => {
                write!(f, "o código {symbology} não aceita \"{content}\": {reason}")
            }
            Self::EmptyDocument => write!(f, "o documento não tem nenhuma página para imprimir"),
            Self::InvalidPage(why) => write!(f, "a página não vira etiqueta: {why}"),
            Self::InvalidJob(why) => write!(f, "impressão inválida: {why}"),
            Self::InvalidDestination(why) => write!(f, "destino inválido: {why}"),
            Self::Output(why) => write!(f, "falha ao enviar para a impressora: {why}"),
            Self::OnLabel { label, source } => write!(f, "etiqueta {label}: {source}"),
        }
    }
}

impl std::error::Error for LabelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::OnLabel { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
