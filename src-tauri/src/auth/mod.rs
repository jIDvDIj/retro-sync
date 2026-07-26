//! Autenticação com o Google via OAuth2 + PKCE.
//!
//! `AuthManager` é a única porta de entrada: fluxo interativo de conexão,
//! status, desconexão e `access_token()` com renovação automática (usado
//! pelo módulo `drive`). Tokens nunca cruzam a boundary — o frontend só
//! recebe `AuthStatus`.

#![allow(dead_code)]

mod oauth;
mod token_store;

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{AppError, AppResult};
use crate::secrets::SecretStore;
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
    secrets: Arc<dyn SecretStore>,
    /// Token "sempre renovável" para testes de retry: quando setado, uma
    /// invalidação (401) é seguida por uma renovação sem OAuth real — os
    /// testes de `send_with_retry` não precisam mockar o endpoint do Google.
    #[cfg(test)]
    test_fixed_token: RwLock<Option<String>>,
}

impl AuthManager {
    pub fn new(http: reqwest::Client, secrets: Arc<dyn SecretStore>) -> Self {
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
            secrets,
            #[cfg(test)]
            test_fixed_token: RwLock::new(None),
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
        let secrets = self.secrets.clone();
        run_blocking(move || TokenStore::save(&stored, &*secrets)).await?;

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

    /// Variante mobile do fluxo interativo: usa deep link em vez de TCP loopback.
    /// O chamador (comando Tauri) configura o listener e passa o receptor do canal.
    #[cfg(mobile)]
    pub async fn connect_mobile<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        redirect_rx: tokio::sync::oneshot::Receiver<String>,
    ) -> AppResult<AuthStatus> {
        let config = self.config()?;
        let tokens =
            oauth::authorize_interactive_mobile(&self.http, config, app, redirect_rx).await?;

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
        let secrets = self.secrets.clone();
        run_blocking(move || TokenStore::save(&stored, &*secrets)).await?;

        self.cache_token(&tokens).await;
        tracing::info!(
            email = email.as_deref().unwrap_or("?"),
            "conectado ao Google Drive (mobile)"
        );

        Ok(AuthStatus {
            connected: true,
            email,
        })
    }

    /// Conectado = existe refresh token no keyring (não exige rede).
    pub async fn status(&self) -> AppResult<AuthStatus> {
        let secrets = self.secrets.clone();
        let stored = run_blocking(move || TokenStore::load(&*secrets)).await?;
        Ok(match stored {
            Some(auth) => AuthStatus {
                connected: true,
                email: auth.email,
            },
            None => AuthStatus::disconnected(),
        })
    }

    pub async fn disconnect(&self) -> AppResult<AuthStatus> {
        let secrets = self.secrets.clone();
        run_blocking(move || TokenStore::clear(&*secrets)).await?;
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

        #[cfg(test)]
        if let Some(token) = self.test_fixed_token.read().await.clone() {
            *self.cached.write().await = Some(CachedToken {
                access_token: token.clone(),
                expires_at: Instant::now() + Duration::from_secs(3600),
            });
            return Ok(token);
        }

        let config = self.config()?;
        let secrets = self.secrets.clone();
        let stored = run_blocking(move || TokenStore::load(&*secrets))
            .await?
            .ok_or_else(|| AppError::Auth("não conectado ao Google Drive".into()))?;

        let tokens = oauth::refresh_access_token(&self.http, config, &stored.refresh_token).await?;
        self.cache_token(&tokens).await;
        tracing::debug!("access token renovado");
        Ok(tokens.access_token)
    }

    /// Popula o access token em cache diretamente, sem OAuth — evita que os
    /// testes do `DriveClient` precisem mockar também o endpoint de refresh.
    #[cfg(test)]
    pub(crate) async fn set_test_access_token(&self, token: &str) {
        *self.test_fixed_token.write().await = Some(token.to_string());
        *self.cached.write().await = Some(CachedToken {
            access_token: token.to_string(),
            expires_at: Instant::now() + Duration::from_secs(3600),
        });
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::token_store::{StoredAuth, TokenStore};
    use super::AuthManager;
    use crate::constants::KEYRING_REFRESH_TOKEN_KEY;
    use crate::secrets::{MemSecrets, SecretStore};

    fn manager(secrets: &Arc<MemSecrets>) -> AuthManager {
        AuthManager::new(reqwest::Client::new(), secrets.clone())
    }

    #[tokio::test]
    async fn status_desconectado_sem_token_salvo() {
        let secrets = Arc::new(MemSecrets::default());
        let status = manager(&secrets).status().await.unwrap();
        assert!(!status.connected);
        assert!(status.email.is_none());
    }

    #[tokio::test]
    async fn status_conectado_le_email_do_token_salvo() {
        let secrets = Arc::new(MemSecrets::default());
        TokenStore::save(
            &StoredAuth {
                refresh_token: "tok".into(),
                email: Some("dev@retrosync".into()),
            },
            &*secrets,
        )
        .unwrap();

        let status = manager(&secrets).status().await.unwrap();
        assert!(status.connected);
        assert_eq!(status.email.as_deref(), Some("dev@retrosync"));
    }

    #[tokio::test]
    async fn token_ilegivel_degrada_para_desconectado() {
        let secrets = Arc::new(MemSecrets::default());
        secrets
            .set(KEYRING_REFRESH_TOKEN_KEY, "não é json")
            .unwrap();

        let status = manager(&secrets).status().await.unwrap();
        assert!(!status.connected, "token corrompido não pode conectar");
    }

    #[tokio::test]
    async fn disconnect_apaga_o_token_persistido() {
        let secrets = Arc::new(MemSecrets::default());
        TokenStore::save(
            &StoredAuth {
                refresh_token: "tok".into(),
                email: None,
            },
            &*secrets,
        )
        .unwrap();

        let m = manager(&secrets);
        assert!(m.status().await.unwrap().connected);

        let after = m.disconnect().await.unwrap();
        assert!(!after.connected);
        assert!(secrets.get(KEYRING_REFRESH_TOKEN_KEY).unwrap().is_none());
    }

    #[test]
    fn token_store_roundtrip_persiste_e_limpa() {
        let secrets = MemSecrets::default();
        let auth = StoredAuth {
            refresh_token: "abc".into(),
            email: Some("x@y".into()),
        };

        TokenStore::save(&auth, &secrets).unwrap();
        let loaded = TokenStore::load(&secrets).unwrap().unwrap();
        assert_eq!(loaded.refresh_token, "abc");
        assert_eq!(loaded.email.as_deref(), Some("x@y"));

        TokenStore::clear(&secrets).unwrap();
        assert!(TokenStore::load(&secrets).unwrap().is_none());
    }
}
