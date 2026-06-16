import { useCallback, useMemo, useState } from "react";

import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { useDiscovery } from "../hooks/useDiscovery";
import { errorMessage } from "../lib/errors";
import { addEmulator, addEmulatorManual, detectEmulator } from "../lib/ipc";
import type { DiscoveredEmulator, EmulatorProfile } from "../types/ipc";

interface Props {
  /** Emuladores já configurados — filtrados das recomendações. */
  existingNames: string[];
  onClose: () => void;
  /** Chamado após cada adição bem-sucedida (recarrega a lista no App). */
  onAdded: () => void;
}

/** Rótulo curto da origem de uma sugestão com saves. */
const SOURCE_LABEL: Record<DiscoveredEmulator["source"], string> = {
  dataDir: "saves encontrados",
  both: "saves encontrados",
  registry: "instalado",
};

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

  const wrap = useCallback(async (key: string, fn: () => Promise<void>) => {
    setBusy(key);
    setError(null);
    try {
      await fn();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  }, []);

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
      title: "Selecione a pasta raiz do emulador",
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
      title: "Selecione uma subpasta da raiz",
    });
    if (typeof selected !== "string") return;
    const rel = relativeUnder(root, selected);
    if (!rel) {
      setError("selecione uma subpasta dentro da pasta raiz");
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
          <h2>Adicionar emulador</h2>
          <button className="secondary" onClick={onClose}>
            Fechar
          </button>
        </div>

        <section className="settings-section">
          <h3>Recomendados</h3>
          {discovery.loading ? (
            <p className="muted">procurando emuladores instalados…</p>
          ) : discovery.error ? (
            <p className="error">{discovery.error}</p>
          ) : recommendations.length === 0 ? (
            <p className="muted">Nenhum emulador novo detectado automaticamente.</p>
          ) : (
            <div className="discovery-list">
              {recommendations.map((d) => (
                <div className="discovery-row" key={d.name}>
                  <div className="discovery-info">
                    <span className="discovery-name">{d.name}</span>
                    <span className="muted discovery-meta">
                      {d.profile ? SOURCE_LABEL[d.source] : "instalado, sem saves ainda"}
                    </span>
                  </div>
                  {d.profile ? (
                    <button disabled={busy !== null} onClick={() => addRecommended(d)}>
                      {busy === `rec:${d.name}` ? "Adicionando…" : "Adicionar"}
                    </button>
                  ) : (
                    <span className="muted discovery-hint">abra o emulador uma vez</span>
                  )}
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="settings-section">
          <h3>Apontar pasta</h3>
          <p className="muted">
            Para instalações portáteis ou emuladores fora da lista, selecione a pasta raiz.
          </p>
          <div className="settings-row">
            <button className="secondary" disabled={busy === "detect"} onClick={pickRoot}>
              {busy === "detect" ? "Detectando…" : "Selecionar pasta…"}
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
                <span className="muted discovery-meta">detectado nesta pasta</span>
              </div>
              <button disabled={busy !== null} onClick={addDetected}>
                {busy === "add-detected" ? "Adicionando…" : "Adicionar"}
              </button>
            </div>
          ) : null}

          {needsManual ? (
            <div className="manual-form">
              <p className="muted">
                Nenhum emulador reconhecido nesta pasta. Informe os dados manualmente — as pastas
                devem estar dentro da raiz.
              </p>
              <label className="manual-field">
                <span>Nome</span>
                <input
                  value={manualName}
                  onChange={(e) => setManualName(e.target.value)}
                  placeholder="ex.: Dolphin"
                />
              </label>
              <ManualPathRow label="Saves" value={savesRel} onPick={() => pickSub(setSavesRel)} />
              <ManualPathRow
                label="Savestates"
                value={statesRel}
                onPick={() => pickSub(setStatesRel)}
              />
              <ManualPathRow
                label="Config"
                value={configRel}
                onPick={() => pickSub(setConfigRel)}
              />
              <button disabled={busy !== null || manualIncomplete} onClick={addManual}>
                {busy === "add-manual" ? "Adicionando…" : "Adicionar manualmente"}
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
  return (
    <div className="manual-path-row">
      <span className="manual-path-label">{label}</span>
      <button className="secondary" onClick={onPick}>
        {value || "Selecionar subpasta…"}
      </button>
    </div>
  );
}
