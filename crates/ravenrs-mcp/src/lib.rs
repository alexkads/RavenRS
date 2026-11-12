//! Embedded RavenRS MCP server, shared by both command line entry points.

mod api;
mod docs;
mod expression;
mod report;
mod server;

use rmcp::ServiceExt;

/// Serve the read-only RavenRS tools over standard input and output.
pub async fn serve_stdio() -> Result<(), Box<dyn std::error::Error>> {
    let service = server::RavenServer::new().serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
