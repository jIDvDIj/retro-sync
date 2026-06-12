//! Persistência local em SQLite via `rusqlite`.
//!
//! - `db`: conexão única + migrações, acesso async via `spawn_blocking`;
//! - `manifest`: tabela `sync_manifest` — estado de cada arquivo no último sync;
//! - `queue`: fila de operações pendentes (resiliência offline);
//! - `emulators`: perfis configurados pelo usuário.

pub mod db;
pub mod emulators;
pub mod manifest;
pub mod queue;
