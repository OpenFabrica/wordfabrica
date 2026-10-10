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
    // Relative to the launch directory, like the default web and data directories.
    // Packaged deployments can set WORDCRAFT_FONTS_DIR to their installed resource path.
    let font_dir = PathBuf::from(std::env::var_os("WORDCRAFT_FONTS_DIR").unwrap_or_else(|| "resources/fonts/server".into()));
    let fonts = Arc::new(wordcraft_server::fonts::Catalog::load(Some(&font_dir))?);
    println!("Server font files: {}", fonts.len());
    // Plugins are optional: without WORDCRAFT_PLUGINS_DIR, a missing `plugins` directory means none.
    let plugin_dir = match std::env::var_os("WORDCRAFT_PLUGINS_DIR") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => Some(PathBuf::from("plugins")).filter(|dir| dir.is_dir()),
    };
    let plugins = Arc::new(wordcraft_server::plugins::Catalog::load(plugin_dir.as_deref())?);
    println!("Plugins: {}", plugins.len());
    // Third-party integration is off unless WORDCRAFT_INTEGRATION_SECRET is set.
    let integration = match wordcraft_server::integration::Settings::from_env(&data)? {
        Some(settings) => {
            if let wordcraft_server::integration::StorageSettings::Local(root) = &settings.storage {
                std::fs::create_dir_all(root)?;
                if root.canonicalize()?.starts_with(web.canonicalize()?) {
                    return Err("storage directory must be outside the public web directory".into());
                }
            }
            println!("Integration: on ({:?} storage, public URL {})", settings.storage, settings.public_url);
            let service = wordcraft_server::integration::Service::new(settings, store.clone())?;
            wordcraft_server::callbacks::spawn(service.clone());
            Some(service)
        }
        None => None,
    };
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("WordCraft: http://{} (data: {})", listener.local_addr()?, data.canonicalize()?.display());
    axum::serve(listener, wordcraft_server::router_with_integration(store, web, fonts, plugins, integration))
        .with_graceful_shutdown(async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                eprintln!("shutdown signal: {error}");
            }
        })
        .await?;
    Ok(())
}
