//! `DriveApi` — a porta do `SyncEngine` para o Drive (issue #82).
//!
//! O engine depende deste trait, nunca do `DriveClient` concreto: em produção
//! o `DriveClient` (HTTP real) o implementa por delegação; nos testes, o
//! `MockDrive` (`drive::mock`) opera sobre um mapa em memória, permitindo
//! exercitar o engine de ponta a ponta sem rede e sem credenciais.

use async_trait::async_trait;

use super::{BatchUploadOp, DeviceTag, DriveClient, DriveFile, RemoteFile};
use crate::error::AppResult;
use crate::sync::SyncCategory;

/// Operações do Google Drive das quais o `SyncEngine` depende.
///
/// Espelha 1:1 os métodos públicos do `DriveClient` consumidos pelo engine —
/// novas necessidades entram aqui primeiro, mantendo o mock em sincronia.
#[async_trait]
pub trait DriveApi: Send + Sync {
    /// Garante `RetroSync/` na raiz do Drive e retorna seu ID.
    async fn ensure_root(&self) -> AppResult<String>;

    /// Garante `RetroSync/<emulator>/<categoria>` e retorna o ID da categoria.
    async fn ensure_category_folder(
        &self,
        emulator: &str,
        category: SyncCategory,
    ) -> AppResult<String>;

    /// Garante a cadeia de subpastas `rel_dir` (separador `/`) sob `base_id`.
    async fn ensure_subpath(
        &self,
        base_id: &str,
        base_key: &str,
        rel_dir: &str,
    ) -> AppResult<String>;

    /// Lista recursivamente os arquivos sob `folder_id`, com caminhos relativos.
    async fn list_tree(&self, folder_id: &str) -> AppResult<Vec<RemoteFile>>;

    /// Filho direto por nome (sem recursão).
    async fn find_child(&self, folder_id: &str, name: &str) -> AppResult<Option<DriveFile>>;

    /// Baixa o conteúdo inteiro de um arquivo.
    async fn download(&self, file_id: &str) -> AppResult<Vec<u8>>;

    /// Cria um arquivo novo preservando o mtime e marcando o dispositivo.
    async fn upload_new(
        &self,
        parent_id: &str,
        name: &str,
        content: Vec<u8>,
        mtime_ms: i64,
        device: DeviceTag<'_>,
    ) -> AppResult<DriveFile>;

    /// Atualiza o conteúdo de um arquivo existente preservando o mtime.
    async fn upload_existing(
        &self,
        file_id: &str,
        content: Vec<u8>,
        mtime_ms: i64,
        device: DeviceTag<'_>,
    ) -> AppResult<DriveFile>;

    /// Envia arquivos novos e pequenos em um único `multipart/mixed`
    /// (FEATURE-004). Retorna os `DriveFile` na MESMA ordem das operações.
    async fn upload_batch(&self, ops: Vec<BatchUploadOp>) -> AppResult<Vec<DriveFile>>;

    /// Invalida um caminho lógico de pasta e sua subárvore no cache.
    async fn invalidate_folder_path(&self, cache_key: &str);

    /// Zera todo o cache de pastas (logout/troca de conta).
    async fn clear_folder_cache(&self);
}

#[async_trait]
impl DriveApi for DriveClient {
    async fn ensure_root(&self) -> AppResult<String> {
        DriveClient::ensure_root(self).await
    }

    async fn ensure_category_folder(
        &self,
        emulator: &str,
        category: SyncCategory,
    ) -> AppResult<String> {
        DriveClient::ensure_category_folder(self, emulator, category).await
    }

    async fn ensure_subpath(
        &self,
        base_id: &str,
        base_key: &str,
        rel_dir: &str,
    ) -> AppResult<String> {
        DriveClient::ensure_subpath(self, base_id, base_key, rel_dir).await
    }

    async fn list_tree(&self, folder_id: &str) -> AppResult<Vec<RemoteFile>> {
        DriveClient::list_tree(self, folder_id).await
    }

    async fn find_child(&self, folder_id: &str, name: &str) -> AppResult<Option<DriveFile>> {
        DriveClient::find_child(self, folder_id, name).await
    }

    async fn download(&self, file_id: &str) -> AppResult<Vec<u8>> {
        DriveClient::download(self, file_id).await
    }

    async fn upload_new(
        &self,
        parent_id: &str,
        name: &str,
        content: Vec<u8>,
        mtime_ms: i64,
        device: DeviceTag<'_>,
    ) -> AppResult<DriveFile> {
        DriveClient::upload_new(self, parent_id, name, content, mtime_ms, device).await
    }

    async fn upload_existing(
        &self,
        file_id: &str,
        content: Vec<u8>,
        mtime_ms: i64,
        device: DeviceTag<'_>,
    ) -> AppResult<DriveFile> {
        DriveClient::upload_existing(self, file_id, content, mtime_ms, device).await
    }

    async fn upload_batch(&self, ops: Vec<BatchUploadOp>) -> AppResult<Vec<DriveFile>> {
        DriveClient::upload_batch(self, ops).await
    }

    async fn invalidate_folder_path(&self, cache_key: &str) {
        DriveClient::invalidate_folder_path(self, cache_key).await
    }

    async fn clear_folder_cache(&self) {
        DriveClient::clear_folder_cache(self).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::auth::AuthManager;
    use crate::secrets::MemSecrets;
    use crate::storage::db::Db;
    use crate::storage::drive_folders;

    /// Exercita a delegação do trait para o `DriveClient` real pelos caminhos
    /// que dispensam rede (cache persistido + batch vazio). Uma delegação
    /// trocada (copy/paste para o método errado) falharia aqui. Os métodos
    /// restantes (list/find/download/upload_*) exigem HTTP real e ficam para
    /// os testes atrás da feature `integration-tests`.
    #[tokio::test]
    async fn delegacao_para_o_cliente_real_nos_caminhos_sem_rede() {
        let db = Db::open_in_memory().unwrap();
        db.with_sync(|conn| {
            for (key, id) in [
                ("RetroSync", "id-root"),
                ("RetroSync/PPSSPP", "id-emu"),
                ("RetroSync/PPSSPP/saves", "id-saves"),
                ("RetroSync/PPSSPP/saves/jogo", "id-jogo"),
            ] {
                drive_folders::upsert(conn, key, id)?;
            }
            Ok(())
        });
        let auth = Arc::new(AuthManager::new(
            reqwest::Client::new(),
            Arc::new(MemSecrets::default()),
        ));
        let client = DriveClient::new(reqwest::Client::new(), auth, db);
        let api: &dyn DriveApi = &client;

        assert_eq!(api.ensure_root().await.unwrap(), "id-root");
        assert_eq!(
            api.ensure_category_folder("PPSSPP", SyncCategory::Saves)
                .await
                .unwrap(),
            "id-saves"
        );
        assert_eq!(
            api.ensure_subpath("id-saves", "RetroSync/PPSSPP/saves", "jogo")
                .await
                .unwrap(),
            "id-jogo"
        );
        assert!(api.upload_batch(Vec::new()).await.unwrap().is_empty());

        api.invalidate_folder_path("RetroSync/PPSSPP").await;
        api.clear_folder_cache().await;
        assert!(client.folder_cache.read().await.is_empty());
    }
}
