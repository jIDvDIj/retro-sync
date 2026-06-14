import { useCallback, useEffect, useState } from "react";

import { getSettings } from "../lib/ipc";
import type { Settings } from "../types/ipc";

interface UseSettings {
  settings: Settings | null;
  reload: () => Promise<void>;
}

/**
 * Configurações globais carregadas no nível do App e compartilhadas entre o
 * header (exibe o dispositivo) e o modal de configurações (edita-as).
 * `reload` é chamado após qualquer alteração para manter tudo em sincronia.
 */
export function useSettings(): UseSettings {
  const [settings, setSettings] = useState<Settings | null>(null);

  const reload = useCallback(async () => {
    try {
      setSettings(await getSettings());
    } catch {
      // O App ainda funciona sem settings (defaults no backend); ignora.
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  return { settings, reload };
}
