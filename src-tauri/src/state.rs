//! Estado global gerenciado pelo Tauri (`app.manage(AppState)`), acessado
//! pelos comandos via `tauri::State<AppState>`. Os próximos passos adicionam
//! aqui: gerenciador de auth (Passo 3), conexão SQLite (Passo 5), handle do
//! SyncEngine (Passo 5) e do process watcher (Passo 6).

#[derive(Default)]
pub struct AppState {}
