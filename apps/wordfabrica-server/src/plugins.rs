//! An immutable, bounded catalog of administrator-provided plugins (JSON data, never code).
//! Files are read and validated once at startup; the browser only sees the parsed result.
//! Plugins marked `editable` let users replace their tags; that shared version lives in the
//! store and wins over the file's tags.
use axum::{
    Json, Router,
    extract::{Path as UrlPath, State},
    routing::{get, put},
};
use std::{
    io::{self, Read},
    path::Path,
    sync::Arc,
};
use wordfabrica_remote::{MAX_PLUGIN_BYTES, MAX_PLUGINS, Plugin, PluginKind, TagsWrite};

use crate::store::{Error, Store};

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
            let mut plugin: Plugin = serde_json::from_slice(&bytes).map_err(|e| invalid(&format!("{}: {e}", path.display())))?;
            // Revisions belong to the store, never to the file.
            plugin.revision = 0;
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

type Shared = (Arc<Catalog>, Arc<Store>);

pub(crate) fn routes(catalog: Arc<Catalog>, store: Arc<Store>) -> Router {
    Router::new().route("/api/plugins", get(list)).route("/api/plugins/{id}/tags", put(save_tags)).with_state((catalog, store))
}

/// A plugin as users see it: an editable plugin carries the stored tags once someone saved some.
fn current(store: &Store, plugin: &Plugin) -> Result<Plugin, Error> {
    let mut plugin = plugin.clone();
    if plugin.editable
        && let Some((revision, categories)) = store.plugin_tags(&plugin.id)?
    {
        plugin.revision = revision;
        plugin.kind = PluginKind::Tags { categories };
    }
    Ok(plugin)
}

/// Every plugin, in file name order.
async fn list(State((catalog, store)): State<Shared>) -> Result<Json<Vec<Plugin>>, Error> {
    crate::blocking(move || catalog.plugins.iter().map(|p| current(&store, p)).collect()).await.map(Json)
}

/// Replace an editable plugin's tags. Read-only and unknown plugins are 404; invalid tags 400;
/// a stale `revision` 409 (the client reloads). Returns the plugin as now stored.
async fn save_tags(
    State((catalog, store)): State<Shared>,
    UrlPath(id): UrlPath<String>,
    Json(write): Json<TagsWrite>,
) -> Result<Json<Plugin>, Error> {
    crate::blocking(move || {
        let mut plugin = catalog.plugins.iter().find(|p| p.id == id && p.editable).cloned().ok_or(Error::NotFound)?;
        plugin.kind = PluginKind::Tags { categories: write.categories };
        plugin.validate().map_err(|_| Error::Invalid)?;
        let PluginKind::Tags { categories } = &plugin.kind;
        plugin.revision = store.put_plugin_tags(&plugin.id, write.revision, categories)?;
        Ok(Json(plugin))
    })
    .await
}
