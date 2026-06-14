import { useCallback, useEffect, useState } from "react";

import { errorMessage } from "../lib/errors";
import {
  connectGoogleDrive,
  disconnectGoogleDrive,
  getAuthStatus,
  getSettings,
  setDeviceName,
} from "../lib/ipc";
import type { AuthStatus } from "../types/ipc";

type FlowState = "loading" | "idle" | "connecting";

interface Props {
  onConnectionChange?: (connected: boolean) => void;
}

export function ConnectDrive({ onConnectionChange }: Props) {
  const [status, setStatus] = useState<AuthStatus | null>(null);
  const [flow, setFlow] = useState<FlowState>("loading");
  const [error, setError] = useState<string | null>(null);
  const [device, setDevice] = useState("");

  const apply = useCallback(
    (next: AuthStatus) => {
      setStatus(next);
      onConnectionChange?.(next.connected);
    },
    [onConnectionChange],
  );

  useEffect(() => {
    Promise.all([getAuthStatus(), getSettings()])
      .then(([auth, settings]) => {
        apply(auth);
        if (settings.deviceName) setDevice(settings.deviceName);
      })
      .catch((err: unknown) => setError(errorMessage(err)))
      .finally(() => setFlow("idle"));
  }, [apply]);

  const handleConnect = useCallback(async () => {
    const name = device.trim();
    if (!name) return;
    setFlow("connecting");
    setError(null);
    try {
      // O nome do dispositivo é gravado antes de concluir a autenticação:
      // ele identifica esta máquina nos metadados de sync no Drive.
      await setDeviceName(name);
      apply(await connectGoogleDrive());
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setFlow("idle");
    }
  }, [apply, device]);

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

  if (status?.connected) {
    return (
      <div className="connect-drive">
        <div className="connected">
          <span className="account">
            <span className="dot dot-on" />
            {status.email ?? "Conta Google conectada"}
          </span>
          {device ? <span className="device-tag">{device}</span> : null}
          <button className="secondary" onClick={handleDisconnect}>
            Desconectar
          </button>
        </div>
        {error ? <p className="error">{error}</p> : null}
      </div>
    );
  }

  const connecting = flow === "connecting";
  const canConnect = device.trim().length > 0 && !connecting;

  return (
    <div className="connect-drive login">
      <p className="permission-note">
        O RetroSync <strong>não acessa seus dados pessoais</strong>. Ele só consegue ver e modificar
        os arquivos que ele mesmo cria no seu Google Drive.
      </p>
      <label className="field">
        <span>Nome deste dispositivo</span>
        <input
          type="text"
          value={device}
          onChange={(e) => setDevice(e.target.value)}
          placeholder="ex.: PC Gamer, Notebook"
          disabled={connecting}
          maxLength={60}
        />
      </label>
      <button onClick={handleConnect} disabled={!canConnect}>
        {connecting ? "Aguardando autorização no navegador…" : "Conectar ao Google Drive"}
      </button>
      {error ? <p className="error">{error}</p> : null}
    </div>
  );
}
