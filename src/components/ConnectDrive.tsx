import { useCallback, useEffect, useState } from "react";

import { connectGoogleDrive, disconnectGoogleDrive, getAuthStatus } from "../lib/ipc";
import type { AppErrorPayload, AuthStatus } from "../types/ipc";

type FlowState = "loading" | "idle" | "connecting";

function errorMessage(err: unknown): string {
  const payload = err as Partial<AppErrorPayload>;
  return payload?.message ?? "erro inesperado ao falar com o backend";
}

export function ConnectDrive() {
  const [status, setStatus] = useState<AuthStatus | null>(null);
  const [flow, setFlow] = useState<FlowState>("loading");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getAuthStatus()
      .then(setStatus)
      .catch((err: unknown) => setError(errorMessage(err)))
      .finally(() => setFlow("idle"));
  }, []);

  const handleConnect = useCallback(async () => {
    setFlow("connecting");
    setError(null);
    try {
      setStatus(await connectGoogleDrive());
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setFlow("idle");
    }
  }, []);

  const handleDisconnect = useCallback(async () => {
    setError(null);
    try {
      setStatus(await disconnectGoogleDrive());
    } catch (err) {
      setError(errorMessage(err));
    }
  }, []);

  if (flow === "loading") {
    return <p className="status">verificando conexão com o Google Drive…</p>;
  }

  return (
    <section className="connect-drive">
      {status?.connected ? (
        <div className="connected">
          <p>
            <span className="dot dot-on" /> Conectado ao Google Drive
            {status.email ? <strong> ({status.email})</strong> : null}
          </p>
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
    </section>
  );
}
