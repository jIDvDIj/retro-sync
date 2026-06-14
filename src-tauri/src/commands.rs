//! Boundary frontend ↔ backend: todos os `#[tauri::command]` vivem aqui.
//! Toda struct que cruza esta boundary deriva `Serialize`/`Deserialize` e tem
//! interface TypeScript espelhada em `src/types/ipc.ts`.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::auth::AuthStatus;
use crate::constants::TRIGGER_MANUAL;
use crate::emulator::{self, EmulatorProfile};
use crate::error::{AppError, AppResult};
use crate::events::EVT_AUTH_STATUS;
use crate::state::AppState;
use crate::storage::emulators::SyncCategories;
use crate::storage::settings::{NotificationLevel, Settings, TriggerSettings};
use crate::storage::{emulators, manifest, queue, settings};
use crate::sync::{LastSync, SyncDirection, SyncSummary};

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
            manifest::remove_for_emulator(conn, &name)?;
            queue::remove_for_emulator(conn, &name)
        })
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

/// Configurações globais do usuário (nome do dispositivo, etc.).
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    state.db.with(settings::load).await
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
