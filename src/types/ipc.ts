/**
 * Espelho TypeScript dos tipos Rust que cruzam a boundary Tauri.
 * Fonte de verdade: structs em `src-tauri/src/` (serde, rename_all = camelCase).
 * Qualquer mudança lá DEVE ser refletida aqui.
 */

/** `commands::HealthStatus` */
export interface HealthStatus {
  version: string;
  ready: boolean;
  /** `true` quando compilado para Android ou iOS; `false` no desktop. */
  isMobile: boolean;
}

/** `auth::AuthStatus` */
export interface AuthStatus {
  connected: boolean;
  email: string | null;
}

/** `storage::settings::TriggerSettings` — gatilhos de sync automático */
export interface TriggerSettings {
  startup: boolean;
  emulatorStart: boolean;
  emulatorStop: boolean;
}

/** `storage::settings::NotificationLevel` — nível de notificações nativas */
export type NotificationLevel = "all" | "errors_only" | "none";

/** `storage::settings::Settings` — configurações globais do usuário */
export interface Settings {
  deviceName: string | null;
  triggers: TriggerSettings;
  notificationLevel: NotificationLevel;
  /** Início automático com o sistema. Lido do SO, não do banco. */
  autostart: boolean;
  /** Dias de retenção dos backups locais (0 = manter para sempre). */
  backupRetentionDays: number;
  /** Intervalo do scan periódico em minutos (0 = desativado). */
  scanIntervalMinutes: number;
  /** Máximo de versões arquivadas por arquivo no histórico pré-download. */
  maxBackupVersions: number;
}

/** `versioning::FileVersion` — versão arquivada de um arquivo no histórico */
export interface FileVersion {
  /** Carimbo `YYYYMMDD-HHMMSS` extraído do nome arquivado. */
  stamp: string;
  sizeBytes: number;
  modifiedAtMs: number;
  absPath: string;
}

/** `emulator::EmulatorProfile` — paths serializam como string */
export interface EmulatorProfile {
  name: string;
  rootPath: string;
  savesPaths: string[];
  configPaths: string[];
  statePaths: string[];
}

/** `emulator::DiscoverySource` — origem do reconhecimento na descoberta */
export type DiscoverySource = "dataDir" | "registry" | "both";

/** `emulator::DiscoveredEmulator` — sugestão da descoberta automática */
export interface DiscoveredEmulator {
  name: string;
  /** `null` = instalado mas sem pasta de dados ainda (só registro). */
  profile: EmulatorProfile | null;
  source: DiscoverySource;
}

/** `storage::emulators::SyncCategories` — categorias habilitadas por emulador */
export interface SyncCategories {
  saves: boolean;
  savestates: boolean;
  config: boolean;
}

/** `games::SyncedGame` — jogo sincronizado, agregado do manifest (FEATURE-001) */
export interface SyncedGame {
  /** Serial técnico extraído do caminho (`ULUS12345`) ou nome de arquivo. */
  serial: string;
  /** `null` quando o serial não está na base de nomes — a UI mostra o serial. */
  name: string | null;
  emulator: string;
  categories: ("saves" | "savestates" | "config")[];
  lastSyncedAtMs: number;
  sizeBytes: number;
}

/** `sync::SyncDirection` */
export type SyncDirection = "DriveToLocal" | "LocalToDrive" | "Bidirectional";

/** `sync::SyncProgress` — payload do evento `sync:progress` */
export interface SyncProgress {
  emulator: string;
  currentFile: string;
  completed: number;
  total: number;
  /** Bytes transferidos / totais do plano da categoria em andamento. */
  bytesDone: number;
  bytesTotal: number;
  direction: SyncDirection;
}

/** `storage::queue::PendingOp` — arquivo na fila offline (retentado no próximo sync) */
export interface PendingOp {
  emulator: string;
  category: "saves" | "savestates" | "config";
  relPath: string;
  direction: "upload" | "download";
  enqueuedAtMs: number;
  attempts: number;
  lastError: string | null;
  /** A partir de quando pode ser retentado; `null` = morta (esgotou as
   * tentativas — só volta pela ação "tentar novamente"). */
  nextRetryAtMs: number | null;
}

/** `backups::BackupEntry` — cópia de backup local listada no histórico */
export interface BackupEntry {
  emulator: string;
  /** Execução que gerou o backup (`2025-07-01_10-30-00` ou `conflito-…`). */
  run: string;
  category: string;
  relPath: string;
  sizeBytes: number;
  modifiedAtMs: number;
  absPath: string;
}

/** `sync::SyncSummary` — retorno de `sync_now` e payload de `sync:completed` */
export interface SyncSummary {
  uploaded: number;
  downloaded: number;
  skipped: number;
  failed: number;
  queued: number;
  /** Arquivos copiados para backup antes de sobrescritos no primeiro sync. */
  backedUp: number;
  /** Conflitos detectados neste sync (ambos os lados mudaram). */
  conflicts: number;
  durationMs: number;
}

/** `storage::conflicts::Conflict` — conflito pendente; payload de `sync:conflict` */
export interface Conflict {
  emulator: string;
  category: "saves" | "savestates" | "config";
  relPath: string;
  localMtimeMs: number;
  localSize: number;
  localDevice: string | null;
  driveMtimeMs: number;
  driveSize: number;
  driveDevice: string | null;
  driveFileId: string;
  localAbsPath: string;
  detectedAtMs: number;
}

/** `sync::ConflictResolution` — qual versão manter ao resolver um conflito */
export type ConflictResolution = "local" | "drive";

/** `sync::engine::SyncStarted` — payload do evento `sync:started` */
export interface SyncStarted {
  trigger: string;
  direction: SyncDirection;
}

/** `sync::engine::LastSync` — retorno de `get_last_sync` */
export interface LastSync {
  atMs: number;
  trigger: string;
  summary: SyncSummary;
}

/** `sync::engine::SyncError` — payload do evento `sync:error` */
export interface SyncErrorEvent {
  emulator: string | null;
  message: string;
}

/** `watcher::EmulatorStatusEvent` — payload do evento `emulator:status` */
export interface EmulatorStatusEvent {
  emulator: string;
  running: boolean;
}

/** `error::AppError` serializado — todo comando rejeita com este shape */
export interface AppErrorPayload {
  code:
    | "io"
    | "database"
    | "network"
    | "keyring"
    | "serialization"
    | "auth"
    | "emulator_not_detected"
    | "emulator_exists"
    | "file_busy"
    | "drive_not_found"
    | "insufficient_disk_space"
    | "integrity"
    | "other";
  message: string;
  /** Detalhe técnico sem o prefixo (caminho, nome, msg da lib). O frontend
   * localiza o prefixo pelo `code` e anexa este detalhe. */
  detail: string;
}

/** Espelho de `src-tauri/src/events.rs` */
export const EVT = {
  SYNC_STARTED: "sync:started",
  SYNC_PROGRESS: "sync:progress",
  SYNC_COMPLETED: "sync:completed",
  SYNC_ERROR: "sync:error",
  SYNC_CONFLICT: "sync:conflict",
  AUTH_STATUS: "auth:status",
  EMULATOR_STATUS: "emulator:status",
} as const;

export type EventName = (typeof EVT)[keyof typeof EVT];
