mod auth;
mod commands;
mod constants;
mod drive;
mod emulator;
mod error;
mod events;
mod state;
mod storage;
mod sync;
mod watcher;

use std::sync::Arc;

use tauri::Manager;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            init_logging(app.handle())?;
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "RetroSync iniciado");

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = storage::db::Db::open(&data_dir.join(constants::LOCAL_DB_FILE))?;

            let http = reqwest::Client::new();
            let auth = Arc::new(auth::AuthManager::new(http.clone()));
            let drive = Arc::new(drive::DriveClient::new(http, auth.clone()));
            let engine = Arc::new(sync::SyncEngine::new(
                db.clone(),
                drive,
                auth.clone(),
                app.handle().clone(),
            ));

            app.manage(state::AppState {
                auth,
                db: db.clone(),
                engine: engine.clone(),
            });

            // Process watcher: dispara sync ao abrir/fechar um emulador.
            watcher::start(db, engine.clone(), app.handle().clone());

            // Gatilho "ao iniciar o RetroSync": sync bidirecional em background.
            tauri::async_runtime::spawn(async move {
                match engine
                    .sync_all(
                        sync::SyncDirection::Bidirectional,
                        constants::TRIGGER_STARTUP,
                    )
                    .await
                {
                    Ok(summary) => tracing::info!(?summary, "sync de inicialização concluído"),
                    Err(err) => tracing::warn!(error = %err, "sync de inicialização não executado"),
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health_check,
            commands::connect_google_drive,
            commands::get_auth_status,
            commands::disconnect_google_drive,
            commands::detect_emulator,
            commands::add_emulator,
            commands::list_emulators,
            commands::remove_emulator,
            commands::sync_now
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o RetroSync");
}

/// Logs em stdout (dev) e em arquivo diário no diretório de logs do app
/// (`%LOCALAPPDATA%/com.retrosync.app/logs` no Windows).
fn init_logging(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = app.path().app_log_dir()?;
    std::fs::create_dir_all(&log_dir)?;
    let file_appender = tracing_appender::rolling::daily(&log_dir, "retrosync.log");

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer())
        .with(fmt::layer().with_ansi(false).with_writer(file_appender))
        .init();

    Ok(())
}
