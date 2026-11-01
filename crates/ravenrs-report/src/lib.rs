//! RavenRS Reporting: banded reports, defined as data, paginated into
//! documents that the viewer shows and the exporters write.
//!
//! * [`Report`] is the definition. It covers the page, bands, groups, controls,
//!   styles, parameters, calculated fields and data sources, and is saved as
//!   JSON or built in code.
//! * [`ReportEngine`] binds data to it and produces a [`Document`]: pages of
//!   positioned text, lines and shapes, in points.
//! * Expressions come from [`ravenrs_criteria`]: `[Price] * [Quantity]`, `Sum([Total])`,
//!   `Iif(...)`, `FormatString('{0:C2}', ...)`.
//!
//! ```
//! use ravenrs_report::{Band, Control, Group, Report, ReportEngine, Style};
//! use serde_json::json;
//!
//! let report = Report::new("Sales")
//!     .culture("pt-BR")
//!     .group(
//!         Group::new("[Region]")
//!             .header(Band::new(22.0).control(Control::field("[Region]").at(0.0, 0.0, 300.0, 22.0).style(Style::new().bold())))
//!             .footer(Band::new(20.0).control(Control::field("Sum([Amount])").format("C2").at(0.0, 0.0, 300.0, 20.0))),
//!     )
//!     .detail(Band::new(18.0).control(Control::field("[Customer]").at(12.0, 0.0, 300.0, 18.0)));
//! let data = json!([
//!     { "Region": "Sul", "Customer": "Ana", "Amount": 10.5 },
//!     { "Region": "Norte", "Customer": "Bruno", "Amount": 20 },
//!     { "Region": "Sul", "Customer": "Carla", "Amount": 4.5 },
//! ]);
//! let doc = ReportEngine::new().render(&report, &data);
//! assert_eq!(doc.page_text(0), ["Norte", "Bruno", "R$\u{a0}20,00", "Sul", "Ana", "Carla", "R$\u{a0}15,00"]);
//! ```

mod document;
mod engine;
mod model;
mod paint;
pub mod samples;
pub mod wizard;

pub use document::{Document, Item, Page, PlacedLine, TextBlock};
pub use engine::{RenderOptions, ReportEngine};
pub use model::*;
pub use paint::paint_page;
pub use ravenrs_criteria as criteria;
pub use ravenrs_criteria::{Criteria, Culture, DateTime, ParseError, Value, format_composite, format_value};
