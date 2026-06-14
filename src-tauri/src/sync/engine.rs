//! `SyncEngine` — orquestração da sincronização bidirecional.
//!
//! Agnóstico a emuladores: opera sobre `SyncTarget` (rótulo + listas de
//! caminhos). Por categoria: garante as pastas no Drive, lista a árvore
//! remota, varre o estado local, monta o plano via `diff`/`conflict` e
//! executa as transferências com concorrência limitada, emitindo progresso
//! ao frontend. Falhas de rede/arquivo em uso vão para a fila offline.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use futures::stream::{self, StreamExt};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::Mutex;

use super::conflict::SyncAction;
use super::diff::{self, PlannedOp};
use super::{SyncCategory, SyncDirection, SyncProgress, SyncTarget};
use crate::auth::AuthManager;
use crate::constants::{DRIVE_MANIFEST_FILE, DRIVE_MAX_CONCURRENT_TRANSFERS, TMP_SUFFIX};
use crate::drive::DriveClient;
use crate::error::{AppError, AppResult};
use crate::events::{EVT_SYNC_COMPLETED, EVT_SYNC_ERROR, EVT_SYNC_PROGRESS, EVT_SYNC_STARTED};
use crate::storage::db::Db;
use crate::storage::manifest::{self, ManifestEntry};
use crate::storage::{emulators, queue};

/// Resultado agregado de um sync. Espelhado em `src/types/ipc.ts`.
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummary {
    pub uploaded: u32,
    pub downloaded: u32,
    pub skipped: u32,
    pub failed: u32,
    pub queued: u32,
    pub duration_ms: u64,
}

impl SyncSummary {
    fn merge(&mut self, other: &SyncSummary) {
        self.uploaded += other.uploaded;
        self.downloaded += other.downloaded;
        self.skipped += other.skipped;
        self.failed += other.failed;
        self.queued += other.queued;
    }
}

/// Payload do evento `sync:started`. Espelhado em `src/types/ipc.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStarted {
    pub trigger: String,
    pub direction: SyncDirection,
}

/// Payload do evento `sync:error`. Espelhado em `src/types/ipc.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncError {
    pub emulator: Option<String>,
    pub message: String,
}

/// Resumo do último sync concluído, exposto à UI via `get_last_sync` (e
/// atualizado ao vivo pelo evento `sync:completed`). Espelhado em
/// `src/types/ipc.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastSync {
    pub at_ms: i64,
    pub trigger: String,
    pub summary: SyncSummary,
}

/// Célula compartilhada entre o `SyncEngine` (escreve) e o `AppState`
/// (lê via comando). `std::sync::Mutex` basta: o lock é curto e sem `await`.
pub type LastSyncStore = Arc<std::sync::Mutex<Option<LastSync>>>;

enum OpOutcome {
    Uploaded,
    Downloaded,
    Queued,
    Failed,
}

struct CategoryCtx {
    emulator: String,
    category: SyncCategory,
    direction: SyncDirection,
    /// Pasta da categoria no Drive e sua chave de cache.
    folder_id: String,
    folder_key: String,
    /// Destino de downloads de arquivos que ainda não existem localmente
    /// (primeira pasta-base da categoria).
    download_base: PathBuf,
    total: u32,
    completed: AtomicU32,
}

pub struct SyncEngine {
    db: Db,
    drive: Arc<DriveClient>,
    auth: Arc<AuthManager>,
    app: AppHandle,
    last_sync: LastSyncStore,
    /// Serializa execuções: um sync por vez, os demais aguardam.
    running: Mutex<()>,
}

impl SyncEngine {
    pub fn new(
        db: Db,
        drive: Arc<DriveClient>,
        auth: Arc<AuthManager>,
        app: AppHandle,
        last_sync: LastSyncStore,
    ) -> Self {
        Self {
            db,
            drive,
            auth,
            app,
            last_sync,
            running: Mutex::new(()),
        }
    }

    /// Sincroniza todos os emuladores configurados.
    pub async fn sync_all(
        &self,
        direction: SyncDirection,
        trigger: &str,
    ) -> AppResult<SyncSummary> {
        self.sync_filtered(None, direction, trigger).await
    }

