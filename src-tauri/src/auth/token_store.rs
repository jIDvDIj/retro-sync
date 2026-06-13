//! Persistência do refresh token no keychain nativo do SO (Credential
//! Manager no Windows, Keychain no macOS, Secret Service no Linux).
//!
//! As operações do `keyring` são bloqueantes — os chamadores async devem
//! envolvê-las em `tokio::task::spawn_blocking`.

use keyring::Entry;
use serde::{Deserialize, Serialize};

use crate::constants::{KEYRING_REFRESH_TOKEN_KEY, KEYRING_SERVICE};
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAuth {
    pub refresh_token: String,
    pub email: Option<String>,
}

pub struct TokenStore;

impl TokenStore {
    fn entry() -> AppResult<Entry> {
        Ok(Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_TOKEN_KEY)?)
    }

    pub fn save(auth: &StoredAuth) -> AppResult<()> {
        let json = serde_json::to_string(auth)?;
        Self::entry()?.set_password(&json)?;
        Ok(())
    }

    pub fn load() -> AppResult<Option<StoredAuth>> {
        match Self::entry()?.get_password() {
            Ok(json) => Ok(serde_json::from_str(&json).ok()),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn clear() -> AppResult<()> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
