//! Single-process document server. Run from the workspace root after building the web app.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::{path::PathBuf, sync::Arc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = std::env::var("WORDCRAFT_LISTEN").unwrap_or_else(|_| "127.0.0.1:8771".into());
    let data = PathBuf::from(std::env::var_os("WORDCRAFT_DATA_DIR").unwrap_or_else(|| "data".into()));
    let web = PathBuf::from(std::env::var_os("WORDCRAFT_WEB_DIR").unwrap_or_else(|| "dist/web".into()));
    if !web.join("index.html").is_file() {
        return Err("web build missing: run trunk build --release in apps/wordcraft-web".into());
    }
    std::fs::create_dir_all(&data)?;
    if data.canonicalize()?.starts_with(web.canonicalize()?) {
        return Err("data directory must be outside the public web directory".into());
    }
    let store = Arc::new(wordcraft_server::store::Store::open(&data.join("documents.sqlite3"))?);
    let font_dir = std::env::var_os("WORDCRAFT_FONTS_DIR").map(PathBuf::from);
    let fonts = Arc::new(wordcraft_server::fonts::Catalog::load(font_dir.as_deref())?);
    println!("Server font files: {}", fonts.len());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("WordCraft: http://{} (data: {})", listener.local_addr()?, data.canonicalize()?.display());
    axum::serve(listener, wordcraft_server::router_with_fonts(store, web, fonts))
        .with_graceful_shutdown(async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                eprintln!("shutdown signal: {error}");
            }
        })
        .await?;
    Ok(())
}
