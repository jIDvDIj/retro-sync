import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { useErrorMessage } from "../lib/errors";
import { getEmulatorCategories, setEmulatorCategories } from "../lib/ipc";
import type { EmulatorProfile, SyncCategories } from "../types/ipc";

interface Props {
  emulators: EmulatorProfile[];
}

const LABELS = [
  { key: "saves", labelKey: "settings.categories.saves" },
  { key: "savestates", labelKey: "settings.categories.savestates" },
  { key: "config", labelKey: "settings.categories.config" },
] as const satisfies readonly { key: keyof SyncCategories; labelKey: string }[];

/** Toggles de categorias (saves/savestates/config) por emulador configurado. */
export function CategorySettings({ emulators }: Props) {
  const { t } = useTranslation();
  const errorMessage = useErrorMessage();
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
  }, [emulators, errorMessage]);

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
    return <p className="muted">{t("settings.categories.empty")}</p>;
  }

  return (
    <div className="category-list">
      {emulators.map((e) => {
        const c = cats[e.name];
        return (
          <div className="category-row" key={e.name}>
            <span className="category-emulator">{e.name}</span>
            <div className="category-toggles">
              {LABELS.map(({ key, labelKey }) => (
                <label key={key} className="toggle">
                  <input
                    type="checkbox"
                    checked={c ? c[key] : true}
                    disabled={!c}
                    onChange={() => toggle(e.name, key)}
                  />
                  {t(labelKey)}
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
