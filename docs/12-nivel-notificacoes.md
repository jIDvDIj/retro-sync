# 12 — Nível de notificações nativas

> Implementa o **Passo 5** de [FEATURE-002](./features/feature-002-configuracoes-prompt.md).

## O quê

O usuário controla quais eventos geram notificação nativa do SO:

| Nível | Notifica |
| --- | --- |
| `all` | Sync concluído, erros e emulador detectado |
| `errors_only` | Apenas erros de sync |
| `none` | Nenhuma notificação |

Padrão: `all`. Configurável numa seção "Notificações" do modal.

## Por quê

Os gatilhos automáticos (abrir/fechar emulador) sincronizam com frequência; no nível `all` isso
geraria muitas notificações. `errors_only` mantém só o que exige atenção; `none` silencia tudo —
sem perder os syncs em si, apenas os avisos.

## Como

### Backend

`NotificationLevel { All, ErrorsOnly, None }` (default `All`, serde `snake_case`) entrou em
`Settings`. Dois predicados centralizam o gating:

- `notifies_errors()` → `true` exceto em `None`;
- `notifies_info()` → `true` só em `All`.

Pontos de notificação, agora **todos gated**:

| Notificação | Onde | Condição |
| --- | --- | --- |
| Erro de sync (já existia) | `engine.rs` (`notify_error`) | `notifies_errors()` |
| Sync concluído (novo) | `engine.rs` (`notify_completed`) | `notifies_info()` **e** houve transferência |
| Emulador detectado (novo) | `watcher/mod.rs` (na abertura) | `notifies_info()` |

O engine lê o nível uma vez por execução de sync; o watcher lê via `settings::load` por transição
(já fazia isso para os gatilhos do Passo 4). `Settings` passou a derivar `Default` para o
`unwrap_or_default()` desses pontos.

### Boundary

`set_notification_level(level)`; `NotificationLevel = "all" | "errors_only" | "none"`; `Settings`
ganhou `notificationLevel`.

## Arquivos

| Arquivo | Mudança |
| --- | --- |
| `src-tauri/src/constants.rs` | `SETTING_NOTIFICATION_LEVEL` |
| `src-tauri/src/storage/settings.rs` | `NotificationLevel` + predicados + get/set; `Settings: Default` |
| `src-tauri/src/commands.rs`, `lib.rs` | `set_notification_level` |
| `src-tauri/src/sync/engine.rs` | Gate de erro + nova notificação de sync concluído |
| `src-tauri/src/watcher/mod.rs` | Notificação de emulador detectado (gated) |
| `src/components/SettingsModal.tsx` | Seção "Notificações" (select) |
| `src/lib/ipc.ts`, `src/types/ipc.ts` | Boundary |

## Decisões

- **Sync concluído só notifica com transferência** (`uploaded + downloaded > 0`): no nível `all`,
  notificar "nada a sincronizar" a cada gatilho automático seria justamente o ruído que o Passo 5
  busca controlar. O nível ainda governa; este filtro evita notificações vazias.
- **Gating por predicados, não por comparação espalhada**: `notifies_errors`/`notifies_info`
  concentram a regra; os call sites só perguntam "devo notificar isto?".
- **Notificações continuam no backend** (decisão do Passo 7 original): funcionam mesmo com a
  janela oculta ou durante o shutdown.
