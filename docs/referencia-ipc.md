# Referência — Boundary IPC (Rust ↔ TypeScript)

Catálogo do contrato entre backend e frontend. **Fonte de verdade**: as structs Rust
(serde, `rename_all = "camelCase"`). O espelho TypeScript fica em `src/types/ipc.ts`;
os wrappers tipados de `invoke`, em `src/lib/ipc.ts`.

> Regra: toda struct que cruza a boundary deriva `Serialize`/`Deserialize` e tem interface
> TS espelhada. Mudou um lado, atualize o outro.

## Comandos (`invoke`)

| Comando | Parâmetros | Retorno | Passo |
| --- | --- | --- | --- |
| `health_check` | — | `HealthStatus` | 2 |
| `connect_google_drive` | — | `AuthStatus` | 3 |
| `get_auth_status` | — | `AuthStatus` | 3 |
| `disconnect_google_drive` | — | `AuthStatus` | 3 |
| `detect_emulator` | `path: string` | `EmulatorProfile \| null` | 4 |
| `add_emulator` | `path: string` | `EmulatorProfile` | 5 |
| `add_emulator_manual` | `name, path, savesPaths, statePaths, configPaths` | `EmulatorProfile` | FEAT-003 |
| `discover_emulators` | — | `DiscoveredEmulator[]` | FEAT-003 |
| `pick_emulator_folder` | — | `string` (URI SAF) | Android |
| `list_emulators` | — | `EmulatorProfile[]` | 5 |
| `list_synced_games` | — | `SyncedGame[]` | FEAT-001 |
| `remove_emulator` | `name: string` | `void` | 5 |
| `sync_now` | — | `SyncSummary` | 5 |
| `get_last_sync` | — | `LastSync \| null` | 7 |
| `get_settings` | — | `Settings` | v1.1·1 |
| `set_device_name` | `name: string` | `void` | v1.1·1 |
| `set_triggers` | `triggers: TriggerSettings` | `void` | v1.1·4 |
| `set_notification_level` | `level: NotificationLevel` | `void` | v1.1·5 |
| `set_autostart` | `enabled: boolean` | `void` | v1.1·8 |
| `open_backup_folder` | — | `void` | v1.1·6 |
| `get_emulator_categories` | `name: string` | `SyncCategories` | v1.1·3 |
| `set_emulator_categories` | `name: string, categories: SyncCategories` | `void` | v1.1·3 |
| `list_conflicts` | — | `Conflict[]` | v1.1·7 |
| `resolve_conflict` | `emulator, category, relPath, keep` | `void` | v1.1·7 |

Os wrappers correspondentes vivem em `src/lib/ipc.ts` (`healthCheck`, `connectGoogleDrive`,
`getAuthStatus`, `disconnectGoogleDrive`, `detectEmulator`, `addEmulator`, `listEmulators`,
`removeEmulator`, `syncNow`, `getLastSync`, `getSettings`, `setDeviceName`).

## Eventos (`emit` → `listen`)

| Evento | Constante (`EVT`) | Payload | Quando |
| --- | --- | --- | --- |
| `auth:status` | `AUTH_STATUS` | `AuthStatus` | Conexão/desconexão |
| `sync:started` | `SYNC_STARTED` | `SyncStarted` | Início de um sync |
| `sync:progress` | `SYNC_PROGRESS` | `SyncProgress` | A cada arquivo processado |
| `sync:completed` | `SYNC_COMPLETED` | `SyncSummary` | Fim de um sync |
| `sync:error` | `SYNC_ERROR` | `SyncErrorEvent` | Falha de um emulador no sync |
| `sync:conflict` | `SYNC_CONFLICT` | `Conflict` | Conflito detectado (ambos os lados mudaram) |
| `emulator:status` | `EMULATOR_STATUS` | `EmulatorStatusEvent` | Abertura/fechamento de emulador |

## Tipos

### `HealthStatus`
```ts
{ version: string; ready: boolean }
```

### `AuthStatus`
```ts
{ connected: boolean; email: string | null }
```

### `Settings`
```ts
{ deviceName: string | null; triggers: TriggerSettings; notificationLevel: NotificationLevel; autostart: boolean }
```
Configurações globais do usuário (`storage::settings::Settings`). Persistidas na tabela
`app_settings` (chave→valor); cresce ao longo dos passos da v1.1. Exceção: `autostart` não
fica no banco — o estado vive no SO (registro do Windows / LaunchAgent) e é lido pelo plugin
de autostart no `get_settings`; escrita via `set_autostart`.

