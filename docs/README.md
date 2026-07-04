# Documentação do RetroSync

Documentação técnica do RetroSync — aplicação desktop (Tauri v2 + Rust + React/TS)
que sincroniza automaticamente saves, savestates e configurações de emuladores de
retrogames com o Google Drive.

> Esta pasta documenta **o que** foi construído, **por que** cada decisão foi tomada
> e **como** as peças se encaixam. Para a visão geral do produto, veja o
> [`README.md`](../README.md) na raiz; para começar a desenvolver, o
> [Guia do Desenvolvedor](./guia-desenvolvedor.md).

## Índice

| Documento | Conteúdo |
| --- | --- |
| [Guia do Desenvolvedor](./guia-desenvolvedor.md) | Onboarding: pré-requisitos, setup, credenciais, rodar/buildar, qualidade, fixes WSL |
| [01 — Arquitetura](./01-arquitetura.md) | Diagrama, módulos, fluxo de dados, gatilhos de sync, estrutura de pastas |
| [02 — Scaffolding](./02-scaffolding.md) | Projeto base, dependências, tooling, separação de módulos |
| [03 — Autenticação](./03-autenticacao.md) | OAuth2 + PKCE, keyring, renovação de token, configuração via `.env` |
| [04 — Detecção de emuladores](./04-deteccao-emuladores.md) | Perfis PPSSPP/PCSX2, `detect_emulator`, marcadores de filesystem |
| [05 — Sincronização](./05-sincronizacao.md) | SyncEngine, manifest SQLite, cliente Drive, resolução de conflito, fila offline |
| [06 — Monitoramento de processos](./06-monitoramento-processos.md) | Process watcher (`sysinfo`), debounce, gatilhos de sync por abertura/fechamento |
| [07 — UI e system tray](./07-ui-system-tray.md) | Tela principal React, bandeja, sync de despedida, notificações nativas |
| [08 — Login e nome do dispositivo](./08-login-dispositivo.md) | Aviso de permissão, nome do dispositivo, infraestrutura `app_settings` |
| [09 — Configurações: dispositivo](./09-configuracoes-dispositivo.md) | Modal de configurações, edição do nome sem refazer login |
| [10 — Categorias de sync por emulador](./10-categorias-sync.md) | Toggles saves/savestates/config por emulador; filtro no engine |
| [11 — Sync automático por gatilho](./11-gatilhos-automaticos.md) | Ligar/desligar startup, emulator-start e emulator-stop |
| [12 — Nível de notificações nativas](./12-nivel-notificacoes.md) | all / errors_only / none; gating de erro, conclusão e detecção |
| [13 — Primeiro sync: Drive vence + backup](./13-primeiro-sync-backup.md) | BUG-001 — backup local antes de sobrescrever no primeiro sync |
| [14 — Resolução de conflito](./14-resolucao-conflito.md) | BUG-002 — conflito explícito, bloqueio por emulador e modal de resolução |
| [15 — Proxy Cloudflare Worker (OAuth)](./15-proxy-worker-oauth.md) | FEATURE-005 — Worker esconde o `client_secret`; o que vai para o GitHub Actions |
| [16 — Internacionalização (i18n)](./16-internacionalizacao.md) | i18next/react-i18next, inglês padrão, seletor de idioma, tradução de erros por `code`+`detail` |
| [Referência — Boundary IPC](./referencia-ipc.md) | Catálogo de comandos, eventos e tipos compartilhados Rust ↔ TS |
| [Decisões técnicas](./decisoes-tecnicas.md) | Registro consolidado das decisões e seus trade-offs |
| [Riscos técnicos](./riscos.md) | Riscos identificados e mitigações |
| [Distribuição pública e confiança](./distribuicao-publica.md) | SmartScreen, OAuth Google, GitHub Attestations (descartado), Microsoft Store |
| [17 — Suporte Android](./17-suporte-android.md) | Fases 3/5/6/7: scaffolding APK, OAuth deep link, SecretStore, gatilhos lifecycle e UI mobile |
| [Portabilidade multiplataforma](./multiplataforma-checklist.md) | Checklist faseado para Windows/Linux/macOS/Steam Deck/Android/iOS; abstração de storage, OAuth e keyring mobile |
| [Como adicionar por plataforma](./plataformas-como-adicionar.md) | Guia prático: comandos gerais, desktop-only, mobile-only; módulos `platform/`; dependências condicionais; checklist |
| [Bugs](./bugs/) | Bugs documentados com causa raiz e soluções consideradas — inclui [BUG-005](./bugs/bug-005-validacao-filesystem-mobile.md): validações `PathBuf` incompatíveis com URIs SAF no mobile |
| [Features](./features/) | Propostas de funcionalidades futuras — identificação de jogos, configurações, perfis-como-dados, batch upload, proxy Worker, otimização de performance do sync |
| [Referência — Plataformas (RomM)](./referencia-plataformas-romm.md) | Análise do modelo de plataformas do RomM: `UniversalPlatformSlug`, detecção por pasta, organização de saves por console e o que trazer para o RetroSync |
| [Setup — Melhorias de tooling](./setup-melhorias-scripts.md) | Passo a passo das configurações pendentes das melhorias de scripts (hook de commit, Codecov, cargo-deny, lint i18n, relnotes) |

