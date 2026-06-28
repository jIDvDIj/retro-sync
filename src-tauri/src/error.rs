//! Tipo de erro unificado do backend. Comandos Tauri retornam `AppResult<T>`;
//! o erro cruza a boundary serializado como `{ code, message }` (ver
//! `AppErrorPayload` em `src/types/ipc.ts`).

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum AppError {
    #[error("erro de IO: {0}")]
    Io(#[from] std::io::Error),

    #[error("erro de banco de dados: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("erro de rede: {0}")]
    Network(#[from] reqwest::Error),

    // No mobile o keyring do SO não está disponível; os segredos ficam no
    // SQLite privado do app (ver `secrets::SqliteSecretStore`).
    #[cfg(desktop)]
    #[error("erro no cofre de credenciais: {0}")]
    Keyring(#[from] keyring::Error),

    #[error("erro de serialização: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("erro de autenticação: {0}")]
    Auth(String),

    #[error("emulador não reconhecido em: {0}")]
    EmulatorNotDetected(String),

    #[error("já existe um emulador com este nome: {0}")]
    EmulatorExists(String),

    #[error("arquivo em uso (modificado durante a leitura): {0}")]
    FileBusy(String),

    #[error("{0}")]
    Other(String),
}

impl AppError {
    fn code(&self) -> &'static str {
        match self {
            AppError::Io(_) => "io",
            AppError::Database(_) => "database",
            AppError::Network(_) => "network",
            #[cfg(desktop)]
            AppError::Keyring(_) => "keyring",
            AppError::Serialization(_) => "serialization",
            AppError::Auth(_) => "auth",
            AppError::EmulatorNotDetected(_) => "emulator_not_detected",
            AppError::EmulatorExists(_) => "emulator_exists",
            AppError::FileBusy(_) => "file_busy",
            AppError::Other(_) => "other",
        }
    }

    /// Detalhe técnico do erro (caminho, nome, mensagem da lib subjacente), sem
    /// o prefixo em português. O frontend localiza o prefixo pelo `code` e anexa
    /// este detalhe. `Other` não tem prefixo — todo o texto vem aqui.
    fn detail(&self) -> String {
        match self {
            AppError::Io(e) => e.to_string(),
            AppError::Database(e) => e.to_string(),
            AppError::Network(e) => e.to_string(),
            #[cfg(desktop)]
            AppError::Keyring(e) => e.to_string(),
            AppError::Serialization(e) => e.to_string(),
            AppError::Auth(s)
            | AppError::EmulatorNotDetected(s)
            | AppError::EmulatorExists(s)
            | AppError::FileBusy(s)
            | AppError::Other(s) => s.clone(),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 3)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.serialize_field("detail", &self.detail())?;
        s.end()
    }
}
