# 10 — Categorias de sync por emulador

> Implementa o **Passo 3** de [FEATURE-002](./features/feature-002-configuracoes-prompt.md).

## O quê

Para cada emulador configurado, o usuário escolhe quais categorias sincronizar: **saves**,
**savestates** e/ou **config**. Por padrão, todas ativas. Caso de uso típico: desativar `config`
para que resolução/controles não sejam compartilhados entre máquinas diferentes.

A configuração fica numa nova seção "Sincronização por emulador" do modal de configurações.

## Por quê

As três categorias têm valor de compartilhamento diferente: saves e savestates são quase sempre
desejáveis em todos os dispositivos; já as configs (resolução, mapeamento de controle) costumam
ser específicas da máquina. Forçar tudo junto frustraria quem joga no PC e no notebook com
hardware distinto.

## Como

### Backend

- **Migração v3**: tabela `emulator_settings (emulator PK, saves_enabled, savestates_enabled,
  config_enabled)`.
- **`storage/emulators.rs`**: struct `SyncCategories` (default todas `true`) + `get_categories`
  (retorna o default se não há linha), `set_categories`, `remove_categories`. O `remove_emulator`
  agora também limpa as settings do emulador.
- **`SyncEngine::sync_filtered`**: ao montar os `SyncTarget`, carrega as categorias do emulador e
  faz `target.categories.retain(...)`, removendo as desativadas. O núcleo do engine segue
  agnóstico — ele só recebe menos categorias no target.

### Boundary

| Comando | Retorno |
| --- | --- |
| `get_emulator_categories(name)` | `SyncCategories` |
| `set_emulator_categories(name, categories)` | `void` |

`SyncCategories = { saves, savestates, config }` espelhado em `src/types/ipc.ts`.

### Frontend

- **`components/CategorySettings.tsx`** (novo): para cada emulador, carrega as categorias e
  mostra três checkboxes. Toggle é **otimista** (atualiza a UI na hora, reverte em caso de falha).
- **`SettingsModal`**: nova seção que recebe `emulators` (do App) e embute o `CategorySettings`.

## Arquivos

| Arquivo | Mudança |
| --- | --- |
| `src-tauri/src/storage/db.rs` | Migração v3 (`emulator_settings`) |
| `src-tauri/src/storage/emulators.rs` | `SyncCategories` + get/set/remove + testes |
| `src-tauri/src/commands.rs` | `get_emulator_categories`, `set_emulator_categories`; limpeza no remove |
| `src-tauri/src/lib.rs` | Registro dos comandos |
| `src-tauri/src/sync/engine.rs` | Filtro de categorias desativadas ao montar targets |
| `src/components/CategorySettings.tsx` | **Novo** — toggles por emulador |
| `src/components/SettingsModal.tsx`, `src/App.tsx` | Seção de categorias |
| `src/lib/ipc.ts`, `src/types/ipc.ts`, `src/App.css` | Boundary + estilos |

## Decisões

- **Default aplicado na leitura** (não na escrita): emuladores existentes antes da v1.1 não
  precisam de linha em `emulator_settings` — `get_categories` devolve "tudo ativo" quando ausente.
  Só grava quando o usuário muda algo.
- **Filtro no target, não no diff**: manter o `diff`/`conflict` intocados preserva o princípio de
  núcleo agnóstico. Desativar uma categoria simplesmente faz o engine não enxergá-la.
- **Tabela separada** em vez de campo no `EmulatorProfile`: o perfil é dado de *detecção*
  (caminhos no disco); as categorias são *preferência do usuário*. Misturá-los acoplaria
  re-detecção e configuração.
