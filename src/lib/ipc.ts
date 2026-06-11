/**
 * Wrappers tipados sobre `invoke()`. O restante do frontend nunca chama
 * `invoke` diretamente — só funções deste módulo.
 */

import { invoke } from "@tauri-apps/api/core";

import type { AuthStatus, HealthStatus } from "../types/ipc";

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
