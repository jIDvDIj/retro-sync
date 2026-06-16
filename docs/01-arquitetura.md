# 01 — Arquitetura

## Visão geral

RetroSync é um app Tauri v2 com dois lados bem separados:

- **Frontend** (`src/`): React + TypeScript + Vite. Responsável apenas por apresentação
  e interação. Dispara comandos via `invoke()` e reage a eventos via `listen()`.
- **Backend/Core** (`src-tauri/`): Rust. Concentra 100% da lógica de negócio —
  autenticação, sincronização, monitoramento de processos, persistência.

A comunicação acontece exclusivamente pela **boundary do Tauri**: comandos
(`#[tauri::command]` ↔ `invoke`) e eventos (`emit` ↔ `listen`).

## Diagrama de componentes

```
┌─────────────────────────── RetroSync (Tauri v2) ────────────────────────────┐
│                                                                              │
│  ┌─────────── Frontend (React + TS + Vite) ───────────┐                      │
│  │                                                     │                      │
│  │  App.tsx ─ EmulatorCard ─ SyncStatus ─ ConnectDrive │                      │
│  │       │                          ▲                  │                      │
│  │  lib/ipc.ts (invoke tipado)      │ hooks/useSyncEvents (listen)            │
│  └───────┼──────────────────────────┼──────────────────┘                      │
│          │ invoke()                 │ emit()                                  │
│  ════════▼══════════ BOUNDARY TAURI ┴══════════════════════════════          │
│          │                          │                                         │
│  ┌───────▼──────────┐      ┌────────┴────────┐                                │
│  │   commands.rs    │      │    events.rs    │   (src-tauri)                  │
│  │ connect_drive    │      │ sync:progress   │                                │
│  │ detect_emulator  │      │ sync:completed  │                                │
│  │ add_emulator     │      │ sync:error      │                                │
│  │ sync_now         │      │ auth:status     │                                │
│  └───────┬──────────┘      └────────▲────────┘                                │
│          │                          │                                         │
│  ┌───────▼──────────────────────────┴──────────────────────────────┐          │
│  │                      AppState (state.rs)                        │          │
│  │                                                                 │          │
│  │   ┌────────┐   ┌─────────────────────┐   ┌──────────────────┐   │          │
│  │   │  auth  │──▶│     SyncEngine      │◀──│     watcher      │   │          │
│  │   │ OAuth2 │   │       (sync)        │   │ sysinfo, loop 2s │   │          │
│  │   │  PKCE  │   │ diff · conflitos ·  │   │  (Passo 6)       │   │          │
│  │   │keyring │   │ fila offline        │   │ mpsc: Started/   │   │          │
│  │   └────────┘   └──────┬──────┬───────┘   │       Stopped    │   │          │
│  │                       │      │           └──────────────────┘   │          │
│  │              ┌────────▼─┐  ┌─▼─────────┐   ┌───────────────┐    │          │
│  │              │  drive   │  │  storage  │   │   emulator    │    │          │
│  │              │ reqwest  │  │ rusqlite  │   │ perfis PPSSPP │    │          │
│  │              │ retry/   │  │ manifest, │   │ PCSX2, detect │    │          │
│  │              │ backoff  │  │ fila, cfg │   └───────────────┘    │          │
│  │              └────┬─────┘  └───────────┘                        │          │
│  └───────────────────┼─────────────────────────────────────────────┘          │
│                      │ HTTPS                                                  │
└──────────────────────┼────────────────────────────────────────────────────────┘
                       ▼
              Google Drive API v3
         RetroSync/ ─ PPSSPP/ ─ PCSX2/ ─ sync_manifest.json
```

## Módulos do backend

| Módulo | Responsabilidade |
| --- | --- |
| `commands` | Boundary única: todos os `#[tauri::command]`. Nenhuma lógica de negócio aqui — só orquestra os módulos. |
| `events` | Constantes com os nomes dos eventos emitidos ao frontend. |
| `constants` | Nomes de pastas do Drive, chaves do keyring, parâmetros de runtime. Zero magic strings no resto do código. |
| `error` | `AppError` (thiserror) unificado, serializado para o frontend como `{ code, message }`. |
| `state` | `AppState` gerenciado pelo Tauri — handles de `auth`, `db` e `engine`. |
| `auth` | OAuth2 + PKCE, keyring, renovação de access token. |
| `drive` | Cliente da API do Google Drive v3: retry, pastas idempotentes, upload/download. |
| `emulator` | Perfis de emuladores e detecção automática. |
| `storage` | SQLite: manifest de sync, fila offline, emuladores configurados. |
| `sync` | SyncEngine: diff, resolução de conflito, orquestração das transferências. |
| `watcher` | Monitor de processos (Passo 6 — atualmente só o tipo `WatcherEvent`). |

