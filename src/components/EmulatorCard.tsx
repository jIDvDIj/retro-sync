import { useState } from "react";

import { errorMessage } from "../lib/errors";
import type { EmulatorProfile } from "../types/ipc";

interface Props {
  profile: EmulatorProfile;
  running: boolean;
  onRemove: (name: string) => Promise<void>;
}

/** Card de um emulador configurado: nome, pasta, estado e remoção. */
export function EmulatorCard({ profile, running, onRemove }: Props) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  return (
    <article className="emulator-card">
      <div className="emulator-head">
        <span className="emulator-name">{profile.name}</span>
        <span className={`badge ${running ? "badge-running" : "badge-idle"}`}>
          {running ? "em execução" : "parado"}
        </span>
      </div>
      <p className="emulator-path" title={profile.rootPath}>
        {profile.rootPath}
      </p>
      <div className="emulator-foot">
        <button className="secondary" onClick={handleRemove} disabled={busy}>
          {busy ? "Removendo…" : "Remover"}
        </button>
        {error ? <span className="error">{error}</span> : null}
      </div>
    </article>
  );
}
