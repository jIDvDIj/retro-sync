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

use tauri::Manager;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(state::AppState::new())
        .setup(|app| {
            init_logging(app.handle())?;
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "RetroSync iniciado");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health_check,
            commands::connect_google_drive,
            commands::get_auth_status,
            commands::disconnect_google_drive
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
