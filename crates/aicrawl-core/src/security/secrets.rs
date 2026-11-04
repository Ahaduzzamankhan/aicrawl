//! API key generation and hashing.
//!
//! AICrawl stores only a SHA-256 hash of each API key. There is no encryption
//! key, no provider credential and therefore no secret material at rest beyond
//! the database hash.

use sha2::{Digest, Sha256};

use crate::error::{AicrawlError, Result};

/// Generates a new API key in the form `ac_live_<43 chars>`.
pub fn generate_api_key() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| AicrawlError::Config(format!("OS random source unavailable: {error}")))?;
    let body = encode_url_safe(&bytes);
    Ok(format!("ac_live_{body}"))
}

fn encode_url_safe(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 4);
    for chunk in bytes.chunks(3) {
        let mut buffer = [0u8; 3];
        buffer[..chunk.len()].copy_from_slice(chunk);
        let value = ((buffer[0] as u32) << 16) | ((buffer[1] as u32) << 8) | buffer[2] as u32;
        let indexes = [
            (value >> 18) & 0x3f,
            (value >> 12) & 0x3f,
            (value >> 6) & 0x3f,
            value & 0x3f,
        ];
        let take = match chunk.len() {
            1 => 2,
            2 => 3,
            _ => 4,
        };
        for index in &indexes[..take] {
            out.push(ALPHABET[*index as usize] as char);
        }
    }
    out
}

/// SHA-256 hash used to store API keys without their plaintext.
pub fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Display prefix that identifies a key without revealing it.
pub fn key_prefix(key: &str) -> String {
    let visible: String = key.chars().take(12).collect();
    format!("{visible}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_keys_are_unique_and_hash_stable() {
        let a = generate_api_key().unwrap();
        let b = generate_api_key().unwrap();
        assert_ne!(a, b);
        assert!(a.starts_with("ac_live_"));
        assert_eq!(a.len(), "ac_live_".len() + 43);
        assert!(a
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'));
        assert_eq!(hash_api_key(&a), hash_api_key(&a));
        assert_ne!(hash_api_key(&a), hash_api_key(&b));
        assert_eq!(hash_api_key(&a).len(), 64);
        assert!(key_prefix(&a).ends_with('…'));
    }
}
