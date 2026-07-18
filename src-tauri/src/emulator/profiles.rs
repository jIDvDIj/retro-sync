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

use super::{DiscoveredEmulator, DiscoverySource, EmulatorProfile};

/// Especificação declarativa de um emulador — espelha cada `[[emulator]]` do
/// `profiles.toml`.
#[derive(Debug, Clone, Deserialize)]
struct ProfileSpec {
    /// Nome canônico (vira o nome da pasta no Drive).
    name: String,
    /// Nomes de processo do SO, consumidos pelo watcher (só-desktop).
    #[cfg_attr(not(desktop), allow(dead_code))]
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
    /// Padrões glob de arquivos a ignorar no sync (defaults do emulador).
    #[serde(default)]
    exclude: Vec<String>,
    /// Locais padrão de dados por SO (Sinal A da descoberta).
    #[serde(default)]
    data_dirs: DataDirs,
    /// Pistas no registro do Windows (Sinal B da descoberta).
    #[serde(default)]
    #[cfg_attr(not(windows), allow(dead_code))]
    registry: RegistryHints,
}

/// Locais padrão de dados, por SO — só o do SO atual é lido em cada build, daí
/// o `allow(dead_code)` (os outros campos ficam ociosos por plataforma).
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
struct DataDirs {
    #[serde(default)]
    windows: Vec<String>,
    #[serde(default)]
    macos: Vec<String>,
    #[serde(default)]
    linux: Vec<String>,
}

/// Pistas de registro (lidas apenas no Windows).
#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(not(windows), allow(dead_code))]
struct RegistryHints {
    #[serde(default)]
    uninstall_names: Vec<String>,
    #[serde(default)]
    app_paths: Vec<String>,
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

/// Variante assíncrona de [`detect`], usada no mobile: em vez de checar
/// `is_dir()` no filesystem, delega a existência de cada caminho candidato a
/// `exists` (tipicamente uma chamada ao plugin SAF). `root_display` só é usado
/// para preencher `EmulatorProfile::root_path` no perfil resultante.
#[cfg(mobile)]
pub async fn detect_async<F, Fut>(root_display: &str, mut exists: F) -> Option<EmulatorProfile>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    for spec in specs() {
        if let Some(profile) = try_match_async(root_display, spec, &mut exists).await {
            return Some(profile);
        }
    }
    None
}

#[cfg(mobile)]
async fn try_match_async<F, Fut>(
    root_display: &str,
    spec: &ProfileSpec,
    exists: &mut F,
) -> Option<EmulatorProfile>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    // 1. Resolve a base: primeiro base_candidate existente, ou a própria raiz.
    let base = if spec.base_candidates.is_empty() {
        String::new()
    } else {
        let mut found = None;
        for c in &spec.base_candidates {
            if exists(c.clone()).await {
                found = Some(c.clone());
                break;
            }
        }
        found?
    };
    let join_base = |d: &str| -> String {
        if base.is_empty() {
            d.to_string()
        } else {
            format!("{base}/{d}")
        }
    };

    // 2. required: todas precisam existir sob a base.
    for d in &spec.required {
        if !exists(join_base(d)).await {
            return None;
        }
    }
    // 3. markers: ao menos uma precisa existir (quando há marcadores).
    if !spec.markers.is_empty() {
        let mut any = false;
        for d in &spec.markers {
            if exists(join_base(d)).await {
                any = true;
                break;
            }
        }
        if !any {
            return None;
        }
    }

    let join_vec = |dirs: &[String]| -> Vec<PathBuf> {
        dirs.iter().map(|d| PathBuf::from(join_base(d))).collect()
    };
    Some(EmulatorProfile {
        name: spec.name.clone(),
        root_path: PathBuf::from(root_display),
        saves_paths: join_vec(&spec.saves),
        config_paths: join_vec(&spec.config),
        state_paths: join_vec(&spec.states),
        exclude_patterns: spec.exclude.clone(),
    })
}

/// Nomes de processo do emulador de nome canônico `name`; vazio se desconhecido.
/// Só-desktop: consumido pelo process watcher, inexistente no mobile.
#[cfg(desktop)]
pub fn process_names(name: &str) -> Vec<String> {
    specs()
        .iter()
        .find(|s| s.name == name)
        .map(|s| s.process_names.clone())
        .unwrap_or_default()
}

