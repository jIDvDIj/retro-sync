import type { AppErrorPayload } from "../types/ipc";

/** Extrai a mensagem legível de um erro rejeitado por um comando Tauri. */
export function errorMessage(err: unknown): string {
  const payload = err as Partial<AppErrorPayload>;
  return payload?.message ?? "erro inesperado ao falar com o backend";
}
