# 14 — Resolução de conflito

> Implementa o **Passo 7** de [FEATURE-002](./features/feature-002-configuracoes-prompt.md) e
> resolve o [BUG-002](./bugs/bug-002-conflito-edicao-simultanea.md).

## O quê

Quando **ambos os lados** (local e Drive) mudaram desde o último sync, o RetroSync não escolhe
sozinho: registra um **conflito**, pausa o sync daquele emulador e avisa o usuário (notificação
nativa). No card do emulador afetado aparece um botão **"Resolver conflito"** que abre um modal com
os dois lados (data, tamanho e dispositivo de origem). O usuário escolhe qual manter e o sync é
desbloqueado.

Enquanto houver conflito pendente, o sync daquele emulador fica **bloqueado** — botão manual e
gatilhos automáticos não executam para ele. Emuladores sem conflito funcionam normalmente.

## Por quê

O BUG-002: em edição simultânea entre dispositivos (incluindo após período offline), o sync
resolvia por mtime bruto e **sobrescrevia silenciosamente** o lado perdedor — perda irrecuperável,
às vezes do dispositivo que não fez nada de errado. A única resolução correta é o usuário decidir,
com contexto (qual sessão de jogo manter). Daí o conflito explícito + bloqueio + escolha.

## Como

### Detecção (`sync/conflict.rs`)

Novo `SyncAction::Conflict`. Com manifest presente, `decide` agora classifica pelo que mudou desde
o último sync: só local → `Upload`; só Drive → `Download`; **ambos** → `Conflict` (a não ser que o
mtime tenha ficado idêntico). É incluído no plano em **qualquer direção** — nunca sobrescrever sem
o usuário, mesmo num sync de mão única.

### Registro e bloqueio (`sync/engine.rs`, `storage/conflicts.rs`)

- Tabela `sync_conflicts` (migração v4) guarda os metadados dos dois lados + `local_abs_path` e
  `drive_file_id` para a resolução.
- `record_conflict` grava a linha, emite `sync:conflict` e notifica (gated pelo nível). Nenhuma
  transferência é feita para o arquivo em conflito; os demais arquivos do emulador seguem normais
  naquele sync.
- Antes de sincronizar cada emulador, `sync_filtered` checa `conflicts::has_for_emulator` e
  **pula** o emulador bloqueado.

### Atribuição de dispositivo (`drive/`)

Para mostrar "de qual dispositivo veio cada versão", todo upload marca `appProperties.device` no
arquivo do Drive (constante `DRIVE_APP_PROP_DEVICE`); `DriveFile` lê de volta via `device()`. O
lado local usa o nome deste dispositivo. (O nome no snapshot, do Passo 1, continua para
diagnóstico.)

### Resolução (`SyncEngine::resolve_conflict`)

- **Manter Drive**: backup do local → baixa a versão remota por cima → manifest atualizado.
- **Manter local**: envia o local por cima do `drive_file_id` → manifest atualizado.
- Em ambos, remove a linha de `sync_conflicts` (desbloqueia).

### Boundary e UI

| Comando | Uso |
| --- | --- |
| `list_conflicts()` | Lista para a UI decidir quais cards mostram o botão |
| `resolve_conflict(emulator, category, relPath, keep)` | `keep` ∈ `local`/`drive` |

Evento `sync:conflict` (payload `Conflict`). `useConflicts` carrega a lista e recarrega no evento e
após resolução. `EmulatorCard` mostra o badge/botão; `ConflictModal` mostra os dois lados e
resolve.

## Arquivos

| Arquivo | Mudança |
| --- | --- |
| `src-tauri/src/sync/conflict.rs` | `SyncAction::Conflict` para "ambos mudaram" |
| `src-tauri/src/sync/diff.rs` | `Conflict` sempre incluído no plano |
| `src-tauri/src/sync/engine.rs` | `record_conflict`, bloqueio, `resolve_conflict`, device nos uploads |
| `src-tauri/src/storage/conflicts.rs` | **Novo** — tabela `sync_conflicts` + CRUD |
| `src-tauri/src/storage/db.rs` | Migração v4 |
| `src-tauri/src/drive/files.rs`, `drive/mod.rs` | `appProperties.device` (escrita e leitura) |
| `src-tauri/src/events.rs`, `commands.rs`, `lib.rs` | Evento + `list_conflicts`/`resolve_conflict` |
| `src/hooks/useConflicts.ts`, `components/ConflictModal.tsx` | **Novos** |
| `src/components/EmulatorCard.tsx`, `App.tsx` | Badge/botão + modal por emulador |
| `src/lib/ipc.ts`, `src/types/ipc.ts`, `src/App.css` | Boundary + estilos |

## Decisões

- **Bloqueio por emulador, conflito por arquivo**: um conflito pausa o emulador inteiro (mais
  simples e seguro de raciocinar), mas a resolução é por arquivo. Os demais arquivos ainda
  sincronizam no sync em que o conflito é detectado; o bloqueio vale dos próximos syncs em diante.
- **`appProperties` em vez de só o snapshot**: dá a origem precisa de **cada versão** no Drive, não
  apenas de quem publicou o snapshot por último — o que o modal de conflito exige.
- **Resolver = backup + sobrescrever, nunca deletar**: mantém o princípio não-destrutivo; a versão
  preterida localmente vai para backup, e o Drive nunca é apagado (só sobrescrito, com histórico de
  revisões do próprio Drive como rede de segurança extra).
