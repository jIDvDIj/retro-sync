//! Scan do estado local e montagem do plano de sincronização:
//! união (local ∪ Drive ∪ manifest) → `conflict::decide` por arquivo →
//! filtro pela direção do sync.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::conflict::{decide, SyncAction};
use super::SyncDirection;
use crate::constants::TMP_SUFFIX;
use crate::drive::{DriveFile, RemoteFile};
use crate::error::{AppError, AppResult};
use crate::storage::manifest::ManifestEntry;

#[derive(Debug, Clone)]
pub struct LocalFile {
    /// Relativo à pasta-base da categoria, sempre com separador `/`.
    pub rel_path: String,
    pub abs_path: PathBuf,
    pub mtime_ms: i64,
    #[allow(dead_code)]
    pub size_bytes: i64,
}

/// Operação planejada (apenas Upload/Download; NoOps são contados à parte).
#[derive(Debug, Clone)]
pub struct PlannedOp {
    pub rel_path: String,
    pub action: SyncAction,
    pub local: Option<LocalFile>,
    pub remote: Option<DriveFile>,
}

/// Varre as pastas-base de uma categoria (relativas a `root`). Em `rel_path`
/// duplicado entre bases, a primeira base vence. Ignora symlinks e arquivos
/// temporários do RetroSync. Pastas inexistentes são puladas sem erro.
pub fn scan_local_bases(root: &Path, bases: &[PathBuf]) -> AppResult<Vec<LocalFile>> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for base in bases {
        let base_abs = root.join(base);
        if base_abs.is_dir() {
            walk(&base_abs, &base_abs, &mut seen, &mut out)?;
        }
    }
    Ok(out)
}

fn walk(
    base: &Path,
    dir: &Path,
    seen: &mut HashSet<String>,
    out: &mut Vec<LocalFile>,
) -> AppResult<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            walk(base, &path, seen, out)?;
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(TMP_SUFFIX) {
            continue;
        }

        let rel = path
            .strip_prefix(base)
            .map_err(|e| AppError::Other(format!("caminho fora da base no scan: {e}")))?;
        let rel_path = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        if !seen.insert(rel_path.clone()) {
            continue;
        }

        let metadata = entry.metadata()?;
        out.push(LocalFile {
            rel_path,
            abs_path: path,
            mtime_ms: system_time_ms(metadata.modified()?),
            size_bytes: metadata.len() as i64,
        });
    }
    Ok(())
}

