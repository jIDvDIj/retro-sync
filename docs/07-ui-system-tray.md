# 07 — UI e System Tray

**Commit**: `bbc8ac7` — *feat: UI principal, system tray e notificações nativas de sync*

## Objetivo

Fechar a v1.0: a tela principal em React (lista de emuladores, status de sync ao vivo,
último sync, sync manual), o seletor de pasta nativo, o ícone na bandeja com menu de
contexto e as notificações nativas de erro — incluindo o 4º gatilho de sync (ao fechar o
app).

## Arquivos

### Backend

| Arquivo | Conteúdo |
| --- | --- |
| `lib.rs` | Tray (menu Abrir/Sincronizar/Sair), `CloseRequested` → esconder, sync de despedida |
| `sync/engine.rs` | `LastSync` + `LastSyncStore`, gravação do último sync, `notify_error` |
| `commands.rs` | `get_last_sync` |
| `state.rs` | `AppState.last_sync` (célula compartilhada com o engine) |
| `constants.rs` | `MAIN_WINDOW_LABEL`, `TRAY_MENU_OPEN/SYNC/QUIT` |

### Frontend

| Arquivo | Conteúdo |
| --- | --- |
| `hooks/useSyncEvents.ts` | Assinante único de `sync:*` e `emulator:status`; estado consolidado |
| `hooks/useEmulators.ts` | Lista de emuladores, recarga e remoção |
| `components/AddEmulator.tsx` | Seletor de pasta nativo (`plugin-dialog`) + `add_emulator` |
| `components/EmulatorCard.tsx` | Card por emulador, com badge "em execução" |
| `components/SyncStatus.tsx` | Barra: último sync, progresso ao vivo, "Sincronizar agora" |
| `components/ConnectDrive.tsx` | Agora notifica o `App` da conexão (`onConnectionChange`) |
| `lib/errors.ts` | `errorMessage` compartilhado |
| `App.tsx` / `App.css` | Composição e estilos |

## System tray e ciclo de vida da janela

O app passa a **viver na bandeja**:

- **Fechar a janela** (`WindowEvent::CloseRequested`) → `prevent_close()` + `window.hide()`.
  O app continua rodando (watcher e syncs ativos).
- **Menu da tray** (`TrayIconBuilder` + `Menu`), itens com IDs constantes:
  - **Abrir** / clique esquerdo no ícone → mostra e foca a janela;
  - **Sincronizar agora** → sync bidirecional manual em background;
  - **Sair** → roda o **sync de despedida** (`TRIGGER_SHUTDOWN`) e então `app.exit(0)`.

Todas as operações de janela/tray são feitas no Rust, então **não exigem permissões novas
nas capabilities** — o sistema de permissões só controla o que o frontend (JS) invoca.
Ver [Decisões técnicas](./decisoes-tecnicas.md#app-vive-na-tray-fechar-a-janela--sair).

## Gatilho de fechamento

É o último dos cinco gatilhos da especificação. O sync de despedida fica no handler do
menu **"Sair"** (saída intencional e controlável), não no `RunEvent::ExitRequested`, que
exigiria prevenir e re-disparar o exit. Como fechar a janela apenas esconde, o único
caminho de saída real passa pelo "Sair" — então o sync de despedida sempre roda.

## Último sync (`LastSync`)

Célula `Arc<Mutex<Option<LastSync>>>` compartilhada entre o `SyncEngine` (escreve ao
concluir, **antes** de emitir `sync:completed`) e o `AppState` (lida por `get_last_sync`).
A UI busca no mount (cobre o startup sync que roda antes da tela montar) e atualiza ao vivo
pelo evento. Sem migração de schema SQLite — o estado é efêmero por execução, e cada
inicialização produz um novo sync de qualquer forma.
Ver [Decisões técnicas](./decisoes-tecnicas.md#último-sync-em-célula-compartilhada).

## Notificações nativas

Disparadas pelo **backend** (`NotificationExt`, no mesmo ponto em que o engine emite
`sync:error`), não pelo frontend. Assim funcionam mesmo com a janela oculta e durante o
sync de despedida no shutdown, quando o webview pode não estar responsivo.
Ver [Decisões técnicas](./decisoes-tecnicas.md#notificações-de-erro-no-backend).

## Fluxo do frontend

- `useSyncEvents` é o **assinante único** dos eventos, distribuído para a barra de status
  (progresso, último sync, erro) e para os cards (badge "em execução" via `emulator:status`).
- `useEmulators` gerencia a lista (`list_emulators`) com recarga após adicionar/remover.
- `AddEmulator` abre o seletor de pasta nativo (`@tauri-apps/plugin-dialog`) e chama
  `add_emulator`; erro de "emulador não reconhecido" aparece inline.
- O estado de conexão é elevado ao `App` (callback do `ConnectDrive`) para habilitar/
  desabilitar "Adicionar emulador" e "Sincronizar agora".

## Comando exposto

| Comando | Assinatura | Descrição |
| --- | --- | --- |
| `get_last_sync` | `() -> Option<LastSync>` | Último sync concluído nesta execução |

Ver [Referência — Boundary IPC](./referencia-ipc.md#tipos).

## Testes

A UI, o tray e o ciclo de vida da janela não têm testes unitários (dependem de display e
do SO; o alvo é Windows nativo). A suíte Rust segue em **52 testes**, com clippy e ESLint
limpos. A camada de lógica testável (conflito, diff, debounce do watcher, storage) já está
coberta nos passos anteriores.

## Como testar manualmente (Windows, `npm run tauri dev`)

1. Conecte o Drive → **Adicionar emulador** abre o seletor de pasta nativo → escolha a
   pasta do PPSSPP/PCSX2; o card aparece;
2. **Sincronizar agora** mostra o progresso e depois "Último sync há … · ↑N ↓N (Ys)";
3. Feche a janela no `X` → o app continua na bandeja; clique no ícone → a janela volta;
4. Abra um emulador → o card vira "em execução" e dispara sync (Passo 6);
5. Menu da tray → **Sair** → roda o sync de despedida e encerra.

> **Notificações em dev**: no Windows, notificações nativas podem não aparecer até o app
> estar instalado (registro do AppUserModelID no WebView2). É limitação do SO, não do código.
