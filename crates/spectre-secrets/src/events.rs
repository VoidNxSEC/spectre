use crate::types::SecretId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecretEvent {
    Rotated {
        secret_id: SecretId,
        old_version: u32,
        new_version: u32,
    },
    Expired {
        secret_id: SecretId,
    },
}
