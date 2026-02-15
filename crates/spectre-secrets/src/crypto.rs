use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use argon2::Argon2;
use rand::rngs::OsRng;
use rand::RngCore;

/// Salt length in bytes (128-bit)
const SALT_LEN: usize = 16;

/// Generate a cryptographically secure random salt
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    salt
}

pub struct CryptoEngine {
    key: Key<Aes256Gcm>,
}

impl CryptoEngine {
    /// Derive a 256-bit key from password and salt using Argon2id
    pub fn new(password: &str, salt: &[u8]) -> Result<Self> {
        if salt.len() < 8 {
            return Err(anyhow!(
                "Salt must be at least 8 bytes (got {})",
                salt.len()
            ));
        }

        let mut key_bytes = [0u8; 32];

        // Argon2id with default parameters (19 MiB memory, 2 iterations, 1 parallelism)
        let argon2 = Argon2::default();
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key_bytes)
            .map_err(|e| anyhow!("Argon2id key derivation failed: {}", e))?;

        Ok(Self {
            key: *Key::<Aes256Gcm>::from_slice(&key_bytes),
        })
    }

    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new(&self.key);
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, data)
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 12 {
            return Err(anyhow!("Data too short"));
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let cipher = Aes256Gcm::new(&self.key);

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("Decryption failed: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let salt = generate_salt();
        let engine = CryptoEngine::new("strong-password-123!", &salt).unwrap();
        let plaintext = b"hello world secret data";

        let ciphertext = engine.encrypt(plaintext).unwrap();
        let decrypted = engine.decrypt(&ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_password_fails() {
        let salt = generate_salt();
        let engine1 = CryptoEngine::new("correct-password", &salt).unwrap();
        let engine2 = CryptoEngine::new("wrong-password", &salt).unwrap();

        let ciphertext = engine1.encrypt(b"secret").unwrap();
        let result = engine2.decrypt(&ciphertext);

        assert!(result.is_err());
    }

    #[test]
    fn test_different_salts_produce_different_keys() {
        let salt1 = generate_salt();
        let salt2 = generate_salt();
        let engine1 = CryptoEngine::new("same-password", &salt1).unwrap();
        let engine2 = CryptoEngine::new("same-password", &salt2).unwrap();

        let ciphertext = engine1.encrypt(b"data").unwrap();
        let result = engine2.decrypt(&ciphertext);

        assert!(result.is_err());
    }

    #[test]
    fn test_empty_salt_rejected() {
        let result = CryptoEngine::new("password", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_short_salt_rejected() {
        let result = CryptoEngine::new("password", &[1, 2, 3, 4, 5, 6, 7]);
        assert!(result.is_err());
    }

    #[test]
    fn test_short_data_decrypt_fails() {
        let salt = generate_salt();
        let engine = CryptoEngine::new("password", &salt).unwrap();
        let result = engine.decrypt(&[0u8; 5]);
        assert!(result.is_err());
    }
}
