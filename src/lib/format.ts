/**
 * Formatação humanizada de bytes, taxas e durações — única fonte para toda a
 * UI (ConflictModal, SyncStatus, fila de pendências, histórico de backups).
 *
 * Convenção:
 * - tamanhos de arquivo usam prefixo binário IEC (KiB, MiB, GiB);
 * - taxas de transferência usam prefixo decimal SI (KB/s, MB/s).
 */

import { currentLocale } from "../i18n";

const IEC_UNITS = ["B", "KiB", "MiB", "GiB", "TiB"] as const;
const SI_UNITS = ["B/s", "KB/s", "MB/s", "GB/s"] as const;

function formatUnitPrefixed(value: number, base: number, units: readonly string[]): string {
  let scaled = Math.max(0, value);
  let unit = 0;
  while (scaled >= base && unit < units.length - 1) {
    scaled /= base;
    unit += 1;
  }
  const digits = unit === 0 ? 0 : 1;
  const text = scaled.toLocaleString(currentLocale(), {
    minimumFractionDigits: 0,
    maximumFractionDigits: digits,
  });
  return `${text} ${units[unit]}`;
}

/** Tamanho em bytes → "820 B" · "34,2 KiB" · "1,2 GiB" (prefixo binário). */
export function formatBytes(bytes: number): string {
  return formatUnitPrefixed(bytes, 1024, IEC_UNITS);
}

/** Taxa em bytes/s → "950 B/s" · "1,2 MB/s" (prefixo decimal). */
export function formatRate(bytesPerSec: number): string {
  return formatUnitPrefixed(bytesPerSec, 1000, SI_UNITS);
}

/** Duração em ms → "45s" · "1m 30s" · "1h 15m". */
export function formatDuration(ms: number): string {
  const secs = Math.max(0, Math.round(ms / 1000));
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
  return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
}

/**
 * Estimativa de conclusão arredondada PARA CIMA no múltiplo de 10 s mais
 * próximo. `null` quando não há taxa medida ainda.
 */
export function formatEta(remainingBytes: number, bytesPerSec: number): string | null {
  if (bytesPerSec <= 0) return null;
  const secs = Math.max(10, Math.ceil(remainingBytes / bytesPerSec / 10) * 10);
  return formatDuration(secs * 1000);
}
