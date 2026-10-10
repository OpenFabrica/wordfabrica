//! SQLite is embedded in the process: one transaction compares and replaces a snapshot.
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use wordcraft_remote::{MAX_BYTES, MAX_PLUGIN_BYTES, Metadata, Snapshot, TagCategory, WriteRequest, valid_id};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid document ID, title or snapshot")]
    Invalid,
    #[error("document not found")]
    NotFound,
    #[error("another editor saved a newer version; save a copy or reload")]
    Conflict,
    #[error("document exceeds the 16 MiB limit")]
    TooLarge,
    #[error("storage temporarily unavailable")]
    Storage,
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        eprintln!("storage error: {error}");
        Self::Storage
    }
}

pub struct Store(Mutex<Connection>);

impl Store {
    /// Open a durable store. SQLite serializes writers, including other server processes.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
             CREATE TABLE IF NOT EXISTS documents (
               id TEXT PRIMARY KEY, title TEXT NOT NULL, revision INTEGER NOT NULL,
               updated_at INTEGER NOT NULL, content BLOB NOT NULL
             );
             CREATE TABLE IF NOT EXISTS plugin_tags (
               id TEXT PRIMARY KEY, revision INTEGER NOT NULL,
               updated_at INTEGER NOT NULL, categories BLOB NOT NULL
             );",
        )?;
        Ok(Self(Mutex::new(connection)))
    }

    pub fn list(&self) -> Result<Vec<Metadata>, Error> {
        let connection = self.0.lock().map_err(|_| Error::Storage)?;
        let mut statement = connection.prepare("SELECT id,title,revision,updated_at FROM documents ORDER BY updated_at DESC, id")?;
        let rows = statement.query_map([], |row| {
            Ok(Metadata { id: row.get(0)?, title: row.get(1)?, revision: row.get::<_, i64>(2)? as u64, updated_at: row.get::<_, i64>(3)? as u64 })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get(&self, id: &str) -> Result<Snapshot, Error> {
        if !valid_id(id) {
            return Err(Error::Invalid);
        }
        let connection = self.0.lock().map_err(|_| Error::Storage)?;
        let row = connection
            .query_row("SELECT title,revision,updated_at,content FROM documents WHERE id=?1", [id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64, row.get::<_, i64>(2)? as u64, row.get::<_, Vec<u8>>(3)?))
            })
            .optional()?
            .ok_or(Error::NotFound)?;
        let document = serde_json::from_slice(&row.3).map_err(|_| Error::Storage)?;
        Ok(Snapshot { metadata: Metadata { id: id.into(), title: row.0, revision: row.1, updated_at: row.2 }, document })
    }

    /// Compare-and-swap. Failure leaves the previous snapshot intact; acknowledgement follows commit.
    pub fn put(&self, id: &str, mut request: WriteRequest) -> Result<Metadata, Error> {
        let title = request.title.trim();
        if !valid_id(id) || title.is_empty() || title.chars().count() > 200 || title.chars().any(char::is_control) {
            return Err(Error::Invalid);
        }
        let revision = request.revision.checked_add(1).filter(|r| *r <= i64::MAX as u64).ok_or(Error::Invalid)?;
        request.document.ensure_nonempty();
        let content = serde_json::to_vec(&request.document).map_err(|_| Error::Invalid)?;
        if content.len() > MAX_BYTES {
            return Err(Error::TooLarge);
        }
        let updated_at = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| Error::Storage)?.as_millis().min(i64::MAX as u128) as u64;
        let mut connection = self.0.lock().map_err(|_| Error::Storage)?;
        let transaction = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current: Option<i64> = transaction.query_row("SELECT revision FROM documents WHERE id=?1", [id], |row| row.get(0)).optional()?;
        if current.unwrap_or(0) as u64 != request.revision {
            // A reply may be lost after commit. Retrying the identical snapshot is idempotent.
            if current == Some(revision as i64) {
                let stored = transaction.query_row("SELECT title,content,updated_at FROM documents WHERE id=?1", [id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?, row.get::<_, i64>(2)?))
                })?;
                if stored.0 == title && stored.1 == content {
                    return Ok(Metadata { id: id.into(), title: title.into(), revision, updated_at: stored.2 as u64 });
                }
            }
            return Err(Error::Conflict);
        }
        transaction.execute(
            "INSERT INTO documents(id,title,revision,updated_at,content) VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT(id) DO UPDATE SET title=excluded.title,revision=excluded.revision,updated_at=excluded.updated_at,content=excluded.content",
            params![id, title, revision as i64, updated_at as i64, content],
        )?;
        transaction.commit()?;
        Ok(Metadata { id: id.into(), title: title.into(), revision, updated_at })
    }

    /// Users' version of an editable plugin's tags: `(revision, categories)`, or `None` while
    /// nobody has changed the file's tags.
    pub fn plugin_tags(&self, id: &str) -> Result<Option<(u64, Vec<TagCategory>)>, Error> {
        let connection = self.0.lock().map_err(|_| Error::Storage)?;
        let row = connection
            .query_row("SELECT revision,categories FROM plugin_tags WHERE id=?1", [id], |row| {
                Ok((row.get::<_, i64>(0)? as u64, row.get::<_, Vec<u8>>(1)?))
            })
            .optional()?;
        row.map(|(revision, bytes)| serde_json::from_slice(&bytes).map(|c| (revision, c)).map_err(|_| Error::Storage)).transpose()
    }

    /// Compare-and-swap an editable plugin's tags (the caller validates them). `revision` is the
    /// version the edit started from (0 = the file's tags). Returns the new revision.
    pub fn put_plugin_tags(&self, id: &str, revision: u64, categories: &[TagCategory]) -> Result<u64, Error> {
        let next = revision.checked_add(1).filter(|r| *r <= i64::MAX as u64).ok_or(Error::Invalid)?;
        let content = serde_json::to_vec(categories).map_err(|_| Error::Invalid)?;
        if content.len() > MAX_PLUGIN_BYTES {
            return Err(Error::TooLarge);
        }
        let updated_at = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| Error::Storage)?.as_millis().min(i64::MAX as u128) as u64;
        let mut connection = self.0.lock().map_err(|_| Error::Storage)?;
        let transaction = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current: Option<i64> = transaction.query_row("SELECT revision FROM plugin_tags WHERE id=?1", [id], |row| row.get(0)).optional()?;
        if current.unwrap_or(0) as u64 != revision {
            return Err(Error::Conflict);
        }
        transaction.execute(
            "INSERT INTO plugin_tags(id,revision,updated_at,categories) VALUES (?1,?2,?3,?4)
             ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,updated_at=excluded.updated_at,categories=excluded.categories",
            params![id, next as i64, updated_at as i64, content],
        )?;
        transaction.commit()?;
        Ok(next)
    }
}
