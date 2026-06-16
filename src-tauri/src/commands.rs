//! Boundary frontend ↔ backend: todos os `#[tauri::command]` vivem aqui.
//! Toda struct que cruza esta boundary deriva `Serialize`/`Deserialize` e tem
//! interface TypeScript espelhada em `src/types/ipc.ts`.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::auth::AuthStatus;
use crate::constants::{LOCAL_BACKUP_DIR, TRIGGER_MANUAL};
use crate::emulator::{self, EmulatorProfile};
use crate::error::{AppError, AppResult};
use crate::events::EVT_AUTH_STATUS;
use crate::state::AppState;
use crate::storage::conflicts::{self, Conflict};
use crate::storage::emulators::SyncCategories;
use crate::storage::settings::{NotificationLevel, Settings, TriggerSettings};
use crate::storage::{emulators, manifest, queue, settings};
use crate::sync::{ConflictResolution, LastSync, SyncCategory, SyncDirection, SyncSummary};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub version: String,
    pub ready: bool,
}

/// Verificação mínima de que a boundary frontend ↔ Rust está funcional.
#[tauri::command]
pub fn health_check() -> AppResult<HealthStatus> {
    Ok(HealthStatus {
        version: env!("CARGO_PKG_VERSION").to_string(),
        ready: true,
    })
}

/// Abre o navegador para o consentimento OAuth2 e aguarda a autorização.
/// Resolve quando o fluxo termina (ou falha/expira em 5 minutos).
#[tauri::command]
pub async fn connect_google_drive(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<AuthStatus> {
    let status = state.auth.connect().await?;
    let _ = app.emit(EVT_AUTH_STATUS, &status);
    Ok(status)
}

/// Status atual sem disparar fluxo interativo (consulta apenas o keyring).
#[tauri::command]
pub async fn get_auth_status(state: State<'_, AppState>) -> AppResult<AuthStatus> {
    state.auth.status().await
}

/// Remove o refresh token do keyring e limpa o token em memória.
#[tauri::command]
pub async fn disconnect_google_drive(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<AuthStatus> {
    let status = state.auth.disconnect().await?;
    let _ = app.emit(EVT_AUTH_STATUS, &status);
    Ok(status)
}

/// Identifica o emulador presente na pasta selecionada pelo usuário.
/// `Ok(None)` = pasta válida, mas nenhum emulador suportado reconhecido.
#[tauri::command]
pub async fn detect_emulator(path: String) -> AppResult<Option<EmulatorProfile>> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("pasta não encontrada: {}", root.display()),
        )
        .into());
    }

    tokio::task::spawn_blocking(move || Ok(emulator::detect_emulator(&root)))
        .await
        .map_err(|e| AppError::Other(format!("tarefa bloqueante abortada: {e}")))?
}

/// Detecta o emulador na pasta e o registra para sincronização.
#[tauri::command]
pub async fn add_emulator(state: State<'_, AppState>, path: String) -> AppResult<EmulatorProfile> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("pasta não encontrada: {}", root.display()),
        )
        .into());
    }

    let profile = tokio::task::spawn_blocking(move || emulator::detect_emulator(&root))
        .await
        .map_err(|e| AppError::Other(format!("tarefa bloqueante abortada: {e}")))?
        .ok_or(AppError::EmulatorNotDetected(path))?;

    let to_store = profile.clone();
    state
        .db
        .with(move |conn| emulators::upsert(conn, &to_store))
        .await?;
    tracing::info!(emulador = %profile.name, raiz = %profile.root_path.display(), "emulador adicionado");
    Ok(profile)
}

/// Registra um emulador cujas pastas o usuário informou manualmente — fallback
/// quando a detecção automática falha (instalação portátil ou fora do catálogo).
/// Os caminhos chegam relativos à raiz. Não sobrescreve um emulador já existente.
#[tauri::command]
pub async fn add_emulator_manual(
    state: State<'_, AppState>,
    name: String,
    path: String,
    saves_paths: Vec<String>,
    state_paths: Vec<String>,
    config_paths: Vec<String>,
) -> AppResult<EmulatorProfile> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("pasta não encontrada: {}", root.display()),
        )
        .into());
    }

    let profile = tokio::task::spawn_blocking(move || {
        emulator::build_manual_profile(&root, name, saves_paths, state_paths, config_paths)
    })
    .await
    .map_err(|e| AppError::Other(format!("tarefa bloqueante abortada: {e}")))?
    .map_err(AppError::Other)?;

    let name_check = profile.name.clone();
    if state
        .db
        .with(move |conn| emulators::exists(conn, &name_check))
        .await?
    {
        return Err(AppError::EmulatorExists(profile.name));
    }

    let to_store = profile.clone();
    state
        .db
        .with(move |conn| emulators::upsert(conn, &to_store))
        .await?;
    tracing::info!(emulador = %profile.name, raiz = %profile.root_path.display(), "emulador manual adicionado");
    Ok(profile)
}

/// Varre locais conhecidos e o registro do Windows por emuladores do catálogo
/// instalados no sistema. Não persiste nada — a UI usa o resultado para sugerir
/// adições em um clique.
#[tauri::command]
pub async fn discover_emulators() -> AppResult<Vec<emulator::DiscoveredEmulator>> {
    tokio::task::spawn_blocking(emulator::discover_installed)
        .await
        .map_err(|e| AppError::Other(format!("tarefa bloqueante abortada: {e}")))
}

#[tauri::command]
pub async fn list_emulators(state: State<'_, AppState>) -> AppResult<Vec<EmulatorProfile>> {
    state.db.with(emulators::list).await
}

