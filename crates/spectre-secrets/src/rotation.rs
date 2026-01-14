use crate::types::Secret;
use crate::events::SecretEvent;
use anyhow::Result;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RotationPolicy {
    Manual,
    TimeBased { duration: Duration },
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self::TimeBased {
            duration: Duration::days(30),
        }
    }
}

pub struct RotationEngine;

impl RotationEngine {
    pub fn new() -> Self {
        Self
    }

    /// Check if a secret should be rotated based on the provided policy
    pub fn should_rotate(&self, secret: &Secret, policy: &RotationPolicy) -> bool {
        match policy {
            RotationPolicy::Manual => false, // Only manual trigger
            RotationPolicy::TimeBased { duration } => {
                let age = Utc::now() - secret.metadata.updated_at;
                age >= *duration
            }
        }
    }

    /// Rotate a secret: increment version, update timestamps, and return event
    /// Note: This does not generate new ciphertext yet (requires crypto engine integration)
    pub fn rotate(&self, secret: &mut Secret) -> Result<SecretEvent> {
        let old_version = secret.version;
        let new_version = old_version + 1;

        secret.version = new_version;
        secret.metadata.updated_at = Utc::now();
        // Reset expiration if needed, for now we just verify rotation updates metadata

        Ok(SecretEvent::Rotated {
            secret_id: secret.id.clone(),
            old_version,
            new_version,
        })
    }
}
