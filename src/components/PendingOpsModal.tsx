import { useState } from "react";
import { useTranslation } from "react-i18next";

import { currentLocale } from "../i18n";
import { useErrorMessage } from "../lib/errors";
import { syncNow } from "../lib/ipc";
import type { PendingOp } from "../types/ipc";
import { Modal } from "./ui/Modal";

interface Props {
  emulator: string;
  ops: PendingOp[];
  onClose: () => void;
}

/**
 * Fila offline visível de um emulador: cada arquivo preso com direção, tentativas e o último erro.
 * As pendências são retentadas automaticamente a cada sync; o botão só
 * antecipa a próxima tentativa.
 */
export function PendingOpsModal({ emulator, ops, onClose }: Props) {
  const { t } = useTranslation();
  const errorMessage = useErrorMessage();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const retryNow = async () => {
    setBusy(true);
    setError(null);
    try {
      await syncNow();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal title={t("pending.title", { emulator })} onClose={onClose}>
      <p className="muted">{t("pending.intro")}</p>

      {ops.length === 0 ? (
        <p className="muted">{t("pending.empty")}</p>
      ) : (
        <div className="pending-list">
          {ops.map((op) => (
            <div className="pending-row" key={`${op.category}/${op.relPath}/${op.direction}`}>
              <span className="pending-path">
                {op.direction === "upload" ? "↑" : "↓"} {op.category} · {op.relPath}
              </span>
              <span className="pending-meta">
                <span>{t(op.direction === "upload" ? "pending.upload" : "pending.download")}</span>
                <span>{t("pending.attempts", { count: op.attempts })}</span>
                <span>{new Date(op.enqueuedAtMs).toLocaleString(currentLocale())}</span>
              </span>
              {op.lastError ? <span className="pending-error">{op.lastError}</span> : null}
            </div>
          ))}
        </div>
      )}

      <div className="settings-row">
        <button onClick={retryNow} disabled={busy || ops.length === 0}>
          {busy ? t("pending.retrying") : t("pending.retryNow")}
        </button>
      </div>
      {error ? <p className="error">{error}</p> : null}
    </Modal>
  );
}
