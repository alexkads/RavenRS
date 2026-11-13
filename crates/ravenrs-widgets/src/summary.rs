//! [`Summary`]: an aggregate of a column, with its words in [`Labels`].

use crate::labels::Labels;

/// An aggregate shown in group rows and in the footer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Summary {
    /// Number of non-empty cells.
    Count,
    /// Sum of the numeric values.
    Sum,
    /// Mean of the numeric values.
    Average,
    /// Smallest numeric value.
    Min,
    /// Largest numeric value.
    Max,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Summary {
    /// Short caption shown before the aggregate ("Sum", "Avg"…).
    pub fn label(self) -> &'static str {
        match self {
            Summary::Count => "Count",
            Summary::Sum => "Sum",
            Summary::Average => "Avg",
            Summary::Min => "Min",
            Summary::Max => "Max",
        }
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part3"))]
impl Labels {
    /// What a grid's footer calls this summary.
    pub fn summary(&self, summary: Summary) -> &str {
        let index = match summary {
            Summary::Count => 0,
            Summary::Sum => 1,
            Summary::Average => 2,
            Summary::Min => 3,
            _ => 4,
        };
        &self.grid_summaries[index]
    }
}
