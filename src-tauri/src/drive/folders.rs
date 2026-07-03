//! Criação idempotente da estrutura de pastas no Drive, com cache de IDs.
//!
//! `RetroSync/` na raiz do Drive, `RetroSync/<Emulador>/<categoria>/` por
//! emulador, e subpastas arbitrárias sob a categoria (`ensure_subpath`)
//! para espelhar a árvore local nos uploads.

use serde_json::json;

use super::{DriveClient, DriveFile, DRIVE_API_BASE, FILE_FIELDS, FOLDER_MIME_TYPE};
use crate::constants::DRIVE_ROOT_FOLDER;
use crate::error::AppResult;
use crate::sync::SyncCategory;

/// Alias da API do Drive para a raiz "Meu Drive".
const MY_DRIVE_ROOT_ID: &str = "root";

impl DriveClient {
    pub async fn ensure_root(&self) -> AppResult<String> {
        self.ensure_folder_cached(MY_DRIVE_ROOT_ID, DRIVE_ROOT_FOLDER, DRIVE_ROOT_FOLDER)
            .await
    }

    /// Garante `RetroSync/<emulator>/<categoria>` e retorna o ID da categoria.
    pub async fn ensure_category_folder(
        &self,
        emulator: &str,
        category: SyncCategory,
    ) -> AppResult<String> {
        let root_id = self.ensure_root().await?;
        let emulator_key = format!("{DRIVE_ROOT_FOLDER}/{emulator}");
        let emulator_id = self
            .ensure_folder_cached(&root_id, emulator, &emulator_key)
            .await?;
        let category_key = format!("{emulator_key}/{}", category.as_str());
        self.ensure_folder_cached(&emulator_id, category.as_str(), &category_key)
            .await
    }

    /// Garante a cadeia de subpastas `rel_dir` (separador `/`) sob `base_id`.
    pub async fn ensure_subpath(
        &self,
        base_id: &str,
        base_key: &str,
        rel_dir: &str,
    ) -> AppResult<String> {
        let mut current_id = base_id.to_string();
        let mut current_key = base_key.to_string();
        for segment in rel_dir.split('/').filter(|s| !s.is_empty()) {
            current_key = format!("{current_key}/{segment}");
            current_id = self
                .ensure_folder_cached(&current_id, segment, &current_key)
                .await?;
        }
        Ok(current_id)
    }

    async fn ensure_folder_cached(
        &self,
        parent_id: &str,
        name: &str,
        cache_key: &str,
    ) -> AppResult<String> {
        if let Some(id) = self.folder_cache.read().await.get(cache_key) {
            return Ok(id.clone());
        }

        let folder = match self.find_folder(parent_id, name).await? {
            Some(existing) => existing,
            None => {
                tracing::info!(path = cache_key, "criando pasta no Drive");
                self.create_folder(parent_id, name).await?
            }
        };

        self.folder_cache
            .write()
            .await
            .insert(cache_key.to_string(), folder.id.clone());

        // Espelha o ID no SQLite para sobreviver a reinícios (FEATURE-006).
        // Best-effort: uma falha aqui só faz o próximo boot re-resolver esta pasta.
        let (key, id) = (cache_key.to_string(), folder.id.clone());
        if let Err(err) = self
            .db
            .with(move |conn| crate::storage::drive_folders::upsert(conn, &key, &id))
            .await
        {
            tracing::warn!(error = %err, path = cache_key, "falha ao persistir ID de pasta do Drive");
        }

        Ok(folder.id)
    }

    async fn find_folder(&self, parent_id: &str, name: &str) -> AppResult<Option<DriveFile>> {
        self.find_child_filtered(parent_id, name, Some(FOLDER_MIME_TYPE))
            .await
    }

    async fn create_folder(&self, parent_id: &str, name: &str) -> AppResult<DriveFile> {
        let url = format!("{DRIVE_API_BASE}/files");
        let metadata = json!({
            "name": name,
            "mimeType": FOLDER_MIME_TYPE,
            "parents": [parent_id],
        });
        let response = self
            .send_with_retry("folders.create", |token| {
                self.http
                    .post(&url)
                    .bearer_auth(token)
                    .query(&[("fields", FILE_FIELDS)])
                    .json(&metadata)
            })
            .await?;
        Ok(response.json::<DriveFile>().await?)
    }
}
