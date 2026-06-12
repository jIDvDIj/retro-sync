//! Emuladores configurados pelo usuário (perfil completo serializado).

use rusqlite::{params, Connection};

use crate::emulator::EmulatorProfile;
use crate::error::AppResult;

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
}
