/**
 * Espelho TypeScript dos tipos Rust que cruzam a boundary Tauri.
 * Fonte de verdade: structs em `src-tauri/src/` (serde, rename_all = camelCase).
 * Qualquer mudança lá DEVE ser refletida aqui.
 */

/** `commands::HealthStatus` */
export interface HealthStatus {
  version: string;
  ready: boolean;
}

/** `auth::AuthStatus` */
export interface AuthStatus {
  connected: boolean;
  email: string | null;
}

/** `emulator::EmulatorProfile` — paths serializam como string */
export interface EmulatorProfile {
  name: string;
  rootPath: string;
  savesPaths: string[];
  configPaths: string[];
  statePaths: string[];
}

/** `sync::SyncDirection` */
export type SyncDirection = "DriveToLocal" | "LocalToDrive" | "Bidirectional";

/** `sync::SyncProgress` — payload do evento `sync:progress` */
export interface SyncProgress {
  emulator: string;
  currentFile: string;
  completed: number;
  total: number;
  direction: SyncDirection;
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
    | "other";
  message: string;
}

/** Espelho de `src-tauri/src/events.rs` */
export const EVT = {
  SYNC_STARTED: "sync:started",
  SYNC_PROGRESS: "sync:progress",
  SYNC_COMPLETED: "sync:completed",
  SYNC_ERROR: "sync:error",
  AUTH_STATUS: "auth:status",
  EMULATOR_STATUS: "emulator:status",
} as const;

export type EventName = (typeof EVT)[keyof typeof EVT];
