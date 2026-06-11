//! Nomes dos eventos Tauri emitidos pelo backend. O frontend espelha estes
//! valores em `src/types/ipc.ts` (objeto `EVT`).

#![allow(dead_code)]

pub const EVT_SYNC_STARTED: &str = "sync:started";
pub const EVT_SYNC_PROGRESS: &str = "sync:progress";
pub const EVT_SYNC_COMPLETED: &str = "sync:completed";
pub const EVT_SYNC_ERROR: &str = "sync:error";
pub const EVT_AUTH_STATUS: &str = "auth:status";
pub const EVT_EMULATOR_STATUS: &str = "emulator:status";
