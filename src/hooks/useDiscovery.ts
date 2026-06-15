import { useCallback, useEffect, useState } from "react";

import { errorMessage } from "../lib/errors";
import { discoverEmulators } from "../lib/ipc";
import type { DiscoveredEmulator } from "../types/ipc";

interface UseDiscovery {
  discovered: DiscoveredEmulator[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

/** Descoberta automática de emuladores instalados no sistema (não persiste nada). */
export function useDiscovery(): UseDiscovery {
  const [discovered, setDiscovered] = useState<DiscoveredEmulator[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setDiscovered(await discoverEmulators());
      setError(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { discovered, loading, error, refresh };
}
