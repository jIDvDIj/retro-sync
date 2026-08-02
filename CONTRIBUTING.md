# Contribuindo com o RetroSync

Obrigado pelo interesse em contribuir. Antes de abrir um PR, leia isto.

## Fluxo de contribuição

1. Faça um fork do repositório e trabalhe numa branch própria.
2. Abra um Pull Request contra a `main`. Todo PR passa por CI (lint, format,
   testes Rust em Windows/Linux, clippy, cargo-audit, cargo-deny, cobertura)
   e exige aprovação do mantenedor antes de ser mesclado — inclusive PRs de
   colaboradores frequentes.
3. Siga [Conventional Commits](https://www.conventionalcommits.org/) nas
   mensagens (`tipo(escopo): descrição`), em inglês.
4. Rode `sh scripts/install-hooks.sh` uma vez após clonar, para instalar os
   hooks de validação de commit.

## Credenciais do Google OAuth — use as suas, não peça as de produção

O RetroSync se autentica no Google Drive via OAuth2 + PKCE. Para rodar e
testar o fluxo de login **localmente**, você precisa do seu **próprio**
OAuth Client ID do Google Cloud Console — nunca peça ou compartilhe o
`client_id`/`client_secret` de produção do projeto.

Passos:

1. Crie um projeto no [Google Cloud Console](https://console.cloud.google.com/).
2. Configure a tela de consentimento OAuth (tipo "Externo" está OK para
   testes — o escopo usado, `drive.file`, é não-sensível e não exige
   verificação do Google).
3. Crie uma credencial OAuth do tipo **Desktop app**.
4. Copie `.env.example` para `.env` na raiz do repositório e preencha
   `RETROSYNC_GOOGLE_CLIENT_ID` e `RETROSYNC_GOOGLE_CLIENT_SECRET` com os
   valores do seu client de teste. Não configure `RETROSYNC_TOKEN_PROXY_URL`
   nem `RETROSYNC_PROXY_SECRET` localmente — essas variáveis apontam para o
   Worker de produção e não são necessárias fora dele.
5. **Nunca** faça commit do seu `.env` — ele já está no `.gitignore`.

Se seu PR não envolve o fluxo de autenticação, você pode rodar o app sem
essas variáveis: ele inicia normalmente, só a conexão ao Drive fica
indisponível.

## O que NÃO fazer em um PR

- Não altere `.github/workflows/*.yml`, `src-tauri/build.rs`, `worker/`,
  `package.json` ou `Cargo.toml` "de passagem" dentro de um PR sobre outra
  coisa — essas áreas exigem revisão extra por afetarem o pipeline de
  build/release e o manuseio de credenciais.
- Não commite arquivos gerados localmente (`src-tauri/gen/android/`
  contém partes versionadas e partes ignoradas de propósito — veja o
  `.gitignore` antes de forçar a inclusão de algo).
- Não inclua segredos, tokens ou credenciais reais em código, testes ou
  mensagens de commit, mesmo de exemplo.
