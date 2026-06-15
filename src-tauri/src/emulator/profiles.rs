//! Catálogo de emuladores conhecidos, dirigido por dados.
//!
//! Os perfis vivem em `profiles.toml` (embutido no binário via `include_str!`) e
//! são interpretados aqui — substituem os antigos módulos por emulador. Adicionar
//! um emulador conhecido passa a ser editar dados, não escrever código.
//!
//! O catálogo é parseado uma única vez (no primeiro uso) e a validade do TOML
//! embutido é coberta pelo teste `profiles_toml_parseia_*`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

use super::EmulatorProfile;

/// Especificação declarativa de um emulador — espelha cada `[[emulator]]` do
/// `profiles.toml`.
#[derive(Debug, Clone, Deserialize)]
struct ProfileSpec {
    /// Nome canônico (vira o nome da pasta no Drive).
    name: String,
    /// Nomes de processo do SO, consumidos pelo watcher.
    process_names: Vec<String>,
    /// Candidatos a "base" relativos à raiz; o primeiro existente é usado.
    /// Vazio = a própria raiz é a base.
    #[serde(default)]
    base_candidates: Vec<String>,
    /// Pastas (sob a base) das quais TODAS precisam existir para confirmar.
    #[serde(default)]
    required: Vec<String>,
    /// Pastas (sob a base) das quais ao menos UMA precisa existir.
    #[serde(default)]
    markers: Vec<String>,
    /// Pastas de saves, relativas à base.
    saves: Vec<String>,
    /// Pastas de savestates, relativas à base.
    states: Vec<String>,
    /// Pastas de configuração, relativas à base.
    config: Vec<String>,
}

const PROFILES_TOML: &str = include_str!("profiles.toml");

/// Catálogo parseado uma única vez, no primeiro uso.
fn specs() -> &'static [ProfileSpec] {
    static SPECS: OnceLock<Vec<ProfileSpec>> = OnceLock::new();
    SPECS
        .get_or_init(|| {
            #[derive(Deserialize)]
            struct Catalog {
                emulator: Vec<ProfileSpec>,
            }
            toml::from_str::<Catalog>(PROFILES_TOML)
                .expect("profiles.toml embutido deve ser válido")
                .emulator
        })
        .as_slice()
}

/// Identifica o emulador presente em `root` testando cada perfil do catálogo.
pub fn detect(root: &Path) -> Option<EmulatorProfile> {
    specs().iter().find_map(|spec| try_match(root, spec))
}

/// Nomes de processo do emulador de nome canônico `name`; vazio se desconhecido.
pub fn process_names(name: &str) -> Vec<String> {
    specs()
        .iter()
        .find(|s| s.name == name)
        .map(|s| s.process_names.clone())
        .unwrap_or_default()
}

/// Tenta casar um perfil com `root`. `None` quando a base ou os marcadores não
/// batem.
fn try_match(root: &Path, spec: &ProfileSpec) -> Option<EmulatorProfile> {
    // 1. Resolve a base: primeiro base_candidate existente, ou a própria raiz.
    let base = if spec.base_candidates.is_empty() {
        PathBuf::new()
    } else {
        spec.base_candidates
            .iter()
            .map(PathBuf::from)
            .find(|c| root.join(c).is_dir())?
    };
    let base_abs = root.join(&base);

    // 2. required: todas precisam existir sob a base.
    if !spec.required.iter().all(|d| base_abs.join(d).is_dir()) {
        return None;
    }
    // 3. markers: ao menos uma precisa existir (quando há marcadores).
    if !spec.markers.is_empty() && !spec.markers.iter().any(|d| base_abs.join(d).is_dir()) {
        return None;
    }

    let join = |dirs: &[String]| -> Vec<PathBuf> { dirs.iter().map(|d| base.join(d)).collect() };
    Some(EmulatorProfile {
        name: spec.name.clone(),
        root_path: root.to_path_buf(),
        saves_paths: join(&spec.saves),
        config_paths: join(&spec.config),
        state_paths: join(&spec.states),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_toml_parseia_e_contem_perfis_conhecidos() {
        let names: Vec<&str> = specs().iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"PPSSPP"), "esperava PPSSPP no catálogo");
        assert!(names.contains(&"PCSX2"), "esperava PCSX2 no catálogo");
    }

    #[test]
    fn process_names_conhecido_e_desconhecido() {
        assert!(!process_names("PPSSPP").is_empty());
        assert!(process_names("Inexistente").is_empty());
    }
}
