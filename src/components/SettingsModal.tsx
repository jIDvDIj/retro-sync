import { useState } from "react";

import { errorMessage } from "../lib/errors";
import { setDeviceName, setNotificationLevel } from "../lib/ipc";
import type { EmulatorProfile, NotificationLevel, Settings } from "../types/ipc";
import { CategorySettings } from "./CategorySettings";
import { TriggerSettingsSection } from "./TriggerSettings";

const NOTIFICATION_OPTIONS: { value: NotificationLevel; label: string }[] = [
  { value: "all", label: "Tudo (sync, erros, emulador detectado)" },
  { value: "errors_only", label: "Apenas erros" },
  { value: "none", label: "Nenhuma" },
];

interface Props {
  settings: Settings;
  emulators: EmulatorProfile[];
  onClose: () => void;
  /** Recarrega as settings no App após qualquer alteração. */
  onSaved: () => void;
}

/**
 * Modal de configurações. Cresce ao longo da v1.1 (dispositivo, categorias por
 * emulador, gatilhos automáticos, nível de notificações).
 */
export function SettingsModal({ settings, emulators, onClose, onSaved }: Props) {
  const [device, setDevice] = useState(settings.deviceName ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [notifLevel, setNotifLevel] = useState<NotificationLevel>(settings.notificationLevel);
  const [notifError, setNotifError] = useState<string | null>(null);

  const changeNotifLevel = async (level: NotificationLevel) => {
    const prev = notifLevel;
    setNotifLevel(level); // otimista
    setNotifError(null);
    try {
      await setNotificationLevel(level);
      onSaved();
    } catch (err) {
      setNotifError(errorMessage(err));
      setNotifLevel(prev); // reverte em falha
    }
  };

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

        <section className="settings-section">
          <h3>Sincronização automática</h3>
          <p className="muted">
            Mesmo com tudo desligado, o botão “Sincronizar agora” continua disponível.
          </p>
          <TriggerSettingsSection triggers={settings.triggers} onChanged={onSaved} />
        </section>

        <section className="settings-section">
          <h3>Notificações</h3>
          <p className="muted">
            Syncs automáticos frequentes podem gerar notificações invasivas — reduza o ruído aqui.
          </p>
          <label className="field">
            <span>Nível de notificações nativas</span>
            <select
              value={notifLevel}
              onChange={(e) => changeNotifLevel(e.target.value as NotificationLevel)}
            >
              {NOTIFICATION_OPTIONS.map(({ value, label }) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          {notifError ? <p className="error">{notifError}</p> : null}
        </section>

        <section className="settings-section">
          <h3>Sincronização por emulador</h3>
          <p className="muted">
            Escolha quais categorias sincronizar. Desative “Config” para não compartilhar
            resolução e controles entre dispositivos diferentes.
          </p>
          <CategorySettings emulators={emulators} />
        </section>
      </div>
    </div>
  );
}
