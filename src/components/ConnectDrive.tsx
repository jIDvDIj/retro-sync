import { useCallback, useEffect, useState } from "react";

import { errorMessage } from "../lib/errors";
import { connectGoogleDrive, disconnectGoogleDrive, getAuthStatus } from "../lib/ipc";
import type { AuthStatus } from "../types/ipc";

type FlowState = "loading" | "idle" | "connecting";

interface Props {
  onConnectionChange?: (connected: boolean) => void;
}

export function ConnectDrive({ onConnectionChange }: Props) {
  const [status, setStatus] = useState<AuthStatus | null>(null);
  const [flow, setFlow] = useState<FlowState>("loading");
  const [error, setError] = useState<string | null>(null);

  const apply = useCallback(
    (next: AuthStatus) => {
      setStatus(next);
      onConnectionChange?.(next.connected);
    },
    [onConnectionChange],
  );

  useEffect(() => {
    getAuthStatus()
      .then(apply)
      .catch((err: unknown) => setError(errorMessage(err)))
      .finally(() => setFlow("idle"));
  }, [apply]);

  const handleConnect = useCallback(async () => {
    setFlow("connecting");
    setError(null);
    try {
      apply(await connectGoogleDrive());
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setFlow("idle");
    }
  }, [apply]);

  const handleDisconnect = useCallback(async () => {
    setError(null);
    try {
      apply(await disconnectGoogleDrive());
    } catch (err) {
      setError(errorMessage(err));
    }
  }, [apply]);

  if (flow === "loading") {
    return <p className="muted">verificando conexão com o Google Drive…</p>;
  }

  return (
    <div className="connect-drive">
      {status?.connected ? (
        <div className="connected">
          <span className="account">
            <span className="dot dot-on" />
            {status.email ?? "Conta Google conectada"}
          </span>
          <button className="secondary" onClick={handleDisconnect}>
            Desconectar
          </button>
        </div>
      ) : (
        <button onClick={handleConnect} disabled={flow === "connecting"}>
          {flow === "connecting"
            ? "Aguardando autorização no navegador…"
            : "Conectar ao Google Drive"}
        </button>
      )}
      {error ? <p className="error">{error}</p> : null}
    </div>
  );
}
