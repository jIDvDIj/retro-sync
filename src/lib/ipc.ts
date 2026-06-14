/**
 * Wrappers tipados sobre `invoke()`. O restante do frontend nunca chama
 * `invoke` diretamente — só funções deste módulo.
 */

import { invoke } from "@tauri-apps/api/core";

import type {
  AuthStatus,
  Conflict,
  ConflictResolution,
  EmulatorProfile,
  HealthStatus,
  LastSync,
  NotificationLevel,
  Settings,
  SyncCategories,
  SyncSummary,
  TriggerSettings,
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

/** Configurações globais do usuário (nome do dispositivo, etc.). */
export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

/** Define o nome deste dispositivo (obrigatório no login). */
export function setDeviceName(name: string): Promise<void> {
  return invoke<void>("set_device_name", { name });
}

/** Liga/desliga os gatilhos de sync automático (sync manual não é afetado). */
export function setTriggers(triggers: TriggerSettings): Promise<void> {
  return invoke<void>("set_triggers", { triggers });
}

/** Define o nível de notificações nativas (all | errors_only | none). */
export function setNotificationLevel(level: NotificationLevel): Promise<void> {
  return invoke<void>("set_notification_level", { level });
}

/** Abre a pasta de backups locais no gerenciador de arquivos do SO. */
export function openBackupFolder(): Promise<void> {
  return invoke<void>("open_backup_folder");
}

/** Conflitos pendentes (ambos os lados mudaram desde o último sync). */
export function listConflicts(): Promise<Conflict[]> {
  return invoke<Conflict[]>("list_conflicts");
}

/** Resolve um conflito mantendo a versão `local` ou `drive`. */
export function resolveConflict(
  emulator: string,
  category: Conflict["category"],
  relPath: string,
  keep: ConflictResolution,
): Promise<void> {
  return invoke<void>("resolve_conflict", { emulator, category, relPath, keep });
}

/** Categorias de sync habilitadas para um emulador (default: todas ativas). */
export function getEmulatorCategories(name: string): Promise<SyncCategories> {
  return invoke<SyncCategories>("get_emulator_categories", { name });
}

/** Define quais categorias sincronizar para um emulador. */
export function setEmulatorCategories(name: string, categories: SyncCategories): Promise<void> {
  return invoke<void>("set_emulator_categories", { name, categories });
}
