import { useCallback, useEffect, useState } from "react";

import { useErrorMessage } from "../lib/errors";
import { listEmulators, removeEmulator } from "../lib/ipc";
import type { EmulatorProfile } from "../types/ipc";

interface UseEmulators {
  emulators: EmulatorProfile[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  remove: (name: string) => Promise<void>;
}

/** Lista de emuladores configurados, com recarga e remoção. */
export function useEmulators(): UseEmulators {
  const errorMessage = useErrorMessage();
  const [emulators, setEmulators] = useState<EmulatorProfile[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setEmulators(await listEmulators());
      setError(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  }, [errorMessage]);

  useEffect(() => {
    // IIFE em vez de `void refresh()` direto — ver comentário equivalente em
    // useSettings.ts (react-hooks/set-state-in-effect).
    void (async () => {
      await refresh();
    })();
  }, [refresh]);

  const remove = useCallback(
    async (name: string) => {
      await removeEmulator(name);
      await refresh();
    },
    [refresh],
  );

  return { emulators, loading, error, refresh, remove };
}
