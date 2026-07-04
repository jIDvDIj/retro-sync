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
use crate::storage::db::Db;
use crate::storage::drive_folders;

pub struct DriveClient {
    pub(crate) http: reqwest::Client,
    pub(crate) auth: Arc<AuthManager>,
    /// Banco local — espelha o `folder_cache` na tabela `drive_folders` para que
    /// os IDs sobrevivam a reinícios (FEATURE-006).
    pub(crate) db: Db,
    /// Cache de IDs de pastas por caminho lógico (ex.: "RetroSync/PPSSPP/saves").
    /// Semente carregada do SQLite no boot; escrito a cada ID novo resolvido.
    pub(crate) folder_cache: RwLock<HashMap<String, String>>,
}

impl DriveClient {
    pub fn new(http: reqwest::Client, auth: Arc<AuthManager>, db: Db) -> Self {
        // Popula o cache com os IDs persistidos: o primeiro sync após o boot pula
        // a re-resolução das pastas já conhecidas (FEATURE-006).
        let seed = db
            .with_conn_blocking(drive_folders::load_all)
            .unwrap_or_else(|err| {
                tracing::warn!(error = %err, "cache de pastas do Drive indisponível; seguindo vazio");
                HashMap::new()
            });
        if !seed.is_empty() {
            tracing::debug!(pastas = seed.len(), "cache de IDs de pasta do Drive restaurado do SQLite");
        }
        Self {
            http,
            auth,
            db,
            folder_cache: RwLock::new(seed),
        }
    }

    /// Invalida um caminho lógico de pasta e sua subárvore no cache (memória +
    /// SQLite). Chamado quando uma operação encontra `notFound` num ID cacheado
    /// (pasta movida/apagada no Drive); a próxima resolução reencontra ou recria.
    pub async fn invalidate_folder_path(&self, cache_key: &str) {
        let prefix = format!("{cache_key}/");
        self.folder_cache
            .write()
            .await
            .retain(|k, _| k != cache_key && !k.starts_with(&prefix));
        let key = cache_key.to_string();
        if let Err(err) = self
            .db
            .with(move |conn| drive_folders::remove_subtree(conn, &key))
            .await
        {
            tracing::warn!(error = %err, cache_key, "falha ao invalidar cache de pasta no SQLite");
        }
    }

    /// Zera todo o cache de pastas (logout/troca de conta — os IDs são por conta
    /// Google e ficam inválidos ao autenticar com outra).
    pub async fn clear_folder_cache(&self) {
        self.folder_cache.write().await.clear();
        if let Err(err) = self.db.with(drive_folders::clear).await {
            tracing::warn!(error = %err, "falha ao limpar cache de pastas no SQLite");
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

                    // 404: o objeto (arquivo/pasta) não existe mais. Erro tipado
                    // para o engine invalidar o cache de pastas e re-resolver
                    // quando um ID cacheado ficou obsoleto (FEATURE-006).
                    if status == reqwest::StatusCode::NOT_FOUND {
                        return Err(AppError::DriveObjectNotFound(format!("{op_name}: {body}")));
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
