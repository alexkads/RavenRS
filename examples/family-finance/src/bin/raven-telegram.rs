//! Run the Telegram bot separately from the desktop window.

fn main() {
    let Some(path) = std::env::args_os().nth(1) else {
        eprintln!("usage: raven-telegram <existing-sqlite-db>");
        std::process::exit(2);
    };
    if let Err(error) = family_finance::telegram::serve(path.into()) {
        eprintln!("raven-telegram: {error}");
        std::process::exit(1);
    }
}
