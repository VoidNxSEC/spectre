use anyhow::{Context, Result};
use secrecy::{ExposeSecret, Secret};
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

/// Manages retrieval of sensitive configuration
pub struct SecretManager;

impl SecretManager {
    /// Retrieve a secret, prioritizing file-based secrets (Docker/K8s/NixOS)
    /// over environment variables.
    pub fn get(key: &str) -> Result<Secret<String>> {
        // 1. Try file-based secret (e.g., /run/secrets/my_secret)
        let secret_path = PathBuf::from(format!("/run/secrets/{}", key.to_lowercase()));
        if secret_path.exists() {
            let content = fs::read_to_string(&secret_path)
                .with_context(|| format!("Failed to read secret file: {:?}", secret_path))?;
            info!("Loaded secret '{}' from file", key);
            return Ok(Secret::new(content.trim().to_string()));
        }

        // 2. Fallback to Env Var (Development)
        if let Ok(val) = env::var(key) {
            warn!("Loaded secret '{}' from ENV VAR. Use file-based secrets in production.", key);
            return Ok(Secret::new(val));
        }

        anyhow::bail!("Secret '{}' not found in /run/secrets/ or ENV", key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_get_secret_from_env() {
        env::set_var("TEST_SECRET", "env_value");
        let secret = SecretManager::get("TEST_SECRET").unwrap();
        assert_eq!(secret.expose_secret(), "env_value");
        env::remove_var("TEST_SECRET");
    }
}
