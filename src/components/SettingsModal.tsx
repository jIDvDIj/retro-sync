import { useState } from "react";

import { errorMessage } from "../lib/errors";
import { setDeviceName } from "../lib/ipc";
import type { Settings } from "../types/ipc";

interface Props {
  settings: Settings;
  onClose: () => void;
  /** Recarrega as settings no App após qualquer alteração. */
  onSaved: () => void;
}

/**
 * Modal de configurações. Cresce ao longo da v1.1 (dispositivo, categorias por
 * emulador, gatilhos automáticos, nível de notificações).
 */
export function SettingsModal({ settings, onClose, onSaved }: Props) {
  const [device, setDevice] = useState(settings.deviceName ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  const dirty = device.trim() !== (settings.deviceName ?? "");

  const saveDevice = async () => {
    const name = device.trim();
    if (!name) return;
    setBusy(true);
    setError(null);
    setSaved(false);
    try {
      await setDeviceName(name);
      onSaved();
      setSaved(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h2>Configurações</h2>
          <button className="secondary" onClick={onClose}>
            Fechar
          </button>
        </div>

        <section className="settings-section">
          <h3>Dispositivo</h3>
          <p className="muted">
            Identifica esta máquina nos metadados de sync. Alterá-lo aqui não exige refazer o
            login.
          </p>
          <label className="field">
            <span>Nome deste dispositivo</span>
            <input
              type="text"
              value={device}
              onChange={(e) => {
                setDevice(e.target.value);
                setSaved(false);
              }}
              placeholder="ex.: PC Gamer, Notebook"
              maxLength={60}
            />
          </label>
          <div className="settings-row">
            <button onClick={saveDevice} disabled={busy || !dirty || device.trim().length === 0}>
              {busy ? "Salvando…" : "Salvar nome"}
            </button>
            {saved && !dirty ? <span className="saved-hint">Salvo ✓</span> : null}
          </div>
          {error ? <p className="error">{error}</p> : null}
        </section>
      </div>
    </div>
  );
}
