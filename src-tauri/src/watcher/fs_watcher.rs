//! Watcher de filesystem (gatilho `file-change`): reage a escritas nas pastas
//! de saves/savestates sem esperar o emulador fechar — útil em sessões longas.
//!
//! - **Eventos nativos** via a crate `notify` (`ReadDirectoryChangesW` no
//!   Windows, `inotify` no Linux, `FSEvents` no macOS);
//! - **Debounce agregador**: cada evento reinicia a janela do emulador; o sync
//!   só dispara `FS_WATCHER_DEBOUNCE_SECS` após o ÚLTIMO evento (agrupa
//!   rajadas de escrita em um único sync);
//! - **Anti-loop**: eventos em arquivos que o próprio sync acabou de baixar
//!   (`SyncEngine::is_recent_download`) e em temporários `.retrosync-tmp` são
//!   ignorados;
//! - **Nunca com o jogo aberto**: o disparo é adiado enquanto qualquer
//!   emulador estiver rodando (o gatilho `emulator-stop` cobre o fechamento).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use super::RunningEmulators;
use crate::constants::{
    FS_WATCHER_DEBOUNCE_SECS, FS_WATCHER_RECONCILE_SECS, TMP_SUFFIX, TRIGGER_FILE_CHANGE,
};
use crate::storage::db::Db;
use crate::storage::emulators;
use crate::sync::{SyncDirection, SyncEngine};

/// Pastas observadas de um emulador (absolutas: raiz + bases de saves/states).
struct WatchedEmulator {
    name: String,
    dirs: Vec<PathBuf>,
}

/// Emuladores cuja janela de debounce venceu (sem eventos novos há pelo menos
/// `debounce`). Função pura para ser testável.
fn due_emulators(
    pending: &HashMap<String, Instant>,
    now: Instant,
    debounce: Duration,
) -> Vec<String> {
    pending
        .iter()
        .filter(|(_, last)| now.duration_since(**last) >= debounce)
        .map(|(name, _)| name.clone())
        .collect()
}

/// Emulador dono de `path`, se o caminho está sob alguma pasta observada.
fn owner_of<'a>(watched: &'a [WatchedEmulator], path: &Path) -> Option<&'a str> {
    watched
        .iter()
        .find(|w| w.dirs.iter().any(|dir| path.starts_with(dir)))
        .map(|w| w.name.as_str())
}

/// Monta a lista de pastas observadas a partir dos perfis configurados
/// (saves + savestates; config fica de fora — muda o tempo todo com o app
/// aberto e já sincroniza nos gatilhos de processo).
async fn watch_list(db: &Db) -> Vec<WatchedEmulator> {
    let profiles = match db.with(emulators::list).await {
        Ok(profiles) => profiles,
        Err(err) => {
            tracing::warn!(error = %err, "fs-watcher: falha ao listar emuladores");
            return Vec::new();
        }
    };
    profiles
        .into_iter()
        .map(|p| WatchedEmulator {
            dirs: p
                .saves_paths
                .iter()
                .chain(&p.state_paths)
                .map(|rel| p.root_path.join(rel))
                .filter(|abs| abs.is_dir())
                .collect(),
            name: p.name,
        })
        .filter(|w| !w.dirs.is_empty())
        .collect()
}

/// (Re)cria o watcher nativo observando as pastas de `watched`. Devolve `None`
/// (com warning) se o backend nativo não puder ser iniciado.
fn build_watcher(
    watched: &[WatchedEmulator],
    tx: mpsc::Sender<PathBuf>,
) -> Option<RecommendedWatcher> {
    let mut watcher =
        match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(event) = res else { return };
            // Só mudanças de conteúdo/estrutura interessam (Access geraria ruído).
            if !matches!(
                event.kind,
                notify::EventKind::Create(_)
                    | notify::EventKind::Modify(_)
                    | notify::EventKind::Remove(_)
            ) {
                return;
            }
            for path in event.paths {
                // `blocking_send` roda na thread do backend nativo, fora do runtime.
                let _ = tx.blocking_send(path);
            }
        }) {
            Ok(watcher) => watcher,
            Err(err) => {
                tracing::warn!(error = %err, "fs-watcher: backend nativo indisponível");
                return None;
            }
        };

    for w in watched {
        for dir in &w.dirs {
            if let Err(err) = watcher.watch(dir, RecursiveMode::Recursive) {
                tracing::warn!(pasta = %dir.display(), error = %err, "fs-watcher: falha ao observar pasta");
            }
        }
    }
    Some(watcher)
}

