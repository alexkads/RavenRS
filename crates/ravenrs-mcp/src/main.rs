//! A read-only MCP server for developers building with RavenRS.
//!
//! Everything it answers from is embedded at build time, so it reads no files,
//! opens no sockets and always describes the RavenRS it was built from. See the
//! README for the tools and how to install it.

mod api;
mod docs;
mod expression;
mod server;

use rmcp::ServiceExt;

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
    let service = server::RavenServer::new().serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
