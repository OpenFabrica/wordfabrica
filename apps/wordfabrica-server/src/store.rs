//! SQLite is embedded in the process: one transaction compares and replaces a snapshot.
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use wordfabrica_doc::Document;
use wordfabrica_remote::{MAX_BYTES, MAX_PLUGIN_BYTES, Metadata, Snapshot, TagCategory, WriteRequest, valid_id};

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
    #[error("missing, expired or invalid signature")]
    Unauthorized,
    #[error("link expired or invalid")]
    Forbidden,
    #[error("this document is open read-only")]
    ReadOnly,
    #[error("{0}")]
    Rejected(&'static str),
    #[error("could not download the document from the given URL")]
    Fetch,
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
             );
             CREATE TABLE IF NOT EXISTS integrations (
               id TEXT PRIMARY KEY, key TEXT NOT NULL, file_name TEXT NOT NULL,
               callback_url TEXT NOT NULL, created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS outbox (
               seq INTEGER PRIMARY KEY AUTOINCREMENT, doc_id TEXT NOT NULL, revision INTEGER NOT NULL,
               title TEXT NOT NULL, saved_at INTEGER NOT NULL, user_id TEXT NOT NULL, user_name TEXT NOT NULL, state TEXT NOT NULL,
               attempts INTEGER NOT NULL DEFAULT 0, next_at INTEGER NOT NULL, last_error TEXT,
               UNIQUE(doc_id, revision)
             );",
        )?;
        Ok(Self(Mutex::new(connection)))
    }

    pub fn list(&self) -> Result<Vec<Metadata>, Error> {
        let connection = self.0.lock().map_err(|_| Error::Storage)?;
        // Third-party documents are reachable only through their access links, never listed.
        let mut statement = connection.prepare(
            "SELECT id,title,revision,updated_at FROM documents WHERE id NOT IN (SELECT id FROM integrations) ORDER BY updated_at DESC, id",
        )?;
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
        Ok(Snapshot { metadata: Metadata { id: id.into(), title: row.0, revision: row.1, updated_at: row.2 }, document, read_only: false })
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
        let updated_at = now_millis()?;
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
        let updated_at = now_millis()?;
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

    /// Create a third party's document at revision 1 together with its integration record,
    /// in one transaction so neither exists without the other.
    pub fn create_integration(&self, id: &str, title: &str, mut document: Document, info: &Integration) -> Result<Metadata, Error> {
        let title = title.trim();
        if !valid_id(id) || title.is_empty() || title.chars().count() > 200 || title.chars().any(char::is_control) {
            return Err(Error::Invalid);
        }
        document.ensure_nonempty();
        let content = serde_json::to_vec(&document).map_err(|_| Error::Invalid)?;
        if content.len() > MAX_BYTES {
            return Err(Error::TooLarge);
        }
        let updated_at = now_millis()?;
        let mut connection = self.0.lock().map_err(|_| Error::Storage)?;
        let transaction = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO documents(id,title,revision,updated_at,content) VALUES (?1,?2,1,?3,?4)",
            params![id, title, updated_at as i64, content],
        )?;
        transaction.execute(
            "INSERT INTO integrations(id,key,file_name,callback_url,created_at) VALUES (?1,?2,?3,?4,?5)",
            params![id, info.key, info.file_name, info.callback_url, updated_at as i64],
        )?;
        transaction.commit()?;
        Ok(Metadata { id: id.into(), title: title.into(), revision: 1, updated_at })
    }

    /// The integration record of a document, or `None` for the editor's own documents.
    pub fn integration(&self, id: &str) -> Result<Option<Integration>, Error> {
        if !valid_id(id) {
            return Ok(None);
        }
        let connection = self.0.lock().map_err(|_| Error::Storage)?;
        connection
            .query_row("SELECT key,file_name,callback_url FROM integrations WHERE id=?1", [id], |row| {
                Ok(Integration { key: row.get(0)?, file_name: row.get(1)?, callback_url: row.get(2)? })
            })
            .optional()
            .map_err(Into::into)
    }

    /// Queue a callback for an explicitly saved revision. Retrying the same revision is a no-op,
    /// an older revision never overtakes a newer one, and older undelivered callbacks of the
    /// same document are superseded: the third party only needs the latest file.
    pub fn enqueue(&self, id: &str, revision: u64, title: &str, saved_at: u64, user: (&str, &str)) -> Result<(), Error> {
        let mut connection = self.0.lock().map_err(|_| Error::Storage)?;
        let transaction = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT OR IGNORE INTO outbox(doc_id,revision,title,saved_at,user_id,user_name,state,next_at)
             SELECT ?1,?2,?3,?4,?5,?6,'pending',0 WHERE NOT EXISTS (SELECT 1 FROM outbox WHERE doc_id=?1 AND revision>?2)",
            params![id, revision as i64, title, saved_at as i64, user.0, user.1],
        )?;
        transaction
            .execute("UPDATE outbox SET state='superseded' WHERE doc_id=?1 AND state='pending' AND revision<?2", params![id, revision as i64])?;
        transaction.commit()?;
        Ok(())
    }

    /// Pending callbacks whose retry time has come, oldest first.
    pub fn due_callbacks(&self, now: u64, limit: usize) -> Result<Vec<Callback>, Error> {
        let connection = self.0.lock().map_err(|_| Error::Storage)?;
        let mut statement = connection.prepare(
            "SELECT o.seq,o.doc_id,o.revision,o.title,o.saved_at,o.attempts,i.key,i.file_name,i.callback_url,o.user_id,o.user_name
             FROM outbox o JOIN integrations i ON i.id=o.doc_id
             WHERE o.state='pending' AND o.next_at<=?1 ORDER BY o.seq LIMIT ?2",
        )?;
        let rows = statement.query_map(params![now.min(i64::MAX as u64) as i64, limit.min(i64::MAX as usize) as i64], |row| {
            Ok(Callback {
                seq: row.get(0)?,
                id: row.get(1)?,
                revision: row.get::<_, i64>(2)? as u64,
                title: row.get(3)?,
                saved_at: row.get::<_, i64>(4)? as u64,
                attempts: row.get::<_, i64>(5)? as u32,
                info: Integration { key: row.get(6)?, file_name: row.get(7)?, callback_url: row.get(8)? },
                user_id: row.get(9)?,
                user_name: row.get(10)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Record a callback attempt: delivered, retry at `next_at` (ms), or given up (`next_at` = `None`).
    pub fn finish_callback(&self, seq: i64, delivered: bool, next_at: Option<u64>, error: Option<&str>) -> Result<(), Error> {
        let state = match (delivered, next_at) {
            (true, _) => "done",
            (false, Some(_)) => "pending",
            (false, None) => "failed",
        };
        let connection = self.0.lock().map_err(|_| Error::Storage)?;
        connection.execute(
            "UPDATE outbox SET state=?2, attempts=attempts+1, next_at=?3, last_error=?4 WHERE seq=?1 AND state='pending'",
            params![seq, state, next_at.unwrap_or(0).min(i64::MAX as u64) as i64, error],
        )?;
        Ok(())
    }

    /// Current revision, latest explicitly saved revision and its callback state, for status queries.
    pub fn integration_status(&self, id: &str) -> Result<(u64, Option<(u64, String)>), Error> {
        let connection = self.0.lock().map_err(|_| Error::Storage)?;
        let revision: i64 =
            connection.query_row("SELECT revision FROM documents WHERE id=?1", [id], |row| row.get(0)).optional()?.ok_or(Error::NotFound)?;
        let saved = connection
            .query_row("SELECT revision,state FROM outbox WHERE doc_id=?1 AND state!='superseded' ORDER BY revision DESC LIMIT 1", [id], |row| {
                Ok((row.get::<_, i64>(0)? as u64, row.get::<_, String>(1)?))
            })
            .optional()?;
        Ok((revision as u64, saved))
    }
}

/// How a third party's document is delivered back: their key, the file name and the callback URL.
#[derive(Clone, Debug, PartialEq)]
pub struct Integration {
    pub key: String,
    pub file_name: String,
    pub callback_url: String,
}

/// One queued callback, joined with its integration record.
#[derive(Clone, Debug)]
pub struct Callback {
    pub seq: i64,
    pub id: String,
    pub revision: u64,
    pub title: String,
    pub saved_at: u64,
    pub attempts: u32,
    pub info: Integration,
    /// Who made the save (from their access link).
    pub user_id: String,
    pub user_name: String,
}

/// Unix milliseconds, clamped to the SQLite integer range.
fn now_millis() -> Result<u64, Error> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| Error::Storage)?.as_millis().min(i64::MAX as u128) as u64)
}
