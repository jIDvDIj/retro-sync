# 13 — Primeiro sync: Drive vence + backup local

> Implementa o **Passo 6** de [FEATURE-002](./features/feature-002-configuracoes-prompt.md) e
> resolve o [BUG-001](./bugs/bug-001-perda-save-primeiro-sync.md).

## O quê

Quando um arquivo existe **tanto no dispositivo quanto no Drive e nunca foi sincronizado antes**
(sem manifest), o **Drive sempre vence**. Antes de sobrescrever o arquivo local, ele é copiado
para uma pasta de backup. Após o sync, a UI mostra um aviso de que backups foram criados, com um
botão que abre a pasta no gerenciador de arquivos do SO.

## Por quê

O BUG-001: ao instalar o RetroSync num segundo dispositivo que já tem saves locais, o sync inicial
comparava mtime bruto e podia **subir** um save local recém-criado (mtime de hoje, pouco
progresso) por cima de um save valioso no Drive (mtime antigo, 100 h de jogo) — perda
irreversível e silenciosa.

A correção: no primeiro sync de um arquivo, mtime não é evidência confiável de "valor"; o Drive
(fonte de verdade já estabelecida em outra máquina) deve vencer. O backup garante que, mesmo se a
escolha não for a desejada, o save local não se perde.

## Como

### Decisão (`sync/conflict.rs`)

Novo `SyncAction::DownloadWithBackup`. Em `decide`, o ramo "ambos existem" passou a depender de
haver manifest:

- **com manifest** (`last_synced = Some`): resolução normal por mtime (conflito real fica para o
  Passo 7);
- **sem manifest** (`last_synced = None`): mtimes iguais → `NoOp`; diferentes →
  `DownloadWithBackup` (Drive vence, faz backup).

### Execução (`sync/engine.rs`)

`do_download_with_backup` copia `local.abs_path` para
`<app_data>/backups/<emulador>/<timestamp-do-sync>/<categoria>/<rel_path>` e **só então** chama o
download normal. O backup roda **antes** do download — se a cópia falhar, o download não acontece,
de modo que nunca há sobrescrita sem backup. `SyncSummary` ganhou `backed_up`; a contagem é
agregada via `OpOutcome::DownloadedWithBackup`.

### UI

- Comando `open_backup_folder` abre `<app_data>/backups` no gerenciador via a crate `open` (já era
  dependência), criando a pasta se preciso.
- `SyncStatus` mostra um banner quando `lastSync.summary.backedUp > 0`, com o botão "Abrir pasta de
  backup".

## Arquivos

| Arquivo | Mudança |
| --- | --- |
| `src-tauri/src/sync/conflict.rs` | `DownloadWithBackup` + lógica first-sync-Drive-wins + testes |
| `src-tauri/src/sync/diff.rs` | `DownloadWithBackup` tratado como download no filtro de direção |
| `src-tauri/src/sync/engine.rs` | `backed_up`, `backup_dir`, `backup_base`, `do_download_with_backup` |
| `src-tauri/src/constants.rs` | `LOCAL_BACKUP_DIR` |
| `src-tauri/src/commands.rs`, `lib.rs` | `open_backup_folder`; `backup_dir` no engine |
| `src/components/SyncStatus.tsx` | Banner de backups + botão |
| `src/lib/ipc.ts`, `src/types/ipc.ts`, `src/App.css` | Boundary + estilos |

## Decisões

- **Backup antes do download, abortando em falha**: a ordem é o que torna a operação segura — um
  download que sobrescreve sem backup é exatamente a perda que se quer evitar.
- **Backup local (não no Drive)**: o aviso da UI abre o gerenciador de arquivos do SO; um caminho
  local é o que faz sentido abrir. Fica em `<app_data>/backups`, agrupado por sync (timestamp).
- **Drive-first só no primeiro sync**: depois que há manifest, a resolução volta a ser por mtime
  (e, no Passo 7, por conflito explícito). O backup é restrito ao primeiro sync para não acumular
  cópias a cada sincronização.
