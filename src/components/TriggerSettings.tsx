import { useState } from "react";

import { errorMessage } from "../lib/errors";
import { setTriggers } from "../lib/ipc";
import type { TriggerSettings as Triggers } from "../types/ipc";

interface Props {
  triggers: Triggers;
  /** Recarrega as settings no App após alterar. */
  onChanged: () => void;
}

const ITEMS: { key: keyof Triggers; label: string; hint: string }[] = [
  { key: "startup", label: "Ao abrir o RetroSync", hint: "sincroniza quando o app inicia" },
  {
    key: "emulatorStart",
    label: "Antes de abrir o emulador",
    hint: "baixa os saves frescos do Drive",
  },
  { key: "emulatorStop", label: "Ao fechar o emulador", hint: "sobe os saves da sessão" },
];

/** Toggles dos gatilhos de sync automático. O sync manual nunca é afetado. */
export function TriggerSettingsSection({ triggers, onChanged }: Props) {
  const [state, setState] = useState<Triggers>(triggers);
  const [error, setError] = useState<string | null>(null);

  const toggle = async (key: keyof Triggers) => {
    const next = { ...state, [key]: !state[key] };
    setState(next); // otimista
    setError(null);
    try {
      await setTriggers(next);
      onChanged();
    } catch (err) {
      setError(errorMessage(err));
      setState(state); // reverte em falha
    }
  };

  return (
    <div className="trigger-list">
      {ITEMS.map(({ key, label, hint }) => (
        <label key={key} className="trigger-row">
          <input type="checkbox" checked={state[key]} onChange={() => toggle(key)} />
          <span className="trigger-text">
            <span className="trigger-label">{label}</span>
            <span className="muted">{hint}</span>
          </span>
        </label>
      ))}
      {error ? <p className="error">{error}</p> : null}
    </div>
  );
}
