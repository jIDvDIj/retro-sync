//! Resolução de conflito por timestamp: o lado mais recente vence; nunca há
//! deleção (regra da v1.0). Tolerância de ±2s absorve granularidade de
//! filesystem e pequenos desvios de relógio; o par de mtimes registrado no
//! manifest no último sync permite reconhecer "nada mudou" mesmo quando os
//! relógios local e remoto divergem além da tolerância.

/// Diferenças de timestamp até este valor são tratadas como "iguais".
pub const TIMESTAMP_TOLERANCE_MS: i64 = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncAction {
    Upload,
    Download,
    /// Download em que o arquivo local existente é copiado para uma pasta de
    /// backup antes de ser sobrescrito. Usado no primeiro sync de um arquivo
    /// que existe nos dois lados (Drive vence — BUG-001).
    DownloadWithBackup,
    NoOp,
}

/// Decide a ação para um arquivo dado seu mtime local, o `modifiedTime` no
/// Drive e o par `(local, drive)` registrado no manifest no último sync.
pub fn decide(
    local_mtime_ms: Option<i64>,
    drive_mtime_ms: Option<i64>,
    last_synced: Option<(i64, i64)>,
) -> SyncAction {
    match (local_mtime_ms, drive_mtime_ms) {
        (None, None) => SyncAction::NoOp,
        (Some(_), None) => SyncAction::Upload,
        (None, Some(_)) => SyncAction::Download,
        (Some(local), Some(drive)) => match last_synced {
            // Já sincronizado antes: resolução normal por mtime. O conflito real
            // (ambos mudaram desde o último sync) é tratado no Passo 7.
            Some((last_local, last_drive)) => {
                let unchanged = eq_within_tolerance(local, last_local)
                    && eq_within_tolerance(drive, last_drive);
                if unchanged || eq_within_tolerance(local, drive) {
                    SyncAction::NoOp
                } else if local > drive {
                    SyncAction::Upload
                } else {
                    SyncAction::Download
                }
            }
            // Primeiro sync deste arquivo (sem manifest) e ele existe nos dois
            // lados: o Drive sempre vence, com backup local antes de sobrescrever
            // (BUG-001). mtime igual = nada a fazer.
            None => {
                if eq_within_tolerance(local, drive) {
                    SyncAction::NoOp
                } else {
                    SyncAction::DownloadWithBackup
                }
            }
        },
    }
}

fn eq_within_tolerance(a: i64, b: i64) -> bool {
    (a - b).abs() <= TIMESTAMP_TOLERANCE_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: i64 = 1_700_000_000_000;

    #[test]
    fn arquivo_so_local_sobe() {
        assert_eq!(decide(Some(T), None, None), SyncAction::Upload);
    }

    #[test]
    fn arquivo_so_no_drive_desce() {
        assert_eq!(decide(None, Some(T), None), SyncAction::Download);
    }

    #[test]
    fn inexistente_dos_dois_lados_e_noop() {
        assert_eq!(decide(None, None, None), SyncAction::NoOp);
    }

    #[test]
    fn timestamps_iguais_sao_noop() {
        assert_eq!(decide(Some(T), Some(T), None), SyncAction::NoOp);
    }

    #[test]
    fn diferenca_dentro_da_tolerancia_e_noop() {
        assert_eq!(
            decide(Some(T + TIMESTAMP_TOLERANCE_MS), Some(T), None),
            SyncAction::NoOp
        );
        assert_eq!(
            decide(Some(T), Some(T + TIMESTAMP_TOLERANCE_MS), None),
            SyncAction::NoOp
        );
    }

    #[test]
    fn primeiro_sync_drive_vence_mesmo_com_local_mais_recente() {
        // Sem manifest e ambos existem: Drive vence (com backup), mesmo que o
        // mtime local seja mais novo — BUG-001.
        assert_eq!(
            decide(Some(T + 60_000), Some(T), None),
            SyncAction::DownloadWithBackup
        );
    }

    #[test]
    fn primeiro_sync_drive_vence_com_drive_mais_recente() {
        assert_eq!(
            decide(Some(T), Some(T + 60_000), None),
            SyncAction::DownloadWithBackup
        );
    }

    #[test]
    fn sem_mudanca_desde_o_ultimo_sync_e_noop_mesmo_com_relogio_divergente() {
        // Local e Drive diferem em 1 min (skew), mas ambos estão idênticos ao
        // que o manifest registrou — nada a fazer.
        let local = T;
        let drive = T + 60_000;
        assert_eq!(
            decide(Some(local), Some(drive), Some((local, drive))),
            SyncAction::NoOp
        );
    }

    #[test]
    fn mudanca_local_desde_o_ultimo_sync_sobe() {
        let drive = T;
        let novo_local = T + 120_000;
        assert_eq!(
            decide(Some(novo_local), Some(drive), Some((T, drive))),
            SyncAction::Upload
        );
    }

    #[test]
    fn mudanca_no_drive_desde_o_ultimo_sync_desce() {
        let local = T;
        let novo_drive = T + 120_000;
        assert_eq!(
            decide(Some(local), Some(novo_drive), Some((local, T))),
            SyncAction::Download
        );
    }

    #[test]
    fn conflito_real_vence_o_mais_recente() {
        // Mudou dos dois lados desde o último sync: timestamp decide.
        let last = (T, T);
        assert_eq!(
            decide(Some(T + 300_000), Some(T + 60_000), Some(last)),
            SyncAction::Upload
        );
        assert_eq!(
            decide(Some(T + 60_000), Some(T + 300_000), Some(last)),
            SyncAction::Download
        );
    }
}