/// Sobe o watcher de filesystem. Reconciliação periódica reabsorve mudanças na
/// lista de emuladores (adicionados/removidos/raiz trocada).
pub fn start(db: Db, engine: Arc<SyncEngine>, running: RunningEmulators) {
    tauri::async_runtime::spawn(async move {
        let (tx, mut rx) = mpsc::channel::<PathBuf>(256);
        let mut watched = watch_list(&db).await;
        // O watcher precisa permanecer vivo — dropar cancela as observações.
        let mut _watcher = build_watcher(&watched, tx.clone());

        // Última atividade por emulador (janela de debounce em aberto).
        let mut pending: HashMap<String, Instant> = HashMap::new();
        let debounce = Duration::from_secs(FS_WATCHER_DEBOUNCE_SECS);
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        let mut reconcile = tokio::time::interval(Duration::from_secs(FS_WATCHER_RECONCILE_SECS));
        reconcile.reset(); // o primeiro tick de `interval` é imediato

        loop {
            tokio::select! {
                Some(path) = rx.recv() => {
                    if path
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().ends_with(TMP_SUFFIX))
                    {
                        continue;
                    }
                    // Anti-loop: escrita feita pelo próprio sync há pouco.
                    if engine.is_recent_download(&path) {
                        continue;
                    }
                    if let Some(name) = owner_of(&watched, &path) {
                        pending.insert(name.to_string(), Instant::now());
                    }
                }
                _ = tick.tick() => {
                    let due = due_emulators(&pending, Instant::now(), debounce);
                    if due.is_empty() {
                        continue;
                    }
                    // Jogo aberto: mantém a janela pendente — o sync sai quando
                    // o emulador fechar (ou no próximo tick sem processo).
                    let busy = running.lock().map(|set| !set.is_empty()).unwrap_or(false);
                    if busy {
                        continue;
                    }
                    for name in due {
                        pending.remove(&name);
                        tracing::info!(emulador = %name, "mudança de arquivo detectada; sync Local → Drive");
                        if let Err(err) = engine
                            .sync_emulator(&name, SyncDirection::LocalToDrive, TRIGGER_FILE_CHANGE)
                            .await
                        {
                            tracing::warn!(emulador = %name, error = %err, "sync do fs-watcher falhou");
                        }
                    }
                }
                _ = reconcile.tick() => {
                    let fresh = watch_list(&db).await;
                    let changed = fresh.len() != watched.len()
                        || fresh.iter().zip(&watched).any(|(a, b)| {
                            a.name != b.name || a.dirs != b.dirs
                        });
                    if changed {
                        watched = fresh;
                        _watcher = build_watcher(&watched, tx.clone());
                        tracing::info!(emuladores = watched.len(), "fs-watcher: pastas observadas reconciliadas");
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_emulators_respeita_a_janela_de_debounce() {
        let now = Instant::now();
        let debounce = Duration::from_secs(8);
        let mut pending = HashMap::new();
        pending.insert("PPSSPP".to_string(), now - Duration::from_secs(10));
        pending.insert("PCSX2".to_string(), now - Duration::from_secs(2));

        let due = due_emulators(&pending, now, debounce);

        assert_eq!(due, vec!["PPSSPP".to_string()]);
    }

    #[test]
    fn owner_of_casa_caminho_com_a_pasta_observada() {
        let watched = vec![WatchedEmulator {
            name: "PPSSPP".into(),
            dirs: vec![PathBuf::from("/emu/PSP/SAVEDATA")],
        }];
        assert_eq!(
            owner_of(&watched, Path::new("/emu/PSP/SAVEDATA/GAME01/SAVE.bin")),
            Some("PPSSPP")
        );
        assert_eq!(owner_of(&watched, Path::new("/outro/lugar.bin")), None);
    }
}