    /// Sincroniza um único emulador (gatilhos do process watcher).
    pub async fn sync_emulator(
        &self,
        name: &str,
        direction: SyncDirection,
        trigger: &str,
    ) -> AppResult<SyncSummary> {
        self.sync_filtered(Some(name), direction, trigger).await
    }

    async fn sync_filtered(
        &self,
        only: Option<&str>,
        direction: SyncDirection,
        trigger: &str,
    ) -> AppResult<SyncSummary> {
        let _guard = self.running.lock().await;

        let status = self.auth.status().await?;
        if !status.connected {
            return Err(AppError::Auth(
                "não conectado ao Google Drive — sync ignorado".into(),
            ));
        }

        let notif = self
            .db
            .with(crate::storage::settings::notification_level)
            .await
            .unwrap_or_default();

        let profiles = self.db.with(emulators::list).await?;
        // Por emulador: monta o target e remove as categorias que o usuário
        // desativou nas configurações (default: todas ativas).
        let mut targets: Vec<SyncTarget> = Vec::new();
        for profile in profiles
            .iter()
            .filter(|p| only.is_none_or(|name| p.name == name))
        {
            let name = profile.name.clone();
            let cats = self
                .db
                .with(move |conn| emulators::get_categories(conn, &name))
                .await?;
            let mut target = SyncTarget::from_profile(profile);
            target.categories.retain(|(category, _)| match category {
                SyncCategory::Saves => cats.saves,
                SyncCategory::Savestates => cats.savestates,
                SyncCategory::Config => cats.config,
            });
            targets.push(target);
        }
        if targets.is_empty() {
            tracing::info!(trigger, "nenhum emulador configurado; nada a sincronizar");
            return Ok(SyncSummary::default());
        }

        let started_at = Instant::now();
        tracing::info!(
            trigger,
            ?direction,
            emuladores = targets.len(),
            "sync iniciado"
        );
        let _ = self.app.emit(
            EVT_SYNC_STARTED,
            &SyncStarted {
                trigger: trigger.to_string(),
                direction,
            },
        );

        let mut summary = SyncSummary::default();
        for target in &targets {
            match self.sync_target(target, direction).await {
                Ok(partial) => summary.merge(&partial),
                Err(err) => {
                    summary.failed += 1;
                    tracing::error!(emulador = %target.label, error = %err, "sync do emulador falhou");
                    let _ = self.app.emit(
                        EVT_SYNC_ERROR,
                        &SyncError {
                            emulator: Some(target.label.clone()),
                            message: err.to_string(),
                        },
                    );
                    if notif.notifies_errors() {
                        self.notify_error(&target.label, &err.to_string());
                    }
                }
            }
        }

        if let Err(err) = self.publish_manifest_snapshot().await {
            tracing::warn!(error = %err, "falha ao publicar sync_manifest.json no Drive");
        }

        summary.duration_ms = started_at.elapsed().as_millis() as u64;
        tracing::info!(?summary, trigger, "sync concluído");

        let last = LastSync {
            at_ms: chrono::Utc::now().timestamp_millis(),
            trigger: trigger.to_string(),
            summary: summary.clone(),
        };
        if let Ok(mut guard) = self.last_sync.lock() {
            *guard = Some(last);
        }

        // Notifica a conclusão só quando houve transferência — evita "sync
        // concluído" repetido em syncs automáticos que nada fizeram.
        if notif.notifies_info() && (summary.uploaded + summary.downloaded > 0) {
            self.notify_completed(&summary);
        }

        let _ = self.app.emit(EVT_SYNC_COMPLETED, &summary);
        Ok(summary)
    }

    /// Notificação nativa do SO de sync concluído (nível `all`).
    fn notify_completed(&self, summary: &SyncSummary) {
        if let Err(err) = self
            .app
            .notification()
            .builder()
            .title("RetroSync — sincronização concluída")
            .body(format!(
                "↑ {} enviados · ↓ {} baixados",
                summary.uploaded, summary.downloaded
            ))
            .show()
        {
            tracing::debug!(error = %err, "não foi possível exibir notificação nativa");
        }
    }

