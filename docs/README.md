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
| [Referência — Boundary IPC](./referencia-ipc.md) | Catálogo de comandos, eventos e tipos compartilhados Rust ↔ TS |
| [Decisões técnicas](./decisoes-tecnicas.md) | Registro consolidado das decisões e seus trade-offs |
| [Riscos técnicos](./riscos.md) | Riscos identificados e mitigações |
| [Distribuição pública e confiança](./distribuicao-publica.md) | SmartScreen, OAuth Google, GitHub Attestations (descartado), Microsoft Store |
| [Bugs](./bugs/) | Bugs documentados com causa raiz e soluções consideradas |
| [Features](./features/) | Propostas de funcionalidades futuras — identificação de jogos, configurações, perfis-como-dados, batch upload, proxy Worker, otimização de performance do sync |

## Estado atual

| Passo | Descrição | Status |
| --- | --- | --- |
| 1 | Arquitetura e decisões técnicas | ✅ Concluído |
| 2 | Scaffolding do projeto | ✅ Concluído (`dc3ddf7`) |
| 3 | Autenticação Google OAuth2 | ✅ Concluído (`0ea3a86`, `637d911`) |
| 4 | Detecção de emuladores | ✅ Concluído (`d5a1da3`) |
| 5 | Módulo de sincronização | ✅ Concluído (`f3639fc`) |
| 6 | Monitoramento de processos | ✅ Concluído (`60a0ae6`) |
| 7 | UI e system tray | ✅ Concluído (`bbc8ac7`) |

**v1.0 funcionalmente completa** — os 7 passos concluídos. **Lint/format**: ESLint, Prettier,
rustfmt e clippy limpos.

### v1.1 — Configurações e segurança de dados (FEATURE-002, BUG-001, BUG-002)

| Passo | Descrição | Doc | Status |
| --- | --- | --- | --- |
| 1 | Login + nome do dispositivo | [08](./08-login-dispositivo.md) | ✅ Concluído |
| 2 | Nome do dispositivo nas configurações | [09](./09-configuracoes-dispositivo.md) | ✅ Concluído |
| 3 | Categorias de sync por emulador | [10](./10-categorias-sync.md) | ✅ Concluído |
| 4 | Sync automático por gatilho | [11](./11-gatilhos-automaticos.md) | ✅ Concluído |
| 5 | Nível de notificações nativas | [12](./12-nivel-notificacoes.md) | ✅ Concluído |
| 6 | Primeiro sync (Drive vence + backup) | [13](./13-primeiro-sync-backup.md) | ✅ Concluído |
| 7 | Resolução de conflito | [14](./14-resolucao-conflito.md) | ✅ Concluído |

**v1.1 completa** — os 5 passos da FEATURE-002 + BUG-001 + BUG-002. **72 testes** unitários Rust
passando; ESLint, Prettier, rustfmt e clippy limpos.

### Segurança — proxy de credenciais (FEATURE-005)

| Item | Descrição | Doc | Status |
| --- | --- | --- | --- |
| Proxy Worker | Cloudflare Worker esconde o `client_secret`; só `CLIENT_ID`/`TOKEN_PROXY_URL`/`PROXY_SECRET` no CI | [15](./15-proxy-worker-oauth.md) | ✅ Concluído |

### Correções pós-v1.1

| Bug | Descrição | Doc | Status |
| --- | --- | --- | --- |
| BUG-003 | Troca do `root_path` de um emulador já configurado zerava só o perfil, não o manifest → sobrescrita do Drive por instalação mais antiga | [bug-003](./bugs/bug-003-troca-de-caminho-do-emulador.md) | ✅ Resolvido |
| BUG-004 | Saves independentes de dispositivos diferentes eram sobrescritos no primeiro sync sem conflito → `device_id` estável no keyring, conflito explícito quando a origem é outro dispositivo | [bug-004](./bugs/bug-004-conflito-entre-dispositivos-primeiro-sync.md) | ✅ Resolvido |

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
4. **Não-destrutivo** — a v1.0 nunca deleta arquivos no Drive.
5. **Offline-first** — falhas de rede viram pendências persistidas, não erros fatais.
6. **Sem magic strings** — nomes de pastas, chaves e parâmetros são constantes nomeadas.
