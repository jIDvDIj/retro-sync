import { useState } from "react";
import { useTranslation } from "react-i18next";

import { useErrorMessage } from "../lib/errors";
import { setTriggers } from "../lib/ipc";
import type { TriggerSettings as Triggers } from "../types/ipc";

interface Props {
  triggers: Triggers;
  /** Recarrega as settings no App após alterar. */
  onChanged: () => void;
}

const ITEMS = [
  {
    key: "startup",
    labelKey: "settings.triggers.startupLabel",
    hintKey: "settings.triggers.startupHint",
  },
  {
    key: "emulatorStart",
    labelKey: "settings.triggers.emulatorStartLabel",
    hintKey: "settings.triggers.emulatorStartHint",
  },
  {
    key: "emulatorStop",
    labelKey: "settings.triggers.emulatorStopLabel",
    hintKey: "settings.triggers.emulatorStopHint",
  },
] as const satisfies readonly { key: keyof Triggers; labelKey: string; hintKey: string }[];

/** Toggles dos gatilhos de sync automático. O sync manual nunca é afetado. */
export function TriggerSettingsSection({ triggers, onChanged }: Props) {
  const { t } = useTranslation();
  const errorMessage = useErrorMessage();
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
      {ITEMS.map(({ key, labelKey, hintKey }) => (
        <label key={key} className="trigger-row">
          <input type="checkbox" checked={state[key]} onChange={() => toggle(key)} />
          <span className="trigger-text">
            <span className="trigger-label">{t(labelKey)}</span>
            <span className="muted">{t(hintKey)}</span>
          </span>
        </label>
      ))}
      {error ? <p className="error">{error}</p> : null}
    </div>
  );
}
