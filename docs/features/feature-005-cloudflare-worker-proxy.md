# FEATURE-005 — Proxy Cloudflare Worker para credenciais OAuth

**Status:** ✅ implementada — ver [15 — Proxy Cloudflare Worker (OAuth)](../15-proxy-worker-oauth.md)  
**Componentes afetados:** `src-tauri/src/auth/oauth.rs`, `src-tauri/build.rs`, `.env.example`, `worker/`

---

## Problema atual

O `client_secret` do Google OAuth é compilado no binário via `option_env!` em
`src-tauri/src/auth/oauth.rs` (lido de `RETROSYNC_GOOGLE_CLIENT_SECRET` pelo `build.rs`).
Qualquer pessoa que baixar o app distribuído pode extrair o secret com ferramentas como
`strings` ou um descompilador — e usá-lo para fazer chamadas ao token endpoint do Google em
nome do app, consumindo quota ou tentando abusar das credenciais.

O Google reconhece que secrets de apps desktop não são verdadeiramente confidenciais, mas
para distribuição ampla (> 100 usuários, necessidade de verificação OAuth) o risco de abuso
aumenta proporcionalmente ao número de downloads.

---

## Solução proposta: Cloudflare Worker como proxy do token endpoint

Um Worker minúsculo (~30 linhas de JavaScript) fica entre o app e o Google. Ele guarda
o `CLIENT_SECRET` como **secret cifrado do Cloudflare** — nunca exposto no binário nem no
repositório. O `CLIENT_ID` permanece compilado no binário (é público por natureza e necessário
para construir a URL de autorização antes de qualquer chamada ao Worker). O app distribuído
passa a conhecer a URL pública do Worker em vez do secret.

### O que o Worker expõe

| Endpoint | Recebe do app | Faz no Google |
|----------|---------------|---------------|
| `POST /token` | `{ code, code_verifier, redirect_uri }` | `grant_type=authorization_code` com client_id + secret |
| `POST /refresh` | `{ refresh_token }` | `grant_type=refresh_token` com client_id + secret |

---

## Código do Worker

```javascript
const GOOGLE_TOKEN = "https://oauth2.googleapis.com/token";

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (request.method !== "POST") {
      return new Response("Method Not Allowed", { status: 405 });
    }

    const body = await request.json();
    let form;

    if (url.pathname === "/token") {
      form = new URLSearchParams({
        grant_type: "authorization_code",
        client_id: env.CLIENT_ID,
        client_secret: env.CLIENT_SECRET,
        code: body.code,
        code_verifier: body.code_verifier,
        redirect_uri: body.redirect_uri,
      });
    } else if (url.pathname === "/refresh") {
      form = new URLSearchParams({
        grant_type: "refresh_token",
        client_id: env.CLIENT_ID,
        client_secret: env.CLIENT_SECRET,
        refresh_token: body.refresh_token,
      });
    } else {
      return new Response("Not Found", { status: 404 });
    }

    const response = await fetch(GOOGLE_TOKEN, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: form,
    });

    const data = await response.json();
    return Response.json(data, { status: response.status });
  },
};
```

O secret é configurado via CLI do Cloudflare. O `CLIENT_ID` vai como variável comum no `wrangler.toml`:

```bash
wrangler secret put CLIENT_SECRET
```

**Free tier:** 100 000 requisições/dia, sem cartão de crédito. Suficiente para qualquer
volume de logins e refreshes de token (o sync em si vai direto do app para a Drive API,
não passa pelo Worker).

---

## Mudanças no Rust

### `src-tauri/src/auth/oauth.rs`

`OAuthConfig` troca `client_secret` por `token_proxy_url`:

```rust
#[derive(Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    /// URL do proxy que guarda o client_secret (ex.: Cloudflare Worker).
    /// Se ausente, tenta usar client_secret direto (modo desenvolvimento).
    pub token_proxy_url: Option<String>,
    pub client_secret: Option<String>, // apenas para dev sem Worker
}

impl OAuthConfig {
    pub fn from_env() -> Option<Self> {
        let client_id = option_env!("RETROSYNC_GOOGLE_CLIENT_ID")
            .map(str::to_owned)
            .or_else(|| std::env::var("RETROSYNC_GOOGLE_CLIENT_ID").ok())?;
        let token_proxy_url = option_env!("RETROSYNC_TOKEN_PROXY_URL")
            .map(str::to_owned)
            .or_else(|| std::env::var("RETROSYNC_TOKEN_PROXY_URL").ok());
        let client_secret = option_env!("RETROSYNC_GOOGLE_CLIENT_SECRET")
            .map(str::to_owned)
            .or_else(|| std::env::var("RETROSYNC_GOOGLE_CLIENT_SECRET").ok());
        Some(Self { client_id, token_proxy_url, client_secret })
    }
}
```

`exchange_code` e `refresh_access_token` passam a rotear para o Worker quando
`token_proxy_url` está presente, ou usam o fluxo direto com `client_secret` como fallback
(compatibilidade com builds de desenvolvimento).

---

## Mudanças no `.env` / build

```diff
 RETROSYNC_GOOGLE_CLIENT_ID=...
-RETROSYNC_GOOGLE_CLIENT_SECRET=...
+RETROSYNC_TOKEN_PROXY_URL=https://retrosync-auth.<usuario>.workers.dev
```

O `RETROSYNC_GOOGLE_CLIENT_SECRET` continua suportado para desenvolvimento local sem Worker.
Builds de release (CI) usam apenas `RETROSYNC_TOKEN_PROXY_URL`.

---

## Compatibilidade e fallback

| Variável presente | Comportamento |
|-------------------|---------------|
| `RETROSYNC_TOKEN_PROXY_URL` | Usa o Worker (modo produção) |
| `RETROSYNC_GOOGLE_CLIENT_SECRET` | Chama o Google diretamente (modo dev) |
| Nenhuma | App inicia, mas troca de token falha com erro de autenticação |

---

## Quando implementar

Antes de distribuição ampla (> 100 usuários). Acima desse limite o Google bloqueia novos
logins até a verificação OAuth ser concluída — e credenciais expostas no binário se tornam
risco real de abuso. A implementação é pequena (< 50 linhas de Rust alteradas + o Worker) e
independe de outras features.
