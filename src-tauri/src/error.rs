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

    #[error("erro no cofre de credenciais: {0}")]
    Keyring(#[from] keyring::Error),

    #[error("erro de serialização: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("erro de autenticação: {0}")]
    Auth(String),

    #[error("emulador não reconhecido em: {0}")]
    EmulatorNotDetected(String),

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
            AppError::Keyring(_) => "keyring",
            AppError::Serialization(_) => "serialization",
            AppError::Auth(_) => "auth",
            AppError::EmulatorNotDetected(_) => "emulator_not_detected",
            AppError::FileBusy(_) => "file_busy",
            AppError::Other(_) => "other",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}
