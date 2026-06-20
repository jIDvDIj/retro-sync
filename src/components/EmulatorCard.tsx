import { useState } from "react";
import { useTranslation } from "react-i18next";

import { useErrorMessage } from "../lib/errors";
import type { Conflict, EmulatorProfile } from "../types/ipc";
import { ConflictModal } from "./ConflictModal";

interface Props {
  profile: EmulatorProfile;
  running: boolean;
  /** Conflitos pendentes deste emulador (bloqueiam o sync dele). */
  conflicts: Conflict[];
  onRemove: (name: string) => Promise<void>;
  /** Recarrega a lista de conflitos após uma resolução. */
  onConflictResolved: () => void;
}

/** Card de um emulador configurado: nome, pasta, estado, conflito e remoção. */
export function EmulatorCard({ profile, running, conflicts, onRemove, onConflictResolved }: Props) {
  const { t } = useTranslation();
  const errorMessage = useErrorMessage();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showConflicts, setShowConflicts] = useState(false);

  const handleRemove = async () => {
    setBusy(true);
    setError(null);
    try {
      await onRemove(profile.name);
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  };

  const hasConflict = conflicts.length > 0;

  return (
    <article className={`emulator-card${hasConflict ? " has-conflict" : ""}`}>
      <div className="emulator-head">
        <span className="emulator-name">{profile.name}</span>
        {hasConflict ? (
          <span className="badge badge-conflict">{t("emulator.conflictBadge")}</span>
        ) : (
          <span className={`badge ${running ? "badge-running" : "badge-idle"}`}>
            {running ? t("emulator.running") : t("emulator.idle")}
          </span>
        )}
      </div>
      <p className="emulator-path" title={profile.rootPath}>
        {profile.rootPath}
      </p>
      <div className="emulator-foot">
        {hasConflict ? (
          <button onClick={() => setShowConflicts(true)}>
            {t("emulator.resolveConflict", { count: conflicts.length })}
          </button>
        ) : null}
        <button className="secondary" onClick={handleRemove} disabled={busy}>
          {busy ? t("emulator.removing") : t("emulator.remove")}
        </button>
        {error ? <span className="error">{error}</span> : null}
      </div>

      {showConflicts && hasConflict ? (
        <ConflictModal
          emulator={profile.name}
          conflicts={conflicts}
          onClose={() => setShowConflicts(false)}
          onResolved={onConflictResolved}
        />
      ) : null}
    </article>
  );
}
