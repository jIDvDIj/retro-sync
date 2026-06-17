//! Identificador estável deste dispositivo, persistido no keychain do SO.
//!
//! Diferente do nome amigável (`device_name`, mutável, na tabela
//! `app_settings`), o `device_id` é um UUID v4 gerado uma única vez e guardado
//! no keyring. Por viver fora do SQLite, sobrevive à reinstalação do app e à
//! limpeza do banco — é a identidade estável usada para reconhecer "quem
//! publicou esta versão" na resolução de conflito entre dispositivos, sem
//! depender do nome, que o usuário pode renomear ou repetir entre máquinas.
//!
//! As operações do `keyring` são bloqueantes (Credential Manager no Windows,
//! Keychain no macOS, Secret Service no Linux); o chamador em contexto async
//! deve envolvê-las em `tokio::task::spawn_blocking`.

use keyring::Entry;
use uuid::Uuid;

use crate::constants::{KEYRING_DEVICE_ID_KEY, KEYRING_SERVICE};
use crate::error::AppResult;

fn entry() -> AppResult<Entry> {
    Ok(Entry::new(KEYRING_SERVICE, KEYRING_DEVICE_ID_KEY)?)
}

/// Lê o `device_id` do keyring; gera e persiste um UUID v4 na primeira vez —
/// ou se o valor guardado estiver corrompido (não for um UUID). Bloqueante.
pub fn get_or_create() -> AppResult<String> {
    let entry = entry()?;
    match entry.get_password() {
        Ok(existing) if is_valid(&existing) => Ok(existing),
        // NoEntry (primeira vez) ou valor corrompido: (re)gera.
        Ok(_) | Err(keyring::Error::NoEntry) => {
            let id = Uuid::new_v4().to_string();
            entry.set_password(&id)?;
            Ok(id)
        }
        Err(e) => Err(e.into()),
    }
}

fn is_valid(id: &str) -> bool {
    Uuid::parse_str(id).is_ok()
}

/// Resolve o `device_id` em contexto async (via `spawn_blocking`), degradando
/// para `None` — com aviso — se o keyring estiver indisponível. A ausência de
/// identidade estável não deve abortar um sync; apenas desliga a detecção de
/// conflito entre dispositivos para esta execução.
pub async fn current() -> Option<String> {
    match tokio::task::spawn_blocking(get_or_create).await {
        Ok(Ok(id)) => Some(id),
        Ok(Err(err)) => {
            tracing::warn!(
                error = %err,
                "device_id indisponível (keyring); conflitos entre dispositivos não serão detectados nesta execução"
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
