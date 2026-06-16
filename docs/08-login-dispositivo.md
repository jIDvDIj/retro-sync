# 08 — Login e nome do dispositivo

> Implementa o **Passo 1** de [FEATURE-002 — Tela de configurações](./features/feature-002-configuracoes-prompt.md).
> É também a base de infraestrutura (tabela `app_settings`) que os passos 2 a 5 reutilizam.

## O quê

O login virou uma **tela dedicada e separada** da tela principal: enquanto o usuário não está
conectado, a única coisa renderizada é a tela de login. A tela principal (emuladores, sync,
configurações) só é montada **depois** que o login conclui. Essa tela de login:

1. **Explica a permissão** antes do login — o RetroSync não acessa dados pessoais, só vê e
   modifica os arquivos que ele mesmo cria no Drive (reflexo do escopo OAuth `drive.file`).
2. **Pede o nome do dispositivo**. O campo é **obrigatório**: o botão "Conectar ao Google
   Drive" só habilita após o preenchimento (ex.: "PC Gamer", "Notebook").
3. **Exibe o nome do dispositivo** na tela principal (etiqueta ao lado da conta conectada) como
   identificador da máquina atual; "Desconectar" leva de volta à tela de login.

O nome é gravado nos **metadados de sync publicados no Drive** (campo `device` do
`sync_manifest.json`), de modo que, num conflito, o usuário saiba de qual dispositivo cada
versão do save provém (ver [Passo 7](./14-resolucao-conflito.md)).

## Por quê

O aviso de permissão dá transparência sobre o escopo mínimo (`drive.file`) — o usuário sabe
que o app não enxerga o resto do Drive dele. O nome do dispositivo é coletado **antes** de
concluir a autenticação porque ele identifica a origem de cada versão sincronizada; sem ele,
a resolução de conflito (Passo 7) não teria como rotular "este save veio do PC ou do
notebook?".

## Como

### Infraestrutura de configurações (`app_settings`)

Migração **v2** do SQLite adiciona uma tabela chave→valor:

```sql
CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
```

O `migrate()` virou incremental (um `if version < N` por migração), preservando bancos já
existentes. O módulo `storage/settings.rs` concentra o acesso: um struct `Settings` agrega as
configurações expostas ao frontend, persistidas como linhas chave→valor com defaults aplicados
na leitura. Começa só com `device_name`; cresce nos passos 4 e 5.

### Boundary

| Camada | Adição |
| --- | --- |
| `commands.rs` | `get_settings() -> Settings`, `set_device_name(name)` (rejeita vazio) |
| `lib.rs` | Ambos registrados no `invoke_handler` |
| `src/types/ipc.ts` | `interface Settings { deviceName: string \| null }` |
| `src/lib/ipc.ts` | `getSettings()`, `setDeviceName(name)` |

### Telas separadas e gating (`useAuth`, `LoginScreen`, `AccountStatus`)

O `App` decide qual tela mostrar a partir do hook `useAuth` (status de auth no nível do App):

- `auth.loading` → tela de espera ("verificando conexão…");
- `!auth.connected` → renderiza **só** a `LoginScreen` (nenhum hook ou comando da tela principal
  roda nesse estado);
- conectado → monta a `MainScreen` (emuladores, sync, conflitos, configurações).

A `LoginScreen` pré-preenche o nome com `settings.deviceName` (vindo do `useSettings` no `App`).
No clique de "Conectar": grava `setDeviceName(name)` **antes** de `connectGoogleDrive()` e
devolve o novo `AuthStatus` ao `App` via `onConnected`, que atualiza o `useAuth` e recarrega as
settings — disparando a troca para a tela principal.

O `AccountStatus` fica no cabeçalho da tela principal: mostra a conta + a etiqueta do
dispositivo + "Desconectar" (que zera o status e volta à `LoginScreen`).

### Gravação no Drive

`SyncEngine::publish_manifest_snapshot` passou a incluir `"device": <nome>` no topo do
`sync_manifest.json` — o "metadado de sync no Drive" que identifica quem publicou o snapshot.

## Arquivos

| Arquivo | Mudança |
| --- | --- |
| `src-tauri/src/storage/db.rs` | Migração v2 (`app_settings`) + `migrate()` incremental |
| `src-tauri/src/storage/settings.rs` | **Novo** — `Settings`, `load`, `device_name`, `set_device_name` |
| `src-tauri/src/storage/mod.rs` | `pub mod settings` |
| `src-tauri/src/constants.rs` | `SETTING_DEVICE_NAME` |
| `src-tauri/src/commands.rs` | `get_settings`, `set_device_name` |
| `src-tauri/src/lib.rs` | Registro dos comandos |
| `src-tauri/src/sync/engine.rs` | `device` no snapshot do Drive |
| `src/hooks/useAuth.ts` | **Novo** — status de auth no nível do App; faz o gating entre as telas |
| `src/components/LoginScreen.tsx` | **Novo** — tela de login dedicada (aviso + campo obrigatório) |
| `src/components/AccountStatus.tsx` | **Novo** — conta conectada + etiqueta + desconectar (cabeçalho) |
| `src/App.tsx` | Alterna entre `LoginScreen` e `MainScreen` conforme o status de auth |
| `src/lib/ipc.ts`, `src/types/ipc.ts` | Espelho de `Settings` e wrappers |
| `src/App.css` | Estilos de `.login-screen`, `.login-card`, `.field`, `.device-tag` |

> O `ConnectDrive.tsx` (componente único que misturava login e status conectado) foi **removido**:
> suas responsabilidades se dividiram entre `LoginScreen` (tela de login) e `AccountStatus`
> (cabeçalho da tela principal).

## Decisões

- **Chave→valor em vez de colunas tipadas**: `app_settings` cresce ao longo dos passos 2 a 5
  sem novas migrações por configuração. Defaults moram no `load()`.
- **`set_device_name` dedicado** (em vez de um `update_settings` monolítico): a tela de login
  só conhece o nome do dispositivo; um update do objeto inteiro sobrescreveria configurações
  que o login não enxerga. Comandos focados evitam esse risco.
- **Nome no snapshot, não em cada arquivo (ainda)**: o Passo 1 grava o dispositivo no topo do
  `sync_manifest.json`. A atribuição por arquivo (via `appProperties`) entra no Passo 7, onde
  é de fato consumida.
- **Login como tela separada, não estado embutido na tela principal**: o gating fica num único
  ponto (`App` + `useAuth`), e os hooks da tela principal (`useEmulators`, `useSyncEvents`,
  `useConflicts`) só montam quando há conexão — não disparam comandos enquanto desconectado.
  Some também a necessidade de espalhar `disabled={!connected}` pelos componentes internos.
