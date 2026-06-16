import { useCallback, useEffect, useState } from "react";

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { listConflicts } from "../lib/ipc";
import { EVT, type Conflict } from "../types/ipc";

interface UseConflicts {
  conflicts: Conflict[];
  reload: () => Promise<void>;
}

/**
 * Conflitos pendentes, carregados no nível do App. Recarrega ao montar, sempre
 * que o backend emite `sync:conflict` e após uma resolução. Cada card de
 * emulador filtra os seus.
 */
export function useConflicts(): UseConflicts {
  const [conflicts, setConflicts] = useState<Conflict[]>([]);

  const reload = useCallback(async () => {
    try {
      setConflicts(await listConflicts());
    } catch {
      // sem conexão/erro: mantém a lista atual
    }
  }, []);

  useEffect(() => {
    void reload();
    let unlisten: UnlistenFn | undefined;
    listen(EVT.SYNC_CONFLICT, () => void reload())
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => unlisten?.();
  }, [reload]);

  return { conflicts, reload };
}
