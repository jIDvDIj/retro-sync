//! Autenticação com o Google via OAuth2 + PKCE (Passo 3).
//!
//! Responsabilidades:
//! - Fluxo de autorização com redirect loopback (sem client secret);
//! - Armazenamento do refresh token no keychain do SO (`keyring`);
//! - Renovação automática do access token, transparente para o frontend.
//!
//! Tokens nunca cruzam a boundary: o frontend só recebe `AuthStatus`.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Estado da conexão com o Google Drive exposto ao frontend.
/// Espelhado em `src/types/ipc.ts` (`AuthStatus`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub connected: bool,
    pub email: Option<String>,
}
