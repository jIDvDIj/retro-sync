/**
 * Wrappers tipados sobre `invoke()`. O restante do frontend nunca chama
 * `invoke` diretamente — só funções deste módulo.
 */

import { invoke } from "@tauri-apps/api/core";

import type {
  AuthStatus,
  Conflict,
  ConflictResolution,
  DiscoveredEmulator,
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

/**
 * Registra um emulador com pastas informadas manualmente (fallback quando a
 * detecção falha). Caminhos relativos à raiz. Rejeita com `emulator_exists` se
 * já houver um emulador com o mesmo nome.
 */
export function addEmulatorManual(
  name: string,
  path: string,
  savesPaths: string[],
  statePaths: string[],
  configPaths: string[],
): Promise<EmulatorProfile> {
  return invoke<EmulatorProfile>("add_emulator_manual", {
    name,
    path,
    savesPaths,
    statePaths,
    configPaths,
  });
}

export function listEmulators(): Promise<EmulatorProfile[]> {
  return invoke<EmulatorProfile[]>("list_emulators");
}

/** Emuladores do catálogo detectados instalados no sistema. Não persiste nada. */
export function discoverEmulators(): Promise<DiscoveredEmulator[]> {
  return invoke<DiscoveredEmulator[]>("discover_emulators");
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

/** Liga/desliga o início automático do RetroSync junto com o sistema. */
export function setAutostart(enabled: boolean): Promise<void> {
  return invoke<void>("set_autostart", { enabled });
}

/** Abre a pasta de backups locais no gerenciador de arquivos do SO. */
export function openBackupFolder(): Promise<void> {
  return invoke<void>("open_backup_folder");
}

/**
 * Abre o seletor de pasta nativo do SO (SAF no Android) e retorna a URI da
 * árvore concedida. No desktop lança erro — use o seletor de ficheiros nativo.
 */
export function pickEmulatorFolder(): Promise<string> {
  return invoke<string>("pick_emulator_folder");
}

/**
 * Tenta reconhecer automaticamente o emulador na árvore SAF `tree` (retornada
 * por {@link pickEmulatorFolder}), testando o mesmo catálogo do desktop via
 * chamadas ao plugin nativo. `null` quando nenhum emulador é reconhecido —
 * cai no formulário manual.
 */
export function detectEmulatorMobile(tree: string): Promise<EmulatorProfile | null> {
  return invoke<EmulatorProfile | null>("detect_emulator_mobile", { tree });
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
