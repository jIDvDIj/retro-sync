/**
 * Wrappers tipados sobre `invoke()`. O restante do frontend nunca chama
 * `invoke` diretamente — só funções deste módulo.
 */

import { invoke } from "@tauri-apps/api/core";

import type {
  AuthStatus,
  EmulatorProfile,
  HealthStatus,
  LastSync,
  SyncSummary,
} from "../types/ipc";

export function healthCheck(): Promise<HealthStatus> {
  return invoke<HealthStatus>("health_check");
}

/** Abre o navegador para o consentimento OAuth2; resolve ao fim do fluxo. */
export function connectGoogleDrive(): Promise<AuthStatus> {
  return invoke<AuthStatus>("connect_google_drive");
}

/** Consulta o status sem disparar fluxo interativo. */
export function getAuthStatus(): Promise<AuthStatus> {
  return invoke<AuthStatus>("get_auth_status");
}

export function disconnectGoogleDrive(): Promise<AuthStatus> {
  return invoke<AuthStatus>("disconnect_google_drive");
}

/** `null` = pasta válida, mas nenhum emulador suportado reconhecido nela. */
export function detectEmulator(path: string): Promise<EmulatorProfile | null> {
  return invoke<EmulatorProfile | null>("detect_emulator", { path });
}

/** Detecta e registra o emulador da pasta para sincronização. */
export function addEmulator(path: string): Promise<EmulatorProfile> {
  return invoke<EmulatorProfile>("add_emulator", { path });
}

export function listEmulators(): Promise<EmulatorProfile[]> {
  return invoke<EmulatorProfile[]>("list_emulators");
}

/** Remove da sincronização; nada é apagado no Drive nem no disco. */
export function removeEmulator(name: string): Promise<void> {
  return invoke<void>("remove_emulator", { name });
}

/** Sync manual bidirecional; resolve com o resumo ao terminar. */
export function syncNow(): Promise<SyncSummary> {
  return invoke<SyncSummary>("sync_now");
}

/** Último sync concluído nesta execução; `null` se ainda não houve nenhum. */
export function getLastSync(): Promise<LastSync | null> {
  return invoke<LastSync | null>("get_last_sync");
}
