//! Cliente da API do Google Drive v3 via `reqwest` (Passos 3 e 5).
//!
//! Responsabilidades:
//! - Chamadas HTTP com retry exponencial + jitter (máx. `DRIVE_MAX_RETRIES`);
//! - Upload (simples até 5 MB, resumable acima), download e listagem;
//! - Criação idempotente da estrutura `RetroSync/<Emulador>/{saves,savestates,config}`;
//! - Concorrência limitada por semáforo (`DRIVE_MAX_CONCURRENT_TRANSFERS`).
//!
//! Escopo OAuth: `drive.file` — o app só enxerga o que ele mesmo criou.