/// Remove o emulador da sincronização (manifest e pendências inclusos).
/// Nada é apagado no Drive nem no disco local.
#[tauri::command]
pub async fn remove_emulator(state: State<'_, AppState>, name: String) -> AppResult<()> {
    state
        .db
        .with(move |conn| {
            emulators::remove(conn, &name)?;
            emulators::remove_categories(conn, &name)?;
            conflicts::remove_for_emulator(conn, &name)?;
            manifest::remove_for_emulator(conn, &name)?;
            queue::remove_for_emulator(conn, &name)
        })
        .await
}

/// Conflitos pendentes (ambos os lados mudaram). A UI exibe o botão de resolver
/// no card do emulador afetado.
#[tauri::command]
pub async fn list_conflicts(state: State<'_, AppState>) -> AppResult<Vec<Conflict>> {
    state.db.with(conflicts::list_all).await
}

/// Resolve um conflito mantendo a versão escolhida (`local` ou `drive`) e
/// desbloqueia o sync do emulador.
#[tauri::command]
pub async fn resolve_conflict(
    state: State<'_, AppState>,
    emulator: String,
    category: SyncCategory,
    rel_path: String,
    keep: ConflictResolution,
) -> AppResult<()> {
    state
        .engine
        .resolve_conflict(&emulator, category, &rel_path, keep)
        .await
}

/// Categorias de sync habilitadas para um emulador (default: todas ativas).
#[tauri::command]
pub async fn get_emulator_categories(
    state: State<'_, AppState>,
    name: String,
) -> AppResult<SyncCategories> {
    state
        .db
        .with(move |conn| emulators::get_categories(conn, &name))
        .await
}

/// Define quais categorias (saves/savestates/config) sincronizar para um
/// emulador. Desativar `config`, p.ex., evita compartilhar resolução/controles
/// entre dispositivos diferentes.
#[tauri::command]
pub async fn set_emulator_categories(
    state: State<'_, AppState>,
    name: String,
    categories: SyncCategories,
) -> AppResult<()> {
    state
        .db
        .with(move |conn| emulators::set_categories(conn, &name, &categories))
        .await
}

/// Sync manual (botão da UI / menu da tray). Bidirecional.
#[tauri::command]
pub async fn sync_now(state: State<'_, AppState>) -> AppResult<SyncSummary> {
    state
        .engine
        .sync_all(SyncDirection::Bidirectional, TRIGGER_MANUAL)
        .await
}

/// Configurações globais do usuário (nome do dispositivo, etc.). O flag de
/// autostart não vive no banco — é lido do SO via plugin e injetado aqui.
#[tauri::command]
pub async fn get_settings(app: AppHandle, state: State<'_, AppState>) -> AppResult<Settings> {
    let mut settings = state.db.with(settings::load).await?;
    settings.autostart = autostart_enabled(&app)?;
    Ok(settings)
}

/// Liga/desliga o início automático do RetroSync junto com o sistema. O estado
/// é persistido pelo SO (registro do Windows / LaunchAgent), não no banco local.
/// Ao subir pelo SO, o app é lançado com `--minimized` e fica só na bandeja.
#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> AppResult<()> {
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|e| AppError::Other(format!("falha ao configurar o autostart: {e}")))
}

/// Lê do SO se o RetroSync está registrado para iniciar com o sistema.
fn autostart_enabled(app: &AppHandle) -> AppResult<bool> {
    app.autolaunch()
        .is_enabled()
        .map_err(|e| AppError::Other(format!("autostart indisponível: {e}")))
}

/// Abre a pasta de backups locais no gerenciador de arquivos do SO. A pasta é
/// criada se ainda não existir (BUG-001 — backups do primeiro sync).
#[tauri::command]
pub async fn open_backup_folder(app: AppHandle) -> AppResult<()> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Other(format!("diretório de dados indisponível: {e}")))?
        .join(LOCAL_BACKUP_DIR);
    tokio::fs::create_dir_all(&dir).await?;
    tokio::task::spawn_blocking(move || open::that(&dir))
        .await
        .map_err(|e| AppError::Other(format!("tarefa bloqueante abortada: {e}")))??;
    Ok(())
}

/// Liga/desliga os gatilhos de sync automático. O sync manual (botão/tray) não
/// é afetado por estes flags.
#[tauri::command]
pub async fn set_triggers(state: State<'_, AppState>, triggers: TriggerSettings) -> AppResult<()> {
    state
        .db
        .with(move |conn| settings::set_triggers(conn, &triggers))
        .await
}

/// Define o nível de notificações nativas (all | errors_only | none).
#[tauri::command]
pub async fn set_notification_level(
    state: State<'_, AppState>,
    level: NotificationLevel,
) -> AppResult<()> {
    state
        .db
        .with(move |conn| settings::set_notification_level(conn, level))
        .await
}

/// Define o nome amigável deste dispositivo. Obrigatório no login; pode ser
/// alterado nas configurações sem refazer a autenticação.
#[tauri::command]
pub async fn set_device_name(state: State<'_, AppState>, name: String) -> AppResult<()> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::Other(
            "o nome do dispositivo não pode ser vazio".into(),
        ));
    }
    state
        .db
        .with(move |conn| settings::set_device_name(conn, &trimmed))
        .await
}

/// Último sync concluído (para a UI exibir ao montar). `None` se ainda não
/// houve nenhum nesta execução.
#[tauri::command]
pub fn get_last_sync(state: State<'_, AppState>) -> AppResult<Option<LastSync>> {
    let guard = state
        .last_sync
        .lock()
        .map_err(|_| AppError::Other("lock do último sync envenenado".into()))?;
    Ok(guard.clone())
}
