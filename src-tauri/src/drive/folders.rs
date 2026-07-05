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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::auth::AuthManager;
    use crate::drive::DriveClient;
    use crate::secrets::MemSecrets;
    use crate::storage::db::Db;
    use crate::storage::drive_folders;

    /// Cliente sem rede: qualquer ID que os testes precisem deve vir do cache
    /// persistido em `drive_folders` (carregado no `DriveClient::new`).
    fn client_with_seed(seed: &[(&str, &str)]) -> DriveClient {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            for (key, id) in seed {
                drive_folders::upsert(conn, key, id)?;
            }
            Ok(())
        });
        let auth = Arc::new(AuthManager::new(
            reqwest::Client::new(),
            Arc::new(MemSecrets::default()),
        ));
        DriveClient::new(reqwest::Client::new(), auth, db)
    }

    #[tokio::test]
    async fn ensure_root_resolve_pelo_cache_persistido() {
        let client = client_with_seed(&[("RetroSync", "id-root")]);
        assert_eq!(client.ensure_root().await.unwrap(), "id-root");
    }

    #[tokio::test]
    async fn ensure_category_folder_resolve_a_cadeia_cacheada() {
        let client = client_with_seed(&[
            ("RetroSync", "id-root"),
            ("RetroSync/PPSSPP", "id-emu"),
            ("RetroSync/PPSSPP/saves", "id-saves"),
        ]);
        let id = client
            .ensure_category_folder("PPSSPP", crate::sync::SyncCategory::Saves)
            .await
            .unwrap();
        assert_eq!(id, "id-saves");
    }

    #[tokio::test]
    async fn ensure_subpath_percorre_segmentos_cacheados() {
        let client = client_with_seed(&[
            ("RetroSync/PPSSPP/saves/jogo", "id-jogo"),
            ("RetroSync/PPSSPP/saves/jogo/slot1", "id-slot1"),
        ]);
        let id = client
            .ensure_subpath("id-saves", "RetroSync/PPSSPP/saves", "jogo/slot1")
            .await
            .unwrap();
        assert_eq!(id, "id-slot1");
    }

    #[tokio::test]
    async fn invalidate_folder_path_remove_apenas_a_subarvore() {
        let client = client_with_seed(&[
            ("RetroSync", "id-root"),
            ("RetroSync/PPSSPP", "id-emu"),
            ("RetroSync/PPSSPP/saves", "id-saves"),
            ("RetroSync/PCSX2", "id-outro"),
        ]);

        client.invalidate_folder_path("RetroSync/PPSSPP").await;

        // Memória: subárvore fora, vizinhos ficam.
        let cache = client.folder_cache.read().await;
        assert!(!cache.contains_key("RetroSync/PPSSPP"));
        assert!(!cache.contains_key("RetroSync/PPSSPP/saves"));
        assert!(cache.contains_key("RetroSync"));
        assert!(cache.contains_key("RetroSync/PCSX2"));
        drop(cache);

        // SQLite espelhado: sobrevive a reinício com o mesmo estado.
        let persisted = client
            .db
            .with_conn_blocking(drive_folders::load_all)
            .unwrap();
        assert!(!persisted.contains_key("RetroSync/PPSSPP"));
        assert!(persisted.contains_key("RetroSync/PCSX2"));
    }

    #[tokio::test]
    async fn clear_folder_cache_zera_memoria_e_sqlite() {
        let client = client_with_seed(&[("RetroSync", "id-root"), ("RetroSync/PPSSPP", "id-emu")]);

        client.clear_folder_cache().await;

        assert!(client.folder_cache.read().await.is_empty());
        let persisted = client
            .db
            .with_conn_blocking(drive_folders::load_all)
            .unwrap();
        assert!(persisted.is_empty());
    }
}
