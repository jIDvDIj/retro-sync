import { useCallback, useState } from "react";

import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { errorMessage } from "../lib/errors";
import { addEmulator } from "../lib/ipc";

interface Props {
  onAdded: () => void;
  disabled?: boolean;
}

/** Botão que abre o seletor de pasta nativo e registra o emulador detectado. */
export function AddEmulator({ onAdded, disabled }: Props) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleClick = useCallback(async () => {
    setError(null);
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "Selecione a pasta raiz do emulador",
    });
    if (typeof selected !== "string") return; // cancelado

    setBusy(true);
    try {
      await addEmulator(selected);
      onAdded();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }, [onAdded]);

  return (
    <div className="add-emulator">
      <button onClick={handleClick} disabled={busy || disabled}>
        {busy ? "Detectando…" : "Adicionar emulador"}
      </button>
      {error ? <p className="error">{error}</p> : null}
    </div>
  );
}
