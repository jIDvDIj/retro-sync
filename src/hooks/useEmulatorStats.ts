import { useEffect, useState } from "react";

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { getEmulatorStats } from "../lib/ipc";
import { EVT, type EmulatorStats } from "../types/ipc";

/**
 * Estatísticas acumuladas de um emulador, recarregadas ao fim de cada sync.
 * `null` enquanto carrega ou se o emulador nunca teve atividade.
 */
export function useEmulatorStats(name: string): EmulatorStats | null {
  const [stats, setStats] = useState<EmulatorStats | null>(null);

  useEffect(() => {
    let active = true;
    const reload = () => {
      getEmulatorStats(name)
        .then((s) => {
          if (active) setStats(s);
        })
        .catch(() => {});
    };
    reload();
    const subscription: Promise<UnlistenFn> = listen(EVT.SYNC_COMPLETED, reload);
    return () => {
      active = false;
      subscription.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [name]);

  return stats;
}
