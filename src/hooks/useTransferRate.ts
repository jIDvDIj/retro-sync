import { useEffect, useRef, useState } from "react";

import type { SyncProgress } from "../types/ipc";

/**
 * Velocidade de transferência (bytes/s) estimada a partir de snapshots
 * consecutivos do evento `sync:progress`, suavizada por média móvel
 * exponencial para o número não saltar a cada arquivo.
 *
 * `bytesDone` é por categoria em andamento: quando o contador "volta" (nova
 * categoria/emulador), o histórico é descartado e a medição recomeça.
 * Retorna `0` enquanto não há dois snapshots comparáveis.
 */
export function useTransferRate(progress: SyncProgress | null): number {
  const last = useRef<{ bytes: number; at: number } | null>(null);
  const [rate, setRate] = useState(0);

  useEffect(() => {
    if (!progress) {
      last.current = null;
      setRate(0);
      return;
    }
    const now = performance.now();
    const prev = last.current;
    last.current = { bytes: progress.bytesDone, at: now };

    if (!prev || progress.bytesDone < prev.bytes) return;
    const deltaSecs = (now - prev.at) / 1000;
    if (deltaSecs <= 0) return;

    const instant = (progress.bytesDone - prev.bytes) / deltaSecs;
    setRate((current) => (current === 0 ? instant : current * 0.7 + instant * 0.3));
  }, [progress]);

  return rate;
}
