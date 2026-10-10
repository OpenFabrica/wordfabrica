//! Delivers queued save callbacks to third parties. The queue lives in SQLite (`outbox`), so a
//! restart resumes delivery; failures retry with exponential backoff and finally give up.
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

use crate::{
    blocking,
    integration::Service,
    signing::{self, SIGNATURE_HEADER, TIMESTAMP_HEADER},
    store::Callback,
};

/// Attempts before a callback is marked `failed`.
pub const MAX_ATTEMPTS: u32 = 10;
/// Upper bound of the backoff between attempts.
const MAX_DELAY: Duration = Duration::from_secs(3600);
/// How often the worker looks for retries that came due without a wake-up.
const POLL: Duration = Duration::from_secs(5);

/// Body of `POST {callback_url}`; signed like requests to us: HMAC(secret, "{timestamp}.{body}").
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Payload {
    pub event: String,
    pub id: String,
    pub key: String,
    pub revision: u64,
    pub title: String,
    pub file_name: String,
    pub url: String,
    pub url_expires_at: u64,
    /// Unix milliseconds of the save.
    pub saved_at: u64,
    /// The user whose access link made the save.
    pub user_id: String,
    pub user_name: String,
}

/// Run forever: deliver due callbacks, then sleep until a save wakes us or the poll interval passes.
pub fn spawn(service: Arc<Service>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            deliver_due(&service).await;
            tokio::select! {
                _ = service.notify.notified() => {}
                _ = tokio::time::sleep(POLL) => {}
            }
        }
    })
}

/// Try every due callback once, oldest first. Returns how many were delivered.
pub async fn deliver_due(service: &Service) -> usize {
    let store = service.store.clone();
    let due = match blocking(move || store.due_callbacks(now_millis(), 32)).await {
        Ok(due) => due,
        Err(error) => {
            eprintln!("callback queue: {error}");
            return 0;
        }
    };
    let mut delivered = 0;
    for callback in due {
        let result = send(service, &callback).await;
        let attempts = callback.attempts.saturating_add(1);
        let next_at = match &result {
            Ok(()) => None,
            Err(_) if attempts >= MAX_ATTEMPTS => None,
            Err(_) => Some(now_millis().saturating_add(backoff(service.retry_base, attempts).as_millis().min(u64::MAX as u128) as u64)),
        };
        let error = result.as_ref().err().cloned();
        if let Some(error) = &error {
            eprintln!("callback {} r{} attempt {attempts}: {error}", callback.id, callback.revision);
        }
        delivered += usize::from(result.is_ok());
        let (store, seq) = (service.store.clone(), callback.seq);
        if let Err(error) = blocking(move || store.finish_callback(seq, error.is_none(), next_at, error.as_deref())).await {
            eprintln!("callback queue: {error}");
        }
    }
    delivered
}

/// `base × 2^(attempts-1)`, capped at an hour.
fn backoff(base: Duration, attempts: u32) -> Duration {
    base.saturating_mul(1u32 << attempts.saturating_sub(1).min(16)).min(MAX_DELAY)
}

/// POST one signed callback. Any 2xx counts as delivered.
async fn send(service: &Service, callback: &Callback) -> Result<(), String> {
    // The allow list may have changed since the document was registered.
    let target = url::Url::parse(&callback.info.callback_url).map_err(|e| e.to_string())?;
    if !service.allowed(&target) {
        return Err("callback_url host is no longer allowed".into());
    }
    // A fresh link per attempt, so its lifetime starts when the third party is told about it.
    let (url, url_expires_at) = service.download_url(&callback.id, callback.revision).await.map_err(|e| e.to_string())?;
    let payload = Payload {
        event: "document.saved".into(),
        id: callback.id.clone(),
        key: callback.info.key.clone(),
        revision: callback.revision,
        title: callback.title.clone(),
        file_name: callback.info.file_name.clone(),
        url,
        url_expires_at,
        saved_at: callback.saved_at,
        user_id: callback.user_id.clone(),
        user_name: callback.user_name.clone(),
    };
    let body = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let timestamp = signing::now_secs().to_string();
    let signature = signing::sign(&service.settings.secret, &[timestamp.as_bytes(), &body]);
    let response = service
        .caller
        .post(target)
        .header("content-type", "application/json")
        .header(TIMESTAMP_HEADER, timestamp)
        .header(SIGNATURE_HEADER, signature)
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if response.status().is_success() { Ok(()) } else { Err(format!("HTTP {}", response.status())) }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis().min(u64::MAX as u128) as u64)
}
