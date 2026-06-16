//! Constantes globais do RetroSync — nomes de pastas do Drive, chaves do
//! keyring e parâmetros de runtime. Nenhum magic string fora daqui.

#![allow(dead_code)]

/// Pasta raiz criada no Google Drive do usuário.
pub const DRIVE_ROOT_FOLDER: &str = "RetroSync";

/// Subpastas criadas dentro de `RetroSync/<Emulador>/`.
pub const DRIVE_SAVES_FOLDER: &str = "saves";
pub const DRIVE_STATES_FOLDER: &str = "savestates";
pub const DRIVE_CONFIG_FOLDER: &str = "config";

/// Snapshot do manifest publicado no Drive a cada sync (a fonte de verdade
/// operacional é a tabela SQLite local).
pub const DRIVE_MANIFEST_FILE: &str = "sync_manifest.json";

/// Chave de `appProperties` (privada ao app) que marca, em cada arquivo do
/// Drive, o dispositivo que publicou aquela versão — usada na resolução de
/// conflito para mostrar a origem da versão remota.
pub const DRIVE_APP_PROP_DEVICE: &str = "device";

/// Arquivo SQLite local (criado no diretório de dados do app).
pub const LOCAL_DB_FILE: &str = "retrosync.db";

/// Pasta de backups locais (criada no diretório de dados do app). Recebe o
/// arquivo local antes de ser sobrescrito no primeiro sync (BUG-001).
pub const LOCAL_BACKUP_DIR: &str = "backups";

/// Identificação das credenciais no keychain do SO.
pub const KEYRING_SERVICE: &str = "com.retrosync.app";
pub const KEYRING_REFRESH_TOKEN_KEY: &str = "google_drive_refresh_token";

/// Intervalo de polling do process watcher.
pub const WATCHER_POLL_INTERVAL_SECS: u64 = 2;

/// Ticks consecutivos sem o processo antes de declarar o emulador encerrado.
/// Debounce contra flapping; a abertura é detectada sem atraso. Com 2 ticks
/// de 2s, são ~4s de ausência confirmada antes do sync Local → Drive.
pub const WATCHER_STOP_DEBOUNCE_TICKS: u32 = 2;

/// Máximo de tentativas (com backoff exponencial) por chamada à API do Drive.
pub const DRIVE_MAX_RETRIES: u32 = 3;

/// Máximo de transferências simultâneas com o Drive.
pub const DRIVE_MAX_CONCURRENT_TRANSFERS: usize = 3;

/// Sufixo de arquivos temporários de download (gravação atômica via rename).
/// O scan local ignora arquivos com este sufixo.
pub const TMP_SUFFIX: &str = ".retrosync-tmp";

/// Identificação dos gatilhos de sync (logs e evento `sync:started`).
pub const TRIGGER_STARTUP: &str = "startup";
pub const TRIGGER_SHUTDOWN: &str = "shutdown";
pub const TRIGGER_MANUAL: &str = "manual";
pub const TRIGGER_EMULATOR_START: &str = "emulator-start";
pub const TRIGGER_EMULATOR_STOP: &str = "emulator-stop";

/// Chaves da tabela `app_settings` (configurações globais do usuário).
/// Nome amigável deste dispositivo (ex.: "PC Gamer"), definido no login.
pub const SETTING_DEVICE_NAME: &str = "device_name";

/// Gatilhos de sync automático ligáveis/desligáveis (default: todos ligados).
pub const SETTING_TRIGGER_STARTUP: &str = "trigger_startup";
pub const SETTING_TRIGGER_EMULATOR_START: &str = "trigger_emulator_start";
pub const SETTING_TRIGGER_EMULATOR_STOP: &str = "trigger_emulator_stop";

/// Nível de notificações nativas: all | errors_only | none (default: all).
pub const SETTING_NOTIFICATION_LEVEL: &str = "notification_level";

/// Marca que o default de fábrica do autostart (ligado) já foi aplicado na
/// primeira execução. Impede religar o autostart a cada inicialização — depois
/// disso a escolha do usuário prevalece, inclusive se ele desativar.
pub const SETTING_AUTOSTART_INITIALIZED: &str = "autostart_initialized";

/// Label da janela principal (definida pelo Tauri quando não há `label`).
pub const MAIN_WINDOW_LABEL: &str = "main";

/// Argumento que o lançador do SO injeta quando o app sobe junto com o sistema
/// (registrado pelo plugin de autostart). Com ele o app inicia direto na
/// bandeja, sem abrir a janela principal.
pub const STARTUP_MINIMIZED_FLAG: &str = "--minimized";

/// IDs dos itens do menu da bandeja do sistema.
pub const TRAY_MENU_OPEN: &str = "tray-open";
pub const TRAY_MENU_SYNC: &str = "tray-sync";
pub const TRAY_MENU_QUIT: &str = "tray-quit";
