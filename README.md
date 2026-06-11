# RetroSync

Aplicação desktop que sincroniza automaticamente saves, savestates e configurações de
emuladores de retrogames (PPSSPP, PCSX2) com o Google Drive. Selecione a pasta raiz do
emulador — autenticação, estrutura de pastas no Drive, detecção de arquivos e
sincronização em background acontecem sozinhas.

## Stack

| Camada       | Tecnologia                                        |
| ------------ | ------------------------------------------------- |
| Runtime      | Tauri v2                                          |
| Frontend     | React + TypeScript + Vite                         |
| Backend/Core | Rust (tokio, reqwest, rusqlite, keyring, sysinfo) |

Toda a lógica de negócio vive no backend Rust (`src-tauri/`); o frontend React (`src/`)
apenas dispara comandos (`invoke`) e reage a eventos (`emit`) do Tauri.

## Pré-requisitos (Windows)

1. **Rust** — instale via [rustup](https://rustup.rs/) (toolchain MSVC padrão);
2. **Microsoft C++ Build Tools** — workload "Desktop development with C++"
   ([Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/));
3. **WebView2** — já incluso no Windows 10/11 atualizados;
4. **Node.js** ≥ 20 LTS.

Referência completa: [tauri.app/start/prerequisites](https://tauri.app/start/prerequisites/).

> **Nota sobre WSL**: edite o código onde preferir, mas rode `npm run tauri dev`/`build`
> no Windows nativo (PowerShell). Build dentro do WSL gera binário Linux, exige as libs
> webkit2gtk e sofre com I/O lento em `/mnt/c`.

## Setup

```bash
npm install          # dependências do frontend
npm run tauri dev    # app em modo desenvolvimento (compila o Rust na 1ª vez)
npm run tauri build  # gera o instalador/binário de produção
```

### Variáveis de ambiente

| Variável                     | Uso                                                                                       |
| ---------------------------- | ----------------------------------------------------------------------------------------- |
| `RETROSYNC_GOOGLE_CLIENT_ID` | Client ID OAuth2 (Desktop) do Google Cloud Console, lido em build-time. Nunca commitá-lo. |

## Qualidade de código

```bash
npm run lint                       # ESLint (frontend)
npm run format                     # Prettier (frontend)
cargo fmt --manifest-path src-tauri/Cargo.toml      # rustfmt
cargo clippy --manifest-path src-tauri/Cargo.toml   # lints Rust
cargo test --manifest-path src-tauri/Cargo.toml     # testes unitários Rust
```

## Estrutura do projeto

```
src/                  # Frontend React + TypeScript
├── components/       # Componentes de UI
├── hooks/            # Hooks (eventos Tauri, estado de sync)
├── lib/ipc.ts        # Wrappers tipados de invoke()
└── types/ipc.ts      # Espelho TS das structs Rust da boundary
src-tauri/src/        # Backend Rust
├── commands.rs       # Comandos Tauri (boundary)
├── events.rs         # Nomes de eventos emitidos ao frontend
├── constants.rs      # Pastas do Drive, chaves keyring (sem magic strings)
├── auth/             # OAuth2 PKCE + keyring
├── drive/            # Cliente Google Drive API (reqwest + retry)
├── emulator/         # Perfis e detecção (PPSSPP, PCSX2)
├── storage/          # SQLite: manifest de sync, fila offline
├── sync/             # SyncEngine (diff, conflitos, upload/download)
└── watcher/          # Monitor de processos dos emuladores (sysinfo)
```

## Logs

Logs de operação ficam no diretório de logs do app
(`%LOCALAPPDATA%\com.retrosync.app\logs` no Windows), com rotação diária.
