//! Identificador estável deste dispositivo, persistido via `SecretStore`.
//!
//! Desktop: keyring nativo do SO. Mobile: tabela `secrets` do SQLite.
//! O device_id é um UUID v4 gerado na primeira execução; sobrevive a
//! reinicializações e é usado na resolução de conflito entre dispositivos.

use std::sync::Arc;

use uuid::Uuid;

use crate::constants::KEYRING_DEVICE_ID_KEY;
use crate::error::AppResult;
use crate::secrets::SecretStore;

/// Lê o `device_id`; gera e persiste um UUID v4 na primeira vez. Bloqueante.
pub fn get_or_create(secrets: &dyn SecretStore) -> AppResult<String> {
    match secrets.get(KEYRING_DEVICE_ID_KEY)? {
        Some(existing) if is_valid(&existing) => Ok(existing),
        Some(_) | None => {
            let id = Uuid::new_v4().to_string();
            secrets.set(KEYRING_DEVICE_ID_KEY, &id)?;
            Ok(id)
        }
    }
}

fn is_valid(id: &str) -> bool {
    Uuid::parse_str(id).is_ok()
}

/// Resolve o `device_id` em contexto async, degradando para `None` se o
/// `SecretStore` estiver indisponível. A ausência não deve abortar um sync.
pub async fn current(secrets: Arc<dyn SecretStore>) -> Option<String> {
    match tokio::task::spawn_blocking(move || get_or_create(&*secrets)).await {
        Ok(Ok(id)) => Some(id),
        Ok(Err(err)) => {
            tracing::warn!(
                error = %err,
                "device_id indisponível; conflitos entre dispositivos não serão detectados"
            );
            None
        }
        Err(err) => {
            tracing::warn!(error = %err, "tarefa do device_id abortada");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aceita_uuid_valido_e_rejeita_lixo() {
        assert!(is_valid(&Uuid::new_v4().to_string()));
        assert!(!is_valid(""));
        assert!(!is_valid("PC Gamer"));
        assert!(!is_valid("123"));
    }
}
