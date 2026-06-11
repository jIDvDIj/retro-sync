//! Estado global gerenciado pelo Tauri (`app.manage(AppState)`), acessado
//! pelos comandos via `tauri::State<AppState>`. Os próximos passos adicionam
//! aqui: conexão SQLite (Passo 5), handle do SyncEngine (Passo 5) e do
//! process watcher (Passo 6).

use crate::auth::AuthManager;

pub struct AppState {
    /// Cliente HTTP compartilhado (pool de conexões) — clonável e barato.
    /// Consumido pelo módulo `drive` a partir do Passo 5.
    #[allow(dead_code)]
    pub http: reqwest::Client,
    pub auth: AuthManager,
}

impl AppState {
    pub fn new() -> Self {
        let http = reqwest::Client::new();
        Self {
            auth: AuthManager::new(http.clone()),
            http,
        }
    }
}
