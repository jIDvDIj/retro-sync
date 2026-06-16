//! Operações de arquivo: listagem recursiva, download e upload.
//!
//! Uploads sempre definem `modifiedTime` = mtime local do arquivo, mantendo
//! a comparação de timestamps coerente entre máquinas. Arquivos acima de
//! `SIMPLE_UPLOAD_MAX_BYTES` usam sessão resumable.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;

use super::{
    ms_to_rfc3339, DriveClient, DRIVE_API_BASE, DRIVE_UPLOAD_BASE, FILE_FIELDS, FOLDER_MIME_TYPE,
    LIST_FIELDS, OCTET_STREAM, SIMPLE_UPLOAD_MAX_BYTES,
};
use crate::constants::DRIVE_APP_PROP_DEVICE;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub modified_time: Option<DateTime<Utc>>,
    /// A API devolve int64 como string.
    #[serde(default)]
    #[allow(dead_code)]
    pub size: Option<String>,
    /// Propriedades privadas do app (ex.: `device` = quem publicou a versão).
    #[serde(default)]
    pub app_properties: HashMap<String, String>,
}

impl DriveFile {
    /// Dispositivo que publicou esta versão, se gravado em `appProperties`.
    pub fn device(&self) -> Option<&str> {
        self.app_properties
            .get(DRIVE_APP_PROP_DEVICE)
            .map(String::as_str)
    }
}

/// Adiciona `appProperties.device` ao metadata de upload, quando há nome de
/// dispositivo definido. Identifica a origem de cada versão no Drive.
fn with_device(metadata: &mut serde_json::Value, device: Option<&str>) {
    if let Some(dev) = device {
        let mut props = serde_json::Map::new();
        props.insert(
            DRIVE_APP_PROP_DEVICE.to_string(),
            serde_json::Value::String(dev.to_string()),
        );
        metadata["appProperties"] = serde_json::Value::Object(props);
    }
}

impl DriveFile {
    pub fn is_folder(&self) -> bool {
        self.mime_type == FOLDER_MIME_TYPE
    }

    pub fn modified_ms(&self) -> Option<i64> {
        self.modified_time.map(|t| t.timestamp_millis())
    }
}

/// Arquivo remoto com caminho relativo à pasta de categoria (separador `/`).
#[derive(Debug, Clone)]
pub struct RemoteFile {
    pub rel_path: String,
    pub file: DriveFile,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileList {
    #[serde(default)]
    files: Vec<DriveFile>,
    next_page_token: Option<String>,
}

impl DriveClient {
    async fn list_children(&self, folder_id: &str) -> AppResult<Vec<DriveFile>> {
        let url = format!("{DRIVE_API_BASE}/files");
        let query = format!("'{folder_id}' in parents and trashed = false");
        let mut out = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let token_param = page_token.clone();
            let response = self
                .send_with_retry("files.list", |token| {
                    let mut request = self.http.get(&url).bearer_auth(token).query(&[
                        ("q", query.as_str()),
                        ("fields", LIST_FIELDS),
                        ("pageSize", "1000"),
                    ]);
                    if let Some(t) = token_param.as_deref() {
                        request = request.query(&[("pageToken", t)]);
                    }
                    request
                })
                .await?;

            let page: FileList = response.json().await?;
            out.extend(page.files);
            match page.next_page_token {
                Some(next) => page_token = Some(next),
                None => break,
            }
        }
        Ok(out)
    }

    /// Lista recursivamente todos os arquivos sob `folder_id`, com caminhos
    /// relativos (`sub/pasta/arquivo.ext`).
    pub async fn list_tree(&self, folder_id: &str) -> AppResult<Vec<RemoteFile>> {
        let mut out = Vec::new();
        let mut pending = vec![(folder_id.to_string(), String::new())];

        while let Some((id, prefix)) = pending.pop() {
            for child in self.list_children(&id).await? {
                let rel_path = format!("{prefix}{}", child.name);
                if child.is_folder() {
                    pending.push((child.id.clone(), format!("{rel_path}/")));
                } else {
                    out.push(RemoteFile {
                        rel_path,
                        file: child,
                    });
                }
            }
        }
        Ok(out)
    }

    /// Filho direto por nome (sem recursão); `mime_type` opcionalmente filtra.
    pub(crate) async fn find_child_filtered(
        &self,
        folder_id: &str,
        name: &str,
        mime_type: Option<&str>,
    ) -> AppResult<Option<DriveFile>> {
        let url = format!("{DRIVE_API_BASE}/files");
        let mut query = format!("name = '{name}' and '{folder_id}' in parents and trashed = false");
        if let Some(mime) = mime_type {
            query.push_str(&format!(" and mimeType = '{mime}'"));
        }

        let response = self
            .send_with_retry("files.find", |token| {
                self.http.get(&url).bearer_auth(token).query(&[
                    ("q", query.as_str()),
                    ("fields", LIST_FIELDS),
                    ("pageSize", "1"),
                    // Determinístico: se houver duplicatas (criadas por uma
                    // versão anterior com bug de corrida), converge sempre para
                    // a mais antiga em vez de escolher uma ao acaso.
                    ("orderBy", "createdTime"),
                ])
            })
            .await?;

        let page: FileList = response.json().await?;
        Ok(page.files.into_iter().next())
    }

