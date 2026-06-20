import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { useDiscovery } from "../hooks/useDiscovery";
import { useErrorMessage } from "../lib/errors";
import { addEmulator, addEmulatorManual, detectEmulator } from "../lib/ipc";
import type { DiscoveredEmulator, EmulatorProfile } from "../types/ipc";

interface Props {
  /** Emuladores já configurados — filtrados das recomendações. */
  existingNames: string[];
  onClose: () => void;
  /** Chamado após cada adição bem-sucedida (recarrega a lista no App). */
  onAdded: () => void;
}

/** Chave de tradução do rótulo curto da origem de uma sugestão com saves. */
const SOURCE_LABEL_KEY = {
  dataDir: "addEmulator.sourceSavesFound",
  both: "addEmulator.sourceSavesFound",
  registry: "addEmulator.sourceInstalled",
} as const satisfies Record<DiscoveredEmulator["source"], string>;

/** Caminho de `child` relativo a `root`, ou `null` se não estiver sob a raiz. */
function relativeUnder(root: string, child: string): string | null {
  const trim = (s: string) => s.replace(/[\\/]+$/, "");
  const r = trim(root);
  const c = trim(child);
  if (c === r) return "";
  if (c.startsWith(`${r}/`) || c.startsWith(`${r}\\`)) {
    return c.slice(r.length + 1);
  }
  return null;
}

/**
 * Modal de adição de emulador com as três vias: recomendados (descoberta
 * automática), detecção por pasta e configuração manual (fallback).
 */
