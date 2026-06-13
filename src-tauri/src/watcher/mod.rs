//! Process watcher — detecção de abertura/fechamento dos emuladores.
//!
//! Duas tasks ligadas por um canal `tokio::sync::mpsc`:
//! - **produtor**: loop `tokio::time::interval` (`WATCHER_POLL_INTERVAL_SECS`)
//!   que consulta os processos do SO via `sysinfo` (em `spawn_blocking`) e
//!   publica transições, com debounce contra flapping;
//! - **consumidor**: para cada transição, dispara o sync direcionado e emite
//!   o status ao frontend.
//!
//! Gatilhos (Passo 6 da especificação):
//! - emulador **abriu** → sync Drive → Local (saves frescos antes do jogo carregar);
//! - emulador **fechou** → sync Local → Drive (sobe os saves da sessão).

mod process_watcher;

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use sysinfo::System;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

use crate::constants::{
    TRIGGER_EMULATOR_START, TRIGGER_EMULATOR_STOP, WATCHER_POLL_INTERVAL_SECS,
    WATCHER_STOP_DEBOUNCE_TICKS,
};
use crate::emulator;
use crate::events::EVT_EMULATOR_STATUS;
use crate::storage::db::Db;
use crate::storage::emulators;
use crate::sync::{SyncDirection, SyncEngine};
use process_watcher::{poll_once, MonitoredEmulator, RunStateTracker};

/// Evento publicado no canal watcher → consumidor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatcherEvent {
    /// Emulador (nome canônico do perfil) começou a rodar.
    EmulatorStarted(String),
    /// Emulador deixou de rodar.
    EmulatorStopped(String),
}

/// Payload do evento `emulator:status`. Espelhado em `src/types/ipc.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EmulatorStatusEvent {
    emulator: String,
    running: bool,
}

/// Sobe o produtor e o consumidor do watcher. Chamado uma vez no `setup`.
pub fn start(db: Db, engine: Arc<SyncEngine>, app: AppHandle) {
    let (tx, rx) = mpsc::channel::<WatcherEvent>(32);
    spawn_poll_loop(db, tx);
    spawn_consumer(rx, engine, app);
}

fn spawn_poll_loop(db: Db, tx: mpsc::Sender<WatcherEvent>) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(WATCHER_POLL_INTERVAL_SECS));
        // System e tracker persistem entre ticks; viajam para dentro do
        // `spawn_blocking` a cada poll e voltam com os eventos.
        let mut sys_state: Option<(System, RunStateTracker)> = None;

        loop {
            interval.tick().await;

            let profiles = match db.with(emulators::list).await {
                Ok(profiles) => profiles,
                Err(err) => {
                    tracing::warn!(error = %err, "watcher: falha ao listar emuladores configurados");
                    continue;
                }
            };

            let monitored: Vec<MonitoredEmulator> = profiles
                .into_iter()
                .filter_map(|p| {
                    let process_names = emulator::process_names(&p.name);
                    (!process_names.is_empty()).then_some(MonitoredEmulator {
                        name: p.name,
                        process_names,
                    })
                })
                .collect();
            if monitored.is_empty() {
                continue;
            }

            let (mut system, mut tracker) = sys_state.take().unwrap_or_else(|| {
                (
                    System::new(),
                    RunStateTracker::new(WATCHER_STOP_DEBOUNCE_TICKS),
                )
            });

            let joined = tokio::task::spawn_blocking(move || {
                let events = poll_once(&mut system, &mut tracker, &monitored);
                (system, tracker, events)
            })
            .await;

            let (system, tracker, events) = match joined {
                Ok(out) => out,
                Err(err) => {
                    tracing::warn!(error = %err, "watcher: tarefa de polling abortada");
                    continue;
                }
            };
            sys_state = Some((system, tracker));

            for event in events {
                if tx.send(event).await.is_err() {
                    tracing::debug!("watcher: consumidor encerrado; parando o polling");
                    return;
                }
            }
        }
    });
}

fn spawn_consumer(mut rx: mpsc::Receiver<WatcherEvent>, engine: Arc<SyncEngine>, app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            let (name, running, direction, trigger) = match event {
                WatcherEvent::EmulatorStarted(name) => (
                    name,
                    true,
                    SyncDirection::DriveToLocal,
                    TRIGGER_EMULATOR_START,
                ),
                WatcherEvent::EmulatorStopped(name) => (
                    name,
                    false,
                    SyncDirection::LocalToDrive,
                    TRIGGER_EMULATOR_STOP,
                ),
            };

            let _ = app.emit(
                EVT_EMULATOR_STATUS,
                &EmulatorStatusEvent {
                    emulator: name.clone(),
                    running,
                },
            );
            tracing::info!(emulador = %name, running, trigger, "transição de emulador detectada");

            if let Err(err) = engine.sync_emulator(&name, direction, trigger).await {
                tracing::warn!(emulador = %name, error = %err, "sync disparado pelo watcher falhou");
            }
        }
    });
}
