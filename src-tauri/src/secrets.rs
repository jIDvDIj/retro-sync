//! Abstração de armazenamento seguro de segredos (refresh token, device_id).
//!
//! No desktop usa o cofre nativo do SO (`keyring` — Credential Manager,
//! Keychain, Secret Service). No mobile o `keyring` não tem suporte; os
//! segredos ficam na tabela `secrets` do SQLite privado do app (inacessível
//! a outros apps no sandbox do Android/iOS).

use crate::error::AppResult;

/// Interface de persistência de segredos chave→valor. Operações são síncronas
/// (adequadas para `spawn_blocking`); implementações devem ser `Send + Sync`.
pub trait SecretStore: Send + Sync {
    fn set(&self, key: &str, value: &str) -> AppResult<()>;
    fn get(&self, key: &str) -> AppResult<Option<String>>;
    fn delete(&self, key: &str) -> AppResult<()>;
}

// --- Desktop: keyring do SO ---

#[cfg(desktop)]
pub struct KeyringStore;

#[cfg(desktop)]
impl SecretStore for KeyringStore {
    fn set(&self, key: &str, value: &str) -> AppResult<()> {
        keyring::Entry::new(crate::constants::KEYRING_SERVICE, key)?.set_password(value)?;
        Ok(())
    }

    fn get(&self, key: &str) -> AppResult<Option<String>> {
        match keyring::Entry::new(crate::constants::KEYRING_SERVICE, key)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn delete(&self, key: &str) -> AppResult<()> {
        match keyring::Entry::new(crate::constants::KEYRING_SERVICE, key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

// --- Testes: HashMap em memória ---

/// `SecretStore` em memória para testes: guarda refresh token e device_id num
/// mapa, sem keyring nem SQLite. Compartilhado pelos testes de `auth`,
/// `device` e pelos cenários do `SyncEngine`.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct MemSecrets(std::sync::Mutex<std::collections::HashMap<String, String>>);

#[cfg(test)]
impl SecretStore for MemSecrets {
    fn set(&self, key: &str, value: &str) -> AppResult<()> {
        self.0.lock().unwrap().insert(key.into(), value.into());
        Ok(())
    }

    fn get(&self, key: &str) -> AppResult<Option<String>> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }

    fn delete(&self, key: &str) -> AppResult<()> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}

// --- Mobile: tabela `secrets` no SQLite privado do app ---

#[cfg(mobile)]
pub struct SqliteSecretStore(pub crate::storage::db::Db);

#[cfg(mobile)]
impl SecretStore for SqliteSecretStore {
    fn set(&self, key: &str, value: &str) -> AppResult<()> {
        self.0.with_conn_blocking(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO secrets (key, value) VALUES (?1, ?2)",
                rusqlite::params![key, value],
            )?;
            Ok(())
        })
    }

    fn get(&self, key: &str) -> AppResult<Option<String>> {
        self.0.with_conn_blocking(|conn| {
            match conn.query_row(
                "SELECT value FROM secrets WHERE key = ?1",
                rusqlite::params![key],
                |row| row.get(0),
            ) {
                Ok(v) => Ok(Some(v)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e.into()),
            }
        })
    }

    fn delete(&self, key: &str) -> AppResult<()> {
        self.0.with_conn_blocking(|conn| {
            conn.execute("DELETE FROM secrets WHERE key = ?1", rusqlite::params![key])?;
            Ok(())
        })
    }
}
