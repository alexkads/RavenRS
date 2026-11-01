//! Writes the sample reports as XLSX and CSV: `cargo run -p ravenrs-report-xlsx --example xlsx -- <dir>`.
#![allow(clippy::print_stdout, missing_docs)]

use ravenrs_report::{ReportEngine, samples};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "target/report-xlsx".into());
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
        let xlsx = ravenrs_report_xlsx::to_xlsx(&doc)?;
        println!("{name}: {} KB in {:?}", xlsx.len() / 1024, start.elapsed());
        std::fs::write(format!("{dir}/{name}.xlsx"), xlsx)?;
        std::fs::write(format!("{dir}/{name}.csv"), ravenrs_report_xlsx::to_csv(&doc))?;
    }
    Ok(())
}
