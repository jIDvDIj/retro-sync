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

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .on_window_event(|window, event| {
            // Fechar a janela apenas a esconde — o app continua vivo na
            // bandeja. O sync de despedida roda no "Sair" do menu da tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == constants::MAIN_WINDOW_LABEL {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            init_logging(app.handle())?;
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "RetroSync iniciado");

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = storage::db::Db::open(&data_dir.join(constants::LOCAL_DB_FILE))?;

            let last_sync: sync::LastSyncStore = Arc::new(std::sync::Mutex::new(None));
            let http = reqwest::Client::new();
            let auth = Arc::new(auth::AuthManager::new(http.clone()));
            let drive = Arc::new(drive::DriveClient::new(http, auth.clone()));
            let engine = Arc::new(sync::SyncEngine::new(
                db.clone(),
                drive,
                auth.clone(),
                app.handle().clone(),
                last_sync.clone(),
                data_dir.join(constants::LOCAL_BACKUP_DIR),
            ));

            app.manage(AppState {
                auth,
                db: db.clone(),
                engine: engine.clone(),
                last_sync,
            });

            setup_tray(app.handle())?;

            // Process watcher: dispara sync ao abrir/fechar um emulador.
            let startup_db = db.clone();
            watcher::start(db, engine.clone(), app.handle().clone());

            // Gatilho "ao iniciar o RetroSync": sync bidirecional em background,
            // se o usuário não tiver desativado o gatilho `startup`.
            tauri::async_runtime::spawn(async move {
                let enabled = startup_db
                    .with(storage::settings::triggers)
                    .await
                    .map(|t| t.startup)
                    .unwrap_or(true);
                if !enabled {
                    tracing::info!("gatilho startup desativado; sync de inicialização ignorado");
                    return;
                }
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
            commands::sync_now,
            commands::get_last_sync,
            commands::get_settings,
            commands::set_device_name,
            commands::set_triggers,
            commands::set_notification_level,
            commands::open_backup_folder,
            commands::get_emulator_categories,
            commands::set_emulator_categories,
            commands::list_conflicts,
            commands::resolve_conflict
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o RetroSync");
}

/// Configura o ícone da bandeja e o menu de contexto (Abrir / Sincronizar
/// agora / Sair).
fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let open = MenuItem::with_id(app, constants::TRAY_MENU_OPEN, "Abrir", true, None::<&str>)?;
    let sync = MenuItem::with_id(
        app,
        constants::TRAY_MENU_SYNC,
        "Sincronizar agora",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, constants::TRAY_MENU_QUIT, "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&open, &sync, &PredefinedMenuItem::separator(app)?, &quit],
    )?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("ícone padrão da janela ausente")?;

    TrayIconBuilder::with_id("retrosync-tray")
        .icon(icon)
        .tooltip("RetroSync")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_tray_menu_event)
        .on_tray_icon_event(|tray, event| {
            // Clique esquerdo simples reabre a janela.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn on_tray_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id.as_ref();
    if id == constants::TRAY_MENU_OPEN {
        show_main_window(app);
    } else if id == constants::TRAY_MENU_SYNC {
        spawn_sync(app.clone(), constants::TRIGGER_MANUAL, false);
    } else if id == constants::TRAY_MENU_QUIT {
        spawn_sync(app.clone(), constants::TRIGGER_SHUTDOWN, true);
    }
}

/// Mostra e foca a janela principal, restaurando-a se estiver oculta/minimizada.
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(constants::MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Dispara um sync bidirecional em background. Se `then_exit`, encerra o app
/// ao terminar — é o sync de despedida do menu "Sair".
fn spawn_sync(app: AppHandle, trigger: &'static str, then_exit: bool) {
    tauri::async_runtime::spawn(async move {
        let engine = app.state::<AppState>().engine.clone();
        if let Err(err) = engine
            .sync_all(sync::SyncDirection::Bidirectional, trigger)
            .await
        {
            tracing::warn!(trigger, error = %err, "sync acionado pela bandeja falhou");
        }
        if then_exit {
            app.exit(0);
        }
    });
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
