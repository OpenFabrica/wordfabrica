//! An immutable, bounded catalog of administrator-provided plugins (JSON data, never code).
//! Files are read and validated once at startup; the browser only sees the parsed result.
use axum::{Json, Router, extract::State, routing::get};
use std::{
    io::{self, Read},
    path::Path,
    sync::Arc,
};
use wordcraft_remote::{MAX_PLUGIN_BYTES, MAX_PLUGINS, Plugin};

#[derive(Default)]
pub struct Catalog {
    plugins: Vec<Plugin>,
}

impl Catalog {
    /// Read every `*.json` file directly in `directory`, in name order. A malformed, oversized or
    /// duplicate plugin fails startup instead of publishing a partial catalog. Subdirectories,
    /// symlinks and other files are ignored. `None` publishes no plugins.
    pub fn load(directory: Option<&Path>) -> io::Result<Self> {
        let mut catalog = Self::default();
        let Some(directory) = directory else { return Ok(catalog) };
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_file() && path.extension().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("json")) {
                paths.push(path);
                if paths.len() > MAX_PLUGINS {
                    return Err(invalid(&format!("more than {MAX_PLUGINS} plugins")));
                }
            }
        }
        paths.sort();
        for path in paths {
            let mut bytes = Vec::new();
            std::fs::File::open(&path)?.take(MAX_PLUGIN_BYTES as u64 + 1).read_to_end(&mut bytes)?;
            if bytes.len() > MAX_PLUGIN_BYTES {
                return Err(invalid(&format!("{}: plugin exceeds 1 MiB", path.display())));
            }
            let plugin: Plugin = serde_json::from_slice(&bytes).map_err(|e| invalid(&format!("{}: {e}", path.display())))?;
            plugin.validate().map_err(|e| invalid(&format!("{}: {e}", path.display())))?;
            if catalog.plugins.iter().any(|p| p.id == plugin.id) {
                return Err(invalid(&format!("{}: duplicate plugin id {:?}", path.display(), plugin.id)));
            }
            catalog.plugins.push(plugin);
        }
        Ok(catalog)
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub(crate) fn routes(catalog: Arc<Catalog>) -> Router {
    Router::new().route("/api/plugins", get(list)).with_state(catalog)
}

/// Every plugin, in file name order.
async fn list(State(catalog): State<Arc<Catalog>>) -> Json<Vec<Plugin>> {
    Json(catalog.plugins.clone())
}
