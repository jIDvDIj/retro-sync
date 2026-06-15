import { useState } from "react";

import { errorMessage } from "../lib/errors";
import { openBackupFolder, syncNow } from "../lib/ipc";
import type { SyncState } from "../hooks/useSyncEvents";
import type { SyncSummary } from "../types/ipc";

interface Props {
  state: SyncState;
}

function formatRelative(atMs: number): string {
  const seconds = Math.round((Date.now() - atMs) / 1000);
  if (seconds < 10) return "agora mesmo";
  if (seconds < 60) return `há ${seconds}s`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `há ${minutes} min`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `há ${hours} h`;
  return new Date(atMs).toLocaleString("pt-BR");
}

function summaryLine(summary: SyncSummary): string {
  const parts = [`↑ ${summary.uploaded}`, `↓ ${summary.downloaded}`, `= ${summary.skipped}`];
  if (summary.queued > 0) parts.push(`pendentes ${summary.queued}`);
  if (summary.failed > 0) parts.push(`falhas ${summary.failed}`);
  return `${parts.join(" · ")} (${(summary.durationMs / 1000).toFixed(1)}s)`;
}

/** Barra de status: último sync, progresso ao vivo e sync manual. */
export function SyncStatus({ state }: Props) {
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const syncing = busy || state.phase === "syncing";

  const handleSync = async () => {
    setBusy(true);
    setActionError(null);
    try {
      await syncNow();
    } catch (err) {
      setActionError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleOpenBackups = async () => {
    setActionError(null);
    try {
      await openBackupFolder();
    } catch (err) {
      setActionError(errorMessage(err));
    }
  };

  const backedUp = state.lastSync?.summary.backedUp ?? 0;

  return (
    <section className="sync-status">
      <div className="sync-row">
        <button onClick={handleSync} disabled={syncing}>
          {syncing ? "Sincronizando…" : "Sincronizar agora"}
        </button>
        <div className="sync-info">
          {syncing && state.progress ? (
            <span className="sync-progress">
              {state.progress.emulator} · {state.progress.currentFile} ({state.progress.completed}/
              {state.progress.total})
            </span>
          ) : state.lastSync ? (
            <span>
              Último sync {formatRelative(state.lastSync.atMs)} ·{" "}
              <span className="muted">{summaryLine(state.lastSync.summary)}</span>
            </span>
          ) : (
            <span className="muted">Nenhuma sincronização ainda</span>
          )}
        </div>
      </div>
      {backedUp > 0 ? (
        <div className="backup-banner">
          <span>
            {backedUp} arquivo{backedUp > 1 ? "s" : ""} local
            {backedUp > 1 ? "is" : ""} {backedUp > 1 ? "foram salvos" : "foi salvo"} em backup antes
            do primeiro sync (o Drive venceu).
          </span>
          <button className="secondary" onClick={handleOpenBackups}>
            Abrir pasta de backup
          </button>
        </div>
      ) : null}
      {actionError ? <p className="error">{actionError}</p> : null}
      {state.lastError ? (
        <p className="error">
          Falha no último sync
          {state.lastError.emulator ? ` (${state.lastError.emulator})` : ""}:{" "}
          {state.lastError.message}
        </p>
      ) : null}
    </section>
  );
}
