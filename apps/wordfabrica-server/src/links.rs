//! Per-user access links for third-party documents. The third party decides who may edit or
//! view and asks for a link; the link carries a signed, expiring token that the browser sends
//! back on every document request (`x-wordfabrica-token`). No server-side sessions or cookies,
//! so links also work inside the third party's iframe.
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use crate::signing;

/// Header the editor sends the token in.
pub const TOKEN_HEADER: &str = "x-wordfabrica-token";
/// Default and maximum link lifetimes (seconds). The token is checked on every request, so this
/// is how long an opened editor keeps working, not only how long the link can be opened.
pub const DEFAULT_TTL: u64 = 12 * 3600;
pub const MAX_TTL: u64 = 7 * 24 * 3600;
const MAX_TOKEN_LEN: usize = 2048;

/// What a link allows.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Edit,
    View,
}

/// Signed token contents.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Claims {
    /// Document id the token is for.
    pub doc: String,
    pub user_id: String,
    pub user_name: String,
    pub mode: Mode,
    /// Unix seconds.
    pub exp: u64,
}

/// `base64url(json).hex(hmac("link." + base64url(json)))`. The "link" prefix keeps these
/// signatures distinct from request and callback signatures made with the same secret.
pub fn issue(secret: &[u8], claims: &Claims) -> String {
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).unwrap_or_default());
    let signature = signing::sign(secret, &[b"link", payload.as_bytes()]);
    format!("{payload}.{signature}")
}

/// The claims of a valid, unexpired token for document `doc`; `None` otherwise.
pub fn verify(secret: &[u8], token: &str, doc: &str, now: u64) -> Option<Claims> {
    if token.len() > MAX_TOKEN_LEN {
        return None;
    }
    let (payload, signature) = token.split_once('.')?;
    if !signing::verify(secret, &[b"link", payload.as_bytes()], signature) {
        return None;
    }
    let claims: Claims = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    (claims.doc == doc && claims.exp >= now).then_some(claims)
}
