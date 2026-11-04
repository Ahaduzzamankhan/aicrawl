//! Local secret storage.
//!
//! API keys for AI providers are encrypted at rest with ChaCha20-Poly1305. The
//! encryption key lives in `<data_dir>/secret.key` (created with owner-only
//! permissions) unless `AICRAWL_SECRET_KEY` supplies a base64-encoded 32-byte key.
//! AICrawl never writes provider keys to the config file or to logs.

use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use sha2::{Digest, Sha256};

use crate::error::{AicrawlError, Result};

const PREFIX: &str = "v1:";
const NONCE_LEN: usize = 12;

/// Encrypts provider credentials and generates/hashes API keys.
#[derive(Clone)]
pub struct SecretStore {
    key: [u8; 32],
    key_path: PathBuf,
}

impl std::fmt::Debug for SecretStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretStore")
            .field("key_path", &self.key_path)
            .field("key", &"<redacted>")
            .finish()
    }
}

impl SecretStore {
    /// Opens the store, creating a key file when needed.
    pub fn open(data_dir: &Path) -> Result<Self> {
        fs::create_dir_all(data_dir)?;
        let key_path = data_dir.join("secret.key");

        if let Ok(encoded) = std::env::var("AICRAWL_SECRET_KEY") {
            let decoded = B64.decode(encoded.trim()).map_err(|_| {
                AicrawlError::Config("AICRAWL_SECRET_KEY is not valid base64".into())
            })?;
            if decoded.len() != 32 {
                return Err(AicrawlError::Config(
                    "AICRAWL_SECRET_KEY must decode to exactly 32 bytes".into(),
                ));
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&decoded);
            return Ok(Self { key, key_path });
        }

        if key_path.exists() {
            let raw = fs::read_to_string(&key_path)?;
            let decoded = B64.decode(raw.trim()).map_err(|_| {
                AicrawlError::Config(format!(
                    "{} is corrupt; delete it to regenerate (existing encrypted keys become unreadable)",
                    key_path.display()
                ))
            })?;
            if decoded.len() != 32 {
                return Err(AicrawlError::Config(format!(
                    "{} must contain a base64-encoded 32-byte key",
                    key_path.display()
                )));
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&decoded);
            return Ok(Self { key, key_path });
        }

        let mut key = [0u8; 32];
        getrandom::getrandom(&mut key)
            .map_err(|e| AicrawlError::Config(format!("OS random source unavailable: {e}")))?;
        fs::write(&key_path, B64.encode(key))?;
        restrict_permissions(&key_path)?;
        Ok(Self { key, key_path })
    }

    /// Location of the encryption key (for diagnostics only).
    pub fn key_path(&self) -> &Path {
        &self.key_path
    }

    /// Encrypts a provider API key. Returns a `v1:<base64>` string.
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let cipher = ChaCha20Poly1305::new_from_slice(&self.key)
            .map_err(|_| AicrawlError::Config("invalid encryption key".into()))?;
        let mut nonce_bytes = [0u8; NONCE_LEN];
        getrandom::getrandom(&mut nonce_bytes)
            .map_err(|e| AicrawlError::Config(format!("OS random source unavailable: {e}")))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| AicrawlError::Config("encryption failed".into()))?;
        let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);
        Ok(format!("{PREFIX}{}", B64.encode(combined)))
    }

    /// Decrypts a `v1:` value produced by [`SecretStore::encrypt`].
    pub fn decrypt(&self, encoded: &str) -> Result<String> {
        let body = encoded
            .strip_prefix(PREFIX)
            .ok_or_else(|| AicrawlError::Config("unsupported secret encoding".into()))?;
        let combined = B64
            .decode(body)
            .map_err(|_| AicrawlError::Config("secret is not valid base64".into()))?;
        if combined.len() <= NONCE_LEN {
            return Err(AicrawlError::Config("secret payload is truncated".into()));
        }
        let cipher = ChaCha20Poly1305::new_from_slice(&self.key)
            .map_err(|_| AicrawlError::Config("invalid encryption key".into()))?;
        let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
            .map_err(|_| {
                AicrawlError::Config(
                    "cannot decrypt stored secret: key changed or data corrupted".into(),
                )
            })?;
        String::from_utf8(plaintext)
            .map_err(|_| AicrawlError::Config("decrypted secret is not UTF-8".into()))
    }

    /// Generates a new API key in the form `ac_live_<43 chars>`.
    pub fn generate_api_key(&self) -> Result<String> {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes)
            .map_err(|e| AicrawlError::Config(format!("OS random source unavailable: {e}")))?;
        let body = B64
            .encode(bytes)
            .replace('+', "-")
            .replace('/', "_")
            .replace('=', "");
        Ok(format!("ac_live_{body}"))
    }
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

fn restrict_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms)?;
    }
    #[cfg(not(unix))]
    {
        // Windows ACLs inherit from the user profile directory. The key is still
        // encrypted-at-rest data, never plaintext provider credentials.
        let _ = path;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SecretStore::open(dir.path()).unwrap();
        let secret = "sk-test-123456";
        let encrypted = store.encrypt(secret).unwrap();
        assert!(encrypted.starts_with("v1:"));
        assert!(!encrypted.contains(secret));
        assert_eq!(store.decrypt(&encrypted).unwrap(), secret);

        // A second open with the same key file must decrypt the same value.
        let reopened = SecretStore::open(dir.path()).unwrap();
        assert_eq!(reopened.decrypt(&encrypted).unwrap(), secret);
    }

    #[test]
    fn decrypt_rejects_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let store = SecretStore::open(dir.path()).unwrap();
        let encrypted = store.encrypt("value").unwrap();
        let mut bytes = B64.decode(encrypted.trim_start_matches("v1:")).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        let tampered = format!("v1:{}", B64.encode(bytes));
        assert!(store.decrypt(&tampered).is_err());
    }

    #[test]
    fn api_keys_are_unique_and_hash_stable() {
        let dir = tempfile::tempdir().unwrap();
        let store = SecretStore::open(dir.path()).unwrap();
        let a = store.generate_api_key().unwrap();
        let b = store.generate_api_key().unwrap();
        assert_ne!(a, b);
        assert!(a.starts_with("ac_live_"));
        assert_eq!(hash_api_key(&a), hash_api_key(&a));
        assert_ne!(hash_api_key(&a), hash_api_key(&b));
        assert!(key_prefix(&a).ends_with('…'));
    }
}
