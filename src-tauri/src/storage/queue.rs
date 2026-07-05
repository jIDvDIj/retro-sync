//! Fila de operações pendentes (resiliência offline).
//!
//! Quando uma transferência falha por rede ou arquivo em uso, a intenção é
//! registrada aqui e sobrevive a reinícios do app. O diff do próximo sync
//! re-detecta a diferença e refaz a operação; ao sincronizar o arquivo com
//! sucesso, `resolve` limpa as pendências dele.

use rusqlite::{params, Connection};
use serde::Serialize;

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

/// Pendência exposta à UI (fila offline visível). Espelhada em
/// `src/types/ipc.ts` (`PendingOp`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingOp {
    pub emulator: String,
    pub category: SyncCategory,
    pub rel_path: String,
    /// "upload" | "download" (mesmos valores de [`OpDirection::as_str`]).
    pub direction: String,
    pub enqueued_at_ms: i64,
    pub attempts: u32,
    pub last_error: Option<String>,
}

/// Todas as pendências, mais antigas primeiro — a UI agrupa por emulador.
pub fn list_all(conn: &Connection) -> AppResult<Vec<PendingOp>> {
    let mut stmt = conn.prepare(
        "SELECT emulator, category, rel_path, direction, enqueued_at_ms, attempts, last_error \
         FROM pending_ops ORDER BY enqueued_at_ms ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, u32>(5)?,
            row.get::<_, Option<String>>(6)?,
        ))
    })?;

    let mut out = Vec::new();
    for row in rows {
        let (emulator, category, rel_path, direction, enqueued_at_ms, attempts, last_error) = row?;
        // Linha com categoria desconhecida (schema futuro?) é ignorada em vez
        // de derrubar a listagem inteira.
        let Some(category) = SyncCategory::parse(&category) else {
            continue;
        };
        out.push(PendingOp {
            emulator,
            category,
            rel_path,
            direction,
            enqueued_at_ms,
            attempts,
            last_error,
        });
    }
    Ok(out)
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
    fn list_all_expoe_direcao_tentativas_e_erro() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            enqueue(
                conn,
                "PPSSPP",
                SyncCategory::Saves,
                "a.bin",
                OpDirection::Upload,
                "rede caiu",
            )?;
            enqueue(
                conn,
                "PPSSPP",
                SyncCategory::Saves,
                "a.bin",
                OpDirection::Upload,
                "arquivo em uso",
            )?;
            enqueue(
                conn,
                "PCSX2",
                SyncCategory::Config,
                "b.ini",
                OpDirection::Download,
                "x",
            )?;

            let ops = list_all(conn)?;
            assert_eq!(ops.len(), 2);
            let a = ops.iter().find(|o| o.rel_path == "a.bin").unwrap();
            assert_eq!(a.emulator, "PPSSPP");
            assert_eq!(a.category, SyncCategory::Saves);
            assert_eq!(a.direction, "upload");
            assert_eq!(a.attempts, 2);
            assert_eq!(a.last_error.as_deref(), Some("arquivo em uso"));
            Ok(())
        });
    }

    #[test]
    fn pending_op_serializa_em_camel_case() {
        let op = PendingOp {
            emulator: "PPSSPP".into(),
            category: SyncCategory::Savestates,
            rel_path: "GAME01/state0.bin".into(),
            direction: "download".into(),
            enqueued_at_ms: 1_700_000_000_000,
            attempts: 3,
            last_error: Some("rede".into()),
        };
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["emulator"], "PPSSPP");
        assert_eq!(json["category"], "savestates");
        assert_eq!(json["relPath"], "GAME01/state0.bin");
        assert_eq!(json["direction"], "download");
        assert_eq!(json["enqueuedAtMs"], 1_700_000_000_000i64);
        assert_eq!(json["attempts"], 3);
        assert_eq!(json["lastError"], "rede");
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
