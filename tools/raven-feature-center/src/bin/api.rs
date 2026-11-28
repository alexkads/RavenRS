//! `cargo run --manifest-path tools/raven-feature-center/Cargo.toml --bin api -- [file]`: the center as a web API, on 127.0.0.1:8090, in
//! this SQLite file or else in memory. Sign in as ana, bia or caio, at `/api/auth/token`. A page on
//! another origin can call it if `RAVEN_API_ORIGIN` names that origin. Over a file, four threads answer
//! at the same time; in memory, one.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    let file = std::env::args().nth(1).map(std::path::PathBuf::from);
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:8090").await?;
        println!("http://{}/api/openapi.json", listener.local_addr()?);
        // Records in memory belong to one thread; a file is opened again by each.
        let workers = if file.is_some() { raven_feature_center::servidor::WORKERS_WITH_A_FILE } else { 1 };
        let origins: Vec<String> = std::env::var("RAVEN_API_ORIGIN").into_iter().collect();
        let options = raven_feature_center::servidor::options(
            &origins,
            workers as i32,
            Some(&raven_feature_center::servidor::channels()),
            false,
        );
        ravenrs_app_server::serve_with(listener, options, move || {
            raven_feature_center::api_service(file.as_deref())
        })
        .await
    })
}

#[cfg(target_arch = "wasm32")]
fn main() {}
