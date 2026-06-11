//! Perfis de emuladores e detecção automática (Passo 4).
//!
//! Cada emulador suportado fornece um `EmulatorProfile` descrevendo onde
//! ficam saves, savestates e configurações relativos à pasta raiz.
//! `detect_emulator(root_path)` identifica o emulador a partir de marcadores
//! no filesystem (executáveis, pastas características).

#![allow(dead_code)]

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Descrição de um emulador configurado. Cruza a boundary para o frontend —
/// espelhado em `src/types/ipc.ts` (`EmulatorProfile`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorProfile {
    /// Nome canônico ("PPSSPP", "PCSX2") — usado como nome da pasta no Drive.
    pub name: String,
    /// Pasta raiz selecionada pelo usuário.
    pub root_path: PathBuf,
    /// Pastas de saves, relativas a `root_path`.
    pub saves_paths: Vec<PathBuf>,
    /// Pastas de configuração, relativas a `root_path`.
    pub config_paths: Vec<PathBuf>,
    /// Pastas de savestates, relativas a `root_path`.
    pub state_paths: Vec<PathBuf>,
}