    /// Notificação nativa do SO para erro crítico de sync. Útil quando o
    /// gatilho é automático (startup/watcher/shutdown) e a janela está oculta.
    fn notify_error(&self, emulator: &str, message: &str) {
        if let Err(err) = self
            .app
            .notification()
            .builder()
            .title("RetroSync — falha na sincronização")
            .body(format!("{emulator}: {message}"))
            .show()
        {
            tracing::debug!(error = %err, "não foi possível exibir notificação nativa");
        }
    }

    async fn sync_target(
        &self,
        target: &SyncTarget,
        direction: SyncDirection,
    ) -> AppResult<SyncSummary> {
        let mut summary = SyncSummary::default();

        for (category, bases) in &target.categories {
            if bases.is_empty() {
                continue;
            }

            let folder_id = self
                .drive
                .ensure_category_folder(&target.label, *category)
                .await?;
            let folder_key = format!(
                "{}/{}/{}",
                crate::constants::DRIVE_ROOT_FOLDER,
                target.label,
                category.as_str()
            );

            let remote = self.drive.list_tree(&folder_id).await?;

            let (root, bases_owned) = (target.root.clone(), bases.clone());
            let local =
                tokio::task::spawn_blocking(move || diff::scan_local_bases(&root, &bases_owned))
                    .await
                    .map_err(|e| AppError::Other(format!("tarefa bloqueante abortada: {e}")))??;

            let (emulator, cat) = (target.label.clone(), *category);
            let manifest_entries = self
                .db
                .with(move |conn| manifest::list_for_category(conn, &emulator, cat))
                .await?;

            let (plan, skipped) = diff::build_plan(local, remote, manifest_entries, direction);
            summary.skipped += skipped;
            if plan.is_empty() {
                continue;
            }

            // Pré-cria as subpastas necessárias de forma sequencial, populando
            // o cache de IDs, ANTES dos uploads concorrentes. Sem isto, várias
            // tarefas paralelas do mesmo jogo passam juntas pelo "miss" do cache
            // e criam pastas duplicadas no Drive (uma por arquivo concorrente).
            for dir in upload_dirs(&plan) {
                self.drive
                    .ensure_subpath(&folder_id, &folder_key, dir)
                    .await?;
            }

            let ctx = CategoryCtx {
                emulator: target.label.clone(),
                category: *category,
                direction,
                folder_id,
                folder_key,
                download_base: target.root.join(&bases[0]),
                total: plan.len() as u32,
                completed: AtomicU32::new(0),
            };

            let outcomes = stream::iter(plan.into_iter().map(|op| self.execute_op(&ctx, op)))
                .buffer_unordered(DRIVE_MAX_CONCURRENT_TRANSFERS)
                .collect::<Vec<_>>()
                .await;

            for outcome in outcomes {
                match outcome {
                    OpOutcome::Uploaded => summary.uploaded += 1,
                    OpOutcome::Downloaded => summary.downloaded += 1,
                    OpOutcome::Queued => summary.queued += 1,
                    OpOutcome::Failed => summary.failed += 1,
                }
            }
        }

        Ok(summary)
    }

    async fn execute_op(&self, ctx: &CategoryCtx, op: PlannedOp) -> OpOutcome {
        let rel_path = op.rel_path.clone();
        let result = match op.action {
            SyncAction::Upload => self.do_upload(ctx, &op).await,
            SyncAction::Download => self.do_download(ctx, &op).await,
            SyncAction::NoOp => Ok(()),
        };

        let completed = ctx.completed.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.app.emit(
            EVT_SYNC_PROGRESS,
            &SyncProgress {
                emulator: ctx.emulator.clone(),
                current_file: rel_path.clone(),
                completed,
                total: ctx.total,
                direction: ctx.direction,
            },
        );

        match result {
            Ok(()) => {
                let (emulator, category, rel) = (ctx.emulator.clone(), ctx.category, rel_path);
                let _ = self
                    .db
                    .with(move |conn| queue::resolve(conn, &emulator, category, &rel))
                    .await;
                match op.action {
                    SyncAction::Upload => OpOutcome::Uploaded,
                    _ => OpOutcome::Downloaded,
                }
            }
            Err(err) => {
                let retryable = matches!(err, AppError::Network(_) | AppError::FileBusy(_));
                tracing::warn!(
                    emulador = %ctx.emulator,
                    arquivo = %rel_path,
                    error = %err,
                    retryable,
                    "operação de sync falhou"
                );
                if retryable {
                    let (emulator, category, rel) = (ctx.emulator.clone(), ctx.category, rel_path);
                    let direction = match op.action {
                        SyncAction::Upload => queue::OpDirection::Upload,
                        _ => queue::OpDirection::Download,
                    };
                    let message = err.to_string();
                    let _ = self
                        .db
                        .with(move |conn| {
                            queue::enqueue(conn, &emulator, category, &rel, direction, &message)
                        })
                        .await;
                    OpOutcome::Queued
                } else {
                    OpOutcome::Failed
                }
            }
        }
    }