    pub async fn find_child(&self, folder_id: &str, name: &str) -> AppResult<Option<DriveFile>> {
        self.find_child_filtered(folder_id, name, None).await
    }

    pub async fn download(&self, file_id: &str) -> AppResult<Vec<u8>> {
        let url = format!("{DRIVE_API_BASE}/files/{file_id}");
        let response = self
            .send_with_retry("files.download", |token| {
                self.http
                    .get(&url)
                    .bearer_auth(token)
                    .query(&[("alt", "media")])
            })
            .await?;
        Ok(response.bytes().await?.to_vec())
    }

    /// Cria um arquivo novo em `parent_id` preservando o mtime original e
    /// marcando o dispositivo de origem em `appProperties`.
    pub async fn upload_new(
        &self,
        parent_id: &str,
        name: &str,
        content: Vec<u8>,
        mtime_ms: i64,
        device: Option<&str>,
    ) -> AppResult<DriveFile> {
        let mut metadata = json!({
            "name": name,
            "parents": [parent_id],
            "modifiedTime": ms_to_rfc3339(mtime_ms),
        });
        with_device(&mut metadata, device);
        if content.len() > SIMPLE_UPLOAD_MAX_BYTES {
            let url = format!("{DRIVE_UPLOAD_BASE}/files");
            self.upload_resumable(reqwest::Method::POST, &url, &metadata, content)
                .await
        } else {
            let url = format!("{DRIVE_UPLOAD_BASE}/files");
            self.upload_multipart(reqwest::Method::POST, &url, &metadata, content)
                .await
        }
    }

    /// Atualiza o conteúdo de um arquivo existente preservando o mtime e
    /// atualizando o dispositivo de origem em `appProperties`.
    pub async fn upload_existing(
        &self,
        file_id: &str,
        content: Vec<u8>,
        mtime_ms: i64,
        device: Option<&str>,
    ) -> AppResult<DriveFile> {
        let mut metadata = json!({ "modifiedTime": ms_to_rfc3339(mtime_ms) });
        with_device(&mut metadata, device);
        let url = format!("{DRIVE_UPLOAD_BASE}/files/{file_id}");
        if content.len() > SIMPLE_UPLOAD_MAX_BYTES {
            self.upload_resumable(reqwest::Method::PATCH, &url, &metadata, content)
                .await
        } else {
            self.upload_multipart(reqwest::Method::PATCH, &url, &metadata, content)
                .await
        }
    }

    async fn upload_multipart(
        &self,
        method: reqwest::Method,
        url: &str,
        metadata: &serde_json::Value,
        content: Vec<u8>,
    ) -> AppResult<DriveFile> {
        let (boundary, body) = build_multipart_related(metadata, &content)?;
        let content_type = format!("multipart/related; boundary={boundary}");

        let response = self
            .send_with_retry("files.upload", |token| {
                self.http
                    .request(method.clone(), url)
                    .bearer_auth(token)
                    .query(&[("uploadType", "multipart"), ("fields", FILE_FIELDS)])
                    .header(reqwest::header::CONTENT_TYPE, content_type.clone())
                    .body(body.clone())
            })
            .await?;
        Ok(response.json::<DriveFile>().await?)
    }

    /// Sessão resumable: o initiate tem retry completo; o PUT do conteúdo é
    /// tentativa única — se cair, a pendência fica na fila e o próximo sync
    /// refaz a operação inteira.
    async fn upload_resumable(
        &self,
        method: reqwest::Method,
        url: &str,
        metadata: &serde_json::Value,
        content: Vec<u8>,
    ) -> AppResult<DriveFile> {
        let initiate = self
            .send_with_retry("files.upload.initiate", |token| {
                self.http
                    .request(method.clone(), url)
                    .bearer_auth(token)
                    .query(&[("uploadType", "resumable"), ("fields", FILE_FIELDS)])
                    .header("X-Upload-Content-Type", OCTET_STREAM)
                    .header("X-Upload-Content-Length", content.len().to_string())
                    .json(metadata)
            })
            .await?;

        let session_url = initiate
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                AppError::Other("upload resumable sem header Location na resposta".into())
            })?
            .to_string();

        let response = self
            .http
            .put(&session_url)
            .header(reqwest::header::CONTENT_TYPE, OCTET_STREAM)
            .body(content)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Other(format!(
                "upload resumable falhou ({status}): {body}"
            )));
        }
        Ok(response.json::<DriveFile>().await?)
    }
}

/// Monta o corpo `multipart/related` exigido pelo upload com metadata
/// (o `multipart` do reqwest é form-data, que a API do Drive não aceita).
fn build_multipart_related(
    metadata: &serde_json::Value,
    content: &[u8],
) -> AppResult<(String, Vec<u8>)> {
    let boundary = format!("retrosync-{:016x}", rand::random::<u64>());
    let metadata_json = serde_json::to_vec(metadata)?;

    let mut body = Vec::with_capacity(content.len() + metadata_json.len() + 256);
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
    body.extend_from_slice(&metadata_json);
    body.extend_from_slice(format!("\r\n--{boundary}\r\n").as_bytes());
    body.extend_from_slice(format!("Content-Type: {OCTET_STREAM}\r\n\r\n").as_bytes());
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    Ok((boundary, body))
}
