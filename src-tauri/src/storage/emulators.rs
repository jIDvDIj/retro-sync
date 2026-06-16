//! Emuladores configurados pelo usuário (perfil completo serializado) e suas
//! configurações de sync (quais categorias sincronizar).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::emulator::EmulatorProfile;
use crate::error::AppResult;

/// Categorias de sync habilitadas para um emulador. Espelhado em
/// `src/types/ipc.ts` (`SyncCategories`). Default: todas ativas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCategories {
    pub saves: bool,
    pub savestates: bool,
    pub config: bool,
}

impl Default for SyncCategories {
    fn default() -> Self {
        Self {
            saves: true,
            savestates: true,
            config: true,
        }
    }
}

/// Categorias habilitadas de um emulador; default (todas ativas) se nunca foi
/// configurado.
pub fn get_categories(conn: &Connection, emulator: &str) -> AppResult<SyncCategories> {
    let cats = conn
        .query_row(
            "SELECT saves_enabled, savestates_enabled, config_enabled \
             FROM emulator_settings WHERE emulator = ?1",
            params![emulator],
            |row| {
                Ok(SyncCategories {
                    saves: row.get::<_, i64>(0)? != 0,
                    savestates: row.get::<_, i64>(1)? != 0,
                    config: row.get::<_, i64>(2)? != 0,
                })
            },
        )
        .optional()?;
    Ok(cats.unwrap_or_default())
}

pub fn set_categories(conn: &Connection, emulator: &str, cats: &SyncCategories) -> AppResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO emulator_settings \
         (emulator, saves_enabled, savestates_enabled, config_enabled) VALUES (?1, ?2, ?3, ?4)",
        params![
            emulator,
            cats.saves as i64,
            cats.savestates as i64,
            cats.config as i64
        ],
    )?;
    Ok(())
}

pub fn remove_categories(conn: &Connection, emulator: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM emulator_settings WHERE emulator = ?1",
        params![emulator],
    )?;
    Ok(())
}

pub fn upsert(conn: &Connection, profile: &EmulatorProfile) -> AppResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO emulators (name, root_path, profile_json, added_at_ms) \
         VALUES (?1, ?2, ?3, ?4)",
        params![
            profile.name,
            profile.root_path.to_string_lossy(),
            serde_json::to_string(profile)?,
            chrono::Utc::now().timestamp_millis(),
        ],
    )?;
    Ok(())
}

/// `true` se já existe um emulador registrado com este nome (auto ou manual).
pub fn exists(conn: &Connection, name: &str) -> AppResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM emulators WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn list(conn: &Connection) -> AppResult<Vec<EmulatorProfile>> {
    let mut stmt = conn.prepare("SELECT profile_json FROM emulators ORDER BY name")?;
    let raw = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut profiles = Vec::with_capacity(raw.len());
    for json in raw {
        profiles.push(serde_json::from_str(&json)?);
    }
    Ok(profiles)
}

pub fn remove(conn: &Connection, name: &str) -> AppResult<()> {
    conn.execute("DELETE FROM emulators WHERE name = ?1", params![name])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::storage::db::Db;

    fn sample_profile() -> EmulatorProfile {
        EmulatorProfile {
            name: "PPSSPP".into(),
            root_path: PathBuf::from("/tmp/ppsspp"),
            saves_paths: vec![PathBuf::from("PSP/SAVEDATA")],
            config_paths: vec![PathBuf::from("PSP/SYSTEM")],
            state_paths: vec![PathBuf::from("PSP/PPSSPP_STATE")],
        }
    }

    #[test]
    fn upsert_e_list_fazem_roundtrip_do_perfil() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            upsert(conn, &sample_profile())?;

            let profiles = list(conn)?;
            assert_eq!(profiles, vec![sample_profile()]);
            Ok(())
        });
    }

    #[test]
    fn upsert_substitui_perfil_com_mesmo_nome() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            upsert(conn, &sample_profile())?;
            let mut updated = sample_profile();
            updated.root_path = PathBuf::from("/outro/lugar");
            upsert(conn, &updated)?;

            let profiles = list(conn)?;
            assert_eq!(profiles.len(), 1);
            assert_eq!(profiles[0].root_path, PathBuf::from("/outro/lugar"));
            Ok(())
        });
    }

    #[test]
    fn remove_apaga_o_perfil() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            upsert(conn, &sample_profile())?;
            remove(conn, "PPSSPP")?;
            assert!(list(conn)?.is_empty());
            Ok(())
        });
    }

    #[test]
    fn exists_reflete_presenca_do_perfil() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            assert!(!exists(conn, "PPSSPP")?);
            upsert(conn, &sample_profile())?;
            assert!(exists(conn, "PPSSPP")?);
            assert!(!exists(conn, "PCSX2")?);
            Ok(())
        });
    }

    #[test]
    fn categorias_default_sao_todas_ativas() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            assert_eq!(get_categories(conn, "PPSSPP")?, SyncCategories::default());
            assert_eq!(
                get_categories(conn, "PPSSPP")?,
                SyncCategories {
                    saves: true,
                    savestates: true,
                    config: true,
                }
            );
            Ok(())
        });
    }

    #[test]
    fn set_e_get_categorias_fazem_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            let cats = SyncCategories {
                saves: true,
                savestates: false,
                config: false,
            };
            set_categories(conn, "PPSSPP", &cats)?;
            assert_eq!(get_categories(conn, "PPSSPP")?, cats);
            Ok(())
        });
    }

    #[test]
    fn remove_categorias_volta_ao_default() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            set_categories(
                conn,
                "PPSSPP",
                &SyncCategories {
                    saves: false,
                    savestates: false,
                    config: false,
                },
            )?;
            remove_categories(conn, "PPSSPP")?;
            assert_eq!(get_categories(conn, "PPSSPP")?, SyncCategories::default());
            Ok(())
        });
    }

    #[test]
    fn categorias_serializam_em_camel_case() {
        let json = serde_json::to_value(SyncCategories::default()).unwrap();
        assert_eq!(json["saves"], true);
        assert_eq!(json["savestates"], true);
        assert_eq!(json["config"], true);
    }
}
