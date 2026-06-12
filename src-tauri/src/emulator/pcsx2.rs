//! Perfil do PCSX2 (PS2).
//!
//! Estrutura reconhecida, relativa à pasta selecionada pelo usuário (pasta de
//! dados `Documents/PCSX2` ou instalação portátil): `inis/` obrigatória mais
//! ao menos uma entre `memcards/`, `sstates/` e `bios/`.

use std::path::{Path, PathBuf};

use super::EmulatorProfile;

pub const NAME: &str = "PCSX2";

/// Nomes de processo monitorados pelo watcher (Passo 6).
#[allow(dead_code)]
pub const PROCESS_NAMES: &[&str] = &[
    "pcsx2-qt.exe",
    "pcsx2-qtx64.exe",
    "pcsx2-qtx64-avx2.exe",
    "pcsx2.exe",
    "pcsx2-qt",
];

const INIS_DIR: &str = "inis";
const MEMCARDS_DIR: &str = "memcards";
const SSTATES_DIR: &str = "sstates";
const BIOS_DIR: &str = "bios";

pub fn detect(root: &Path) -> Option<EmulatorProfile> {
    if !root.join(INIS_DIR).is_dir() {
        return None;
    }
    let has_secondary = [MEMCARDS_DIR, SSTATES_DIR, BIOS_DIR]
        .iter()
        .any(|dir| root.join(dir).is_dir());
    if !has_secondary {
        return None;
    }

    Some(EmulatorProfile {
        name: NAME.to_string(),
        root_path: root.to_path_buf(),
        saves_paths: vec![PathBuf::from(MEMCARDS_DIR)],
        config_paths: vec![PathBuf::from(INIS_DIR)],
        state_paths: vec![PathBuf::from(SSTATES_DIR)],
    })
}
