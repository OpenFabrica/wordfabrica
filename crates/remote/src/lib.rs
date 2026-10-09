//! Versioned snapshots shared by the browser shell and the storage service.
//! Revision zero creates a document; subsequent writes must match the stored revision.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use serde::{Deserialize, Serialize};
use wordcraft_doc::Document;

/// Maximum encoded request/snapshot size (including embedded pictures).
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub id: String,
    pub title: String,
    pub revision: u64,
    /// Unix milliseconds, assigned by the server.
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub metadata: Metadata,
    pub document: Document,
}

#[derive(Serialize, Deserialize)]
pub struct WriteRequest {
    pub revision: u64,
    pub title: String,
    pub document: Document,
}

/// IDs are opaque, client-generated UUIDs. They are never filesystem paths.
pub fn valid_id(id: &str) -> bool {
    id.len() == 36 && id.bytes().enumerate().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { c == b'-' } else { c.is_ascii_hexdigit() })
}

/// Bounded, eager font delivery keeps startup predictable without a separate font service.
pub const MAX_FONT_BYTES: usize = 40 * 1024 * 1024;
pub const MAX_FONT_TOTAL: usize = 128 * 1024 * 1024;
pub const MAX_FONT_FILES: usize = 64;

/// One immutable server font file. Its opaque ID changes when the server restarts;
/// a stale catalog fails with 404 rather than resolving to a different font.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FontFile {
    pub id: String,
    pub families: Vec<String>,
    pub bytes: usize,
}
