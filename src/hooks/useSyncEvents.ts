import { useEffect, useState } from "react";

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { getLastSync } from "../lib/ipc";
import {
  EVT,
  type EmulatorStatusEvent,
  type LastSync,
  type SyncErrorEvent,
  type SyncProgress,
  type SyncStarted,
} from "../types/ipc";

export type SyncPhase = "idle" | "syncing";

export interface SyncState {
  phase: SyncPhase;
  progress: SyncProgress | null;
  lastSync: LastSync | null;
  lastError: SyncErrorEvent | null;
  /** Nomes dos emuladores atualmente em execução (via `emulator:status`). */
  running: Set<string>;
}

/**
 * Estado de sincronização consolidado a partir dos eventos do backend.
 * Um único assinante para os eventos `sync:*` e `emulator:status`, usado pela
 * UI inteira (barra de status e cards de emulador).
 */
export function useSyncEvents(): SyncState {
  const [phase, setPhase] = useState<SyncPhase>("idle");
  const [progress, setProgress] = useState<SyncProgress | null>(null);
  const [lastSync, setLastSync] = useState<LastSync | null>(null);
  const [lastError, setLastError] = useState<SyncErrorEvent | null>(null);
  const [running, setRunning] = useState<Set<string>>(new Set());

  useEffect(() => {
    // Estado inicial: o startup sync pode já ter rodado antes da UI montar.
    getLastSync()
      .then((value) => {
        if (value) setLastSync(value);
      })
      .catch(() => {});

    const subscriptions: Promise<UnlistenFn>[] = [
      listen<SyncStarted>(EVT.SYNC_STARTED, () => {
        setPhase("syncing");
        setProgress(null);
        setLastError(null);
      }),
      listen<SyncProgress>(EVT.SYNC_PROGRESS, (event) => {
        setProgress(event.payload);
      }),
      listen(EVT.SYNC_COMPLETED, () => {
        setPhase("idle");
        setProgress(null);
        // O backend grava o LastSync antes de emitir `sync:completed`.
        getLastSync()
          .then((value) => {
            if (value) setLastSync(value);
          })
          .catch(() => {});
      }),
      listen<SyncErrorEvent>(EVT.SYNC_ERROR, (event) => {
        setLastError(event.payload);
      }),
      listen<EmulatorStatusEvent>(EVT.EMULATOR_STATUS, (event) => {
        const { emulator, running: isRunning } = event.payload;
        setRunning((prev) => {
          const next = new Set(prev);
          if (isRunning) next.add(emulator);
          else next.delete(emulator);
          return next;
        });
      }),
    ];

    return () => {
      subscriptions.forEach((promise) => promise.then((unlisten) => unlisten()).catch(() => {}));
    };
  }, []);

  return { phase, progress, lastSync, lastError, running };
}
