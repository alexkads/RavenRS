//! `cargo run -p feature-center --bin api -- [file]`: the center as a web API, on 127.0.0.1:8090, in
//! this SQLite file or else in memory. Sign in as ana, bia or caio, at `/api/auth/token`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    let file = std::env::args().nth(1).map(std::path::PathBuf::from);
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:8090").await?;
        println!("http://{}/api/openapi.json", listener.local_addr()?);
        ravenrs_app_server::serve(listener, move || feature_center::api_service(file.as_deref())).await
    })
}

#[cfg(target_arch = "wasm32")]
fn main() {}
