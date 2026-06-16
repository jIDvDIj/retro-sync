import { useState } from "react";

import { errorMessage } from "../lib/errors";
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
          <span className="badge badge-conflict">conflito</span>
        ) : (
          <span className={`badge ${running ? "badge-running" : "badge-idle"}`}>
            {running ? "em execução" : "parado"}
          </span>
        )}
      </div>
      <p className="emulator-path" title={profile.rootPath}>
        {profile.rootPath}
      </p>
      <div className="emulator-foot">
        {hasConflict ? (
          <button onClick={() => setShowConflicts(true)}>
            Resolver conflito{conflicts.length > 1 ? ` (${conflicts.length})` : ""}
          </button>
        ) : null}
        <button className="secondary" onClick={handleRemove} disabled={busy}>
          {busy ? "Removendo…" : "Remover"}
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
