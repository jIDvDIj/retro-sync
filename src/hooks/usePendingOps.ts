import { useCallback, useEffect, useState } from "react";

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { listPendingOps } from "../lib/ipc";
import { EVT, type PendingOp } from "../types/ipc";

/**
 * Fila offline visível: pendências carregadas do backend e recarregadas ao
 * fim de cada sync (quando itens podem ter sido resolvidos ou enfileirados).
 */
export function usePendingOps(): { ops: PendingOp[]; reload: () => void } {
  const [ops, setOps] = useState<PendingOp[]>([]);

  const reload = useCallback(() => {
    listPendingOps()
      .then(setOps)
      .catch(() => {});
  }, []);

  useEffect(() => {
    reload();
    const subscription: Promise<UnlistenFn> = listen(EVT.SYNC_COMPLETED, reload);
    return () => {
      subscription.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [reload]);

  return { ops, reload };
}
