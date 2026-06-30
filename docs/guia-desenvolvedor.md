# Guia do Desenvolvedor

Tutorial de onboarding para quem vai desenvolver no RetroSync: do clone do repositório até
rodar o app, buildar, validar a qualidade do código e entender por onde começar.

> Para a **visão geral do produto**, veja o [`README.md`](../README.md) na raiz.
> Para **arquitetura e decisões**, comece por [01 — Arquitetura](./01-arquitetura.md).

---

## 1. Pré-requisitos

O alvo de produção é **Windows nativo**. Instale, no Windows (**PowerShell como Administrador**):

| Ferramenta | Comando |
| --- | --- |
| **Rust** | `winget install Rustlang.Rustup` |
| **Microsoft C++ Build Tools** | `winget install Microsoft.VisualStudio.2022.BuildTools` → depois abra o VS Installer e marque a workload "Desenvolvimento para desktop com C++" |
| **WebView2** | `winget install Microsoft.EdgeWebView2Runtime` (já incluso no Windows 10/11 atualizados) |
| **Node.js** | ≥ 20 LTS (verifique com `node --version`) |

Referência oficial: [tauri.app/start/prerequisites](https://tauri.app/start/prerequisites/).

> **Sobre WSL2**: este repositório vive em `/mnt/c`, mas **`npm run tauri dev`/`build` deve
> rodar no Windows nativo (PowerShell)** — build dentro do WSL gera binário Linux, exige as
> libs `webkit2gtk` e sofre com I/O lento do 9p. Edite o código onde preferir; comandos
> `cargo check/test/clippy` rodam bem no WSL (veja a [seção 7](#7-ambiente-wsl-fixes-recorrentes)).
> Detalhes em [02 — Scaffolding](./02-scaffolding.md#nota-sobre-o-ambiente-de-desenvolvimento)
> e [Riscos](./riscos.md).

---

## 2. Clonar e instalar dependências

```bash
git clone <url-do-repo>
cd retro-sync
npm install            # dependências do frontend
```

As dependências do Rust são baixadas automaticamente na primeira compilação
(`tauri dev`/`build` ou `cargo`).

---

## 3. Configurar as credenciais OAuth

Sem credenciais o app **sobe normalmente**, mas o botão de conectar ao Drive retorna um
erro explicativo. Para habilitar o Drive:

1. Crie um projeto no [Google Cloud Console](https://console.cloud.google.com/), ative a
   **Google Drive API**, configure a OAuth consent screen (tipo External, sua conta como
   test user) e crie uma credencial **OAuth Client ID** do tipo **Desktop app**.
2. Copie `.env.example` → `.env` na raiz e preencha:

   ```
   RETROSYNC_GOOGLE_CLIENT_ID=seu-client-id
   RETROSYNC_GOOGLE_CLIENT_SECRET=seu-secret   # só no fluxo de dev local sem Worker
   ```

O `src-tauri/build.rs` injeta essas variáveis em build-time (variáveis do shell têm
precedência sobre o `.env`). O escopo OAuth é `drive.file` — o app só enxerga o que ele
mesmo cria.

> Passo a passo completo, fluxo PKCE e armazenamento de tokens:
> [03 — Autenticação](./03-autenticacao.md#configuração-de-credenciais).
> Em produção o `client_secret` não vai no binário — fica num Cloudflare Worker:
> [15 — Proxy Cloudflare Worker](./15-proxy-worker-oauth.md).

---

## 4. Rodar em desenvolvimento

No **PowerShell** (Windows nativo):

```bash
npm run tauri dev      # compila o Rust na 1ª vez e abre a janela "RetroSync"
```

A janela deve exibir o status do backend pronto — confirma a boundary `invoke` → Rust
funcionando de ponta a ponta.

---

## 5. Build de produção

```bash
npm run tauri build    # gera o instalador/binário de produção
```

Logs de operação ficam no diretório de logs do app
(`%LOCALAPPDATA%\com.retrosync.app\logs` no Windows), com rotação diária.

---

## 6. Qualidade de código

Rode antes de abrir um PR — é o que a CI valida:

```bash
# Frontend
npm run lint            # ESLint
npm run format:check    # Prettier (--check); `npm run format` aplica
npm run build           # tsc + vite build (o que a CI roda em PR)

# Backend Rust
cargo fmt    --manifest-path src-tauri/Cargo.toml     # rustfmt
cargo clippy --manifest-path src-tauri/Cargo.toml     # lints
cargo test   --manifest-path src-tauri/Cargo.toml     # testes unitários
cargo test   --manifest-path src-tauri/Cargo.toml <nome_do_teste>   # um único teste
```

- **CI** (`.github/workflows/ci.yml`): em cada PR roda `lint` + `format:check` + `build` do
  frontend.
- **Release** (`.github/workflows/release.yml`): push de tag `v*` builda e publica (draft)
  para macOS/Linux/Windows via `tauri-action`, validando antes os secrets de credenciais.

---

## 7. Ambiente WSL (fixes recorrentes)

Se você desenvolve a partir do WSL2 (com o repo em `/mnt/c`):

- **Rust não está instalado no WSL**: o `winget` instala apenas no Windows. Para usar
  `cargo check/clippy/test` no WSL, instale o Rust separadamente:

  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  # responda "1" (default); depois recarregue:
  source ~/.cargo/env
  ```

- **Cargo lento / poluindo o `/mnt/c`**: exporte um target dir fora do 9p antes dos comandos
  cargo:

  ```bash
  export CARGO_TARGET_DIR=$HOME/.cache/retro-sync-target
  ```

  Coloque esse `export` no seu `~/.bashrc` ou `~/.zshrc` para não precisar repetir.

- **`Cannot find module @rollup/rollup-linux-x64-gnu`** ao rodar `npm run build`: o
  `node_modules` em `/mnt/c` é compartilhado entre Windows e WSL, e cada `npm install` de um
  lado remove o binário nativo do outro. Correção (aplicar direto, acontece quase toda sessão):

  ```bash
  npm install --no-save @rollup/rollup-linux-x64-gnu
  ```

---

## 8. Por onde começar no código

```
src-tauri/src/        # Backend Rust — TODA a lógica de negócio
├── lib.rs            # run(): setup comum + delega para platform/desktop ou platform/mobile
├── commands.rs       # Boundary #[tauri::command] — toda ela vive aqui
│                     #   desktop-only: set_autostart, open_backup_folder (#[cfg(desktop)])
│                     #   mobile-only:  pick_emulator_folder (#[cfg(mobile)])
├── state.rs          # AppState (auth, db, engine, last_sync)
├── constants.rs      # Pastas do Drive, chaves keyring, triggers (sem magic strings)
├── secrets.rs        # Trait SecretStore: KeyringStore (desktop) / SqliteSecretStore (mobile)
├── platform/
│   ├── desktop.rs    # Bandeja, prevent_close, process watcher, autostart — #[cfg(desktop)]
│   └── mobile.rs     # Init mobile (stub; lifecycle via tauri://resume/pause em lib.rs)
├── auth/             # OAuth2 + PKCE, SecretStore, refresh automático de token
├── drive/            # Cliente Google Drive API (reqwest + retry/backoff)
├── emulator/         # Perfis declarativos (profiles.toml) + detecção (PPSSPP, PCSX2)
├── storage/          # SQLite: manifest, fila offline, emuladores, settings, conflicts
├── sync/             # SyncEngine (diff, conflitos, upload/download)
│   ├── storage.rs    # Trait LocalStorage + DesktopStorage — isola I/O local do engine
│   └── mobile_storage.rs  # Implementação mobile do LocalStorage (SAF/Android)
└── watcher/          # Monitor de processos (sysinfo) → gatilhos de sync — #[cfg(desktop)]
src/                  # Frontend React — UI "burra": só invoke/emit
├── components/       # Telas e modais
├── hooks/            # Auth, descoberta, sync, conflitos, settings
├── lib/ipc.ts        # Único lugar que chama invoke()
└── types/ipc.ts      # Espelho TS das structs Rust + nomes de eventos
worker/               # Cloudflare Worker — proxy do token endpoint OAuth
```

> **Separação desktop / mobile**: código exclusivo do desktop é guardado com `#[cfg(desktop)]`
> e vive em `platform/desktop.rs` (bandeja, watcher, autostart) ou em blocos `#[cfg(desktop)]`
> em `commands.rs`. O `SyncEngine` e todo o core de sync são agnósticos de plataforma — operam
> sobre a trait `LocalStorage`, não sobre `std::fs` diretamente.

Leituras recomendadas, nesta ordem:

1. [01 — Arquitetura](./01-arquitetura.md) — o mapa geral, fluxo de dados e gatilhos de sync.
2. [Referência da boundary IPC](./referencia-ipc.md) — catálogo de comandos, eventos e tipos.
3. O doc do passo da área que você vai mexer (ver [índice](./README.md)).

> **Atenção à boundary tripla**: toda struct/enum que cruza Rust↔TS aparece em três lugares
> — a struct Rust (`#[serde(rename_all = "camelCase")]`), a interface em `src/types/ipc.ts` e
> o wrapper em `src/lib/ipc.ts`. Ao mexer em uma, atualize as três. Detalhes no
> [Referência IPC](./referencia-ipc.md).

---

## 9. Fluxo de contribuição

- **Commits**: [Conventional Commits](https://www.conventionalcommits.org/) no formato
  `tipo(escopo): descrição` (escopos semânticos como `auth`, `sync`).
- **Documentação**: ao concluir um passo ou tomar uma decisão técnica, atualize `docs/`
  (índice em [docs/README.md](./README.md), boundary em [referencia-ipc.md](./referencia-ipc.md),
  decisões em [decisoes-tecnicas.md](./decisoes-tecnicas.md), riscos em [riscos.md](./riscos.md)).
- **Antes do PR**: rode os checks da [seção 6](#6-qualidade-de-código) — lint, format e build
  precisam passar na CI.
</content>
