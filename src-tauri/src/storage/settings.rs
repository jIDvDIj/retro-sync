//! Configurações globais do usuário — tabela `app_settings` (chave→valor).
//!
//! Um único `Settings` agrega as configurações expostas ao frontend; cada
//! campo é persistido como uma linha chave→valor, com defaults aplicados na
//! leitura. Começa com o nome do dispositivo (Passo 1); cresce com gatilhos e
//! nível de notificação nos passos seguintes.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::constants::SETTING_DEVICE_NAME;
use crate::error::AppResult;

/// Configurações globais. Espelhado em `src/types/ipc.ts` (`Settings`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Nome amigável deste dispositivo (ex.: "PC Gamer"). `None` até o usuário
    /// defini-lo no login. Gravado também nos metadados de sync no Drive.
    pub device_name: Option<String>,
}

fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let value = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value)
}

fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

/// Lê todas as configurações, aplicando defaults para chaves ausentes.
pub fn load(conn: &Connection) -> AppResult<Settings> {
    Ok(Settings {
        device_name: get(conn, SETTING_DEVICE_NAME)?,
    })
}

/// Nome do dispositivo isolado (usado pelo engine ao publicar metadados).
pub fn device_name(conn: &Connection) -> AppResult<Option<String>> {
    get(conn, SETTING_DEVICE_NAME)
}

pub fn set_device_name(conn: &Connection, name: &str) -> AppResult<()> {
    set(conn, SETTING_DEVICE_NAME, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::Db;

    #[test]
    fn load_retorna_defaults_quando_vazio() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            assert_eq!(load(conn)?, Settings { device_name: None });
            Ok(())
        });
    }

    #[test]
    fn set_device_name_persiste_e_e_lido() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            set_device_name(conn, "PC Gamer")?;
            assert_eq!(device_name(conn)?, Some("PC Gamer".to_string()));
            assert_eq!(load(conn)?.device_name, Some("PC Gamer".to_string()));
            Ok(())
        });
    }

    #[test]
    fn set_device_name_substitui_valor_anterior() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            set_device_name(conn, "Notebook")?;
            set_device_name(conn, "PC Gamer")?;
            assert_eq!(device_name(conn)?, Some("PC Gamer".to_string()));
            Ok(())
        });
    }

    #[test]
    fn settings_serializa_em_camel_case() {
        let json = serde_json::to_value(Settings {
            device_name: Some("PC Gamer".into()),
        })
        .unwrap();
        assert_eq!(json["deviceName"], "PC Gamer");
    }
}
