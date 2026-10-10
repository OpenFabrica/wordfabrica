//! HMAC-SHA256 signatures shared with the third party: requests to us, callbacks from us,
//! and the server's own expiring download links for local storage.
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const TIMESTAMP_HEADER: &str = "x-wordfabrica-timestamp";
pub const SIGNATURE_HEADER: &str = "x-wordfabrica-signature";
/// Accepted clock difference for signed requests; bounds how long a captured request can be replayed.
pub const MAX_SKEW_SECS: u64 = 300;

/// Lowercase hex HMAC-SHA256 of `parts` joined by `.`.
pub fn sign(secret: &[u8], parts: &[&[u8]]) -> String {
    match mac(secret, parts) {
        Some(mac) => hex::encode(mac.finalize().into_bytes()),
        None => String::new(),
    }
}

/// Constant-time check of a hex signature over `parts` joined by `.`.
pub fn verify(secret: &[u8], parts: &[&[u8]], signature: &str) -> bool {
    let Ok(expected) = hex::decode(signature.trim()) else { return false };
    mac(secret, parts).is_some_and(|mac| mac.verify_slice(&expected).is_ok())
}

fn mac(secret: &[u8], parts: &[&[u8]]) -> Option<Hmac<Sha256>> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).ok()?;
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            mac.update(b".");
        }
        mac.update(part);
    }
    Some(mac)
}

/// Seconds since the Unix epoch (0 if the clock is before 1970).
pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}
