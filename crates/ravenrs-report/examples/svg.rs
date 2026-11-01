//! Writes the sample reports as SVG pages: `cargo run -p ravenrs-report --example svg -- <dir>`.
#![allow(clippy::print_stdout, missing_docs)]

use ravenrs_report::{ReportEngine, samples};

fn main() -> std::io::Result<()> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "target/report-svg".into());
    std::fs::create_dir_all(&dir)?;
    let mut engine = ReportEngine::new();
    for (name, report) in [
        ("invoice", samples::invoice()),
        ("sales", samples::sales_by_region(300)),
        ("customers", samples::customer_list()),
    ] {
        let data = report.embedded_data().cloned().unwrap_or_default();
        let start = std::time::Instant::now();
        let doc = engine.render(&report, &data);
        println!("{name}: {} pages in {:?}, errors: {:?}", doc.pages.len(), start.elapsed(), doc.errors);
        for i in 0..doc.pages.len().min(3) {
            std::fs::write(format!("{dir}/{name}-{}.svg", i + 1), doc.page_svg(i))?;
        }
    }
    Ok(())
}