/// Varre o catálogo por emuladores instalados no sistema. Não persiste nada.
pub fn discover_installed() -> Vec<DiscoveredEmulator> {
    specs().iter().filter_map(discover_one).collect()
}

/// Combina Sinal A (pasta de dados + marcadores) e Sinal B (registro/Windows)
/// para um perfil.
fn discover_one(spec: &ProfileSpec) -> Option<DiscoveredEmulator> {
    // Sinal A: locais de dados conhecidos do SO atual.
    let by_data = data_dirs_for_os(spec)
        .iter()
        .filter_map(|tpl| expand_placeholders(tpl))
        .find_map(|root| try_match(&root, spec));

    // Sinal B: registro (no-op fora do Windows).
    let reg = registry_match(spec);

    // Se o registro aponta a pasta de instalação e os data_dirs não acharam
    // saves, tenta detectar ali também.
    let by_data = by_data.or_else(|| {
        reg.install_location
            .as_deref()
            .and_then(|loc| try_match(loc, spec))
    });

    let make = |profile, source| {
        Some(DiscoveredEmulator {
            name: spec.name.clone(),
            profile,
            source,
        })
    };
    match (by_data, reg.installed) {
        (Some(p), true) => make(Some(p), DiscoverySource::Both),
        (Some(p), false) => make(Some(p), DiscoverySource::DataDir),
        (None, true) => make(None, DiscoverySource::Registry),
        (None, false) => None,
    }
}

/// Os `data_dirs` do SO em que o binário roda.
fn data_dirs_for_os(spec: &ProfileSpec) -> &[String] {
    #[cfg(target_os = "windows")]
    {
        spec.data_dirs.windows.as_slice()
    }
    #[cfg(target_os = "macos")]
    {
        spec.data_dirs.macos.as_slice()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        spec.data_dirs.linux.as_slice()
    }
}

/// Resolve um template de `data_dirs` num caminho absoluto. `None` se o
/// placeholder for desconhecido ou o diretório base do SO não existir.
fn expand_placeholders(template: &str) -> Option<PathBuf> {
    let Some((key, rest)) = split_placeholder(template) else {
        // Sem placeholder: caminho literal.
        return Some(PathBuf::from(template));
    };
    let base = resolve_base(key)?;
    Some(if rest.is_empty() {
        base
    } else {
        base.join(rest)
    })
}

/// Separa `"{key}/rest"` em `(key, rest)`. `None` se não começa com `{...}`.
fn split_placeholder(template: &str) -> Option<(&str, &str)> {
    let stripped = template.strip_prefix('{')?;
    let end = stripped.find('}')?;
    let key = &stripped[..end];
    let rest = stripped[end + 1..].trim_start_matches('/');
    Some((key, rest))
}

/// Diretório base de cada placeholder, via crate `dirs`.
fn resolve_base(key: &str) -> Option<PathBuf> {
    match key {
        "documents" => dirs::document_dir(),
        "localappdata" => dirs::data_local_dir(),
        "appdata" | "config" => dirs::config_dir(),
        "home" => dirs::home_dir(),
        _ => None,
    }
}

/// Resultado da consulta ao registro (Sinal B).
struct RegistryMatch {
    installed: bool,
    install_location: Option<PathBuf>,
}

/// Consulta o registro do Windows: App Paths (por executável) e Uninstall
/// (DisplayName). Devolve "não instalado" fora do Windows.
#[cfg(windows)]
fn registry_match(spec: &ProfileSpec) -> RegistryMatch {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    // App Paths: o valor padrão da chave é o caminho do executável.
    for exe in &spec.registry.app_paths {
        let sub = format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe}");
        for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            if let Ok(key) = RegKey::predef(hive).open_subkey(&sub) {
                let install_location = key
                    .get_value::<String, _>("")
                    .ok()
                    .and_then(|p| PathBuf::from(p).parent().map(Path::to_path_buf));
                return RegistryMatch {
                    installed: true,
                    install_location,
                };
            }
        }
    }

    // Uninstall: procura DisplayName contendo algum uninstall_name.
    let mut install_location = None;
    if registry_uninstall_match(spec, &mut install_location) {
        return RegistryMatch {
            installed: true,
            install_location,
        };
    }

    RegistryMatch {
        installed: false,
        install_location: None,
    }
}