pub fn system_time_ms(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Monta o plano da categoria. Retorna as operações ativas e a contagem de
/// arquivos sem mudança (`skipped`).
pub fn build_plan(
    local: Vec<LocalFile>,
    remote: Vec<RemoteFile>,
    manifest: Vec<ManifestEntry>,
    direction: SyncDirection,
) -> (Vec<PlannedOp>, u32) {
    let local_map: HashMap<String, LocalFile> =
        local.into_iter().map(|f| (f.rel_path.clone(), f)).collect();
    let remote_map: HashMap<String, DriveFile> =
        remote.into_iter().map(|f| (f.rel_path, f.file)).collect();
    let manifest_map: HashMap<String, ManifestEntry> = manifest
        .into_iter()
        .map(|e| (e.rel_path.clone(), e))
        .collect();

    let all_paths: BTreeSet<String> = local_map.keys().chain(remote_map.keys()).cloned().collect();

    let mut ops = Vec::new();
    let mut skipped: u32 = 0;

    for rel_path in all_paths {
        let local_file = local_map.get(&rel_path);
        let remote_file = remote_map.get(&rel_path);
        let last_synced = manifest_map
            .get(&rel_path)
            .and_then(|e| e.local_mtime_ms.zip(e.drive_mtime_ms));

        let action = decide(
            local_file.map(|f| f.mtime_ms),
            remote_file.and_then(|f| f.modified_ms()),
            last_synced,
        );

        let allowed = match action {
            SyncAction::NoOp => false,
            SyncAction::Upload => direction != SyncDirection::DriveToLocal,
            SyncAction::Download | SyncAction::DownloadWithBackup => {
                direction != SyncDirection::LocalToDrive
            }
            // Conflito é registrado em qualquer direção — nunca queremos
            // sobrescrever silenciosamente, mesmo num sync de mão única.
            SyncAction::Conflict => true,
        };

        if allowed {
            ops.push(PlannedOp {
                rel_path,
                action,
                local: local_file.cloned(),
                remote: remote_file.cloned(),
            });
        } else {
            skipped += 1;
        }
    }

    (ops, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::SyncCategory;

    const T: i64 = 1_700_000_000_000;

    fn local_file(rel: &str, mtime: i64) -> LocalFile {
        LocalFile {
            rel_path: rel.to_string(),
            abs_path: PathBuf::from("/tmp").join(rel),
            mtime_ms: mtime,
            size_bytes: 100,
        }
    }

    fn remote_file(rel: &str, mtime: i64) -> RemoteFile {
        RemoteFile {
            rel_path: rel.to_string(),
            file: DriveFile {
                id: format!("id-{rel}"),
                name: rel.rsplit('/').next().unwrap().to_string(),
                mime_type: "application/octet-stream".into(),
                modified_time: chrono::DateTime::from_timestamp_millis(mtime),
                size: Some("100".into()),
                app_properties: std::collections::HashMap::new(),
            },
        }
    }

    fn manifest_entry(rel: &str, local: i64, drive: i64) -> ManifestEntry {
        ManifestEntry {
            emulator: "PPSSPP".into(),
            category: SyncCategory::Saves,
            rel_path: rel.to_string(),
            drive_file_id: Some(format!("id-{rel}")),
            local_mtime_ms: Some(local),
            drive_mtime_ms: Some(drive),
            size_bytes: Some(100),
            last_synced_at_ms: T,
        }
    }

    #[test]
    fn arquivo_novo_local_vira_upload() {
        let (ops, skipped) = build_plan(
            vec![local_file("novo.bin", T)],
            vec![],
            vec![],
            SyncDirection::Bidirectional,
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].action, SyncAction::Upload);
        assert_eq!(skipped, 0);
    }

    #[test]
    fn arquivo_novo_no_drive_vira_download() {
        let (ops, _) = build_plan(
            vec![],
            vec![remote_file("remoto.bin", T)],
            vec![],
            SyncDirection::Bidirectional,
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].action, SyncAction::Download);
        assert!(ops[0].remote.is_some());
    }

    #[test]
    fn arquivo_sem_mudanca_e_pulado() {
        let (ops, skipped) = build_plan(
            vec![local_file("igual.bin", T)],
            vec![remote_file("igual.bin", T)],
            vec![manifest_entry("igual.bin", T, T)],
            SyncDirection::Bidirectional,
        );
        assert!(ops.is_empty());
        assert_eq!(skipped, 1);
    }

    #[test]
    fn local_mais_recente_vira_upload_com_remote_id() {
        let (ops, _) = build_plan(
            vec![local_file("save.bin", T + 60_000)],
            vec![remote_file("save.bin", T)],
            vec![manifest_entry("save.bin", T, T)],
            SyncDirection::Bidirectional,
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].action, SyncAction::Upload);
        assert_eq!(ops[0].remote.as_ref().unwrap().id, "id-save.bin");
    }

    #[test]
    fn primeiro_sync_com_arquivo_nos_dois_lados_baixa_com_backup() {
        // Sem manifest e arquivo presente local e no Drive → DownloadWithBackup.
        let (ops, skipped) = build_plan(
            vec![local_file("save.bin", T + 60_000)],
            vec![remote_file("save.bin", T)],
            vec![],
            SyncDirection::Bidirectional,
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].action, SyncAction::DownloadWithBackup);
        assert!(ops[0].local.is_some());
        assert!(ops[0].remote.is_some());
        assert_eq!(skipped, 0);
    }

    #[test]
    fn ambos_mudaram_desde_o_ultimo_sync_vira_conflito() {
        // local e drive divergem de (T, T) registrado → Conflict, com os dois
        // lados disponíveis para a UI.
        let (ops, _) = build_plan(
            vec![local_file("save.bin", T + 300_000)],
            vec![remote_file("save.bin", T + 60_000)],
            vec![manifest_entry("save.bin", T, T)],
            SyncDirection::Bidirectional,
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].action, SyncAction::Conflict);
        assert!(ops[0].local.is_some());
        assert!(ops[0].remote.is_some());
    }

    #[test]
    fn direcao_drive_to_local_descarta_uploads() {
        let (ops, skipped) = build_plan(
            vec![local_file("novo.bin", T)],
            vec![remote_file("remoto.bin", T)],
            vec![],
            SyncDirection::DriveToLocal,
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].action, SyncAction::Download);
        assert_eq!(skipped, 1);
    }

    #[test]
    fn direcao_local_to_drive_descarta_downloads() {
        let (ops, skipped) = build_plan(
            vec![local_file("novo.bin", T)],
            vec![remote_file("remoto.bin", T)],
            vec![],
            SyncDirection::LocalToDrive,
        );
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].action, SyncAction::Upload);
        assert_eq!(skipped, 1);
    }

    #[test]
    fn scan_ignora_temporarios_e_entra_em_subpastas() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("SAVEDATA");
        std::fs::create_dir_all(base.join("GAME01")).unwrap();
        std::fs::write(base.join("GAME01/SAVE.bin"), b"abc").unwrap();
        std::fs::write(base.join("topo.txt"), b"x").unwrap();
        std::fs::write(base.join(format!("baixando{TMP_SUFFIX}")), b"parcial").unwrap();

        let files = scan_local_bases(tmp.path(), &[PathBuf::from("SAVEDATA")]).unwrap();

        let mut rels: Vec<_> = files.iter().map(|f| f.rel_path.as_str()).collect();
        rels.sort();
        assert_eq!(rels, vec!["GAME01/SAVE.bin", "topo.txt"]);
    }

    #[test]
    fn scan_de_base_inexistente_retorna_vazio() {
        let tmp = tempfile::tempdir().unwrap();
        let files = scan_local_bases(tmp.path(), &[PathBuf::from("NAO_EXISTE")]).unwrap();
        assert!(files.is_empty());
    }
}
