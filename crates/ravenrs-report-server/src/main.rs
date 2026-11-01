//! Runs the report server: `ravenrs-report-server --config server.json`.

use std::sync::Arc;
use std::time::Duration;

use ravenrs_report_server::config::Config;
use ravenrs_report_server::deliver::{Mailer, SmtpMailer};
use ravenrs_report_server::http;
use ravenrs_report_server::scheduler::Runner;
use ravenrs_report_server::store::Store;

/// How often the scheduler looks for jobs that are due.
const TICK: Duration = Duration::from_secs(15);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    let mut args = std::env::args().skip(1);
    let path = match (args.next().as_deref(), args.next()) {
        (Some("--config"), Some(path)) => path,
        _ => return Err("usage: ravenrs-report-server --config <file>".into()),
    };
    let config = Arc::new(Config::load(std::path::Path::new(&path))?);
    let token = config.api_token()?;
    let store = Store::open(&config.data_dir)?;
    let mailer: Option<Arc<dyn Mailer>> = match &config.smtp {
        Some(smtp) => Some(Arc::new(SmtpMailer::new(smtp)?)),
        None => None,
    };
    let runner = Runner::new(store.clone(), config.clone(), mailer);
    let ticking = runner.clone();
    tokio::spawn(async move { ticking.run_forever(TICK).await });

    let app = http::router(http::AppState::new(store, runner, config.clone(), token));
    let listener = tokio::net::TcpListener::bind(&config.listen).await?;
    tracing::info!(address = %config.listen, "report server listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}
