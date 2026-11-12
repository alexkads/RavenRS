//! An MCP server for developers building with RavenRS. It has no side effects: it
//! answers questions and returns report layouts as JSON for the caller to save.
//!
//! Everything it answers from is embedded at build time, so it reads no files,
//! opens no sockets and always describes the RavenRS it was built from. See the
//! README for the tools and how to install it.

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match std::env::args().nth(1).as_deref() {
        None => {}
        Some("--version" | "-V") => {
            println!("ravenrs-mcp {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some(_) => {
            println!(
                "ravenrs-mcp {}: an MCP server over stdio. Takes no arguments; an MCP client starts it.",
                env!("CARGO_PKG_VERSION")
            );
            return Ok(());
        }
    }
    ravenrs_mcp::serve_stdio().await
}
