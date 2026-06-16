import { useEffect, useState } from "react";

import { errorMessage } from "../lib/errors";
import { getEmulatorCategories, setEmulatorCategories } from "../lib/ipc";
import type { EmulatorProfile, SyncCategories } from "../types/ipc";

interface Props {
  emulators: EmulatorProfile[];
}

const LABELS: { key: keyof SyncCategories; label: string }[] = [
  { key: "saves", label: "Saves" },
  { key: "savestates", label: "Savestates" },
  { key: "config", label: "Config" },
];

/** Toggles de categorias (saves/savestates/config) por emulador configurado. */
export function CategorySettings({ emulators }: Props) {
  const [cats, setCats] = useState<Record<string, SyncCategories>>({});
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    Promise.all(
      emulators.map((e) => getEmulatorCategories(e.name).then((c) => [e.name, c] as const)),
    )
      .then((entries) => {
        if (active) setCats(Object.fromEntries(entries));
      })
      .catch((err: unknown) => setError(errorMessage(err)));
    return () => {
      active = false;
    };
  }, [emulators]);

  const toggle = async (name: string, key: keyof SyncCategories) => {
    const current = cats[name];
    if (!current) return;
    const next = { ...current, [key]: !current[key] };
    setCats((prev) => ({ ...prev, [name]: next })); // otimista
    setError(null);
    try {
      await setEmulatorCategories(name, next);
    } catch (err) {
      setError(errorMessage(err));
      setCats((prev) => ({ ...prev, [name]: current })); // reverte em falha
    }
  };

  if (emulators.length === 0) {
    return <p className="muted">Adicione um emulador para configurar suas categorias.</p>;
  }

  return (
    <div className="category-list">
      {emulators.map((e) => {
        const c = cats[e.name];
        return (
          <div className="category-row" key={e.name}>
            <span className="category-emulator">{e.name}</span>
            <div className="category-toggles">
              {LABELS.map(({ key, label }) => (
                <label key={key} className="toggle">
                  <input
                    type="checkbox"
                    checked={c ? c[key] : true}
                    disabled={!c}
                    onChange={() => toggle(e.name, key)}
                  />
                  {label}
                </label>
              ))}
            </div>
          </div>
        );
      })}
      {error ? <p className="error">{error}</p> : null}
    </div>
  );
}
