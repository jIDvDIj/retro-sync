# 15 — Proxy Cloudflare Worker para OAuth

**Commits**: _(pendente de commit)_

Implementa a [FEATURE-005](./features/feature-005-cloudflare-worker-proxy.md): um Cloudflare
Worker passa a intermediar a troca de tokens com o Google, de modo que o `client_secret`
**nunca** é compilado no binário distribuído nem entra no CI.

## Objetivo

Tirar o `client_secret` do binário de release. Ele passa a viver apenas como secret cifrado
no Cloudflare; o app conhece somente a URL pública do Worker e um `PROXY_SECRET` compartilhado.

## Arquivos

| Arquivo | Conteúdo |
| --- | --- |
| `worker/src/index.js` | Worker: endpoints `POST /token` e `POST /refresh`, valida `X-Proxy-Secret` |
| `worker/wrangler.toml` | Config do Worker — `CLIENT_ID` como var pública (**git-ignored**, ver abaixo) |
| `src-tauri/src/auth/oauth.rs` | `OAuthConfig` ganha `token_proxy_url` + `proxy_secret`; `exchange_code`/`refresh_access_token` roteiam pelo Worker |
| `.env.example` | Documenta `RETROSYNC_TOKEN_PROXY_URL` e `RETROSYNC_PROXY_SECRET` |
| `.github/workflows/release.yml` | Injeta as 3 envs de produção a partir de repository secrets |
| `.gitignore` | Ignora `worker/wrangler.toml` |

## Arquitetura do fluxo

```
App (Tauri/Rust)                  Cloudflare Worker            Google
────────────────                  ─────────────────            ──────
1. PKCE + loopback                                             tela de
   abre navegador  ───────────────────────────────────────►   consentimento
   redirect_uri = http://127.0.0.1:<porta-efêmera>
        │
        │ recebe ?code=...&state=...  (validado anti-CSRF)
        ▼
2. POST {proxy}/token             segura CLIENT_SECRET
   header X-Proxy-Secret  ──────► troca code→token   ───────►  POST /token
   { code, code_verifier,         (grant_type=
     redirect_uri }                authorization_code)
        ◄───────────────────────  { access_token, refresh_token, ... }

3. refresh: POST {proxy}/refresh  idem com               ───►  POST /token
   { refresh_token }              grant_type=refresh_token
```

O **sync em si não passa pelo Worker** — só a troca/refresh de token. As chamadas à Drive API
saem direto do app com o `access_token`.

## O Worker (`worker/src/index.js`)

| Endpoint | Recebe do app | Faz no Google |
| --- | --- | --- |
| `POST /token` | `{ code, code_verifier, redirect_uri }` | `grant_type=authorization_code` com `client_id` + `client_secret` |
| `POST /refresh` | `{ refresh_token }` | `grant_type=refresh_token` com `client_id` + `client_secret` |

Defesas implementadas (além da proposta original da FEATURE-005):

- **`X-Proxy-Secret`** — todo request precisa trazer o header com valor igual ao secret
  `PROXY_SECRET`; caso contrário, `401`. Impede terceiros de esgotar a quota do Worker.
- **Validação de `redirect_uri`** — em `/token`, o `redirect_uri` precisa começar com
  `http://127.0.0.1:`. Casa com o loopback que o app usa (`LOOPBACK_HOST` em `oauth.rs`) e
  impede que o Worker seja usado para trocar códigos de outros fluxos.
- Apenas `POST`; corpo malformado → `400`; rota desconhecida → `404`.

### Credenciais no Cloudflare

| Nome | Tipo | Como configurar |
| --- | --- | --- |
| `CLIENT_ID` | var pública | `[vars]` no `wrangler.toml` |
| `CLIENT_SECRET` | secret cifrado | `wrangler secret put CLIENT_SECRET` |
| `PROXY_SECRET` | secret cifrado | `wrangler secret put PROXY_SECRET` |

Deploy: `wrangler deploy` (de dentro de `worker/`).

> **`worker/wrangler.toml` é git-ignored.** Só `worker/src/index.js` é versionado. Quem for
> reimplantar o Worker precisa recriar o `wrangler.toml` com o `name`, `main`,
> `compatibility_date` e o `[vars] CLIENT_ID`.

