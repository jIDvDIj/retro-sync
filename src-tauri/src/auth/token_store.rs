//! Persistência do refresh token via `SecretStore`.
//!
//! Desktop: keyring nativo do SO. Mobile: tabela `secrets` do SQLite privado.
//! As operações são bloqueantes; os chamadores async devem envolvê-las em
//! `tokio::task::spawn_blocking`.

use serde::{Deserialize, Serialize};

use crate::constants::KEYRING_REFRESH_TOKEN_KEY;
use crate::error::AppResult;
use crate::secrets::SecretStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAuth {
    pub refresh_token: String,
    pub email: Option<String>,
}

pub struct TokenStore;

impl TokenStore {
    pub fn save(auth: &StoredAuth, secrets: &dyn SecretStore) -> AppResult<()> {
        let json = serde_json::to_string(auth)?;
        secrets.set(KEYRING_REFRESH_TOKEN_KEY, &json)?;
        Ok(())
    }

    pub fn load(secrets: &dyn SecretStore) -> AppResult<Option<StoredAuth>> {
        match secrets.get(KEYRING_REFRESH_TOKEN_KEY)? {
            Some(json) => Ok(serde_json::from_str(&json).ok()),
            None => Ok(None),
        }
    }

    pub fn clear(secrets: &dyn SecretStore) -> AppResult<()> {
        secrets.delete(KEYRING_REFRESH_TOKEN_KEY)?;
        Ok(())
    }
}
