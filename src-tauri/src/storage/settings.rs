//! Configurações globais do usuário — tabela `app_settings` (chave→valor).
//!
//! Um único `Settings` agrega as configurações expostas ao frontend; cada
//! campo é persistido como uma linha chave→valor, com defaults aplicados na
//! leitura. Começa com o nome do dispositivo (Passo 1); cresce com gatilhos e
//! nível de notificação nos passos seguintes.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::constants::{
    SETTING_DEVICE_NAME, SETTING_NOTIFICATION_LEVEL, SETTING_TRIGGER_EMULATOR_START,
    SETTING_TRIGGER_EMULATOR_STOP, SETTING_TRIGGER_STARTUP,
};
// Consumido apenas pelas funções de autostart (só-desktop).
#[cfg(desktop)]
use crate::constants::SETTING_AUTOSTART_INITIALIZED;
use crate::error::AppResult;

/// Configurações globais. Espelhado em `src/types/ipc.ts` (`Settings`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Nome amigável deste dispositivo (ex.: "PC Gamer"). `None` até o usuário
    /// defini-lo no login. Gravado também nos metadados de sync no Drive.
    pub device_name: Option<String>,
    /// Gatilhos de sync automático habilitados.
    pub triggers: TriggerSettings,
    /// Quais eventos geram notificação nativa do SO.
    pub notification_level: NotificationLevel,
    /// Início automático junto com o sistema operacional. NÃO é persistido no
    /// banco: o estado vive no SO (registro do Windows / LaunchAgent) e é
    /// preenchido pelo comando `get_settings` via o plugin de autostart.
    /// `load` sempre devolve `false`; o valor real é injetado na camada de
    /// comando.
    #[serde(default)]
    pub autostart: bool,
}

/// Nível de notificações nativas. Espelhado em `src/types/ipc.ts`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationLevel {
    /// Sync concluído, erros e emulador detectado.
    #[default]
    All,
    /// Apenas erros de sync.
    ErrorsOnly,
    /// Nenhuma notificação.
    None,
}

impl NotificationLevel {
    fn as_str(self) -> &'static str {
        match self {
            NotificationLevel::All => "all",
            NotificationLevel::ErrorsOnly => "errors_only",
            NotificationLevel::None => "none",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "all" => Some(NotificationLevel::All),
            "errors_only" => Some(NotificationLevel::ErrorsOnly),
            "none" => Some(NotificationLevel::None),
            _ => None,
        }
    }

    /// Erros de sync devem notificar?
    pub fn notifies_errors(self) -> bool {
        !matches!(self, NotificationLevel::None)
    }

    /// Eventos informativos (sync concluído, emulador detectado) devem notificar?
    pub fn notifies_info(self) -> bool {
        matches!(self, NotificationLevel::All)
    }
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
        notification_level: notification_level(conn)?,
        // Estado do SO, não do banco: o comando `get_settings` injeta o valor
        // real lido pelo plugin de autostart.
        autostart: false,
    })
}

/// Nível de notificações (default: `All`).
pub fn notification_level(conn: &Connection) -> AppResult<NotificationLevel> {
    Ok(get(conn, SETTING_NOTIFICATION_LEVEL)?
        .as_deref()
        .and_then(NotificationLevel::parse)
        .unwrap_or_default())
}

pub fn set_notification_level(conn: &Connection, level: NotificationLevel) -> AppResult<()> {
    set(conn, SETTING_NOTIFICATION_LEVEL, level.as_str())
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

/// O default de fábrica do autostart (ligado) já foi aplicado? `false` na
/// primeiríssima execução. Ver [`mark_autostart_initialized`] e o setup em
/// `lib.rs`. Só-desktop: não há autostart no mobile.
#[cfg(desktop)]
pub fn autostart_initialized(conn: &Connection) -> AppResult<bool> {
    get_bool(conn, SETTING_AUTOSTART_INITIALIZED, false)
}

/// Marca o default de fábrica do autostart como já aplicado. Só-desktop.
#[cfg(desktop)]
pub fn mark_autostart_initialized(conn: &Connection) -> AppResult<()> {
    set_bool(conn, SETTING_AUTOSTART_INITIALIZED, true)
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
            assert_eq!(load(conn)?, Settings::default());
            // Default = todos os gatilhos ligados, notificações em `All`.
            assert_eq!(
                triggers(conn)?,
                TriggerSettings {
                    startup: true,
                    emulator_start: true,
                    emulator_stop: true,
                }
            );
            assert_eq!(notification_level(conn)?, NotificationLevel::All);
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
    fn autostart_initialized_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            // Default: ainda não aplicado na primeira execução.
            assert!(!autostart_initialized(conn)?);
            mark_autostart_initialized(conn)?;
            assert!(autostart_initialized(conn)?);
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
    fn set_e_get_notification_level_fazem_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            set_notification_level(conn, NotificationLevel::ErrorsOnly)?;
            assert_eq!(notification_level(conn)?, NotificationLevel::ErrorsOnly);
            set_notification_level(conn, NotificationLevel::None)?;
            assert_eq!(load(conn)?.notification_level, NotificationLevel::None);
            Ok(())
        });
    }

    #[test]
    fn notification_level_gating() {
        assert!(NotificationLevel::All.notifies_errors());
        assert!(NotificationLevel::All.notifies_info());
        assert!(NotificationLevel::ErrorsOnly.notifies_errors());
        assert!(!NotificationLevel::ErrorsOnly.notifies_info());
        assert!(!NotificationLevel::None.notifies_errors());
        assert!(!NotificationLevel::None.notifies_info());
    }

    #[test]
    fn settings_serializa_em_camel_case() {
        let json = serde_json::to_value(Settings {
            device_name: Some("PC Gamer".into()),
            triggers: TriggerSettings::default(),
            notification_level: NotificationLevel::ErrorsOnly,
            autostart: false,
        })
        .unwrap();
        assert_eq!(json["deviceName"], "PC Gamer");
        assert_eq!(json["triggers"]["startup"], true);
        assert_eq!(json["triggers"]["emulatorStart"], true);
        assert_eq!(json["triggers"]["emulatorStop"], true);
        assert_eq!(json["notificationLevel"], "errors_only");
        assert_eq!(json["autostart"], false);
    }
}
