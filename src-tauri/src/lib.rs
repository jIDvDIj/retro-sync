mod auth;
mod commands;
mod constants;
mod device;
mod drive;
mod emulator;
mod error;
mod events;
mod state;
mod storage;
mod sync;
// O process watcher depende de inspecionar processos do SO (`sysinfo`), o que
// não existe/aplica no mobile — gatilhos automáticos são exclusivos do desktop.
#[cfg(desktop)]
mod watcher;

use std::sync::Arc;

#[cfg(desktop)]
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
#[cfg(desktop)]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
#[cfg(desktop)]
use tauri::{AppHandle, WindowEvent};
#[cfg(desktop)]
use tauri_plugin_autostart::ManagerExt;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init());

    // Recursos só-desktop registrados no builder: autostart ("subir com o
    // sistema") e o fechar-esconde da janela (o app segue vivo na bandeja). No
    // mobile não há bandeja nem ciclo de janela equivalente.
    #[cfg(desktop)]
    {
        builder = builder
            // Início automático com o sistema. Ao subir junto com o login, o SO
            // lança o app com `--minimized` para ele ficar só na bandeja.
            .plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                Some(vec![constants::STARTUP_MINIMIZED_FLAG]),
            ))
            .on_window_event(|window, event| {
                // Fechar a janela apenas a esconde — o app continua vivo na
                // bandeja. O sync de despedida roda no "Sair" do menu da tray.
                if let WindowEvent::CloseRequested { api, .. } = event {
                    if window.label() == constants::MAIN_WINDOW_LABEL {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            });
    }

    builder
        .setup(|app| {
            init_logging(app.handle())?;
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "RetroSync iniciado");

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = storage::db::Db::open(&data_dir.join(constants::LOCAL_DB_FILE))?;

            // Garante a identidade estável deste dispositivo (UUID no keyring,
            // gerado na primeira execução; consumido na detecção de conflito).
            // Keyring indisponível não é fatal — só logamos o aviso.
            match device::get_or_create() {
                Ok(id) => tracing::info!(device_id = %id, "device_id resolvido"),
                Err(err) => tracing::warn!(
                    error = %err,
                    "device_id indisponível (keyring); seguindo sem identidade estável"
                ),
            }

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
                Arc::new(sync::DesktopStorage),
            ));

            app.manage(AppState {
                auth,
                db: db.clone(),
                engine: engine.clone(),
                last_sync,
            });

            // Bandeja, janela escondível, autostart e process watcher são
            // exclusivos do desktop. No mobile o webview único já é exibido pelo
            // sistema e os gatilhos automáticos por processo não existem.
            #[cfg(desktop)]
            {
                setup_tray(app.handle())?;

                // A janela nasce oculta (`visible: false` no tauri.conf.json). Em
                // abertura normal nós a mostramos; quando o SO sobe o app junto com
                // o sistema (flag `--minimized`), ele fica só na bandeja.
                let launched_minimized =
                    std::env::args().any(|a| a == constants::STARTUP_MINIMIZED_FLAG);
                if !launched_minimized {
                    if let Some(window) = app.get_webview_window(constants::MAIN_WINDOW_LABEL) {
                        let _ = window.show();
                    }
                }

                // Process watcher: dispara sync ao abrir/fechar um emulador.
                watcher::start(db.clone(), engine.clone(), app.handle().clone());

                // Default de fábrica: na primeiríssima execução registramos o
                // autostart para o app subir junto com o sistema. Aplicado uma única
                // vez (flag no banco); depois disso a escolha do usuário prevalece,
                // mesmo que ele desative pelo app ou pelo Gerenciador de Tarefas.
                let autostart_db = db.clone();
                let autostart_app = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let already = autostart_db
                        .with(storage::settings::autostart_initialized)
                        .await
                        .unwrap_or(true); // em erro de leitura, não mexe no estado do SO
                    if already {
                        return;
                    }
                    // `State` (de `autolaunch()`) não é `Send`: usa numa statement
                    // isolada para não atravessar um `.await`.
                    let enabled = autostart_app.autolaunch().enable();
                    match enabled {
                        Ok(()) => {
                            let _ = autostart_db
                                .with(storage::settings::mark_autostart_initialized)
                                .await;
                            tracing::info!("autostart ativado por padrão (primeira execução)");
                        }
                        Err(err) => {
                            tracing::warn!(error = %err, "autostart padrão não pôde ser ativado");
                        }
                    }
                });
            }

            // Gatilho "ao iniciar o RetroSync": sync bidirecional em background,
            // se o usuário não tiver desativado o gatilho `startup`. Vale para
            // desktop e mobile (no mobile é o sync ao abrir o app).
            let startup_db = db.clone();
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
            commands::add_emulator_manual,
            commands::discover_emulators,
            commands::list_emulators,
            commands::remove_emulator,
            commands::sync_now,
            commands::get_last_sync,
            commands::get_settings,
            commands::set_device_name,
            commands::set_triggers,
            commands::set_notification_level,
            commands::set_autostart,
            commands::open_backup_folder,
            commands::get_emulator_categories,
            commands::set_emulator_categories,
            commands::list_conflicts,
            commands::resolve_conflict
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o RetroSync");
}

/// Configura o ícone da bandeja e o menu de contexto (Open / Sync now / Quit).
/// Os rótulos ficam em inglês — o menu nativo é construído uma vez no startup,
/// fora do alcance do i18n do frontend.
#[cfg(desktop)]
fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let open = MenuItem::with_id(app, constants::TRAY_MENU_OPEN, "Open", true, None::<&str>)?;
    let sync = MenuItem::with_id(
        app,
        constants::TRAY_MENU_SYNC,
        "Sync now",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, constants::TRAY_MENU_QUIT, "Quit", true, None::<&str>)?;
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

#[cfg(desktop)]
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
#[cfg(desktop)]
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(constants::MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Dispara um sync bidirecional em background. Se `then_exit`, encerra o app
/// ao terminar — é o sync de despedida do menu "Sair".
#[cfg(desktop)]
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
