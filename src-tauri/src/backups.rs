//! Listagem dos backups locais para a UI (histórico de versões).
//!
//! Os backups são gravados pelo `SyncEngine` em
//! `<app_data>/backups/<emulador>/<execução>/<categoria>/<rel_path>`, onde
//! `<execução>` é o timestamp do sync (`2025-07-01_10-30-00`) ou
//! `conflito-<timestamp>` para resoluções de conflito. Este módulo só lê essa
//! árvore — nunca apaga nem altera nada.

use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::error::AppResult;

/// Uma cópia de backup em disco. Espelhada em `src/types/ipc.ts` (`BackupEntry`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub emulator: String,
    /// Rótulo da execução que gerou o backup (`2025-07-01_10-30-00` ou
    /// `conflito-2025-07-01_10-30-00`).
    pub run: String,
    /// Categoria como aparece na pasta (`saves` | `savestates` | `config`).
    pub category: String,
    pub rel_path: String,
    pub size_bytes: i64,
    pub modified_at_ms: i64,
    pub abs_path: String,
}

/// Varre a árvore de backups. Pasta inexistente = lista vazia (nunca houve
/// backup). Entradas fora do formato esperado são ignoradas. Mais recentes
/// primeiro.
pub fn list(dir: &Path) -> AppResult<Vec<BackupEntry>> {
    let mut out = Vec::new();
    for emulator in subdirs(dir) {
        for run in subdirs(&dir.join(&emulator)) {
            for category in subdirs(&dir.join(&emulator).join(&run)) {
                let base = dir.join(&emulator).join(&run).join(&category);
                let mut files = Vec::new();
                collect_files(&base, &base, &mut files)?;
                for (rel_path, size_bytes, modified_at_ms, abs_path) in files {
                    out.push(BackupEntry {
                        emulator: emulator.clone(),
                        run: run.clone(),
                        category: category.clone(),
                        rel_path,
                        size_bytes,
                        modified_at_ms,
                        abs_path,
                    });
                }
            }
        }
    }
    out.sort_by_key(|b| std::cmp::Reverse(b.modified_at_ms));
    Ok(out)
}

/// Nomes das subpastas diretas de `dir` (vazio se `dir` não existe).
fn subdirs(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

fn collect_files(
    base: &Path,
    dir: &Path,
    out: &mut Vec<(String, i64, i64, String)>,
) -> AppResult<()> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_files(base, &path, out)?;
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let rel_path = path
            .strip_prefix(base)
            .map(|rel| {
                rel.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_else(|_| entry.file_name().to_string_lossy().into_owned());
        let modified_at_ms = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        out.push((
            rel_path,
            metadata.len() as i64,
            modified_at_ms,
            path.to_string_lossy().into_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasta_inexistente_retorna_vazio() {
        let tmp = tempfile::tempdir().unwrap();
        let entries = list(&tmp.path().join("nao-existe")).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn lista_arvore_completa_com_subpastas() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("PPSSPP/2025-07-01_10-30-00/saves");
        std::fs::create_dir_all(base.join("GAME01")).unwrap();
        std::fs::write(base.join("GAME01/SAVE.bin"), b"conteudo").unwrap();
        let conflito = tmp.path().join("PCSX2/conflito-2025-07-02_09-00-00/config");
        std::fs::create_dir_all(&conflito).unwrap();
        std::fs::write(conflito.join("inis.ini"), b"x").unwrap();

        let entries = list(tmp.path()).unwrap();

        assert_eq!(entries.len(), 2);
        let save = entries
            .iter()
            .find(|e| e.rel_path == "GAME01/SAVE.bin")
            .unwrap();
        assert_eq!(save.emulator, "PPSSPP");
        assert_eq!(save.run, "2025-07-01_10-30-00");
        assert_eq!(save.category, "saves");
        assert_eq!(save.size_bytes, 8);
        assert!(save.modified_at_ms > 0);

        let ini = entries.iter().find(|e| e.rel_path == "inis.ini").unwrap();
        assert_eq!(ini.emulator, "PCSX2");
        assert_eq!(ini.run, "conflito-2025-07-02_09-00-00");
        assert_eq!(ini.category, "config");
    }

    #[test]
    fn backup_entry_serializa_em_camel_case() {
        let entry = BackupEntry {
            emulator: "PPSSPP".into(),
            run: "2025-07-01_10-30-00".into(),
            category: "saves".into(),
            rel_path: "GAME01/SAVE.bin".into(),
            size_bytes: 8,
            modified_at_ms: 1_700_000_000_000,
            abs_path: "C:/backups/...".into(),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["emulator"], "PPSSPP");
        assert_eq!(json["run"], "2025-07-01_10-30-00");
        assert_eq!(json["relPath"], "GAME01/SAVE.bin");
        assert_eq!(json["sizeBytes"], 8);
        assert_eq!(json["modifiedAtMs"], 1_700_000_000_000i64);
        assert_eq!(json["absPath"], "C:/backups/...");
    }
}
