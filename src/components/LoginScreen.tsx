import { useCallback, useEffect, useState } from "react";

import { errorMessage } from "../lib/errors";
import { connectGoogleDrive, setDeviceName } from "../lib/ipc";
import type { AuthStatus } from "../types/ipc";

interface Props {
  /** Nome do dispositivo já salvo, usado para pré-preencher o campo. */
  initialDeviceName: string | null;
  /** Chamado com o novo status após o login concluir com sucesso. */
  onConnected: (status: AuthStatus) => void;
}

/**
 * Tela de login dedicada. É a única coisa renderizada enquanto o usuário não
 * está conectado — a tela principal só aparece depois que o login conclui.
 *
 * O nome do dispositivo é obrigatório: identifica esta máquina nos metadados de
 * sync no Drive e é gravado antes de concluir a autenticação.
 */
export function LoginScreen({ initialDeviceName, onConnected }: Props) {
  const [device, setDevice] = useState(initialDeviceName ?? "");
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Pré-preenche com o nome já salvo, sem sobrescrever o que o usuário digita.
  useEffect(() => {
    setDevice((cur) => cur || initialDeviceName || "");
  }, [initialDeviceName]);

  const handleConnect = useCallback(async () => {
    const name = device.trim();
    if (!name) return;
    setConnecting(true);
    setError(null);
    try {
      await setDeviceName(name);
      onConnected(await connectGoogleDrive());
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setConnecting(false);
    }
  }, [device, onConnected]);

  const canConnect = device.trim().length > 0 && !connecting;

  return (
    <main className="login-screen">
      <div className="login-card">
        <h1>RetroSync</h1>
        <p className="login-tagline">
          Sincronize saves, savestates e configs dos seus emuladores com o Google Drive.
        </p>

        <p className="permission-note">
          O RetroSync <strong>não acessa seus dados pessoais</strong>. Ele só consegue ver e
          modificar os arquivos que ele mesmo cria no seu Google Drive.
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
            autoFocus
          />
        </label>

        <button className="login-button" onClick={handleConnect} disabled={!canConnect}>
          {connecting ? "Aguardando autorização no navegador…" : "Conectar ao Google Drive"}
        </button>

        {error ? <p className="error">{error}</p> : null}
      </div>
    </main>
  );
}