## Fluxo de dados de um sync

```
gatilho ──▶ SyncEngine.sync_*()
                │
                ├─ 1. auth.status() ........... conectado? senão aborta
                ├─ 2. storage: lista emuladores configurados
                │
                └─ para cada emulador, para cada categoria (saves/savestates/config):
                     ├─ 3. drive.ensure_category_folder() .. cria/acha pasta no Drive
                     ├─ 4. drive.list_tree() ............... estado remoto
                     ├─ 5. diff.scan_local_bases() ......... estado local (disco)
                     ├─ 6. storage.manifest.list() ......... estado do último sync
                     ├─ 7. diff.build_plan() ............... une os 3 + conflict.decide()
                     └─ 8. executa o plano (≤3 simultâneas):
                          ├─ upload / download via drive
                          ├─ atualiza manifest (SQLite)
                          ├─ emite sync:progress
                          └─ falha de rede → fila offline
                │
                ├─ publica sync_manifest.json no Drive (snapshot)
                └─ emite sync:completed (SyncSummary)
```

## Os 5 gatilhos de sincronização

Todos convergem para um único ponto de entrada do `SyncEngine`. Isso mantém o
comportamento idêntico independentemente de quem disparou.

| Gatilho | Origem | Direção | Status |
| --- | --- | --- | --- |
| Iniciar o RetroSync | `setup` hook do Tauri | Bidirecional | ✅ Implementado (Passo 5) |
| Sync manual | comando `sync_now` / tray | Bidirecional | ✅ Implementado (Passo 5) |
| Fechar o RetroSync | `RunEvent::ExitRequested` | Bidirecional | ⏳ Passo 7 |
| Emulador abriu | watcher → mpsc | Drive → Local | ⏳ Passo 6 |
| Emulador fechou | watcher → mpsc | Local → Drive | ⏳ Passo 6 |

O gancho no engine para os gatilhos por emulador já existe: `SyncEngine::sync_emulator(name, direction)`.

## Estrutura de pastas do projeto

```
retro-sync/
├── docs/                         # esta documentação
├── index.html
├── package.json                  # scripts: dev, build, lint, format
├── vite.config.ts
├── tsconfig.json                 # strict: true
├── eslint.config.js              # ESLint 9 flat config
├── .prettierrc
├── .env.example                  # modelo das credenciais OAuth
├── README.md
├── src/                          # ───── Frontend React + TypeScript ─────
│   ├── main.tsx
│   ├── App.tsx
│   ├── components/
│   │   └── ConnectDrive.tsx       # tela de conexão OAuth (Passo 3)
│   ├── lib/
│   │   └── ipc.ts                 # wrappers tipados de invoke()
│   └── types/
│       └── ipc.ts                 # ÚNICO espelho TS das structs Rust
└── src-tauri/                    # ───── Backend Rust ─────
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── build.rs                  # injeta credenciais do .env em build-time
    ├── capabilities/default.json
    └── src/
        ├── main.rs               # entry fino
        ├── lib.rs                # setup Tauri, registro de comandos, logging
        ├── commands.rs           # boundary: todos os #[tauri::command]
        ├── events.rs             # nomes dos eventos
        ├── constants.rs          # constantes globais
        ├── error.rs              # AppError serializável
        ├── state.rs              # AppState compartilhado
        ├── auth/                 # mod, oauth, token_store
        ├── drive/                # mod, client, files, folders
        ├── emulator/             # mod, ppsspp, pcsx2
        ├── storage/              # mod, db, manifest, queue, emulators
        ├── sync/                 # mod, engine, diff, conflict
        └── watcher/              # mod (Passo 6)
```

## Estrutura no Google Drive

Criada automaticamente, de forma idempotente, com escopo `drive.file`:

```
RetroSync/
├── PPSSPP/
│   ├── saves/
│   ├── savestates/
│   └── config/
├── PCSX2/
│   ├── saves/
│   ├── savestates/
│   └── config/
└── sync_manifest.json   ← snapshot do estado de sync (diagnóstico/bootstrap)
```

> A **fonte de verdade operacional** do manifest é a tabela SQLite local. O
> `sync_manifest.json` é um snapshot exportado a cada sync. Veja
> [Decisões técnicas](./decisoes-tecnicas.md#manifest-sqlite--snapshot-json).
