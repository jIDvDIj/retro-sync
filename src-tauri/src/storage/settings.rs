//! Configurações globais do usuário — tabela `app_settings` (chave→valor).
//!
//! Um único `Settings` agrega as configurações expostas ao frontend; cada
//! campo é persistido como uma linha chave→valor, com defaults aplicados na
//! leitura. Começa com o nome do dispositivo (Passo 1); cresce com gatilhos e
//! nível de notificação nos passos seguintes.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::constants::{
    SETTING_DEVICE_NAME, SETTING_TRIGGER_EMULATOR_START, SETTING_TRIGGER_EMULATOR_STOP,
    SETTING_TRIGGER_STARTUP,
};
use crate::error::AppResult;

/// Configurações globais. Espelhado em `src/types/ipc.ts` (`Settings`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Nome amigável deste dispositivo (ex.: "PC Gamer"). `None` até o usuário
    /// defini-lo no login. Gravado também nos metadados de sync no Drive.
    pub device_name: Option<String>,
    /// Gatilhos de sync automático habilitados.
    pub triggers: TriggerSettings,
}

/// Gatilhos de sync automático. Espelhado em `src/types/ipc.ts`.
/// Default: todos ligados. O sync manual nunca é afetado por estes flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerSettings {
    /// Sync ao abrir o RetroSync.
    pub startup: bool,
    /// Download antes de o emulador abrir.
    pub emulator_start: bool,
    /// Upload ao fechar o emulador.
    pub emulator_stop: bool,
}

impl Default for TriggerSettings {
    fn default() -> Self {
        Self {
            startup: true,
            emulator_start: true,
            emulator_stop: true,
        }
    }
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

fn get_bool(conn: &Connection, key: &str, default: bool) -> AppResult<bool> {
    Ok(get(conn, key)?.map(|v| v == "true").unwrap_or(default))
}

fn set_bool(conn: &Connection, key: &str, value: bool) -> AppResult<()> {
    set(conn, key, if value { "true" } else { "false" })
}

/// Lê todas as configurações, aplicando defaults para chaves ausentes.
pub fn load(conn: &Connection) -> AppResult<Settings> {
    Ok(Settings {
        device_name: get(conn, SETTING_DEVICE_NAME)?,
        triggers: triggers(conn)?,
    })
}

/// Gatilhos automáticos habilitados (default: todos ligados).
pub fn triggers(conn: &Connection) -> AppResult<TriggerSettings> {
    Ok(TriggerSettings {
        startup: get_bool(conn, SETTING_TRIGGER_STARTUP, true)?,
        emulator_start: get_bool(conn, SETTING_TRIGGER_EMULATOR_START, true)?,
        emulator_stop: get_bool(conn, SETTING_TRIGGER_EMULATOR_STOP, true)?,
    })
}

pub fn set_triggers(conn: &Connection, triggers: &TriggerSettings) -> AppResult<()> {
    set_bool(conn, SETTING_TRIGGER_STARTUP, triggers.startup)?;
    set_bool(
        conn,
        SETTING_TRIGGER_EMULATOR_START,
        triggers.emulator_start,
    )?;
    set_bool(conn, SETTING_TRIGGER_EMULATOR_STOP, triggers.emulator_stop)?;
    Ok(())
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
            assert_eq!(
                load(conn)?,
                Settings {
                    device_name: None,
                    triggers: TriggerSettings::default(),
                }
            );
            // Default = todos os gatilhos ligados.
            assert_eq!(
                triggers(conn)?,
                TriggerSettings {
                    startup: true,
                    emulator_start: true,
                    emulator_stop: true,
                }
            );
            Ok(())
        });
    }

    #[test]
    fn set_e_get_triggers_fazem_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            let t = TriggerSettings {
                startup: false,
                emulator_start: true,
                emulator_stop: false,
            };
            set_triggers(conn, &t)?;
            assert_eq!(triggers(conn)?, t);
            assert_eq!(load(conn)?.triggers, t);
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
            triggers: TriggerSettings::default(),
        })
        .unwrap();
        assert_eq!(json["deviceName"], "PC Gamer");
        assert_eq!(json["triggers"]["startup"], true);
        assert_eq!(json["triggers"]["emulatorStart"], true);
        assert_eq!(json["triggers"]["emulatorStop"], true);
    }
}
