# 02 — Scaffolding do Projeto

**Commit**: `dc3ddf7` — *feat: scaffolding Tauri v2 + React/TS com módulos Rust, lint e tipos compartilhados*

## Objetivo

Gerar o projeto base, configurar todas as dependências, estabelecer a separação de
módulos do backend e o tooling de qualidade do frontend, e definir os primeiros tipos
compartilhados na boundary.

## O que foi feito

1. **Projeto base** gerado com `create-tauri-app` (template `react-ts`), depois ajustado:
   nomes para `retro-sync`/`RetroSync`, identifier `com.retrosync.app`, janela 900×650
   (mín. 720×480), e remoção do exemplo "greet" do template.
2. **Dependências Rust** declaradas no `Cargo.toml` (ver tabela abaixo).
3. **6 módulos de domínio** criados com doc de responsabilidade e os tipos que cruzam a
   boundary: `auth`, `drive`, `emulator`, `storage`, `sync`, `watcher`. Além de
   `commands`, `events`, `constants`, `error`, `state`.
4. **Tooling do frontend**: TypeScript strict, ESLint 9 (flat config) + typescript-eslint
   + react-hooks + eslint-config-prettier, Prettier.
5. **Logging** com `tracing`: stdout (dev) + arquivo com rotação diária no diretório de
   logs do app.
6. **Tipos compartilhados**: `src/types/ipc.ts` espelha as structs Rust; `src/lib/ipc.ts`
   é o único lugar que chama `invoke()`.

## Dependências do backend (`Cargo.toml`)

| Crate | Uso |
| --- | --- |
| `tauri` (features `tray-icon`, `image-png`) | Runtime; tray já habilitado para o Passo 7 |
| `tauri-plugin-dialog` | Seletor de pasta nativo (Passo 7) |
| `tauri-plugin-notification` | Notificações nativas (Passo 7) |
| `serde`, `serde_json` | Serialização na boundary e no manifest |
| `thiserror` | Tipo de erro unificado |
| `tokio` | Runtime async |
| `reqwest` (rustls) | HTTP para a API do Drive |
| `rusqlite` (bundled) | SQLite local |
| `keyring` | Refresh token no keychain do SO |
| `sysinfo` | Monitoramento de processos (Passo 6) |
| `chrono` | Timestamps RFC 3339 ↔ epoch ms |
| `url` | Construção/parsing de URLs OAuth |
| `open` | Abrir o navegador no fluxo OAuth |
| `futures` | `buffer_unordered` para concorrência de transferências |
| `filetime` | Aplicar mtime do Drive em arquivos baixados |
| `sha2`, `rand`, `base64` | PKCE (challenge S256, verifier, state) |
| `tracing*` | Logging estruturado com rotação diária |

> `tempfile` é dev-dependency, usado nos testes que montam árvores de diretório reais.

## Decisões relevantes

- **Frontend "burro", backend "inteligente"**: evita estado duplicado, mantém tokens
  fora do JS, e torna o frontend substituível. Ver
  [Decisões técnicas](./decisoes-tecnicas.md#frontend-burro-backend-inteligente).
- **`rustls` em vez de OpenSSL** no reqwest: TLS puro Rust, mesma stack em todos os SOs,
  sem dependência de biblioteca de sistema.
- **Versões estáveis conhecidas** (`rusqlite 0.32`, `keyring 3`, `sysinfo 0.33`) em vez
  das majors mais novas, para compatibilidade com o código dos passos seguintes.

## Configuração de credenciais (`build.rs` + `.env`)

**Commit**: `637d911` — *chore: carregar credenciais OAuth de arquivo .env em build-time*

O `src-tauri/build.rs` lê o `.env` da raiz e reexporta as variáveis `RETROSYNC_*` via
`cargo:rustc-env`, tornando-as visíveis ao `option_env!` do código:

- Funciona para `tauri dev` **e** para o build de produção.
- Variáveis definidas no shell têm precedência sobre o `.env`.
- Editar o `.env` dispara rebuild (`cargo:rerun-if-changed`).
- `.env` está no `.gitignore`; `.env.example` documenta o formato.

Ver detalhes em [03 — Autenticação](./03-autenticacao.md#configuração-de-credenciais).

## Como validar

```bash
npm run lint && npm run build                       # frontend
cargo test --manifest-path src-tauri/Cargo.toml     # backend (44 testes hoje)
npm run tauri dev                                   # abre a janela "RetroSync"
```

A janela deve exibir "backend pronto (vX.Y.Z)" — confirma a boundary `invoke` → Rust
funcionando de ponta a ponta (comando `health_check`).

## Nota sobre o ambiente de desenvolvimento

O repositório vive em `/mnt/c` sob WSL2, mas o **alvo é Windows**. Build do Tauri no WSL
gera binário Linux e exige libs `webkit2gtk`; compilar Rust em `/mnt/c` é lento (I/O 9p).
Recomendação: editar onde preferir, mas rodar `npm run tauri dev`/`build` no Windows
nativo (PowerShell). Ver [Riscos](./riscos.md#9-ambiente-de-dev-wsl2).
