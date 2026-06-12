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

/// Arquivo SQLite local (criado no diretório de dados do app).
pub const LOCAL_DB_FILE: &str = "retrosync.db";

/// Identificação das credenciais no keychain do SO.
pub const KEYRING_SERVICE: &str = "com.retrosync.app";
pub const KEYRING_REFRESH_TOKEN_KEY: &str = "google_drive_refresh_token";

/// Intervalo de polling do process watcher.
pub const WATCHER_POLL_INTERVAL_SECS: u64 = 2;

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
