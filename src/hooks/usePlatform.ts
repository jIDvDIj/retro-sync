import { useEffect, useState } from "react";

import { healthCheck } from "../lib/ipc";

/** Detecta a plataforma uma vez ao montar. Padrão `false` até a resposta chegar. */
export function usePlatform() {
  const [isMobile, setIsMobile] = useState(false);

  useEffect(() => {
    healthCheck()
      .then((h) => setIsMobile(h.isMobile))
      .catch(() => {});
  }, []);

  return { isMobile };
}
