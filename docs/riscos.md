# Riscos Técnicos e Mitigações

Riscos identificados na fase de arquitetura (Passo 1) e como cada um está sendo tratado.
A coluna **Status** indica se a mitigação já está no código ou planejada.

| # | Risco | Mitigação | Status |
| --- | --- | --- | --- |
| 1 | Clock skew na resolução de conflito | UTC + tolerância ±2s + par de mtimes do manifest | ✅ Passo 5 |
| 10 | Saves independentes de dispositivos diferentes sobrescritos no primeiro sync | `device_id` estável (keyring) estampado no Drive; conflito explícito quando a origem é outro dispositivo | ✅ BUG-004 |
| 2 | Rate limits do Drive (403/429) | Retry exponencial + jitter; concorrência ≤3; diff evita transferência desnecessária | ✅ Passo 5 |
| 3 | Arquivo em uso durante o sync | Checagem de mtime estável antes do upload; `FileBusy` → fila | ✅ Passo 5 |
| 4 | Detecção de processos frágil | Lista de nomes por perfil (constantes); matching case-insensitive; debounce | ✅ Passo 6 |
| 5 | Drift na boundary Rust↔TS | Tipos concentrados em 1 arquivo/lado; testes de serialização; tagged unions | ✅ contínuo |
| 6 | Keyring no Linux (Secret Service) | Abstração de token storage; fallback futuro sem tocar no resto | ✅ estrutural |
| 7 | Uploads grandes (savestates > 50MB) | Resumable upload acima de 5MB | ✅ Passo 5 |
| 8 | Offline-first | Falha de rede → pendência persistida; retry no próximo gatilho | ✅ Passo 5 |
| 9 | Ambiente de dev WSL2 sobre `/mnt/c` | Rodar `tauri dev`/`build` no Windows nativo | ✅ documentado |

## Detalhamento

### 1. Clock skew na resolução de conflitos
`mtime` local e `modifiedTime` do Drive vêm de relógios diferentes; entre duas máquinas o
problema dobra. Tudo é comparado em UTC (epoch ms), com tolerância de ±2s. O par
`(local, drive)` do último sync, gravado no manifest, permite reconhecer "nada mudou" mesmo
com skew maior que a tolerância. Como a v1.0 nunca deleta, o pior caso é sobrescrita de um
save antigo no lado perdedor — recuperável pelo histórico de revisões do Drive.

### 2. Rate limits da API do Drive
`send_with_retry` aplica backoff exponencial (500ms/1s/2s) com jitter, máx. 3 tentativas,
tratando 429, 403 *RateLimitExceeded* e 5xx. A concorrência de transferências é limitada a
3 simultâneas. O diff pelo manifest local evita listar/baixar o que não mudou.

### 3. Arquivo em uso no momento do sync
Ao fechar o emulador, ele pode ainda estar gravando o save (ou um savestate grande). Antes
do upload, o engine verifica estabilidade (mtime antes/depois da leitura); se mudou, é
`FileBusy` → entra na fila e é retentado.

### 4. Detecção de processos frágil
Nomes variam (`PPSSPPWindows64.exe`, `pcsx2-qt.exe`, …) e emuladores spawnam processos
auxiliares. Mitigado no Passo 6: lista de nomes por perfil (constantes), matching por
igualdade case-insensitive (não `contains`, que daria falso positivo), debounce no watcher
(só emite `Stopped` após `WATCHER_STOP_DEBOUNCE_TICKS` = 2 ticks sem o processo) e
`sysinfo` com `ProcessRefreshKind::nothing()` para manter o loop de 2s barato. Ver
[06 — Monitoramento de processos](./06-monitoramento-processos.md).

### 5. Boundary frontend↔Rust com tipos complexos
Drift silencioso quebra em runtime. Tipos concentrados em `src/types/ipc.ts` e structs com
serde camelCase; enums como tagged unions; testes de serialização no Rust. (Um drift real —
`file_busy` ausente no TS — foi pego e corrigido ao escrever esta documentação.) Evolução
possível: `ts-rs`.

### 6. Keyring no Linux
`keyring` depende do Secret Service (GNOME Keyring/KWallet), ausente em setups minimalistas.
A camada de token storage isola o keyring, permitindo fallback futuro sem alterar o resto.
O `device_id` (BUG-004) também mora no keyring e degrada para `None` quando indisponível, sem
abortar o sync.

### 7. Uploads grandes
Savestates de PCSX2 podem passar de 50MB. Upload simples (multipart) até 5MB; acima disso,
sessão resumable do Drive, que sobrevive a quedas de conexão.

### 8. Offline-first
Não há API confiável de "estou online". Tratamos falha de rede como sinal: operações que
falham por conectividade entram na fila SQLite; um retry oportunístico roda no próximo
gatilho de sync. A barra de status da UI exibe a contagem de pendentes no resumo do sync
(`SyncSummary.queued`), em vez de tratar como erro fatal (Passo 7).

### 9. Ambiente de dev (WSL2)
O repositório está em `/mnt/c` sob WSL, mas o alvo é Windows. Build do Tauri no WSL gera
binário Linux e exige `webkit2gtk`; compilar Rust em `/mnt/c` é lento (I/O 9p).
Desenvolver/rodar no Windows nativo (PowerShell). **Atenção adicional**: o `node_modules`
em `/mnt/c` é compartilhado entre Windows e WSL, e cada `npm install` de um lado remove os
binários nativos do outro (ex.: rollup). Se necessário no WSL:
`npm install --no-save @rollup/rollup-linux-x64-gnu`.

### 10. Saves independentes de dispositivos diferentes no primeiro sync
A resolução por mtime + manifest (risco #1) cobre conflitos a partir do **segundo** sync de um
arquivo. No **primeiro** sync (sem manifest) com o arquivo presente local e no Drive, a regra
conservadora era *Drive-vence-com-backup* ([BUG-001](./bugs/bug-001-perda-save-primeiro-sync.md)) —
mas isso decide automaticamente um caso ambíguo quando os dois saves vêm de **máquinas diferentes**.
Mitigado no [BUG-004](./bugs/bug-004-conflito-entre-dispositivos-primeiro-sync.md): cada dispositivo
tem um `device_id` estável (UUID no keyring, chave `retrosync_device_id`) estampado em
`appProperties.deviceId` nos uploads; quando a versão do Drive foi publicada por outro dispositivo,
o primeiro sync vira `Conflict` (o usuário escolhe) em vez de sobrescrever. Origem desconhecida ou
mesmo dispositivo mantêm o comportamento anterior — degradação graciosa.
