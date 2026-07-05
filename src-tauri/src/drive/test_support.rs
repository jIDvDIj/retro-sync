//! Helper compartilhado pelos testes de HTTP do `DriveClient` (`drive::files`,
//! `drive::client`): um cliente autenticado (token seedado, sem OAuth)
//! apontando para um `wiremock::MockServer` local em vez do Google real.

use std::sync::Arc;

use wiremock::MockServer;

use super::DriveClient;
use crate::auth::AuthManager;
use crate::secrets::{MemSecrets, SecretStore};
use crate::storage::db::Db;

pub(crate) async fn client_against(server: &MockServer) -> DriveClient {
    let db = Db::open_in_memory().unwrap();
    let secrets: Arc<dyn SecretStore> = Arc::new(MemSecrets::default());
    let auth = Arc::new(AuthManager::new(reqwest::Client::new(), secrets));
    auth.set_test_access_token("tok-teste").await;
    DriveClient::new(reqwest::Client::new(), auth, db).with_base_url(&server.uri())
}
