import { useState } from "react";
import { useTranslation } from "react-i18next";

import { currentLocale } from "../i18n";
import { useErrorMessage } from "../lib/errors";
import { resolveConflict } from "../lib/ipc";
import type { Conflict, ConflictResolution } from "../types/ipc";

interface Props {
  emulator: string;
  conflicts: Conflict[];
  onClose: () => void;
  /** Recarrega a lista de conflitos no App após uma resolução. */
  onResolved: () => void;
}

function formatDate(ms: number): string {
  return new Date(ms).toLocaleString(currentLocale());
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

/** Modal de resolução de conflito de um emulador (uma ou mais entradas). */
export function ConflictModal({ emulator, conflicts, onClose, onResolved }: Props) {
  const { t } = useTranslation();
  const errorMessage = useErrorMessage();
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const resolve = async (c: Conflict, keep: ConflictResolution) => {
    setBusy(c.relPath);
    setError(null);
    try {
      await resolveConflict(c.emulator, c.category, c.relPath, keep);
      onResolved();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h2>{t("conflict.title", { emulator })}</h2>
          <button className="secondary" onClick={onClose}>
            {t("common.close")}
          </button>
        </div>
        <p className="muted">{t("conflict.intro")}</p>

        {conflicts.map((c) => (
          <div className="conflict-item" key={`${c.category}/${c.relPath}`}>
            <div className="conflict-path">
              {c.category} · {c.relPath}
            </div>
            <div className="conflict-sides">
              <div className="conflict-side">
                <div className="conflict-side-title">
                  {t("conflict.thisDevice")}
                  {c.localDevice ? ` · ${c.localDevice}` : ""}
                </div>
                <div className="muted">
                  {formatDate(c.localMtimeMs)} · {formatSize(c.localSize)}
                </div>
                <button disabled={busy === c.relPath} onClick={() => resolve(c, "local")}>
                  {t("conflict.keepLocal")}
                </button>
              </div>
              <div className="conflict-side">
                <div className="conflict-side-title">
                  {t("conflict.drive")}
                  {c.driveDevice ? ` · ${c.driveDevice}` : ""}
                </div>
                <div className="muted">
                  {formatDate(c.driveMtimeMs)} · {formatSize(c.driveSize)}
                </div>
                <button disabled={busy === c.relPath} onClick={() => resolve(c, "drive")}>
                  {t("conflict.keepDrive")}
                </button>
              </div>
            </div>
          </div>
        ))}
        {error ? <p className="error">{error}</p> : null}
      </div>
    </div>
  );
}
