use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroize;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SecretId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize, Zeroize)]
#[zeroize(drop)]
pub struct Secret {
    #[zeroize(skip)]
    pub id: SecretId,
    #[zeroize(skip)]
    pub version: u32,
    #[zeroize(skip)]
    pub algorithm: String,
    pub ciphertext: Vec<u8>,
    #[zeroize(skip)]
    pub metadata: SecretMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub rotation_policy_id: Option<Uuid>,
}
