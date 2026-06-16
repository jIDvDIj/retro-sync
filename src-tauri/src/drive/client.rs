//! Camada de transporte: toda chamada à API do Drive passa por
//! `send_with_retry` — backoff exponencial com jitter, no máximo
//! `DRIVE_MAX_RETRIES` tentativas, renovação de token em 401 e tratamento
//! de rate limit (429/403 *RateLimitExceeded*/5xx).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use tokio::sync::RwLock;

use crate::auth::AuthManager;
use crate::constants::DRIVE_MAX_RETRIES;
use crate::error::{AppError, AppResult};

pub struct DriveClient {
    pub(crate) http: reqwest::Client,
    pub(crate) auth: Arc<AuthManager>,
    /// Cache de IDs de pastas por caminho lógico (ex.: "RetroSync/PPSSPP/saves").
    pub(crate) folder_cache: RwLock<HashMap<String, String>>,
}

impl DriveClient {
    pub fn new(http: reqwest::Client, auth: Arc<AuthManager>) -> Self {
        Self {
            http,
            auth,
            folder_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Envia a requisição construída por `build` (que recebe o access token),
    /// aplicando a política de retry. `build` é chamada de novo a cada
    /// tentativa para reconstruir o request do zero.
    pub(crate) async fn send_with_retry<F>(
        &self,
        op_name: &str,
        build: F,
    ) -> AppResult<reqwest::Response>
    where
        F: Fn(&str) -> reqwest::RequestBuilder,
    {
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            let token = self.auth.access_token().await?;

            match build(&token).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        return Ok(response);
                    }

                    if status == reqwest::StatusCode::UNAUTHORIZED && attempt < DRIVE_MAX_RETRIES {
                        tracing::debug!(op_name, "401 do Drive; renovando access token");
                        self.auth.invalidate_cached_token().await;
                        continue;
                    }

                    let body = response.text().await.unwrap_or_default();
                    let rate_limited = status == reqwest::StatusCode::TOO_MANY_REQUESTS
                        || (status == reqwest::StatusCode::FORBIDDEN
                            && body.contains("ateLimitExceeded"));

                    if (rate_limited || status.is_server_error()) && attempt < DRIVE_MAX_RETRIES {
                        let delay = backoff_delay(attempt);
                        tracing::warn!(op_name, %status, attempt, ?delay, "Drive instável; aguardando retry");
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    return Err(AppError::Other(format!(
                        "Drive {op_name} falhou ({status}): {body}"
                    )));
                }
                Err(err) => {
                    if attempt < DRIVE_MAX_RETRIES {
                        let delay = backoff_delay(attempt);
                        tracing::warn!(op_name, error = %err, attempt, ?delay, "falha de rede; aguardando retry");
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(err.into());
                }
            }
        }
    }
}

/// 500ms, 1s, 2s... + jitter de até 250ms.
fn backoff_delay(attempt: u32) -> Duration {
    let base = 500u64.saturating_mul(2u64.saturating_pow(attempt.saturating_sub(1)));
    let jitter = rand::thread_rng().gen_range(0..250);
    Duration::from_millis(base + jitter)
}
