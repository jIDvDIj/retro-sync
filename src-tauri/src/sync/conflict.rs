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
        (Some(local), Some(drive)) => {
            if let Some((last_local, last_drive)) = last_synced {
                let unchanged = eq_within_tolerance(local, last_local)
                    && eq_within_tolerance(drive, last_drive);
                if unchanged {
                    return SyncAction::NoOp;
                }
            }
            if eq_within_tolerance(local, drive) {
                SyncAction::NoOp
            } else if local > drive {
                SyncAction::Upload
            } else {
                SyncAction::Download
            }
        }
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
    fn local_mais_recente_sobe() {
        assert_eq!(decide(Some(T + 60_000), Some(T), None), SyncAction::Upload);
    }

    #[test]
    fn drive_mais_recente_desce() {
        assert_eq!(
            decide(Some(T), Some(T + 60_000), None),
            SyncAction::Download
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