    async fn do_upload(&self, ctx: &CategoryCtx, op: &PlannedOp) -> AppResult<()> {
        let local = op
            .local
            .as_ref()
            .ok_or_else(|| AppError::Other("upload planejado sem arquivo local".into()))?;

        let mtime_before = file_mtime_ms(&local.abs_path).await?;
        let content = tokio::fs::read(&local.abs_path).await?;
        let mtime_after = file_mtime_ms(&local.abs_path).await?;
        if mtime_before != mtime_after {
            return Err(AppError::FileBusy(local.rel_path.clone()));
        }

        let (dir_part, file_name) = split_rel_path(&op.rel_path);
        let parent_id = match dir_part {
            Some(dir) => {
                self.drive
                    .ensure_subpath(&ctx.folder_id, &ctx.folder_key, dir)
                    .await?
            }
            None => ctx.folder_id.clone(),
        };

        let size_bytes = content.len() as i64;
        let uploaded = match op.remote.as_ref() {
            Some(existing) => {
                self.drive
                    .upload_existing(&existing.id, content, mtime_after)
                    .await?
            }
            None => {
                self.drive
                    .upload_new(&parent_id, file_name, content, mtime_after)
                    .await?
            }
        };

        let drive_mtime = uploaded.modified_ms();
        self.record_synced(
            ctx,
            &op.rel_path,
            uploaded.id,
            mtime_after,
            drive_mtime,
            size_bytes,
        )
        .await
    }

    async fn do_download(&self, ctx: &CategoryCtx, op: &PlannedOp) -> AppResult<()> {
        let remote = op
            .remote
            .as_ref()
            .ok_or_else(|| AppError::Other("download planejado sem arquivo remoto".into()))?;

        let content = self.drive.download(&remote.id).await?;

        let dest = match op.local.as_ref() {
            Some(local) => local.abs_path.clone(),
            None => ctx.download_base.join(rel_to_native(&op.rel_path)),
        };
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Gravação atômica: temp + rename evita save corrompido se cair no meio.
        let tmp = dest.with_file_name(format!(
            "{}{TMP_SUFFIX}",
            dest.file_name().unwrap_or_default().to_string_lossy()
        ));
        let size_bytes = content.len() as i64;
        tokio::fs::write(&tmp, &content).await?;
        tokio::fs::rename(&tmp, &dest).await?;

        // mtime local = modifiedTime do Drive, para o diff convergir.
        let drive_mtime = remote.modified_ms();
        if let Some(ms) = drive_mtime {
            let ft =
                filetime::FileTime::from_unix_time(ms / 1000, ((ms % 1000) * 1_000_000) as u32);
            filetime::set_file_mtime(&dest, ft)?;
        }

        self.record_synced(
            ctx,
            &op.rel_path,
            remote.id.clone(),
            drive_mtime.unwrap_or(0),
            drive_mtime,
            size_bytes,
        )
        .await
    }

    async fn record_synced(
        &self,
        ctx: &CategoryCtx,
        rel_path: &str,
        drive_file_id: String,
        local_mtime_ms: i64,
        drive_mtime_ms: Option<i64>,
        size_bytes: i64,
    ) -> AppResult<()> {
        let entry = ManifestEntry {
            emulator: ctx.emulator.clone(),
            category: ctx.category,
            rel_path: rel_path.to_string(),
            drive_file_id: Some(drive_file_id),
            local_mtime_ms: Some(local_mtime_ms),
            drive_mtime_ms,
            size_bytes: Some(size_bytes),
            last_synced_at_ms: chrono::Utc::now().timestamp_millis(),
        };
        self.db
            .with(move |conn| manifest::upsert(conn, &entry))
            .await
    }

