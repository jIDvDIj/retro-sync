# 03 — Autenticação (Google OAuth2)

**Commits**: `0ea3a86` — *feat: autenticação Google OAuth2 com PKCE, keyring e tela de
conexão*; `637d911` — *chore: carregar credenciais OAuth de arquivo .env em build-time*

## Objetivo

Permitir que o usuário conecte sua conta Google de forma segura, sem expor client secret
nem tokens, com renovação de access token transparente.

## Arquivos

| Arquivo | Conteúdo |
| --- | --- |
| `auth/oauth.rs` | Fluxo PKCE, loopback server, troca/refresh de tokens, userinfo |
| `auth/token_store.rs` | Persistência do refresh token no keyring (save/load/clear) |
| `auth/mod.rs` | `AuthManager` — porta de entrada (connect, status, disconnect, access_token) |
| `commands.rs` | `connect_google_drive`, `get_auth_status`, `disconnect_google_drive` |
| `components/ConnectDrive.tsx` | Tela de conexão em React |

## Fluxo OAuth2 com PKCE (RFC 8252 — apps nativos)

```
1. Gera code_verifier (aleatório) + code_challenge = BASE64URL(SHA256(verifier))
2. Sobe um listener TCP em 127.0.0.1:porta-efêmera
3. Abre o navegador do sistema na tela de consentimento do Google
   (com client_id, redirect_uri=loopback, scope, code_challenge S256, state aleatório)
4. Usuário autoriza → Google redireciona para 127.0.0.1:porta?code=...&state=...
5. Valida o state (anti-CSRF), extrai o authorization code
6. Troca code + code_verifier por tokens no token endpoint
7. Salva o refresh token no keyring; mantém o access token em memória
```

Detalhes de robustez:

- **Timeout de 5 minutos** no fluxo interativo.
- **Requisições alheias ignoradas**: o listener descarta chamadas sem `code`/`error`
  (ex.: o `favicon.ico` que o navegador pede) sem encerrar o fluxo.
- **Páginas de retorno** (sucesso/erro) em HTML para o usuário fechar a aba.

## Escopo OAuth

```
openid email https://www.googleapis.com/auth/drive.file
```

- `drive.file` — **não-sensível**: o app só enxerga arquivos/pastas que ele mesmo criou.
  Evita o processo de verificação restrita do Google e reduz a superfície de risco.
- `openid email` — para exibir a conta conectada na UI.

Ver [Decisões técnicas](./decisoes-tecnicas.md#escopo-oauth-drivefile).

## Armazenamento e renovação de tokens

- **Refresh token** → keyring (Credential Manager no Windows, Keychain no macOS, Secret
  Service no Linux). Salvo como JSON `{ refresh_token, email }`, para o e-mail aparecer
  na UI ao reabrir sem precisar de rede.
- **Access token** → apenas em memória (`AuthManager`, atrás de `RwLock`), renovado
  automaticamente quando faltam menos de 60s para expirar.
- **`invalidate_cached_token()`** força renovação após um 401 do Drive.

> **Tokens nunca cruzam a boundary.** O frontend só recebe `AuthStatus { connected, email }`.

## Client secret — por que existe e por que é seguro

O Google exige o client secret no token endpoint para clientes do tipo **Desktop app**.
Em apps instalados ele **não é tratado como confidencial** (é o mesmo modelo de rclone,
gcloud SDK, etc.) — a segurança vem do PKCE, não do secret. Mesmo assim, ele nunca entra
no código: vem de variável de ambiente.

> **Evolução (produção):** desde a [FEATURE-005](./15-proxy-worker-oauth.md), o `client_secret`
> **não vai mais no binário de release** — um Cloudflare Worker faz a troca de token server-side
> e guarda o secret. O caminho com `RETROSYNC_GOOGLE_CLIENT_SECRET` descrito abaixo permanece
> como **fallback de desenvolvimento local** (sem Worker). O tipo de cliente continua sendo
> **Desktop app**, pois o redirect loopback exige isso. Ver [15 — Proxy Cloudflare Worker (OAuth)](./15-proxy-worker-oauth.md).

## Configuração de credenciais

O `build.rs` lê o `.env` da raiz em build-time:

```
RETROSYNC_GOOGLE_CLIENT_ID=seu-client-id
RETROSYNC_GOOGLE_CLIENT_SECRET=seu-secret
```

- Lido por `option_env!` (build-time) com fallback para `std::env::var` (runtime, dev).
- Shell tem precedência sobre o `.env`.
- `.env` ignorado pelo git; `.env.example` commitado.
- Sem Client ID configurado, o app sobe normalmente, mas o botão de conectar retorna um
  erro explicativo.

### Como criar as credenciais (Google Cloud Console)

1. Criar projeto;
2. Ativar a **Google Drive API**;
3. Configurar a OAuth consent screen (External; adicionar sua conta como test user);
4. Criar credencial **OAuth Client ID** do tipo **Desktop app**;
5. Preencher o `.env`.

## Comandos expostos

| Comando | Assinatura | Descrição |
| --- | --- | --- |
| `connect_google_drive` | `() -> AuthStatus` | Abre o navegador e aguarda a autorização |
| `get_auth_status` | `() -> AuthStatus` | Status sem fluxo interativo (consulta o keyring) |
| `disconnect_google_drive` | `() -> AuthStatus` | Remove o refresh token e limpa o cache |

Todos emitem o evento `auth:status` ao alterar o estado.

## Testes

5 testes unitários em `oauth.rs`:

- `code_challenge` confere com o vetor oficial da RFC 7636;
- tamanho do verifier dentro de 43–128 chars;
- parsing do redirect extrai `code`/`state`;
- parsing extrai `error=access_denied`;
- parsing ignora requisições alheias.

## Como testar manualmente

```powershell
$env:RETROSYNC_GOOGLE_CLIENT_ID = "seu-client-id"
$env:RETROSYNC_GOOGLE_CLIENT_SECRET = "seu-secret"
npm run tauri dev
```

Clicar em **Conectar ao Google Drive** → autorizar no navegador → a UI passa a exibir o
e-mail. Fechar e reabrir o app deve manter a conexão (refresh token persistido).
