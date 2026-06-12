//! Sincronização bidirecional com o Google Drive.
//!
//! O `SyncEngine` é agnóstico a emuladores: recebe `SyncTarget`s (rótulo +
//! listas de caminhos), nunca conhece PPSSPP ou PCSX2. Conflitos são
//! resolvidos por timestamp (mais recente vence; nunca deleta) e o progresso
//! é emitido ao frontend via eventos Tauri (`events::EVT_SYNC_*`).

mod conflict;
mod diff;
mod engine;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use engine::{SyncEngine, SyncSummary};

use crate::constants::{DRIVE_CONFIG_FOLDER, DRIVE_SAVES_FOLDER, DRIVE_STATES_FOLDER};
use crate::emulator::EmulatorProfile;

/// Direção de uma operação de sync. Espelhado em `src/types/ipc.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncDirection {
    DriveToLocal,
    LocalToDrive,
    Bidirectional,
}

/// Categoria de arquivos sincronizados; o valor textual é também o nome da
/// subpasta no Drive e a chave na coluna `category` do SQLite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncCategory {
    Saves,
    Savestates,
    Config,
}

impl SyncCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            SyncCategory::Saves => DRIVE_SAVES_FOLDER,
            SyncCategory::Savestates => DRIVE_STATES_FOLDER,
            SyncCategory::Config => DRIVE_CONFIG_FOLDER,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            DRIVE_SAVES_FOLDER => Some(SyncCategory::Saves),
            DRIVE_STATES_FOLDER => Some(SyncCategory::Savestates),
            DRIVE_CONFIG_FOLDER => Some(SyncCategory::Config),
            _ => None,
        }
    }
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

/// Alvo de sincronização agnóstico: o engine só enxerga rótulo + caminhos.
#[derive(Debug, Clone)]
pub struct SyncTarget {
    /// Nome da pasta do emulador no Drive (ex.: "PPSSPP").
    pub label: String,
    pub root: PathBuf,
    pub categories: Vec<(SyncCategory, Vec<PathBuf>)>,
}

impl SyncTarget {
    pub fn from_profile(profile: &EmulatorProfile) -> Self {
        Self {
            label: profile.name.clone(),
            root: profile.root_path.clone(),
            categories: vec![
                (SyncCategory::Saves, profile.saves_paths.clone()),
                (SyncCategory::Savestates, profile.state_paths.clone()),
                (SyncCategory::Config, profile.config_paths.clone()),
            ],
        }
    }
}
