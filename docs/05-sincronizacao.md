# 05 — Módulo de Sincronização

**Commit**: `f3639fc` — *feat: SyncEngine com manifest SQLite, cliente Drive com retry e
fila offline*

É o coração do app. Reúne três camadas — `storage` (SQLite), `drive` (cliente da API) e
`sync` (engine + diff + conflito) — e a integração no `state`/`commands`/`lib.rs`.

## Camada `storage` (SQLite via `rusqlite`)

`rusqlite` é síncrono: a conexão única vive atrás de `Arc<Mutex<Connection>>` e todo
acesso async passa por `Db::with`, que executa em `spawn_blocking`. WAL habilitado.

| Arquivo | Conteúdo |
| --- | --- |
| `storage/db.rs` | Conexão, migrações (`user_version`), `Db::with` |
| `storage/manifest.rs` | Tabela `sync_manifest` — estado de cada arquivo no último sync |
| `storage/queue.rs` | Tabela `pending_ops` — fila offline |
| `storage/emulators.rs` | Tabela `emulators` — perfis configurados |

### Schema (migração v1)

```sql
sync_manifest(emulator, category, rel_path, drive_file_id,
              local_mtime_ms, drive_mtime_ms, size_bytes, last_synced_at_ms,
              PRIMARY KEY (emulator, category, rel_path))

pending_ops(id, emulator, category, rel_path, direction,
            enqueued_at_ms, attempts, last_error,
            UNIQUE (emulator, category, rel_path, direction))

emulators(name PRIMARY KEY, root_path, profile_json, added_at_ms)
```

## Camada `drive` (Google Drive API v3)

| Arquivo | Conteúdo |
| --- | --- |
| `drive/client.rs` | `DriveClient` + `send_with_retry` (transporte com retry) |
| `drive/folders.rs` | Criação idempotente de pastas, com cache de IDs |
| `drive/files.rs` | Listagem recursiva, download, upload multipart/resumable |

### Política de retry (`send_with_retry`)

Toda chamada à API passa por aqui:

- máx. `DRIVE_MAX_RETRIES` = 3 tentativas;
- backoff exponencial 500ms / 1s / 2s + jitter de até 250ms;
- **401** → renova o access token e tenta de novo;
- **429**, **403 RateLimitExceeded**, **5xx** → aguarda e retenta;
- falha de rede → aguarda e retenta;
- a closure `build` reconstrói o request a cada tentativa (com token fresco).

### Pastas idempotentes

`ensure_root` → `ensure_category_folder` → `ensure_subpath` criam (ou encontram) a árvore
`RetroSync/<Emulador>/<categoria>/<subpastas>`, com **cache de IDs** em memória por caminho
lógico para evitar buscas repetidas. Nunca cria duplicatas: busca por nome+parent antes de
criar.

### Upload e download

- **Upload** preserva o mtime original em `modifiedTime`. Multipart/related montado à mão
  (o `multipart` do reqwest é form-data, que o Drive não aceita) até 5 MB; acima disso,
  **sessão resumable** (sobrevive a quedas de conexão).
- **Download** grava em arquivo temporário (`.retrosync-tmp`) + `rename` atômico, depois
  aplica o `modifiedTime` do Drive como mtime local (crate `filetime`). Um save nunca fica
  corrompido por queda no meio da escrita.
- **Nenhum método de delete existe** — a v1.0 é não-destrutiva por construção.

## Camada `sync`

| Arquivo | Conteúdo |
| --- | --- |
| `sync/mod.rs` | `SyncDirection`, `SyncCategory`, `SyncProgress`, `SyncTarget` |
| `sync/conflict.rs` | `decide()` — resolução de conflito por timestamp |
| `sync/diff.rs` | Scan local + `build_plan()` — une os 3 estados |
| `sync/engine.rs` | `SyncEngine` — orquestração async |

### Agnosticismo: `SyncTarget`

O engine só enxerga `SyncTarget { label, root, categories: Vec<(SyncCategory, Vec<PathBuf>)> }`.
A conversão `EmulatorProfile → SyncTarget` é pura função de dados. Nenhuma menção a PPSSPP
ou PCSX2 dentro de `sync/`.

### Resolução de conflito (`conflict::decide`)

Entrada: mtime local, `modifiedTime` do Drive e o par `(local, drive)` registrado no
manifest no último sync. Regras:

| Situação | Ação |
| --- | --- |
| Só existe local | Upload |
| Só existe no Drive | Download |
| Não existe em nenhum | NoOp |
| Inalterado desde o último sync (ambos batem com o manifest, ±2s) | NoOp |
| Diferença ≤ 2s | NoOp |
| Local mais recente | Upload |
| Drive mais recente | Download |

