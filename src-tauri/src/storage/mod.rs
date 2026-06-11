//! Persistência local em SQLite via `rusqlite` (Passo 5).
//!
//! Responsabilidades:
//! - Conexão e migrações do banco (`retrosync.db` no diretório de dados do app);
//! - Tabela `sync_manifest`: estado conhecido de cada arquivo (hash, mtime
//!   local, modifiedTime e fileId no Drive) — fonte de verdade do diff;
//! - Tabela de fila de operações pendentes (resiliência offline);
//! - Emuladores configurados pelo usuário.
//!
//! `rusqlite` é síncrono: todo acesso passa por `tokio::task::spawn_blocking`.
