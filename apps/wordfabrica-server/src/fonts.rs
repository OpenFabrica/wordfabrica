//! An immutable, bounded catalog of administrator-provided fonts. No user filesystem paths
//! enter HTTP handlers; downloads address only validated in-memory entries.
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use std::{
    io::{self, Read},
    path::Path as FsPath,
    sync::Arc,
};
use wordfabrica_remote::{FontFile, MAX_FONT_BYTES, MAX_FONT_FILES, MAX_FONT_TOTAL};

#[derive(Default)]
pub struct Catalog {
    entries: Vec<(FontFile, Bytes)>,
}

impl Catalog {
    /// Read a configured directory at startup, including up to four subdirectory levels.
    /// Reject malformed fonts and limits instead of publishing an incomplete catalog.
    /// Symlinks and unrelated files are excluded. `None` uses only the browser's bundled fonts.
    pub fn load(directory: Option<&FsPath>) -> io::Result<Self> {
        let mut catalog = Self::default();
        if let Some(directory) = directory {
            let mut paths = Vec::new();
            scan(directory, 0, &mut 0, &mut paths)?;
            paths.sort();
            let mut total = 0usize;
            for path in paths {
                let file = std::fs::File::open(&path)?;
                if file.metadata()?.len() > MAX_FONT_BYTES as u64 {
                    return Err(invalid("font exceeds 40 MiB"));
                }
                let mut bytes = Vec::new();
                file.take(MAX_FONT_BYTES as u64 + 1).read_to_end(&mut bytes)?;
                total = total.saturating_add(bytes.len());
                if bytes.len() > MAX_FONT_BYTES || total > MAX_FONT_TOTAL {
                    return Err(invalid("font catalog exceeds size limit"));
                }
                let mut families: Vec<_> = wordfabrica_fonts::font_names(&bytes).into_iter().map(|(family, _)| family).collect();
                families.sort();
                families.dedup();
                if families.is_empty() {
                    return Err(invalid(&format!("invalid font: {}", path.display())));
                }
                let entry = FontFile { id: uuid::Uuid::new_v4().to_string(), families, bytes: bytes.len() };
                catalog.entries.push((entry, Bytes::from(bytes)));
            }
        }
        Ok(catalog)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn scan(directory: &FsPath, depth: usize, visited: &mut usize, paths: &mut Vec<std::path::PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        *visited += 1;
        if *visited > 1024 {
            return Err(invalid("font directory exceeds 1024 entries"));
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            if depth >= 4 {
                return Err(invalid("font directory exceeds four nested levels"));
            }
            scan(&entry.path(), depth + 1, visited, paths)?;
        } else if kind.is_file()
            && entry.path().extension().and_then(|s| s.to_str()).is_some_and(|s| ["ttf", "otf", "ttc"].iter().any(|ext| s.eq_ignore_ascii_case(ext)))
        {
            paths.push(entry.path());
            if paths.len() > MAX_FONT_FILES {
                return Err(invalid("font catalog exceeds 64 files"));
            }
        }
    }
    Ok(())
}

pub(crate) fn routes(catalog: Arc<Catalog>) -> Router {
    Router::new().route("/api/fonts", get(list)).route("/api/fonts/{id}", get(download)).with_state(catalog)
}
async fn list(State(catalog): State<Arc<Catalog>>) -> Json<Vec<FontFile>> {
    Json(catalog.entries.iter().map(|(metadata, _)| metadata.clone()).collect())
}
async fn download(State(catalog): State<Arc<Catalog>>, Path(id): Path<String>) -> Response {
    match catalog.entries.iter().find(|(metadata, _)| metadata.id == id) {
        Some((_, bytes)) => ([(header::CONTENT_TYPE, "application/octet-stream")], bytes.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests;