### Portabilidade multiplataforma (Android/iOS/Linux/macOS/Steam Deck)

| Fase | Descrição | Doc | Status |
| --- | --- | --- | --- |
| 0 | Código compilável para mobile (`#[cfg(desktop)]` em tray/autostart/watcher) | [checklist](./multiplataforma-checklist.md) | ✅ Concluído |
| 2 | Abstração de storage (trait `LocalStorage` + `FileLoc`); todo o I/O do engine isolado | [checklist](./multiplataforma-checklist.md) | ✅ Concluído |
| 3 | Scaffolding Android: SDK/NDK, `tauri android init`, APK debug em device físico | [17](./17-suporte-android.md) | ✅ Concluído |
| 5 | OAuth via Worker redirect: client Web app único, `/oauth/callback` no Worker, deep link | [17](./17-suporte-android.md) | ✅ Concluído |
| 6 | `SecretStore` trait: `KeyringStore` (desktop) / `SqliteSecretStore` (mobile) | [17](./17-suporte-android.md) | ✅ Concluído |
| 7 | Gatilhos lifecycle (`resume`/`pause`) + UI mobile (`AddEmulatorModal`, `SettingsModal`) | [17](./17-suporte-android.md) | ✅ Concluído |
| 8 | APK assinado (`retrosync.jks`) + job `android` no CI; secrets GitHub pendentes | [17](./17-suporte-android.md) | 🟡 Secrets pendentes |
| 1 | Desktop: descoberta Steam Deck/Flatpak (feito) + empacotamento Flatpak/macOS (precisa de máquina Linux/macOS) | [checklist](./multiplataforma-checklist.md) | 🟡 Em andamento |
| 4 | Storage mobile: interface Rust↔plugin (`MobileStorage`) pronta; `StoragePlugin.kt` escrito, validação em device pendente | [checklist](./multiplataforma-checklist.md) | 🟡 Validação pendente |

## Visão geral em uma frase

O usuário conecta sua conta Google, aponta a pasta raiz de um emulador, e o RetroSync
detecta o emulador, cria a estrutura `RetroSync/<Emulador>/{saves,savestates,config}`
no Drive e mantém os arquivos sincronizados nos dois sentidos — resolvendo conflitos
pelo arquivo mais recente e nunca apagando nada.

## Princípios de design

1. **Backend "inteligente", frontend "burro"** — toda a lógica vive no Rust; o React
   só dispara comandos e reage a eventos.
2. **Agnosticismo a emuladores no núcleo** — o SyncEngine opera sobre caminhos, não
   conhece PPSSPP nem PCSX2.
3. **Segurança por padrão** — tokens nunca cruzam a boundary; credenciais no keychain
   do SO; escopo OAuth mínimo (`drive.file`).
4. **Não-destrutivo** — nunca deleta arquivos no Drive.
5. **Offline-first** — falhas de rede viram pendências persistidas, não erros fatais.
6. **Sem magic strings** — nomes de pastas, chaves e parâmetros são constantes nomeadas.
