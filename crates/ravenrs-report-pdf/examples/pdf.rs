//! Writes the sample reports as PDF: `cargo run -p ravenrs-report-pdf --example pdf -- <dir>`.
#![allow(clippy::print_stdout, missing_docs)]

use ravenrs_report::{ReportEngine, samples};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "target/report-pdf".into());
    std::fs::create_dir_all(&dir)?;
    let mut engine = ReportEngine::new();
    for (name, report) in [
        ("invoice", samples::invoice()),
        ("sales", samples::sales_by_region(10_000)),
        ("customers", samples::customer_list()),
    ] {
        let data = report.embedded_data().cloned().unwrap_or_default();
        let doc = engine.render(&report, &data);
        let start = std::time::Instant::now();
        let pdf = ravenrs_report_pdf::to_pdf(&doc)?;
        println!("{name}: {} pages, {} KB in {:?}", doc.pages.len(), pdf.len() / 1024, start.elapsed());
        std::fs::write(format!("{dir}/{name}.pdf"), pdf)?;
    }
    Ok(())
}
