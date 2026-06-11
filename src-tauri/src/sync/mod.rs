//! `SyncEngine` — orquestração da sincronização bidirecional (Passo 5).
//!
//! Agnóstico a emuladores: recebe listas de caminhos, nunca conhece PPSSPP
//! ou PCSX2. Calcula o diff (estado local × manifest SQLite × Drive), resolve
//! conflitos por timestamp (mais recente vence; nunca deleta) e emite
//! progresso ao frontend via eventos Tauri (`events::EVT_SYNC_*`).

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Direção de uma operação de sync. Espelhado em `src/types/ipc.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncDirection {
    DriveToLocal,
    LocalToDrive,
    Bidirectional,
}

/// Payload do evento `sync:progress`. Espelhado em `src/types/ipc.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub emulator: String,
    pub current_file: String,
    pub completed: u32,
    pub total: u32,
    pub direction: SyncDirection,
}