## Mudanças no Rust (`oauth.rs`)

`OAuthConfig::from_env()` lê quatro variáveis (todas via `option_env!` em build-time, com
fallback para `std::env::var` em runtime/dev):

| Variável | Papel |
| --- | --- |
| `RETROSYNC_GOOGLE_CLIENT_ID` | obrigatória; sem ela, `from_env` retorna `None` |
| `RETROSYNC_TOKEN_PROXY_URL` | URL do Worker — quando presente, ativa o modo proxy |
| `RETROSYNC_PROXY_SECRET` | enviado no header `X-Proxy-Secret` |
| `RETROSYNC_GOOGLE_CLIENT_SECRET` | **fallback de dev**: chama o Google direto, sem Worker |

A seleção do caminho (`exchange_code` / `refresh_access_token`):

| Configuração | Comportamento |
| --- | --- |
| `token_proxy_url` presente | Roteia para o Worker (**produção**) |
| só `client_secret` presente | Chama o token endpoint do Google direto (**dev local**) |
| nenhum dos dois | App sobe, mas a troca de token falha com erro de auth |

## Tipo de cliente OAuth — precisa ser **Desktop app**

O app usa **redirect loopback** (`http://127.0.0.1:<porta-efêmera>`), gerado em tempo de
execução. Só o tipo de cliente **Desktop app** (instalado) aceita loopback em porta arbitrária
— clientes **Web application** exigem que cada `redirect_uri` seja registrado exatamente
(host + porta), o que é incompatível com a porta efêmera.

O Worker **não** é um redirect URI e não precisa estar registrado no Google: ele apenas executa
a troca server-side com o `client_secret` que ele guarda. Ou seja, o proxy Worker é totalmente
compatível com o cliente Desktop — não há motivo para trocar o tipo do cliente por causa dele.

Ver [Decisões técnicas](./decisoes-tecnicas.md#proxy-worker-esconde-o-client_secret).

## O que vai para o GitHub Actions

O `release.yml` injeta três **repository secrets** no step de build (Settings → Secrets and
variables → Actions):

| Repository secret | Valor |
| --- | --- |
| `RETROSYNC_GOOGLE_CLIENT_ID` | mesmo `CLIENT_ID` do `wrangler.toml` |
| `RETROSYNC_TOKEN_PROXY_URL` | `https://retrosync-auth.....` |
| `RETROSYNC_PROXY_SECRET` | mesmo valor passado em `wrangler secret put PROXY_SECRET` |

> **`RETROSYNC_GOOGLE_CLIENT_SECRET` não vai para o CI.** É exatamente o ponto da feature —
> o secret fica só no Cloudflare. Localmente, ele segue disponível como fallback de dev.

### Ressalva de segurança honesta

O `PROXY_SECRET` é embutido no binário (`option_env!`), então é **extraível** de uma release
distribuída (`strings`, descompilador). Ele não é um segredo forte depois de publicado — serve
para barrar abuso casual do Worker e permitir rotação, não para deter um atacante determinado.
A proteção real é o `client_secret`, que nunca sai do Cloudflare. Para um app pessoal/de nicho
o trade-off é adequado; para endurecer, seria preciso atestação real do cliente (fora de escopo).

## Como testar

Produção (binário de release): conectar a conta Google deve funcionar sem o `client_secret`
no binário. Verificar com `strings` no executável que o secret do Google **não** aparece.

Dev local sem Worker (fallback), no PowerShell:

```powershell
$env:RETROSYNC_GOOGLE_CLIENT_ID = "seu-client-id"
$env:RETROSYNC_GOOGLE_CLIENT_SECRET = "seu-secret"
npm run tauri dev
```

Dev apontando para o Worker:

```powershell
$env:RETROSYNC_GOOGLE_CLIENT_ID   = "seu-client-id"
$env:RETROSYNC_TOKEN_PROXY_URL    = "https://retrosync-auth...."
$env:RETROSYNC_PROXY_SECRET       = "mesmo-valor-do-wrangler-secret"
npm run tauri dev
```

Em ambos: **Conectar ao Google Drive** → autorizar → a UI exibe o e-mail; fechar e reabrir
mantém a conexão (refresh token persistido, renovado via Worker).