A **tolerância de ±2s** absorve granularidade de filesystem e pequenos desvios de relógio.
O **par de mtimes do manifest** permite reconhecer "nada mudou" mesmo quando os relógios
local e remoto divergem além da tolerância — sem isso, qualquer skew causaria re-sync
eterno. Conflito real (mudou dos dois lados) → vence o mais recente. Ver
[Decisões técnicas](./decisoes-tecnicas.md#resolução-de-conflito-por-timestamp).

### Montagem do plano (`diff::build_plan`)

1. Scan local (`scan_local_bases`): varre as pastas-base, ignora symlinks e arquivos
   `.retrosync-tmp`, normaliza caminhos com separador `/`. Pasta inexistente é pulada sem
   erro.
2. União de todos os `rel_path` (local ∪ Drive).
3. Para cada um, `conflict::decide()`.
4. Filtra pela `SyncDirection`: `DriveToLocal` descarta uploads, `LocalToDrive` descarta
   downloads, `Bidirectional` mantém ambos. NoOps viram contagem `skipped`.

### Execução (`SyncEngine`)

- **Um sync por vez**: `Mutex<()>` serializa execuções concorrentes.
- **Verifica conexão** antes de tudo; se desconectado, aborta com erro claro.
- Por categoria: garante pastas → lista árvore remota → scan local → carrega manifest →
  monta plano → executa com **≤3 transferências simultâneas** (`buffer_unordered`).
- Cada operação: transfere → atualiza manifest → emite `sync:progress` → em falha de
  rede/arquivo em uso, enfileira pendência.
- Ao fim: publica `sync_manifest.json` no Drive (best-effort) e emite `sync:completed`
  com o `SyncSummary`.

### Verificação de arquivo estável

Antes de um upload, o engine lê o mtime, lê o conteúdo, relê o mtime; se mudou no meio, o
arquivo está sendo gravado (emulador ainda salvando) → erro `FileBusy` → vai para a fila e
é retentado no próximo sync.

### Fila offline — registro de intenção, não de replay

Falha de rede ou `FileBusy` → pendência persistida em `pending_ops` (com dedupe e contagem
de tentativas). O **próximo sync re-detecta a diferença pelo diff** (a fonte da verdade) e
refaz a operação; ao concluir o arquivo, `resolve` limpa a pendência. Mais simples e imune
a replay de operação obsoleta. Ver
[Decisões técnicas](./decisoes-tecnicas.md#fila-offline-como-registro-de-intenção).

## Comandos expostos

| Comando | Assinatura | Descrição |
| --- | --- | --- |
| `add_emulator` | `(path) -> EmulatorProfile` | Detecta e persiste o emulador |
| `list_emulators` | `() -> Vec<EmulatorProfile>` | Emuladores configurados |
| `remove_emulator` | `(name) -> ()` | Remove da sync (manifest + fila); não toca Drive/disco |
| `sync_now` | `() -> SyncSummary` | Sync manual bidirecional |

## Eventos emitidos

`sync:started`, `sync:progress`, `sync:completed`, `sync:error`. Ver
[Referência — Boundary IPC](./referencia-ipc.md#eventos).

## Gatilho de startup ativo

No `lib.rs` `setup`, após construir o `AppState`, um sync bidirecional roda em background
(`TRIGGER_STARTUP`). Os gatilhos de fechamento (Passo 7) e de processos (Passo 6) usarão
os mesmos métodos do engine.

## Testes (31 novos, 44 no total)

- `conflict`: 11 testes cobrindo todas as regras, incluindo skew de relógio e conflito real;
- `diff`: 8 testes (planos por direção, scan ignorando temporários/symlinks, base inexistente);
- `manifest`: 6 testes (roundtrip, replace, filtros, remoção, camelCase);
- `queue`: 3 testes (dedupe, resolve, remoção por emulador);
- `emulators`: 3 testes (roundtrip, replace, remoção).

## Como testar manualmente

Com o Drive conectado (`npm run tauri dev`, console F12):

```js
const { invoke } = window.__TAURI__.core;
await invoke("add_emulator", { path: "C:\\Users\\User\\Documents\\PPSSPP" });
await invoke("sync_now");
// → { uploaded: N, downloaded: 0, skipped: 0, failed: 0, queued: 0, durationMs: ... }
```

Verifique no Drive: `RetroSync/PPSSPP/{saves,savestates,config}` + `sync_manifest.json`.
Rodar `sync_now` de novo: tudo em `skipped`. Apagar um save local e rodar de novo: volta
do Drive com o mtime original.
