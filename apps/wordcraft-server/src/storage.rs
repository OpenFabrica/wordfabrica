//! File storage for third-party documents: original uploads and DOCX exports of explicit saves.
//! One interface over a local directory or a MinIO (S3-compatible) bucket.
use object_store::{ObjectStoreExt, PutPayload, aws::AmazonS3, local::LocalFileSystem, path::Path as Key, signer::Signer};
use std::{path::Path, time::Duration};
use url::Url;

use crate::store::Error;

/// Connection settings for a MinIO / S3-compatible bucket. The bucket must already exist.
#[derive(Clone)]
pub struct MinioSettings {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
}

// Settings get logged at startup; never print credentials.
impl std::fmt::Debug for MinioSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MinIO {}/{} ({})", self.endpoint.trim_end_matches('/'), self.bucket, self.region)
    }
}

/// Where integration files live. Keys are fixed patterns built from validated IDs, never user paths.
#[derive(Debug)]
pub enum Files {
    Local(LocalFileSystem),
    Minio(AmazonS3),
}

impl Files {
    /// A local directory (created if missing). Writes go to a temporary file, then rename.
    pub fn local(root: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(root).map_err(|e| format!("{}: {e}", root.display()))?;
        LocalFileSystem::new_with_prefix(root).map(Self::Local).map_err(|e| format!("{}: {e}", root.display()))
    }

    /// A MinIO bucket addressed path-style (`endpoint/bucket/key`); plain HTTP allowed for local MinIO.
    pub fn minio(settings: &MinioSettings) -> Result<Self, String> {
        object_store::aws::AmazonS3Builder::new()
            .with_endpoint(&settings.endpoint)
            .with_allow_http(settings.endpoint.starts_with("http://"))
            .with_virtual_hosted_style_request(false)
            .with_bucket_name(&settings.bucket)
            .with_access_key_id(&settings.access_key)
            .with_secret_access_key(&settings.secret_key)
            .with_region(&settings.region)
            .build()
            .map(Self::Minio)
            .map_err(|e| format!("MinIO: {e}"))
    }

    /// Write (or overwrite) one object.
    pub async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), Error> {
        let (key, payload) = (Key::from(key), PutPayload::from(bytes));
        let result = match self {
            Self::Local(store) => store.put(&key, payload).await,
            Self::Minio(store) => store.put(&key, payload).await,
        };
        result.map(|_| ()).map_err(storage_error)
    }

    /// Read one object; a missing object is `NotFound`.
    pub async fn get(&self, key: &str) -> Result<Vec<u8>, Error> {
        let key = Key::from(key);
        let result = match self {
            Self::Local(store) => store.get(&key).await,
            Self::Minio(store) => store.get(&key).await,
        };
        let bytes = result.map_err(storage_error)?.bytes().await.map_err(storage_error)?;
        Ok(bytes.to_vec())
    }

    /// A presigned GET URL for MinIO. Local storage returns `None`: the server signs its own link.
    pub async fn presign(&self, key: &str, ttl: Duration) -> Result<Option<Url>, Error> {
        match self {
            Self::Local(_) => Ok(None),
            Self::Minio(store) => store.signed_url(object_store::signer::Method::GET, &Key::from(key), ttl).await.map(Some).map_err(storage_error),
        }
    }
}

fn storage_error(error: object_store::Error) -> Error {
    if matches!(error, object_store::Error::NotFound { .. }) {
        return Error::NotFound;
    }
    eprintln!("file storage error: {error}");
    Error::Storage
}

/// Object key of a third party's original upload.
pub fn original_key(id: &str) -> String {
    format!("originals/{id}.docx")
}

/// Object key of the DOCX exported for one explicitly saved revision.
pub fn saved_key(id: &str, revision: u64) -> String {
    format!("saved/{id}/{revision}.docx")
}
