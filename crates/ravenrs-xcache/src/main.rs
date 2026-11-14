//! The `ravenrs-xcache` binary: `RUSTC_WRAPPER=ravenrs-xcache`, or its own commands.

fn main() {
    let arguments: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    std::process::exit(ravenrs_xcache::main(arguments));
}
