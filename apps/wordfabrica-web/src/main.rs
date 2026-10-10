//! WordFabrica in the browser.
//!
//! Runs the same [`wordfabrica_ui_egui::WordApp`] as the desktop app through eframe's web runner
//! (wgpu: WebGPU where available, WebGL2 otherwise). Build with `trunk build --release` from this
//! directory (output in `dist/web`).
//!
//! Differences from the desktop app: no TCP control channel; Open and Insert › Pictures use the
//! browser file picker (bytes arrive through `Services::inbox`); hosted Save persists via HTTP.
//! Save As/Export download. `?local` keeps the standalone, download-only mode.
//! URL flags: `?webgl` forces WebGL2; `?sample` opens the sample document.
//! With a server, plugins from `/api/plugins` add panes to the HTML chrome (`docs/plugins.md`).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_arch = "wasm32")]
mod fonts;
#[cfg(target_arch = "wasm32")]
mod locale;
#[cfg(target_arch = "wasm32")]
mod plugins;
#[cfg(target_arch = "wasm32")]
mod remote;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("wordfabrica-web only runs in the browser: build it with `trunk build --release` in apps/wordfabrica-web");
}
