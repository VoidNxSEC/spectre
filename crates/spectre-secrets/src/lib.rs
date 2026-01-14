//! SPECTRE Secrets - Secret storage and rotation engine

pub mod storage;
pub mod rotation;
pub mod crypto;
pub mod types;
pub mod events;

pub use types::Secret;
pub use storage::SecretStorage;
pub use rotation::{RotationEngine, RotationPolicy};

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    #[tokio::test]
    async fn test_secret_flow() {
        use crate::storage::{InMemoryStorage, SecretStorage};
        use crate::crypto::CryptoEngine;
        use crate::types::{Secret, SecretId, SecretMetadata};

        // Setup
        let storage = InMemoryStorage::new();
        // In real usage, salt should be consistent/managed
        let crypto = CryptoEngine::new("test-password", b"test-salt").unwrap();
        
        let secret_id = SecretId(Uuid::new_v4());
        let plaintext = b"super-secret-data";

        // Encrypt
        let ciphertext = crypto.encrypt(plaintext).unwrap();

        // Create Secret
        let secret = Secret {
            id: secret_id.clone(),
            version: 1,
            algorithm: "AES-GCM".to_string(),
            ciphertext,
            metadata: SecretMetadata {
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                expires_at: None,
                rotation_policy_id: None,
            },
        };

        // Store
        storage.store(&secret).await.unwrap();

        // Retrieve
        let retrieved = storage.retrieve(&secret_id).await.unwrap();
        assert_eq!(retrieved.id, secret_id);

        // Decrypt
        let decrypted = crypto.decrypt(&retrieved.ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