/// Percorre as chaves de Uninstall (HKLM, WOW6432Node e HKCU) procurando um
/// `DisplayName` que contenha algum dos nomes; preenche `install_location` com
/// o `InstallLocation` da primeira correspondência.
#[cfg(windows)]
fn registry_uninstall_match(spec: &ProfileSpec, install_location: &mut Option<PathBuf>) -> bool {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    if spec.registry.uninstall_names.is_empty() {
        return false;
    }
    let roots = [
        (
            RegKey::predef(HKEY_LOCAL_MACHINE),
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            RegKey::predef(HKEY_LOCAL_MACHINE),
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            RegKey::predef(HKEY_CURRENT_USER),
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];
    for (hive, base) in &roots {
        let Ok(root) = hive.open_subkey(base) else {
            continue;
        };
        for sub in root.enum_keys().flatten() {
            let Ok(key) = root.open_subkey(&sub) else {
                continue;
            };
            let Ok(display) = key.get_value::<String, _>("DisplayName") else {
                continue;
            };
            if spec
                .registry
                .uninstall_names
                .iter()
                .any(|n| display.contains(n.as_str()))
            {
                if let Ok(loc) = key.get_value::<String, _>("InstallLocation") {
                    if !loc.is_empty() {
                        *install_location = Some(PathBuf::from(loc));
                    }
                }
                return true;
            }
        }
    }
    false
}

#[cfg(not(windows))]
fn registry_match(_spec: &ProfileSpec) -> RegistryMatch {
    RegistryMatch {
        installed: false,
        install_location: None,
    }
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
        exclude_patterns: spec.exclude.clone(),
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

    #[test]
    fn split_placeholder_separa_chave_e_resto() {
        assert_eq!(
            split_placeholder("{documents}/PPSSPP"),
            Some(("documents", "PPSSPP"))
        );
        assert_eq!(
            split_placeholder("{home}/Library/Application Support/PPSSPP"),
            Some(("home", "Library/Application Support/PPSSPP"))
        );
        assert_eq!(split_placeholder("{home}"), Some(("home", "")));
        // Sem placeholder = literal.
        assert_eq!(split_placeholder("/caminho/literal"), None);
    }

    #[test]
    fn expand_placeholders_trata_literal_e_desconhecido() {
        // Placeholder desconhecido não resolve.
        assert_eq!(expand_placeholders("{desconhecido}/x"), None);
        assert_eq!(resolve_base("desconhecido"), None);
        // Caminho literal volta como está.
        assert_eq!(
            expand_placeholders("/abs/literal"),
            Some(PathBuf::from("/abs/literal"))
        );
    }

    #[test]
    fn data_dirs_do_so_atual_nao_sao_vazios_no_catalogo() {
        // Cada perfil deve ter ao menos um data_dir para o SO de teste, senão a
        // descoberta automática nunca o encontraria nesta plataforma.
        for spec in specs() {
            assert!(
                !data_dirs_for_os(spec).is_empty(),
                "{} sem data_dirs para este SO",
                spec.name
            );
        }
    }

    #[test]
    fn catalogo_inclui_caminhos_flatpak_no_linux_para_steam_deck() {
        // No Steam Deck (EmuDeck) os emuladores rodam como Flatpak; seus saves
        // ficam em ~/.var/app/<app-id>/... — sem isso a descoberta automática
        // não os encontra. Tranca as entradas contra regressão/typo.
        let app_id = |name: &str| -> String {
            specs()
                .iter()
                .find(|s| s.name == name)
                .map(|s| s.data_dirs.linux.join("|"))
                .unwrap_or_default()
        };
        assert!(
            app_id("PPSSPP").contains(".var/app/org.ppsspp.PPSSPP"),
            "PPSSPP deve ter o caminho Flatpak nos data_dirs.linux"
        );
        assert!(
            app_id("PCSX2").contains(".var/app/net.pcsx2.PCSX2"),
            "PCSX2 deve ter o caminho Flatpak nos data_dirs.linux"
        );
    }
}
