import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { useErrorMessage } from "../lib/errors";
import { getEmulatorCategories, setEmulatorCategories, setExcludePatterns } from "../lib/ipc";
import type { EmulatorProfile, SyncCategories } from "../types/ipc";

interface Props {
  emulators: EmulatorProfile[];
}

// "config" (versionamento das pastas de configuração do emulador) fica de
// fora de propósito: a opção está permanentemente desativada no backend
// (storage::emulators::SyncCategories) e não deve poder ser reativada pela UI.
const LABELS = [
  { key: "saves", labelKey: "settings.categories.saves" },
  { key: "savestates", labelKey: "settings.categories.savestates" },
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

  // Texto editável dos padrões de exclusão, por emulador (separados por vírgula).
  const [patterns, setPatterns] = useState<Record<string, string>>(() =>
    Object.fromEntries(emulators.map((e) => [e.name, e.excludePatterns.join(", ")])),
  );
  const [savedPatterns, setSavedPatterns] = useState<Record<string, boolean>>({});

  const savePatterns = async (name: string) => {
    const list = (patterns[name] ?? "")
      .split(",")
      .map((p) => p.trim())
      .filter((p) => p.length > 0);
    setError(null);
    try {
      await setExcludePatterns(name, list);
      setSavedPatterns((prev) => ({ ...prev, [name]: true }));
    } catch (err) {
      setError(errorMessage(err));
    }
  };

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
            <label className="field">
              <span>{t("settings.categories.excludeLabel")}</span>
              <input
                type="text"
                value={patterns[e.name] ?? ""}
                placeholder={t("settings.categories.excludePlaceholder")}
                onChange={(ev) => {
                  setPatterns((prev) => ({ ...prev, [e.name]: ev.target.value }));
                  setSavedPatterns((prev) => ({ ...prev, [e.name]: false }));
                }}
              />
            </label>
            <div className="settings-row">
              <button className="secondary" onClick={() => savePatterns(e.name)}>
                {t("settings.categories.excludeSave")}
              </button>
              {savedPatterns[e.name] ? (
                <span className="saved-hint">{t("settings.categories.excludeSaved")}</span>
              ) : null}
            </div>
          </div>
        );
      })}
      {error ? <p className="error">{error}</p> : null}
    </div>
  );
}
