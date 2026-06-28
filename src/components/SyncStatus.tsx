import { useState } from "react";
import { useTranslation } from "react-i18next";

import type { TFunction } from "i18next";

import { currentLocale } from "../i18n";
import { useErrorMessage } from "../lib/errors";
import { openBackupFolder, syncNow } from "../lib/ipc";
import type { SyncState } from "../hooks/useSyncEvents";
import type { SyncSummary } from "../types/ipc";

interface Props {
  state: SyncState;
}

function formatRelative(t: TFunction, atMs: number): string {
  const seconds = Math.round((Date.now() - atMs) / 1000);
  if (seconds < 10) return t("sync.justNow");
  if (seconds < 60) return t("sync.secondsAgo", { count: seconds });
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return t("sync.minutesAgo", { count: minutes });
  const hours = Math.round(minutes / 60);
  if (hours < 24) return t("sync.hoursAgo", { count: hours });
  return new Date(atMs).toLocaleString(currentLocale());
}

function summaryLine(t: TFunction, summary: SyncSummary): string {
  const parts = [`↑ ${summary.uploaded}`, `↓ ${summary.downloaded}`, `= ${summary.skipped}`];
  if (summary.queued > 0) parts.push(t("sync.queued", { count: summary.queued }));
  if (summary.failed > 0) parts.push(t("sync.failed", { count: summary.failed }));
  return `${parts.join(" · ")} (${(summary.durationMs / 1000).toFixed(1)}s)`;
}

/** Barra de status: último sync, progresso ao vivo e sync manual. */
export function SyncStatus({ state }: Props) {
  const { t } = useTranslation();
  const errorMessage = useErrorMessage();
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
          {syncing ? t("sync.syncing") : t("sync.syncNow")}
        </button>
        <div className="sync-info">
          {syncing && state.progress ? (
            <span className="sync-progress">
              {state.progress.emulator} · {state.progress.currentFile} ({state.progress.completed}/
              {state.progress.total})
            </span>
          ) : state.lastSync ? (
            <span>
              {t("sync.lastSync", { when: formatRelative(t, state.lastSync.atMs) })} ·{" "}
              <span className="muted">{summaryLine(t, state.lastSync.summary)}</span>
            </span>
          ) : (
            <span className="muted">{t("sync.never")}</span>
          )}
        </div>
      </div>
      {backedUp > 0 ? (
        <div className="backup-banner">
          <span>{t("sync.backupBanner", { count: backedUp })}</span>
          <button className="secondary" onClick={handleOpenBackups}>
            {t("sync.openBackupFolder")}
          </button>
        </div>
      ) : null}
      {actionError ? <p className="error">{actionError}</p> : null}
      {state.lastError ? (
        <p className="error">
          {t("sync.lastSyncError", {
            emulator: state.lastError.emulator ? ` (${state.lastError.emulator})` : "",
            message: state.lastError.message,
          })}
        </p>
      ) : null}
    </section>
  );
}
