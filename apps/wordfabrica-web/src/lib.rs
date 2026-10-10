//! Native-testable synchronization rules; networking and rendering live in the WASM shell.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
pub mod sync;

#[path = "preference.rs"]
pub mod locale;
