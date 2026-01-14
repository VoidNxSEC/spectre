use crate::types::{Secret, SecretId};
use anyhow::{anyhow, Result};

#[async_trait::async_trait]
pub trait SecretStorage: Send + Sync {
    async fn store(&self, secret: &Secret) -> Result<()>;
    async fn retrieve(&self, id: &SecretId) -> Result<Secret>;
    async fn delete(&self, id: &SecretId) -> Result<()>;
}

use std::collections::HashMap;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct InMemoryStorage {
    secrets: RwLock<HashMap<SecretId, Secret>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl SecretStorage for InMemoryStorage {
    async fn store(&self, secret: &Secret) -> Result<()> {
        let mut secrets = self.secrets.write().await;
        secrets.insert(secret.id.clone(), secret.clone());
        Ok(())
    }

    async fn retrieve(&self, id: &SecretId) -> Result<Secret> {
        let secrets = self.secrets.read().await;
        secrets.get(id)
            .cloned()
            .ok_or_else(|| anyhow!("Secret not found"))
    }

    async fn delete(&self, id: &SecretId) -> Result<()> {
        let mut secrets = self.secrets.write().await;
        secrets.remove(id);
        Ok(())
    }
}
