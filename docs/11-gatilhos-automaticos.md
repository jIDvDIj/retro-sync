# 11 — Sync automático por gatilho

> Implementa o **Passo 4** de [FEATURE-002](./features/feature-002-configuracoes-prompt.md).

## O quê

Cada gatilho de sync automático pode ser ligado/desligado individualmente nas configurações:

| Gatilho | Descrição | Padrão |
| --- | --- | --- |
| `startup` | Sync ao abrir o RetroSync | ligado |
| `emulator-start` | Download antes de o emulador abrir | ligado |
| `emulator-stop` | Upload ao fechar o emulador | ligado |

Quem prefere controle manual pode desligar todos — **o botão "Sincronizar agora" (UI e tray)
continua funcionando**, pois os gatilhos `manual` e `shutdown` não são afetados.

## Por quê

Os syncs automáticos são ótimos por padrão, mas alguns usuários querem decidir exatamente quando
sincronizar (ex.: conexão limitada, ou para evitar sync no meio de um backup manual). Tornar cada
gatilho independente cobre esses casos sem remover a automação de quem a quer.

## Como

### Configurações

`Settings` ganhou o campo `triggers: TriggerSettings { startup, emulator_start, emulator_stop }`
(default todos `true`). Persistido em `app_settings` como três chaves booleanas; `settings::triggers`
lê com default `true` quando ausente. Comando `set_triggers(triggers)`.

### Gating nos pontos de disparo

O gate fica **na origem de cada gatilho**, não no engine (sync manual nunca passa por aqui):

- **`startup`** — em `lib.rs`, o spawn de inicialização lê `settings::triggers` (via um clone do
  `Db`) e retorna cedo se `startup` estiver desligado.
- **`emulator-start` / `emulator-stop`** — o consumidor do watcher (`watcher/mod.rs`) recebe um
  `Db` e, a cada transição, consulta os triggers. O evento `emulator:status` **sempre** é emitido
  (a UI mostra "em execução"); só o sync automático respeita o flag. `running` decide qual flag
  checar (`emulator_start` na abertura, `emulator_stop` no fechamento).

## Arquivos

| Arquivo | Mudança |
| --- | --- |
| `src-tauri/src/constants.rs` | Chaves `SETTING_TRIGGER_*` |
| `src-tauri/src/storage/settings.rs` | `TriggerSettings`, `triggers`, `set_triggers`, helpers bool |
| `src-tauri/src/commands.rs` | `set_triggers` |
| `src-tauri/src/lib.rs` | Gate do `startup` no spawn de inicialização |
| `src-tauri/src/watcher/mod.rs` | `Db` no consumidor; gate de `emulator-start/stop` |
| `src/components/TriggerSettings.tsx` | **Novo** — toggles dos gatilhos |
| `src/components/SettingsModal.tsx` | Seção "Sincronização automática" |
| `src/lib/ipc.ts`, `src/types/ipc.ts`, `src/App.css` | Boundary + estilos |

## Decisões

- **Gate na origem, não no engine**: o engine permanece "burro" e o sync manual continua sempre
  funcional — desligar gatilhos nunca pode quebrar o botão de sync. Cada origem (`startup` no
  setup, watcher no consumidor) já tem acesso ao `Db`, então o custo é uma leitura barata.
- **`emulator:status` sempre emitido**: desligar o sync automático não deve esconder do usuário
  que o emulador está rodando — só evita a transferência.
- **Default na leitura** (como nas categorias): instalações anteriores à v1.1 funcionam sem linhas
  em `app_settings`.
