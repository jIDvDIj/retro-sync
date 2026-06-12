//! Fila de operações pendentes (resiliência offline).
//!
//! Quando uma transferência falha por rede ou arquivo em uso, a intenção é
//! registrada aqui e sobrevive a reinícios do app. O diff do próximo sync
//! re-detecta a diferença e refaz a operação; ao sincronizar o arquivo com
//! sucesso, `resolve` limpa as pendências dele.

use rusqlite::{params, Connection};

use crate::error::AppResult;
use crate::sync::SyncCategory;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpDirection {
    Upload,
    Download,
}

impl OpDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            OpDirection::Upload => "upload",
            OpDirection::Download => "download",
        }
    }
}

/// Registra (ou reforça, somando tentativa) uma pendência.
pub fn enqueue(
    conn: &Connection,
    emulator: &str,
    category: SyncCategory,
    rel_path: &str,
    direction: OpDirection,
    error: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO pending_ops (emulator, category, rel_path, direction, enqueued_at_ms, attempts, last_error) \
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6) \
         ON CONFLICT (emulator, category, rel_path, direction) \
         DO UPDATE SET attempts = attempts + 1, last_error = excluded.last_error",
        params![
            emulator,
            category.as_str(),
            rel_path,
            direction.as_str(),
            chrono::Utc::now().timestamp_millis(),
            error,
        ],
    )?;
    Ok(())
}

/// Remove as pendências de um arquivo após sync bem-sucedido.
pub fn resolve(
    conn: &Connection,
    emulator: &str,
    category: SyncCategory,
    rel_path: &str,
) -> AppResult<()> {
    conn.execute(
        "DELETE FROM pending_ops WHERE emulator = ?1 AND category = ?2 AND rel_path = ?3",
        params![emulator, category.as_str(), rel_path],
    )?;
    Ok(())
}

pub fn remove_for_emulator(conn: &Connection, emulator: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM pending_ops WHERE emulator = ?1",
        params![emulator],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn count(conn: &Connection) -> AppResult<i64> {
    let count = conn.query_row("SELECT COUNT(*) FROM pending_ops", [], |row| row.get(0))?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::Db;

    #[test]
    fn enqueue_deduplica_e_acumula_tentativas() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            enqueue(
                conn,
                "PPSSPP",
                SyncCategory::Saves,
                "a.bin",
                OpDirection::Upload,
                "rede",
            )?;
            enqueue(
                conn,
                "PPSSPP",
                SyncCategory::Saves,
                "a.bin",
                OpDirection::Upload,
                "rede 2",
            )?;
            assert_eq!(count(conn)?, 1);

            let attempts: i64 =
                conn.query_row("SELECT attempts FROM pending_ops", [], |r| r.get(0))?;
            assert_eq!(attempts, 2);
            Ok(())
        });
    }

    #[test]
    fn resolve_limpa_pendencias_do_arquivo() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            enqueue(
                conn,
                "PPSSPP",
                SyncCategory::Saves,
                "a.bin",
                OpDirection::Upload,
                "x",
            )?;
            enqueue(
                conn,
                "PPSSPP",
                SyncCategory::Saves,
                "b.bin",
                OpDirection::Download,
                "x",
            )?;

            resolve(conn, "PPSSPP", SyncCategory::Saves, "a.bin")?;

            assert_eq!(count(conn)?, 1);
            Ok(())
        });
    }

    #[test]
    fn remove_for_emulator_limpa_somente_o_emulador() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            enqueue(
                conn,
                "PPSSPP",
                SyncCategory::Saves,
                "a.bin",
                OpDirection::Upload,
                "x",
            )?;
            enqueue(
                conn,
                "PCSX2",
                SyncCategory::Config,
                "b.ini",
                OpDirection::Upload,
                "x",
            )?;

            remove_for_emulator(conn, "PPSSPP")?;

            assert_eq!(count(conn)?, 1);
            Ok(())
        });
    }
}