    /// Snapshot do manifest publicado na raiz `RetroSync/` (best-effort).
    /// Inclui o nome deste dispositivo — é o "metadado de sync no Drive" que
    /// identifica quem publicou esta versão (usado na resolução de conflito).
    async fn publish_manifest_snapshot(&self) -> AppResult<()> {
        let entries = self.db.with(manifest::list_all).await?;
        let device = self.db.with(crate::storage::settings::device_name).await?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let doc = serde_json::json!({
            "generatedAt": crate::drive::ms_to_rfc3339(now_ms),
            "device": device,
            "entries": entries,
        });
        let bytes = serde_json::to_vec_pretty(&doc)?;

        let root_id = self.drive.ensure_root().await?;
        match self.drive.find_child(&root_id, DRIVE_MANIFEST_FILE).await? {
            Some(existing) => {
                self.drive
                    .upload_existing(&existing.id, bytes, now_ms)
                    .await?;
            }
            None => {
                self.drive
                    .upload_new(&root_id, DRIVE_MANIFEST_FILE, bytes, now_ms)
                    .await?;
            }
        }
        Ok(())
    }
}

async fn file_mtime_ms(path: &std::path::Path) -> AppResult<i64> {
    let metadata = tokio::fs::metadata(path).await?;
    Ok(diff::system_time_ms(metadata.modified()?))
}

/// `"a/b/c.bin"` → `(Some("a/b"), "c.bin")`; `"c.bin"` → `(None, "c.bin")`.
fn split_rel_path(rel_path: &str) -> (Option<&str>, &str) {
    match rel_path.rsplit_once('/') {
        Some((dir, name)) => (Some(dir), name),
        None => (None, rel_path),
    }
}

/// Caminho relativo com `/` → `PathBuf` nativo da plataforma.
fn rel_to_native(rel_path: &str) -> PathBuf {
    rel_path.split('/').collect()
}

/// Diretórios (relativos à categoria) que precisam existir no Drive para os
/// uploads do plano — únicos e ordenados, para que pastas-pai sejam criadas
/// antes das filhas e cada uma só uma vez.
fn upload_dirs(plan: &[PlannedOp]) -> Vec<&str> {
    let mut dirs: Vec<&str> = plan
        .iter()
        .filter(|op| op.action == SyncAction::Upload)
        .filter_map(|op| op.rel_path.rsplit_once('/').map(|(dir, _)| dir))
        .collect();
    dirs.sort_unstable();
    dirs.dedup();
    dirs
}

#[cfg(test)]
mod tests {
    use super::upload_dirs;
    use crate::sync::conflict::SyncAction;
    use crate::sync::diff::PlannedOp;

    fn op(rel_path: &str, action: SyncAction) -> PlannedOp {
        PlannedOp {
            rel_path: rel_path.to_string(),
            action,
            local: None,
            remote: None,
        }
    }

    #[test]
    fn upload_dirs_dedup_e_ordena_apenas_uploads() {
        // Vários arquivos de uma mesma subpasta produzem o diretório uma só vez.
        let plan = vec![
            op("game-b/file1.bin", SyncAction::Upload),
            op("game-a/icon.png", SyncAction::Upload),
            op("game-a/param.sfo", SyncAction::Upload),
            op("game-a/data.bin", SyncAction::Upload),
            // download não cria pasta no Drive
            op("game-c/save.bin", SyncAction::Download),
        ];
        assert_eq!(upload_dirs(&plan), vec!["game-a", "game-b"]);
    }

    #[test]
    fn upload_dirs_ignora_arquivos_na_raiz_da_categoria() {
        // Arquivos sem subpasta (ex.: savestates soltos) não geram diretório.
        let plan = vec![
            op("state-0.bin", SyncAction::Upload),
            op("state-0.jpg", SyncAction::Upload),
        ];
        assert!(upload_dirs(&plan).is_empty());
    }
}
