//! Perfil do PPSSPP (PSP).
//!
//! Estruturas reconhecidas, relativas à pasta selecionada pelo usuário:
//! - Pasta de dados (ex.: `Documents/PPSSPP`): `PSP/{SAVEDATA,PPSSPP_STATE,SYSTEM}`;
//! - Instalação portátil (com `memstick.ini`): `memstick/PSP/{...}`.

use std::path::{Path, PathBuf};

use super::EmulatorProfile;

pub const NAME: &str = "PPSSPP";

/// Nomes de processo monitorados pelo watcher.
pub const PROCESS_NAMES: &[&str] = &["PPSSPPWindows64.exe", "PPSSPPWindows.exe", "PPSSPPSDL"];

const PSP_DIR: &str = "PSP";
const MEMSTICK_DIR: &str = "memstick";
const SAVEDATA_DIR: &str = "SAVEDATA";
const STATES_DIR: &str = "PPSSPP_STATE";
const SYSTEM_DIR: &str = "SYSTEM";

pub fn detect(root: &Path) -> Option<EmulatorProfile> {
    let psp_base = locate_psp_dir(root)?;

    let psp_abs = root.join(&psp_base);
    let has_marker = [SAVEDATA_DIR, STATES_DIR, SYSTEM_DIR]
        .iter()
        .any(|dir| psp_abs.join(dir).is_dir());
    if !has_marker {
        return None;
    }

    Some(EmulatorProfile {
        name: NAME.to_string(),
        root_path: root.to_path_buf(),
        saves_paths: vec![psp_base.join(SAVEDATA_DIR)],
        config_paths: vec![psp_base.join(SYSTEM_DIR)],
        state_paths: vec![psp_base.join(STATES_DIR)],
    })
}

/// `PSP/` direto (pasta de dados) ou `memstick/PSP/` (instalação portátil).
fn locate_psp_dir(root: &Path) -> Option<PathBuf> {
    if root.join(PSP_DIR).is_dir() {
        return Some(PathBuf::from(PSP_DIR));
    }
    let portable = Path::new(MEMSTICK_DIR).join(PSP_DIR);
    if root.join(&portable).is_dir() {
        return Some(portable);
    }
    None
}
