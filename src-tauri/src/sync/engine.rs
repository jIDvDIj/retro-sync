//! `SyncEngine` — orquestração da sincronização bidirecional.
//!
//! Agnóstico a emuladores: opera sobre `SyncTarget` (rótulo + listas de
//! caminhos). Por categoria: garante as pastas no Drive, lista a árvore
//! remota, varre o estado local, monta o plano via `diff`/`conflict` e
//! executa as transferências com concorrência limitada, emitindo progresso
//! ao frontend. Falhas de rede/arquivo em uso vão para a fila offline.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime, Wry};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::Mutex;

use super::conflict::SyncAction;
use super::diff::{self, PlannedOp};
use super::storage::{FileLoc, LocalStorage};
use super::{SyncCategory, SyncDirection, SyncProgress, SyncTarget};
use crate::auth::AuthManager;
use crate::constants::{
    DRIVE_BATCH_MAX_OPS, DRIVE_BATCH_MIN_OPS, DRIVE_MANIFEST_FILE, DRIVE_MAX_CONCURRENT_TRANSFERS,
    DRIVE_SIMPLE_UPLOAD_MAX_BYTES,
};
use crate::drive::{BatchUploadOp, DeviceTag, DriveApi};
use crate::error::{AppError, AppResult};
use crate::events::{
    EVT_SYNC_COMPLETED, EVT_SYNC_CONFLICT, EVT_SYNC_ERROR, EVT_SYNC_PROGRESS, EVT_SYNC_STARTED,
};
use crate::storage::conflicts::{self, Conflict};
use crate::storage::db::Db;
use crate::storage::manifest::{self, ManifestEntry};
use crate::storage::settings::{self, NotificationLevel};
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
    /// Arquivos locais copiados para backup antes de serem sobrescritos no
    /// primeiro sync (BUG-001). `> 0` sinaliza à UI que há backups a oferecer.
    pub backed_up: u32,
    /// Conflitos detectados neste sync (ambos os lados mudaram — BUG-002).
    pub conflicts: u32,
    pub duration_ms: u64,
}

impl SyncSummary {
    fn merge(&mut self, other: &SyncSummary) {
        self.uploaded += other.uploaded;
        self.downloaded += other.downloaded;
        self.skipped += other.skipped;
        self.failed += other.failed;
        self.queued += other.queued;
        self.backed_up += other.backed_up;
        self.conflicts += other.conflicts;
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
    /// Download que também gerou um backup local (primeiro sync, BUG-001).
    DownloadedWithBackup,
    /// Conflito registrado; nenhuma transferência feita (BUG-002).
    Conflicted,
    Queued,
    Failed,
}

/// Escolha do usuário ao resolver um conflito. Espelhado em `src/types/ipc.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictResolution {
    /// Manter a versão local e enviá-la ao Drive.
    Local,
    /// Manter a versão do Drive e baixá-la (com backup do local).
    Drive,
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
    download_base: FileLoc,
    /// Pasta onde gravar backups locais desta categoria neste sync
    /// (`<backup_dir>/<emulador>/<timestamp>/<categoria>`).
    backup_base: FileLoc,
    /// Nome amigável deste dispositivo (marca a origem nos uploads e exibido
    /// nos conflitos).
    device: Option<String>,
    /// ID estável deste dispositivo (estampado nos uploads; alimenta a detecção
    /// de conflito entre dispositivos no primeiro sync).
    device_id: Option<String>,
    /// Nível de notificação vigente (gating da notificação de conflito).
    notif: NotificationLevel,
    total: u32,
    completed: AtomicU32,
    /// Total de bytes do plano e bytes já concluídos — para a UI mostrar
    /// progresso em bytes, velocidade e ETA (não só contagem de arquivos).
    bytes_total: u64,
    bytes_done: AtomicU64,
}

/// Genérico sobre o runtime do Tauri para ser testável: em produção é o `Wry`
/// (default); nos testes de cenário (`sync::scenarios`), o `MockRuntime` do
/// `tauri::test`. O Drive entra pelo trait [`DriveApi`] — `DriveClient` real
/// ou `MockDrive` em memória (issue #82).
pub struct SyncEngine<R: Runtime = Wry> {
    db: Db,
    drive: Arc<dyn DriveApi>,
    auth: Arc<AuthManager>,
    app: AppHandle<R>,
    last_sync: LastSyncStore,
    /// Raiz dos backups locais (`<app_data>/backups`).
    backup_dir: PathBuf,
    /// Acesso ao armazenamento local de saves (filesystem no desktop; SAF /
    /// bookmarks no mobile, futuramente). Todo o I/O local passa por aqui.
    storage: Arc<dyn LocalStorage>,
    /// Leitura do device_id estável para auditoria de conflitos.
    secrets: Arc<dyn crate::secrets::SecretStore>,
    /// Serializa execuções: um sync por vez, os demais aguardam.
    running: Mutex<()>,
}

