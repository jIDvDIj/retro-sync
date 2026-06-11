/**
 * Wrappers tipados sobre `invoke()`. O restante do frontend nunca chama
 * `invoke` diretamente — só funções deste módulo.
 */

import { invoke } from "@tauri-apps/api/core";

import type { HealthStatus } from "../types/ipc";

export function healthCheck(): Promise<HealthStatus> {
  return invoke<HealthStatus>("health_check");
}