### `TriggerSettings`
```ts
{ startup: boolean; emulatorStart: boolean; emulatorStop: boolean }
```
Gatilhos de sync automático (default todos `true`). O sync manual não é afetado por estes flags.

### `NotificationLevel`
```ts
"all" | "errors_only" | "none"
```
Nível de notificações nativas (default `all`). `all` notifica sync concluído, erros e emulador
detectado; `errors_only` só erros; `none` nada.

### `EmulatorProfile`
```ts
{ name: string; rootPath: string; savesPaths: string[];
  configPaths: string[]; statePaths: string[] }
```
> `PathBuf` do Rust serializa como `string`.

### `SyncCategories`
```ts
{ saves: boolean; savestates: boolean; config: boolean }
```
Categorias de sync habilitadas por emulador (`storage::emulators::SyncCategories`). Persistidas em
`emulator_settings`; default (todas `true`) aplicado na leitura quando não há linha.

### `SyncDirection`
```ts
"DriveToLocal" | "LocalToDrive" | "Bidirectional"
```

### `SyncProgress`
```ts
{ emulator: string; currentFile: string; completed: number;
  total: number; direction: SyncDirection }
```

### `SyncSummary`
```ts
{ uploaded: number; downloaded: number; skipped: number;
  failed: number; queued: number; backedUp: number; conflicts: number; durationMs: number }
```
Retorno de `sync_now` e payload de `sync:completed`. `backedUp` > 0 = arquivos locais salvos em
backup antes de sobrescritos no primeiro sync (BUG-001); `conflicts` > 0 = conflitos detectados.

### `Conflict`
```ts
{ emulator: string; category: "saves" | "savestates" | "config"; relPath: string;
  localMtimeMs: number; localSize: number; localDevice: string | null;
  driveMtimeMs: number; driveSize: number; driveDevice: string | null;
  driveFileId: string; localAbsPath: string; detectedAtMs: number }
```
Conflito pendente (`storage::conflicts::Conflict`). Payload de `sync:conflict` e item de
`list_conflicts`. Enquanto houver conflito para um emulador, o sync dele fica bloqueado.

### `ConflictResolution`
```ts
"local" | "drive"
```
Versão a manter ao chamar `resolve_conflict`.

### `SyncStarted`
```ts
{ trigger: string; direction: SyncDirection }
```
`trigger` ∈ `startup` | `shutdown` | `manual` | `emulator-start` | `emulator-stop`.

### `LastSync`
```ts
{ atMs: number; trigger: string; summary: SyncSummary }
```
Retorno de `get_last_sync`. Efêmero por execução: o engine grava ao concluir cada sync
(antes de emitir `sync:completed`); `null` até o primeiro sync da sessão.

### `SyncErrorEvent`
```ts
{ emulator: string | null; message: string }
```

### `EmulatorStatusEvent`
```ts
{ emulator: string; running: boolean }
```
Payload de `emulator:status`, emitido pelo process watcher na abertura (`running: true`)
e no fechamento (`running: false`) de um emulador configurado.

### `AppErrorPayload`
Todo comando que rejeita devolve este shape (de `error::AppError`):
```ts
{ code: "io" | "database" | "network" | "keyring" | "serialization"
       | "auth" | "emulator_not_detected" | "emulator_exists" | "file_busy"
       | "drive_not_found" | "other";
  message: string;   // texto completo (prefixo + detalhe), em português — fallback
  detail: string }   // só o detalhe técnico (caminho, nome, msg da lib), sem prefixo
```
O frontend localiza o prefixo pelo `code` (ver `errors.<code>` no i18n) e anexa o `detail`;
`message` permanece como fallback para `code: "other"` (sem prefixo a traduzir). Por isso o
`code` é um enum fechado: trocá-lo no `error.rs` exige atualizar o union no `ipc.ts` **e** as
chaves `errors.*` nos locales.

## Manutenção do contrato

Hoje o espelhamento é **manual**, concentrado em dois arquivos (um de cada lado) para
minimizar drift, com testes de serialização no Rust (ex.: `perfil_serializa_em_camel_case`,
`entry_serializa_em_camel_case_para_o_snapshot`).

> **Nota de manutenção**: ao documentar (jun/2026) foi encontrado e corrigido um drift — o
> código de erro `file_busy`, adicionado ao Rust no Passo 5, faltava no `AppErrorPayload`
> do TS. Se o número de tipos crescer, considerar `ts-rs` (geração automática das interfaces
> TS a partir das structs Rust), já previsto na arquitetura.
