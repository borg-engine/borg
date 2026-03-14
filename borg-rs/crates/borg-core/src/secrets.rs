use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::Engine;
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;

use crate::traits::SecretStore;

const ENC_PREFIX: &str = "enc1:";
const NONCE_LEN: usize = 12;

// ── PlaintextStore ──────────────────────────────────────────────────────

pub struct PlaintextStore;

#[async_trait]
impl SecretStore for PlaintextStore {
    fn encrypt(&self, plaintext: &str) -> Result<String> {
        Ok(plaintext.to_string())
    }

    fn decrypt(&self, ciphertext: &str) -> Result<String> {
        Ok(ciphertext.to_string())
    }

    async fn store(&self, _key: &str, _secret: &str) -> Result<()> {
        Ok(())
    }

    async fn retrieve(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }
}

// ── EncryptedStore ──────────────────────────────────────────────────────

pub struct EncryptedStore {
    cipher: ChaCha20Poly1305,
}

impl EncryptedStore {
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: ChaCha20Poly1305::new(key.into()),
        }
    }
}

#[async_trait]
impl SecretStore for EncryptedStore {
    fn encrypt(&self, plaintext: &str) -> Result<String> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;

        let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        let encoded = base64::engine::general_purpose::STANDARD.encode(&combined);
        Ok(format!("{ENC_PREFIX}{encoded}"))
    }

    fn decrypt(&self, ciphertext: &str) -> Result<String> {
        let Some(encoded) = ciphertext.strip_prefix(ENC_PREFIX) else {
            return Ok(ciphertext.to_string());
        };

        let combined = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .context("base64 decode failed")?;

        if combined.len() < NONCE_LEN {
            anyhow::bail!("ciphertext too short");
        }

        let (nonce_bytes, ct) = combined.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ct)
            .map_err(|e| anyhow::anyhow!("decryption failed: {e}"))?;

        String::from_utf8(plaintext).context("decrypted data is not valid UTF-8")
    }

    async fn store(&self, _key: &str, _secret: &str) -> Result<()> {
        Ok(())
    }

    async fn retrieve(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }
}

// ── SecretKeyManager ────────────────────────────────────────────────────

pub struct SecretKeyManager;

impl SecretKeyManager {
    pub fn load_or_generate(data_dir: &str) -> Result<[u8; 32]> {
        let path = std::path::Path::new(data_dir).join(".secret_key");

        if path.exists() {
            let bytes = std::fs::read(&path).context("failed to read secret key file")?;
            if bytes.len() != 32 {
                anyhow::bail!(
                    "secret key file has wrong length: expected 32, got {}",
                    bytes.len()
                );
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }

        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("failed to create data directory")?;
        }

        std::fs::write(&path, &key).context("failed to write secret key file")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .context("failed to set key file permissions")?;
        }

        Ok(key)
    }
}

// ── Migration Helper ────────────────────────────────────────────────────

pub fn migrate_plaintext_secrets(store: &EncryptedStore, values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|v| {
            if v.starts_with(ENC_PREFIX) {
                v.clone()
            } else {
                store.encrypt(v).unwrap_or_else(|_| v.clone())
            }
        })
        .collect()
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        for (i, b) in key.iter_mut().enumerate() {
            *b = i as u8;
        }
        key
    }

    #[test]
    fn round_trip() {
        let store = EncryptedStore::new(&test_key());
        let original = "sk-ant-api03-secret-key-value";
        let encrypted = store.encrypt(original).unwrap();
        assert!(encrypted.starts_with(ENC_PREFIX));
        assert_ne!(encrypted, original);
        let decrypted = store.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn plaintext_passthrough() {
        let store = EncryptedStore::new(&test_key());
        let plain = "some-old-unencrypted-api-key";
        let result = store.decrypt(plain).unwrap();
        assert_eq!(result, plain);
    }

    #[test]
    fn different_nonces() {
        let store = EncryptedStore::new(&test_key());
        let original = "same-plaintext";
        let enc1 = store.encrypt(original).unwrap();
        let enc2 = store.encrypt(original).unwrap();
        assert_ne!(enc1, enc2);
        assert_eq!(store.decrypt(&enc1).unwrap(), original);
        assert_eq!(store.decrypt(&enc2).unwrap(), original);
    }

    #[test]
    fn key_file_creation_and_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().to_str().unwrap();

        let key1 = SecretKeyManager::load_or_generate(data_dir).unwrap();
        assert_eq!(key1.len(), 32);

        let key_path = dir.path().join(".secret_key");
        assert!(key_path.exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::metadata(&key_path).unwrap().permissions();
            assert_eq!(perms.mode() & 0o777, 0o600);
        }

        let key2 = SecretKeyManager::load_or_generate(data_dir).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn migrate_plaintext() {
        let store = EncryptedStore::new(&test_key());
        let already_encrypted = store.encrypt("secret1").unwrap();
        let values = vec![
            "plaintext-key".to_string(),
            already_encrypted.clone(),
            "another-plain".to_string(),
        ];
        let migrated = migrate_plaintext_secrets(&store, &values);
        assert!(migrated[0].starts_with(ENC_PREFIX));
        assert_eq!(migrated[1], already_encrypted);
        assert!(migrated[2].starts_with(ENC_PREFIX));
        assert_eq!(store.decrypt(&migrated[0]).unwrap(), "plaintext-key");
        assert_eq!(store.decrypt(&migrated[2]).unwrap(), "another-plain");
    }

    #[test]
    fn plaintext_store_identity() {
        let store = PlaintextStore;
        let val = "my-api-key";
        assert_eq!(store.encrypt(val).unwrap(), val);
        assert_eq!(store.decrypt(val).unwrap(), val);
    }
}
