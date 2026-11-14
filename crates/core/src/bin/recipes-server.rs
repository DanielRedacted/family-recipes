//! Headless server for development and tests: `recipes-server --vault DIR [--data DIR] [--port N]`.
use recipes_core::server::{self, Config, Listener};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
    ).init();
    let mut args = std::env::args().skip(1);
    let (mut vault, mut data, mut port) = (None, None, 8640u16);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--vault" => vault = args.next().map(PathBuf::from),
            "--data" => data = args.next().map(PathBuf::from),
            "--port" => port = args.next().and_then(|p| p.parse().ok()).unwrap_or(port),
            _ => anyhow::bail!("unknown argument {a}"),
        }
    }
    let home = PathBuf::from(std::env::var("HOME")?);
    let vault = vault.unwrap_or_else(|| home.join("Recipes"));
    let data = data.unwrap_or_else(|| home.join("Library/Application Support/Recipes"));
    let state = server::init(Config { vault, data_dir: data, shell: None, pdf_text: None })?;
    let l = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    let port = l.local_addr()?.port();
    state.loopback_port.store(port, std::sync::atomic::Ordering::Relaxed);
    println!("Recipes: http://127.0.0.1:{port}/auth?k={}", state.launch_token);
    server::serve(server::router(state), l, Listener::Loopback).await
}
