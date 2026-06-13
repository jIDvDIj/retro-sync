//! Autenticação com o Google via OAuth2 + PKCE.
//!
//! `AuthManager` é a única porta de entrada: fluxo interativo de conexão,
//! status, desconexão e `access_token()` com renovação automática (usado
//! pelo módulo `drive`). Tokens nunca cruzam a boundary — o frontend só
//! recebe `AuthStatus`.

#![allow(dead_code)]

mod oauth;
mod token_store;

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{AppError, AppResult};
use oauth::OAuthConfig;
use token_store::{StoredAuth, TokenStore};

/// Estado da conexão com o Google Drive exposto ao frontend.
/// Espelhado em `src/types/ipc.ts` (`AuthStatus`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub connected: bool,
    pub email: Option<String>,
}

impl AuthStatus {
    fn disconnected() -> Self {
        Self {
            connected: false,
            email: None,
        }
    }
}

/// Renova o access token quando faltar menos que isso para expirar.
const TOKEN_EXPIRY_MARGIN: Duration = Duration::from_secs(60);

struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

pub struct AuthManager {
    http: reqwest::Client,
    config: Option<OAuthConfig>,
    cached: RwLock<Option<CachedToken>>,
}

impl AuthManager {
    pub fn new(http: reqwest::Client) -> Self {
        let config = OAuthConfig::from_env();
        if config.is_none() {
            tracing::warn!(
                "RETROSYNC_GOOGLE_CLIENT_ID não configurado; conexão ao Drive indisponível"
            );
        }
        Self {
            http,
            config,
            cached: RwLock::new(None),
        }
    }

    fn config(&self) -> AppResult<&OAuthConfig> {
        self.config.as_ref().ok_or_else(|| {
            AppError::Auth(
                "Client ID do Google não configurado — defina RETROSYNC_GOOGLE_CLIENT_ID (veja o README)"
                    .into(),
            )
        })
    }

    /// Fluxo interativo completo: navegador → consentimento → tokens.
    /// Persiste o refresh token no keyring e retorna o novo status.
    pub async fn connect(&self) -> AppResult<AuthStatus> {
        let config = self.config()?;
        let tokens = oauth::authorize_interactive(&self.http, config).await?;

        let refresh_token = tokens.refresh_token.clone().ok_or_else(|| {
            AppError::Auth(
                "o Google não retornou um refresh token; revogue o acesso do RetroSync em \
                 myaccount.google.com/permissions e conecte novamente"
                    .into(),
            )
        })?;

        let email = oauth::fetch_user_email(&self.http, &tokens.access_token)
            .await
            .unwrap_or(None);

        let stored = StoredAuth {
            refresh_token,
            email: email.clone(),
        };
        run_blocking(move || TokenStore::save(&stored)).await?;

        self.cache_token(&tokens).await;
        tracing::info!(
            email = email.as_deref().unwrap_or("?"),
            "conectado ao Google Drive"
        );

        Ok(AuthStatus {
            connected: true,
            email,
        })
    }

    /// Conectado = existe refresh token no keyring (não exige rede).
    pub async fn status(&self) -> AppResult<AuthStatus> {
        let stored = run_blocking(TokenStore::load).await?;
        Ok(match stored {
            Some(auth) => AuthStatus {
                connected: true,
                email: auth.email,
            },
            None => AuthStatus::disconnected(),
        })
    }

    pub async fn disconnect(&self) -> AppResult<AuthStatus> {
        run_blocking(TokenStore::clear).await?;
        *self.cached.write().await = None;
        tracing::info!("desconectado do Google Drive");
        Ok(AuthStatus::disconnected())
    }

    /// Access token válido, renovando automaticamente quando necessário.
    /// API interna para o módulo `drive` — nunca exposta ao frontend.
    pub async fn access_token(&self) -> AppResult<String> {
        if let Some(cached) = self.cached.read().await.as_ref() {
            if cached.expires_at > Instant::now() + TOKEN_EXPIRY_MARGIN {
                return Ok(cached.access_token.clone());
            }
        }

        let config = self.config()?;
        let stored = run_blocking(TokenStore::load)
            .await?
            .ok_or_else(|| AppError::Auth("não conectado ao Google Drive".into()))?;

        let tokens = oauth::refresh_access_token(&self.http, config, &stored.refresh_token).await?;
        self.cache_token(&tokens).await;
        tracing::debug!("access token renovado");
        Ok(tokens.access_token)
    }

    /// Descarta o access token em cache (ex.: após um 401 do Drive),
    /// forçando renovação via refresh token na próxima chamada.
    pub async fn invalidate_cached_token(&self) {
        *self.cached.write().await = None;
    }

    async fn cache_token(&self, tokens: &oauth::TokenResponse) {
        *self.cached.write().await = Some(CachedToken {
            access_token: tokens.access_token.clone(),
            expires_at: Instant::now() + Duration::from_secs(tokens.expires_in),
        });
    }
}

async fn run_blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Other(format!("tarefa bloqueante abortada: {e}")))?
}