impl<R: Runtime> SyncEngine<R> {
    // Construtor de injeção: recebe o wiring completo do app montado no setup.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Db,
        drive: Arc<dyn DriveApi>,
        auth: Arc<AuthManager>,
        app: AppHandle<R>,
        last_sync: LastSyncStore,
        backup_dir: PathBuf,
        storage: Arc<dyn LocalStorage>,
        secrets: Arc<dyn crate::secrets::SecretStore>,
    ) -> Self {
        Self {
            db,
            drive,
            auth,
            app,
            last_sync,
            backup_dir,
            storage,
            secrets,
            running: Mutex::new(()),
        }
    }

    /// Acesso ao armazenamento local — usado pela detecção automática mobile
    /// (`commands::detect_emulator_mobile`), que precisa checar existência de
    /// pastas via SAF fora do fluxo normal de sync.
    #[cfg(mobile)]
    pub fn storage(&self) -> &Arc<dyn LocalStorage> {
        &self.storage
    }

    /// Sincroniza todos os emuladores configurados.
    pub async fn sync_all(
        &self,
        direction: SyncDirection,
        trigger: &str,
    ) -> AppResult<SyncSummary> {
        self.sync_filtered(None, direction, trigger).await
    }

    /// Zera o cache de IDs de pasta do Drive (memória + SQLite). Chamado no
    /// logout para não reaproveitar IDs de outra conta Google (FEATURE-006).
    pub async fn clear_folder_cache(&self) {
        self.drive.clear_folder_cache().await;
    }

    /// Sincroniza um único emulador (gatilhos do process watcher).
    /// Só-desktop: no mobile não há watcher para acionar sync por emulador.
    #[cfg(desktop)]
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
            .with(settings::notification_level)
            .await
            .unwrap_or_default();
        let device = self
            .db
            .with(settings::device_name)
            .await
            .unwrap_or_default();
        // ID estável deste dispositivo (keyring), lido uma vez por sync. `None`
        // se o keyring estiver indisponível — desliga só a detecção de conflito
        // entre dispositivos nesta execução.
        let device_id = crate::device::current(self.secrets.clone()).await;

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

        // Rótulo desta execução, usado para agrupar os backups locais do
        // primeiro sync numa pasta por sync.
        let run_stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

        let mut summary = SyncSummary::default();
        for target in &targets {
            // Emulador com conflito pendente fica bloqueado até o usuário
            // resolver — nem manual nem automático sincroniza (BUG-002).
            let name = target.label.clone();
            let blocked = self
                .db
                .with(move |conn| conflicts::has_for_emulator(conn, &name))
                .await
                .unwrap_or(false);
            if blocked {
                tracing::info!(emulador = %target.label, "conflito pendente; sync do emulador bloqueado");
                continue;
            }

            match self
                .sync_target(
                    target,
                    direction,
                    &run_stamp,
                    device.as_deref(),
                    device_id.as_deref(),
                    notif,
                )
                .await
            {
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

    /// Notificação nativa do SO de conflito (gated pelo nível de notificação).
    fn notify_conflict(&self, emulator: &str, rel_path: &str) {
        if let Err(err) = self
            .app
            .notification()
            .builder()
            .title("RetroSync — conflito de sincronização")
            .body(format!(
                "{emulator}: \"{rel_path}\" mudou nos dois lados. Resolva no app."
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
        run_stamp: &str,
        device: Option<&str>,
        device_id: Option<&str>,
        notif: NotificationLevel,
    ) -> AppResult<SyncSummary> {
        let mut summary = SyncSummary::default();

        for (category, bases) in &target.categories {
            if bases.is_empty() {
                continue;
            }

            let mut folder_id = self
                .drive
                .ensure_category_folder(&target.label, *category)
                .await?;
            let folder_key = format!(
                "{}/{}/{}",
                crate::constants::DRIVE_ROOT_FOLDER,
                target.label,
                category.as_str()
            );

            let remote = match self.drive.list_tree(&folder_id).await {
                Ok(remote) => remote,
                Err(AppError::DriveObjectNotFound(detail)) => {
                    // ID de pasta cacheado ficou obsoleto (pasta movida/apagada
                    // no Drive). Invalida a subárvore e re-resolve — reencontra a
                    // existente ou recria (FEATURE-006).
                    tracing::warn!(
                        emulador = %target.label,
                        categoria = category.as_str(),
                        %detail,
                        "pasta da categoria não encontrada no Drive; invalidando cache e re-resolvendo"
                    );
                    self.drive.invalidate_folder_path(&folder_key).await;
                    folder_id = self
                        .drive
                        .ensure_category_folder(&target.label, *category)
                        .await?;
                    self.drive.list_tree(&folder_id).await?
                }
                Err(err) => return Err(err),
            };

            let local = self.storage.scan(&target.root, bases).await?;

            let (emulator, cat) = (target.label.clone(), *category);
            let manifest_entries = self
                .db
                .with(move |conn| manifest::list_for_category(conn, &emulator, cat))
                .await?;

            let (plan, skipped) =
                diff::build_plan(local, remote, manifest_entries, direction, device_id);
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
                download_base: self.storage.join(
                    &self.storage.root_loc(&target.root),
                    &bases[0].to_string_lossy().replace('\\', "/"),
                ),
                backup_base: FileLoc::from_path(
                    self.backup_dir
                        .join(&target.label)
                        .join(run_stamp)
                        .join(category.as_str()),
                ),
                device: device.map(str::to_string),
                device_id: device_id.map(str::to_string),
                notif,
                total: plan.len() as u32,
                completed: AtomicU32::new(0),
                bytes_total: plan.iter().map(op_bytes).sum(),
                bytes_done: AtomicU64::new(0),
            };

            // FEATURE-004: uploads de arquivos NOVOS e pequenos vão em lote (Batch
            // API), cortando ~100× as chamadas HTTP no primeiro sync de coleções
            // grandes. Os demais (downloads, updates, conflitos, arquivos grandes)
            // e o que o batch não conseguir seguem pelo caminho per-file abaixo.
            let plan = self.batch_new_uploads(&ctx, plan, &mut summary).await;

            let outcomes = stream::iter(plan.into_iter().map(|op| self.execute_op(&ctx, op)))
                .buffer_unordered(DRIVE_MAX_CONCURRENT_TRANSFERS)
                .collect::<Vec<_>>()
                .await;

            for outcome in outcomes {
                match outcome {
                    OpOutcome::Uploaded => summary.uploaded += 1,
                    OpOutcome::Downloaded => summary.downloaded += 1,
                    OpOutcome::DownloadedWithBackup => {
                        summary.downloaded += 1;
                        summary.backed_up += 1;
                    }
                    OpOutcome::Conflicted => summary.conflicts += 1,
                    OpOutcome::Queued => summary.queued += 1,
                    OpOutcome::Failed => summary.failed += 1,
                }
            }
        }

        Ok(summary)
    }

    async fn execute_op(&self, ctx: &CategoryCtx, op: PlannedOp) -> OpOutcome {
        let rel_path = op.rel_path.clone();
        let bytes = op_bytes(&op);
        let result = match op.action {
            SyncAction::Upload => self.do_upload(ctx, &op).await,
            SyncAction::Download => self.do_download(ctx, &op).await,
            SyncAction::DownloadWithBackup => self.do_download_with_backup(ctx, &op).await,
            SyncAction::Conflict => self.record_conflict(ctx, &op).await,
            SyncAction::NoOp => Ok(()),
        };

        self.emit_progress(ctx, &rel_path, bytes);

        match result {
            Ok(()) => {
                // Conflito não é transferência: não limpa a pendência (o
                // emulador fica bloqueado até a resolução).
                if matches!(op.action, SyncAction::Conflict) {
                    return OpOutcome::Conflicted;
                }
                let (emulator, category, rel) = (ctx.emulator.clone(), ctx.category, rel_path);
                let _ = self
                    .db
                    .with(move |conn| queue::resolve(conn, &emulator, category, &rel))
                    .await;
                match op.action {
                    SyncAction::Upload => OpOutcome::Uploaded,
                    SyncAction::DownloadWithBackup => OpOutcome::DownloadedWithBackup,
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

    /// Emite o evento de progresso e avança os contadores (arquivos e bytes)
    /// de concluídos da categoria.
    fn emit_progress(&self, ctx: &CategoryCtx, rel_path: &str, bytes: u64) {
        let completed = ctx.completed.fetch_add(1, Ordering::Relaxed) + 1;
        let bytes_done = ctx.bytes_done.fetch_add(bytes, Ordering::Relaxed) + bytes;
        let _ = self.app.emit(
            EVT_SYNC_PROGRESS,
            &SyncProgress {
                emulator: ctx.emulator.clone(),
                current_file: rel_path.to_string(),
                completed,
                total: ctx.total,
                bytes_done,
                bytes_total: ctx.bytes_total,
                direction: ctx.direction,
            },
        );
    }

    /// Pré-passo de batch (FEATURE-004): envia em lote os uploads de arquivos
    /// novos e pequenos, atualizando manifest/summary/progresso, e devolve o
    /// plano restante para o caminho per-file. Ops inelegíveis ou que não puderam
    /// ser preparadas (arquivo em uso, parent irresolvível) voltam ao restante,
    /// preservando o tratamento de fila/erro individual do `execute_op`.
    async fn batch_new_uploads(
        &self,
        ctx: &CategoryCtx,
        plan: Vec<PlannedOp>,
        summary: &mut SyncSummary,
    ) -> Vec<PlannedOp> {
        let (eligible, mut rest): (Vec<PlannedOp>, Vec<PlannedOp>) =
            plan.into_iter().partition(is_batchable);

        // Poucos elegíveis: o overhead de montar o batch não compensa — deixa o
        // caminho per-file concorrente resolver.
        if eligible.len() < DRIVE_BATCH_MIN_OPS {
            rest.extend(eligible);
            return rest;
        }

        // Prepara cada op (lê conteúdo, confere mtime estável, resolve parent).
        let mut prepared: Vec<PreparedBatchOp> = Vec::with_capacity(eligible.len());
        for op in eligible {
            match self.prepare_batch_op(ctx, op).await {
                Ok(item) => prepared.push(item),
                Err(op) => rest.push(op),
            }
        }

        tracing::info!(
            emulador = %ctx.emulator,
            categoria = ctx.category.as_str(),
            arquivos = prepared.len(),
            "batch upload de arquivos novos"
        );

        for chunk in prepared.chunks(DRIVE_BATCH_MAX_OPS) {
            let ops: Vec<BatchUploadOp> = chunk.iter().map(|p| p.batch.clone()).collect();
            match self.drive.upload_batch(ops).await {
                Ok(files) if files.len() == chunk.len() => {
                    for (p, uploaded) in chunk.iter().zip(files) {
                        let drive_mtime = uploaded.modified_ms();
                        let recorded = self
                            .record_synced(
                                ctx,
                                &p.rel_path,
                                uploaded.id,
                                p.mtime_ms,
                                drive_mtime,
                                p.size_bytes,
                            )
                            .await;
                        if recorded.is_ok() {
                            let (emulator, category, rel) =
                                (ctx.emulator.clone(), ctx.category, p.rel_path.clone());
                            let _ = self
                                .db
                                .with(move |conn| queue::resolve(conn, &emulator, category, &rel))
                                .await;
                            summary.uploaded += 1;
                        } else {
                            summary.failed += 1;
                        }
                        self.emit_progress(ctx, &p.rel_path, p.size_bytes.max(0) as u64);
                    }
                }
                result => {
                    // Falha do batch (rede/parse/sub-request) ou contagem
                    // inesperada: devolve o chunk ao per-file, que aplica
                    // retry/fila por arquivo.
                    if let Err(err) = result {
                        tracing::warn!(
                            emulador = %ctx.emulator,
                            error = %err,
                            arquivos = chunk.len(),
                            "batch falhou; caindo para upload per-file"
                        );
                    }
                    for p in chunk {
                        rest.push(p.op.clone());
                    }
                }
            }
        }

        rest
    }

    /// Prepara uma op elegível para o batch: lê o conteúdo com a mesma proteção
    /// de mtime estável do `do_upload` e resolve o `parent_id`. `Err(op)` devolve
    /// a op original para o caminho per-file quando não pôde ser preparada.
    async fn prepare_batch_op(
        &self,
        ctx: &CategoryCtx,
        op: PlannedOp,
    ) -> Result<PreparedBatchOp, PlannedOp> {
        // Clona o locador para não manter `op` emprestado até o move final.
        let loc = match op.local.as_ref() {
            Some(local) => local.loc.clone(),
            None => return Err(op),
        };

        // Mesma proteção do do_upload: conteúdo estável entre duas leituras de mtime.
        let read = async {
            let before = self.storage.mtime_ms(&loc).await?;
            let content = self.storage.read(&loc).await?;
            let after = self.storage.mtime_ms(&loc).await?;
            if before != after {
                return Err(AppError::FileBusy(op.rel_path.clone()));
            }
            Ok::<_, AppError>((content, after))
        }
        .await;
        let (content, mtime) = match read {
            Ok(v) => v,
            Err(_) => return Err(op),
        };

        let (dir_part, file_name) = split_rel_path(&op.rel_path);
        let parent_id = match dir_part {
            Some(dir) => {
                match self
                    .drive
                    .ensure_subpath(&ctx.folder_id, &ctx.folder_key, dir)
                    .await
                {
                    Ok(id) => id,
                    Err(_) => return Err(op),
                }
            }
            None => ctx.folder_id.clone(),
        };

        let size_bytes = content.len() as i64;
        let batch = BatchUploadOp {
            parent_id,
            name: file_name.to_string(),
            content,
            mtime_ms: mtime,
            device_name: ctx.device.clone(),
            device_id: ctx.device_id.clone(),
        };
        Ok(PreparedBatchOp {
            rel_path: op.rel_path.clone(),
            mtime_ms: mtime,
            size_bytes,
            op,
            batch,
        })
    }

    async fn do_upload(&self, ctx: &CategoryCtx, op: &PlannedOp) -> AppResult<()> {
        let local = op
            .local
            .as_ref()
            .ok_or_else(|| AppError::Other("upload planejado sem arquivo local".into()))?;

        let mtime_before = self.storage.mtime_ms(&local.loc).await?;
        let content = self.storage.read(&local.loc).await?;
        let mtime_after = self.storage.mtime_ms(&local.loc).await?;
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
        let tag = DeviceTag {
            name: ctx.device.as_deref(),
            id: ctx.device_id.as_deref(),
        };
        let uploaded = match op.remote.as_ref() {
            Some(existing) => {
                self.drive
                    .upload_existing(&existing.id, content, mtime_after, tag)
                    .await?
            }
            None => {
                self.drive
                    .upload_new(&parent_id, file_name, content, mtime_after, tag)
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

    /// Primeiro sync de um arquivo que existe nos dois lados: copia o local
    /// para a pasta de backup e só então baixa o do Drive (que vence). O backup
    /// roda ANTES do download — se falhar, o download não acontece, evitando a
    /// perda irreversível que o BUG-001 descreve.
    async fn do_download_with_backup(&self, ctx: &CategoryCtx, op: &PlannedOp) -> AppResult<()> {
        if let Some(local) = op.local.as_ref() {
            let backup_dest = self.storage.join(&ctx.backup_base, &op.rel_path);
            self.storage.copy_to(&local.loc, &backup_dest).await?;
            tracing::info!(
                emulador = %ctx.emulator,
                arquivo = %op.rel_path,
                backup = %backup_dest,
                "backup local antes do primeiro sync (Drive vence)"
            );
        }
        self.do_download(ctx, op).await
    }

    async fn do_download(&self, ctx: &CategoryCtx, op: &PlannedOp) -> AppResult<()> {
        let remote = op
            .remote
            .as_ref()
            .ok_or_else(|| AppError::Other("download planejado sem arquivo remoto".into()))?;

        let content = self.drive.download(&remote.id).await?;

        let dest = match op.local.as_ref() {
            Some(local) => local.loc.clone(),
            None => self.storage.join(&ctx.download_base, &op.rel_path),
        };

        // mtime local = modifiedTime do Drive, para o diff convergir.
        let drive_mtime = remote.modified_ms();
        let size_bytes = content.len() as i64;
        self.storage
            .write_atomic(&dest, &content, drive_mtime)
            .await?;

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

    /// Registra um conflito (ambos os lados mudaram desde o último sync). Não
    /// transfere nada; emite evento e notifica. O emulador fica bloqueado até a
    /// resolução pelo usuário (BUG-002).
    async fn record_conflict(&self, ctx: &CategoryCtx, op: &PlannedOp) -> AppResult<()> {
        let local = op
            .local
            .as_ref()
            .ok_or_else(|| AppError::Other("conflito planejado sem arquivo local".into()))?;
        let remote = op
            .remote
            .as_ref()
            .ok_or_else(|| AppError::Other("conflito planejado sem arquivo remoto".into()))?;

        let conflict = Conflict {
            emulator: ctx.emulator.clone(),
            category: ctx.category,
            rel_path: op.rel_path.clone(),
            local_mtime_ms: local.mtime_ms,
            local_size: local.size_bytes,
            local_device: ctx.device.clone(),
            drive_mtime_ms: remote.modified_ms().unwrap_or(0),
            drive_size: remote
                .size
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            drive_device: remote.device().map(str::to_string),
            drive_file_id: remote.id.clone(),
            local_abs_path: self.storage.loc_to_stored(&local.loc),
            detected_at_ms: chrono::Utc::now().timestamp_millis(),
        };

        let stored = conflict.clone();
        self.db
            .with(move |conn| conflicts::upsert(conn, &stored))
            .await?;

        tracing::warn!(emulador = %ctx.emulator, arquivo = %op.rel_path, "conflito detectado: ambos os lados mudaram");
        let _ = self.app.emit(EVT_SYNC_CONFLICT, &conflict);
        if ctx.notif.notifies_errors() {
            self.notify_conflict(&ctx.emulator, &op.rel_path);
        }
        Ok(())
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
    /// É só registro/auditoria: grava quem (`device`) e quando (`generatedAt`)
    /// publicou a última versão, além de um dump das entradas. O app nunca lê
    /// este arquivo de volta — a fonte de verdade operacional é a tabela
    /// `sync_manifest` no SQLite local.
    async fn publish_manifest_snapshot(&self) -> AppResult<()> {
        let entries = self.db.with(manifest::list_all).await?;
        let device = self.db.with(crate::storage::settings::device_name).await?;
        let device_id = crate::device::current(self.secrets.clone()).await;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let doc = serde_json::json!({
            "generatedAt": crate::drive::ms_to_rfc3339(now_ms),
            "device": device,
            "deviceId": device_id,
            "entries": entries,
        });
        let bytes = serde_json::to_vec_pretty(&doc)?;
        let tag = DeviceTag {
            name: device.as_deref(),
            id: device_id.as_deref(),
        };

        let root_id = self.drive.ensure_root().await?;
        match self.drive.find_child(&root_id, DRIVE_MANIFEST_FILE).await? {
            Some(existing) => {
                self.drive
                    .upload_existing(&existing.id, bytes, now_ms, tag)
                    .await?;
            }
            None => {
                self.drive
                    .upload_new(&root_id, DRIVE_MANIFEST_FILE, bytes, now_ms, tag)
                    .await?;
            }
        }
        Ok(())
    }

    /// Resolve um conflito mantendo a versão escolhida e desbloqueia o emulador.
    pub async fn resolve_conflict(
        &self,
        emulator: &str,
        category: SyncCategory,
        rel_path: &str,
        keep: ConflictResolution,
    ) -> AppResult<()> {
        let (emu, rel) = (emulator.to_string(), rel_path.to_string());
        let conflict = self
            .db
            .with(move |conn| conflicts::get(conn, &emu, category, &rel))
            .await?
            .ok_or_else(|| AppError::Other("conflito não encontrado".into()))?;

        match keep {
            ConflictResolution::Drive => self.resolve_keep_drive(&conflict).await?,
            ConflictResolution::Local => self.resolve_keep_local(&conflict).await?,
        }

        let (emu, rel) = (emulator.to_string(), rel_path.to_string());
        self.db
            .with(move |conn| conflicts::remove(conn, &emu, category, &rel))
            .await?;
        tracing::info!(emulador = %emulator, arquivo = %rel_path, ?keep, "conflito resolvido");
        Ok(())
    }

    /// Mantém o Drive: faz backup do local e baixa a versão remota por cima.
    async fn resolve_keep_drive(&self, c: &Conflict) -> AppResult<()> {
        let dest = self.storage.loc_from_stored(&c.local_abs_path);
        if self.storage.exists(&dest).await {
            let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
            let backup_base = FileLoc::from_path(
                self.backup_dir
                    .join(&c.emulator)
                    .join(format!("conflito-{stamp}"))
                    .join(c.category.as_str()),
            );
            let backup_dest = self.storage.join(&backup_base, &c.rel_path);
            self.storage.copy_to(&dest, &backup_dest).await?;
            tracing::info!(arquivo = %c.rel_path, backup = %backup_dest, "backup local antes de resolver conflito (manter Drive)");
        }

        let content = self.drive.download(&c.drive_file_id).await?;
        let size_bytes = content.len() as i64;
        let drive_mtime = c.drive_mtime_ms;
        self.storage
            .write_atomic(&dest, &content, Some(drive_mtime))
            .await?;

        self.upsert_resolved_manifest(
            c,
            drive_mtime,
            Some(drive_mtime),
            size_bytes,
            &c.drive_file_id,
        )
        .await
    }

    /// Mantém o local: envia a versão local por cima da do Drive.
    async fn resolve_keep_local(&self, c: &Conflict) -> AppResult<()> {
        let src = self.storage.loc_from_stored(&c.local_abs_path);
        let content = self.storage.read(&src).await?;
        let size_bytes = content.len() as i64;
        let local_mtime = self.storage.mtime_ms(&src).await?;
        let device = self
            .db
            .with(settings::device_name)
            .await
            .unwrap_or_default();
        let device_id = crate::device::current(self.secrets.clone()).await;
        let tag = DeviceTag {
            name: device.as_deref(),
            id: device_id.as_deref(),
        };

        let uploaded = self
            .drive
            .upload_existing(&c.drive_file_id, content, local_mtime, tag)
            .await?;
        let drive_mtime = uploaded.modified_ms();

        self.upsert_resolved_manifest(c, local_mtime, drive_mtime, size_bytes, &uploaded.id)
            .await
    }

    async fn upsert_resolved_manifest(
        &self,
        c: &Conflict,
        local_mtime_ms: i64,
        drive_mtime_ms: Option<i64>,
        size_bytes: i64,
        drive_file_id: &str,
    ) -> AppResult<()> {
        let entry = ManifestEntry {
            emulator: c.emulator.clone(),
            category: c.category,
            rel_path: c.rel_path.clone(),
            drive_file_id: Some(drive_file_id.to_string()),
            local_mtime_ms: Some(local_mtime_ms),
            drive_mtime_ms,
            size_bytes: Some(size_bytes),
            last_synced_at_ms: chrono::Utc::now().timestamp_millis(),
        };
        self.db
            .with(move |conn| manifest::upsert(conn, &entry))
            .await
    }
}

/// Op de upload já preparada para o batch: os dados prontos (`batch`) mais o que
/// o engine precisa para registrar o manifest, e a op original (`op`) para o
/// fallback per-file caso o batch falhe.
struct PreparedBatchOp {
    op: PlannedOp,
    batch: BatchUploadOp,
    rel_path: String,
    mtime_ms: i64,
    size_bytes: i64,
}

/// Bytes que a op vai transferir — tamanho local para uploads, tamanho
/// remoto para downloads; conflitos/no-ops não transferem nada.
fn op_bytes(op: &PlannedOp) -> u64 {
    match op.action {
        SyncAction::Upload => op
            .local
            .as_ref()
            .map(|l| l.size_bytes.max(0) as u64)
            .unwrap_or(0),
        SyncAction::Download | SyncAction::DownloadWithBackup => op
            .remote
            .as_ref()
            .and_then(|r| r.size.as_deref())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0),
        SyncAction::Conflict | SyncAction::NoOp => 0,
    }
}

/// Elegível ao batch: upload de arquivo que ainda não existe no Drive e é
/// pequeno o suficiente para `multipart` (o batch não suporta resumable).
fn is_batchable(op: &PlannedOp) -> bool {
    op.action == SyncAction::Upload
        && op.remote.is_none()
        && op
            .local
            .as_ref()
            .is_some_and(|l| l.size_bytes <= DRIVE_SIMPLE_UPLOAD_MAX_BYTES as i64)
}

/// `"a/b/c.bin"` → `(Some("a/b"), "c.bin")`; `"c.bin"` → `(None, "c.bin")`.
fn split_rel_path(rel_path: &str) -> (Option<&str>, &str) {
    match rel_path.rsplit_once('/') {
        Some((dir, name)) => (Some(dir), name),
        None => (None, rel_path),
    }
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
