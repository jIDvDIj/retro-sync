//! Perfis de emuladores e detecção automática.
//!
//! O catálogo de emuladores conhecidos é dirigido por dados: vive em
//! `profiles.toml` e é interpretado por `profiles.rs`. Cada perfil descreve onde
//! ficam saves, savestates e configurações relativos à pasta raiz.
//! `detect_emulator(root_path)` identifica o emulador a partir de marcadores
//! no filesystem (pastas características de cada um).

mod profiles;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Descrição de um emulador configurado. Cruza a boundary para o frontend —
/// espelhado em `src/types/ipc.ts` (`EmulatorProfile`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Identifica o emulador presente em `root_path` e monta o perfil com os
/// caminhos relevantes. `None` quando nenhum emulador suportado é reconhecido.
///
/// Faz I/O síncrono de disco — em contexto async, chamar via `spawn_blocking`.
pub fn detect_emulator(root_path: &Path) -> Option<EmulatorProfile> {
    profiles::detect(root_path)
}

/// Nomes de processo do SO associados a um emulador, para o process watcher.
/// Vazio se o nome canônico não corresponder a um perfil do catálogo.
pub fn process_names(emulator_name: &str) -> Vec<String> {
    profiles::process_names(emulator_name)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn mkdirs(root: &Path, dirs: &[&str]) {
        for dir in dirs {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
    }

    #[test]
    fn detecta_ppsspp_em_pasta_de_dados() {
        let tmp = tempfile::tempdir().unwrap();
        mkdirs(tmp.path(), &["PSP/SAVEDATA", "PSP/SYSTEM"]);

        let profile = detect_emulator(tmp.path()).expect("deveria detectar PPSSPP");

        assert_eq!(profile.name, "PPSSPP");
        assert_eq!(profile.root_path, tmp.path());
        assert_eq!(profile.saves_paths, vec![Path::new("PSP").join("SAVEDATA")]);
        assert_eq!(profile.config_paths, vec![Path::new("PSP").join("SYSTEM")]);
        assert_eq!(
            profile.state_paths,
            vec![Path::new("PSP").join("PPSSPP_STATE")]
        );
    }

    #[test]
    fn detecta_ppsspp_em_instalacao_portatil() {
        let tmp = tempfile::tempdir().unwrap();
        mkdirs(tmp.path(), &["memstick/PSP/SAVEDATA"]);

        let profile = detect_emulator(tmp.path()).expect("deveria detectar PPSSPP portátil");

        assert_eq!(profile.name, "PPSSPP");
        assert_eq!(
            profile.saves_paths,
            vec![Path::new("memstick").join("PSP").join("SAVEDATA")]
        );
    }

    #[test]
    fn nao_detecta_ppsspp_com_psp_sem_marcadores() {
        let tmp = tempfile::tempdir().unwrap();
        mkdirs(tmp.path(), &["PSP"]);

        assert_eq!(detect_emulator(tmp.path()), None);
    }

    #[test]
    fn detecta_pcsx2_em_pasta_de_dados() {
        let tmp = tempfile::tempdir().unwrap();
        mkdirs(tmp.path(), &["inis", "memcards", "sstates"]);

        let profile = detect_emulator(tmp.path()).expect("deveria detectar PCSX2");

        assert_eq!(profile.name, "PCSX2");
        assert_eq!(profile.root_path, tmp.path());
        assert_eq!(profile.saves_paths, vec![PathBuf::from("memcards")]);
        assert_eq!(profile.config_paths, vec![PathBuf::from("inis")]);
        assert_eq!(profile.state_paths, vec![PathBuf::from("sstates")]);
    }

    #[test]
    fn detecta_pcsx2_somente_com_inis_e_bios() {
        let tmp = tempfile::tempdir().unwrap();
        mkdirs(tmp.path(), &["inis", "bios"]);

        let profile = detect_emulator(tmp.path()).expect("deveria detectar PCSX2");
        assert_eq!(profile.name, "PCSX2");
    }

    #[test]
    fn nao_detecta_pcsx2_somente_com_inis() {
        let tmp = tempfile::tempdir().unwrap();
        mkdirs(tmp.path(), &["inis"]);

        assert_eq!(detect_emulator(tmp.path()), None);
    }

    #[test]
    fn nao_detecta_em_pasta_vazia() {
        let tmp = tempfile::tempdir().unwrap();

        assert_eq!(detect_emulator(tmp.path()), None);
    }

    #[test]
    fn perfil_serializa_em_camel_case() {
        let tmp = tempfile::tempdir().unwrap();
        mkdirs(tmp.path(), &["inis", "memcards"]);

        let profile = detect_emulator(tmp.path()).unwrap();
        let json = serde_json::to_value(&profile).unwrap();

        assert_eq!(json["name"], "PCSX2");
        assert!(json["rootPath"].is_string());
        assert!(json["savesPaths"].is_array());
        assert!(json["configPaths"].is_array());
        assert!(json["statePaths"].is_array());
    }
}