export function AddEmulatorModal({ existingNames, onClose, onAdded }: Props) {
  const { t } = useTranslation();
  const errorMessage = useErrorMessage();
  const discovery = useDiscovery();
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Fluxo "apontar pasta": raiz escolhida e resultado da detecção automática.
  const [root, setRoot] = useState<string | null>(null);
  const [detected, setDetected] = useState<EmulatorProfile | null>(null);
  const [needsManual, setNeedsManual] = useState(false);

  // Campos do formulário manual.
  const [manualName, setManualName] = useState("");
  const [savesRel, setSavesRel] = useState("");
  const [statesRel, setStatesRel] = useState("");
  const [configRel, setConfigRel] = useState("");

  const recommendations = useMemo(
    () => discovery.discovered.filter((d) => !existingNames.includes(d.name)),
    [discovery.discovered, existingNames],
  );

  const resetManual = () => {
    setRoot(null);
    setDetected(null);
    setNeedsManual(false);
    setManualName("");
    setSavesRel("");
    setStatesRel("");
    setConfigRel("");
  };

  const wrap = useCallback(
    async (key: string, fn: () => Promise<void>) => {
      setBusy(key);
      setError(null);
      try {
        await fn();
      } catch (err) {
        setError(errorMessage(err));
      } finally {
        setBusy(null);
      }
    },
    [errorMessage],
  );

  const addRecommended = (d: DiscoveredEmulator) =>
    wrap(`rec:${d.name}`, async () => {
      if (!d.profile) return;
      await addEmulator(d.profile.rootPath);
      onAdded();
    });

  const pickRoot = async () => {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: t("addEmulator.pickRootTitle"),
    });
    if (typeof selected !== "string") return;
    resetManual();
    setRoot(selected);
    await wrap("detect", async () => {
      const profile = await detectEmulator(selected);
      if (profile) {
        setDetected(profile);
      } else {
        setNeedsManual(true);
      }
    });
  };

  const addDetected = () =>
    wrap("add-detected", async () => {
      if (!root) return;
      await addEmulator(root);
      onAdded();
      resetManual();
    });

  const pickSub = async (setter: (v: string) => void) => {
    if (!root) return;
    const selected = await openDialog({
      directory: true,
      multiple: false,
      defaultPath: root,
      title: t("addEmulator.pickSubTitle"),
    });
    if (typeof selected !== "string") return;
    const rel = relativeUnder(root, selected);
    if (!rel) {
      setError(t("addEmulator.subfolderError"));
      return;
    }
    setError(null);
    setter(rel);
  };

  const addManual = () =>
    wrap("add-manual", async () => {
      if (!root) return;
      await addEmulatorManual(
        manualName,
        root,
        savesRel ? [savesRel] : [],
        statesRel ? [statesRel] : [],
        configRel ? [configRel] : [],
      );
      onAdded();
      resetManual();
    });

  const manualIncomplete = manualName.trim() === "" || (!savesRel && !statesRel && !configRel);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h2>{t("addEmulator.title")}</h2>
          <button className="secondary" onClick={onClose}>
            {t("common.close")}
          </button>
        </div>

        <section className="settings-section">
          <h3>{t("addEmulator.recommended")}</h3>
          {discovery.loading ? (
            <p className="muted">{t("addEmulator.searching")}</p>
          ) : discovery.error ? (
            <p className="error">{discovery.error}</p>
          ) : recommendations.length === 0 ? (
            <p className="muted">{t("addEmulator.noneDetected")}</p>
          ) : (
            <div className="discovery-list">
              {recommendations.map((d) => (
                <div className="discovery-row" key={d.name}>
                  <div className="discovery-info">
                    <span className="discovery-name">{d.name}</span>
                    <span className="muted discovery-meta">
                      {d.profile
                        ? t(SOURCE_LABEL_KEY[d.source])
                        : t("addEmulator.installedNoSaves")}
                    </span>
                  </div>
                  {d.profile ? (
                    <button disabled={busy !== null} onClick={() => addRecommended(d)}>
                      {busy === `rec:${d.name}` ? t("addEmulator.adding") : t("common.add")}
                    </button>
                  ) : (
                    <span className="muted discovery-hint">{t("addEmulator.openOnce")}</span>
                  )}
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="settings-section">
          <h3>{t("addEmulator.pickFolder")}</h3>
          <p className="muted">{t("addEmulator.pickFolderHint")}</p>
          <div className="settings-row">
            <button className="secondary" disabled={busy === "detect"} onClick={pickRoot}>
              {busy === "detect" ? t("addEmulator.detecting") : t("addEmulator.selectFolder")}
            </button>
            {root ? (
              <span className="muted discovery-meta" title={root}>
                {root}
              </span>
            ) : null}
          </div>

          {detected ? (
            <div className="discovery-row">
              <div className="discovery-info">
                <span className="discovery-name">{detected.name}</span>
                <span className="muted discovery-meta">{t("addEmulator.detectedHere")}</span>
              </div>
              <button disabled={busy !== null} onClick={addDetected}>
                {busy === "add-detected" ? t("addEmulator.adding") : t("common.add")}
              </button>
            </div>
          ) : null}

          {needsManual ? (
            <div className="manual-form">
              <p className="muted">{t("addEmulator.manualIntro")}</p>
              <label className="manual-field">
                <span>{t("addEmulator.nameLabel")}</span>
                <input
                  value={manualName}
                  onChange={(e) => setManualName(e.target.value)}
                  placeholder={t("addEmulator.namePlaceholder")}
                />
              </label>
              <ManualPathRow
                label={t("settings.categories.saves")}
                value={savesRel}
                onPick={() => pickSub(setSavesRel)}
              />
              <ManualPathRow
                label={t("settings.categories.savestates")}
                value={statesRel}
                onPick={() => pickSub(setStatesRel)}
              />
              <ManualPathRow
                label={t("settings.categories.config")}
                value={configRel}
                onPick={() => pickSub(setConfigRel)}
              />
              <button disabled={busy !== null || manualIncomplete} onClick={addManual}>
                {busy === "add-manual" ? t("addEmulator.adding") : t("addEmulator.addManual")}
              </button>
            </div>
          ) : null}
        </section>

        {error ? <p className="error">{error}</p> : null}
      </div>
    </div>
  );
}

interface ManualPathRowProps {
  label: string;
  value: string;
  onPick: () => void;
}

/** Linha do formulário manual: rótulo da categoria + seletor da subpasta. */
function ManualPathRow({ label, value, onPick }: ManualPathRowProps) {
  const { t } = useTranslation();
  return (
    <div className="manual-path-row">
      <span className="manual-path-label">{label}</span>
      <button className="secondary" onClick={onPick}>
        {value || t("addEmulator.selectSubfolder")}
      </button>
    </div>
  );
}
