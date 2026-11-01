//! Prints a sample layout to try the server with:
//! `cargo run -p ravenrs-report-server --example sample_layout > invoice.json`.
#![allow(clippy::print_stdout, missing_docs)]

fn main() {
    println!("{}", ravenrs_report::samples::invoice().to_json());
}
