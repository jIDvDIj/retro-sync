//! Process watcher — detecção de abertura/fechamento dos emuladores (Passo 6).
//!
//! Loop assíncrono (`tokio::time::interval`, `WATCHER_POLL_INTERVAL_SECS`)
//! que consulta os processos do SO via `sysinfo` e publica `WatcherEvent`s
//! num canal `tokio::sync::mpsc` consumido pelo `SyncEngine`. Aplica debounce
//! para ignorar flapping de processos auxiliares dos emuladores.

#![allow(dead_code)]

/// Evento publicado no canal watcher → SyncEngine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatcherEvent {
    /// Emulador (nome canônico do perfil) começou a rodar.
    EmulatorStarted(String),
    /// Emulador deixou de rodar.
    EmulatorStopped(String),
}
