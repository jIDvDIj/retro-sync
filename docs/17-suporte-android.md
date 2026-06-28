# 17 — Suporte Android (Fases 3, 5, 6 e 7)

> Implementação do suporte Android: scaffolding do APK, OAuth via deep link, keyring
> abstrato e UI/gatilhos mobile. O alvo de produção é `aarch64` (arm64-v8a); o iOS
> fica para uma fase futura (exige macOS/Xcode).

Relacionados: [multiplataforma-checklist](./multiplataforma-checklist.md) ·
[03 — Autenticação](./03-autenticacao.md) · [15 — Proxy Worker](./15-proxy-worker-oauth.md).

---

## Fase 3 — Scaffolding Android

### Pré-requisitos instalados (Windows nativo)

| Ferramenta | Versão / local |
| --- | --- |
| Android SDK | `%LOCALAPPDATA%\Android\Sdk` |
| NDK | `26.3.11579264` (via `sdkmanager`) |
| Rust target | `aarch64-linux-android` (`rustup target add`) |
| Java | JDK 21 (via Android Studio JBR) |

Geração do projeto Android (PowerShell):

```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:NDK_HOME     = "$env:ANDROID_HOME\ndk\26.3.11579264"
$env:JAVA_HOME    = "C:\Program Files\Android\Android Studio\jbr"
npm run tauri android init
```

O comando gera `src-tauri/gen/android/` (gitignored via `src-tauri/gen/`).

### Build do APK

```powershell
npm run tauri android build -- --target aarch64          # release (não assinado)
npm run tauri android build -- --target aarch64 --debug  # debug (assinado com debug key)
```

APK de saída:
- debug: `src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`
- release: `…/release/app-universal-release-unsigned.apk`

Para instalar via USB:

```cmd
%LOCALAPPDATA%\Android\Sdk\platform-tools\adb install -r <caminho-do-apk>
```

### Gotcha: symlinks no Windows

`tauri android build` cria symlinks do `.so` na pasta `jniLibs`. O Windows exige
**Developer Mode** habilitado para symlinks sem privilégio de administrador:

```cmd
reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock" /t REG_DWORD /v AllowDevelopmentWithoutDevLicense /d 1 /f
```

---

## Fase 5 — OAuth via deep link

### Problema

O tipo de client **Desktop app** do Google aceita apenas `http://127.0.0.1` como
redirect URI. No Android, a chamada loopback não funciona (sandbox). O Google exige
a URI no formato reverse-DNS do pacote: `com.retrosync.app:/oauth2redirect`.

### Solução: dois clients OAuth

| Client | Tipo | Redirect URI | Usado em |
| --- | --- | --- | --- |
| Desktop app (existente) | Desktop app | `http://127.0.0.1` | Windows/Linux/macOS (via Worker) |
| Android (novo) | Android | `com.retrosync.app:/oauth2redirect` (implícito) | APK Android |

O client Android não tem `client_secret` — a troca de código vai **direto ao Google**,
sem passar pelo Cloudflare Worker.

Para criar o client Android no Google Cloud Console:
- Tipo: **Android**
- Package name: `com.retrosync.app`
- SHA-1: fingerprint do debug keystore:
  ```cmd
  keytool -keystore %USERPROFILE%\.android\debug.keystore -list -v -alias androiddebugkey -storepass android -keypass android
  ```

### Variáveis de ambiente

```env
# .env
RETROSYNC_GOOGLE_CLIENT_ID=<Desktop app client ID>
RETROSYNC_GOOGLE_CLIENT_SECRET=<Desktop app client secret>
RETROSYNC_TOKEN_PROXY_URL=<URL do Worker>
RETROSYNC_PROXY_SECRET=<shared secret do Worker>

RETROSYNC_GOOGLE_CLIENT_ID_ANDROID=<Android client ID>
```

### Implementação

**`build.rs`** — `RETROSYNC_GOOGLE_CLIENT_ID_ANDROID` adicionado a `EMBEDDED_KEYS`.

**`auth/oauth.rs`** — `OAuthConfig::from_env()` com dois ramos:

```rust
#[cfg(mobile)]
{
    // Android: sem secret, sem proxy — direto ao Google.
    let client_id = option_env!("RETROSYNC_GOOGLE_CLIENT_ID_ANDROID")...?;
    return Some(Self { client_id, token_proxy_url: None, proxy_secret: None, client_secret: None });
}
#[cfg(not(mobile))]
{ /* lê as quatro variáveis desktop como antes */ }
```

**`auth/oauth.rs`** — `authorize_interactive_mobile`:
- Usa `tauri_plugin_opener` para abrir o browser (o `open` crate não funciona em
  processos sandboxados no Android).
- Aguarda o deep link `com.retrosync.app:/oauth2redirect?code=...` via
  `oneshot::Receiver<String>` passado pelo `commands.rs`.
- Valida o `state` (CSRF) e troca o código diretamente em
  `https://oauth2.googleapis.com/token` (sem proxy).

**`commands.rs`** — variante `#[cfg(mobile)]` de `connect_google_drive`:
- Registra um listener `app.once("deep-link://new-url", ...)` antes de abrir o browser.
- Filtra pela URL que começa com `com.retrosync.app:/oauth2redirect`.
- Passa o `oneshot::Sender` ao listener e o `Receiver` ao `auth.connect_mobile`.

**`tauri.conf.json`** — plugin de deep link:
```json
"plugins": {
  "deep-link": {
    "mobile": [
      { "scheme": ["com.retrosync.app"], "path_prefix": ["/oauth2redirect"] }
    ]
  }
}
```

**`Cargo.toml`** — deps exclusivas do mobile:
```toml
[target.'cfg(any(target_os = "android", target_os = "ios"))'.dependencies]
tauri-plugin-deep-link = "2"
tauri-plugin-opener     = "2"
```

---

## Fase 6 — Keyring abstrato (`SecretStore`)

### Problema

A crate `keyring` não suporta Android. No desktop ela persiste refresh token e
`device_id` no Keychain/Secret Service do SO. No mobile precisamos de uma alternativa.

### Solução: trait `SecretStore`

```rust
pub trait SecretStore: Send + Sync {
    fn set(&self, key: &str, value: &str) -> AppResult<()>;
    fn get(&self, key: &str) -> AppResult<Option<String>>;
    fn delete(&self, key: &str) -> AppResult<()>;
}
```

Duas implementações:

| Implementação | Plataforma | Backend |
| --- | --- | --- |
| `KeyringStore` | `#[cfg(desktop)]` | crate `keyring` (Keychain / Secret Service / Win Credential Store) |
| `SqliteSecretStore(Db)` | `#[cfg(mobile)]` | tabela `secrets` no SQLite local do app |

**`storage/db.rs`** — migração `SCHEMA_V5`:
```sql
CREATE TABLE IF NOT EXISTS secrets (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```
A tabela existe em todas as plataformas; no desktop fica vazia.

**`db.rs`** — método `with_conn_blocking` (usado pela `SqliteSecretStore`):
```rust
pub fn with_conn_blocking<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T>
```

**Sites de uso atualizados** para `&dyn SecretStore`:
- `auth/token_store.rs` — `save`, `load`, `clear`
- `device.rs` — `get_or_create`, `current`
- `auth/mod.rs` — `AuthManager` guarda `secrets: Arc<dyn SecretStore>`
- `sync/engine.rs` — `SyncEngine` recebe `secrets` e passa para `device::current`

**`lib.rs`** — inicialização por plataforma:
```rust
#[cfg(desktop)]
let secret_store: Arc<dyn secrets::SecretStore> = Arc::new(secrets::KeyringStore);
#[cfg(mobile)]
let secret_store: Arc<dyn secrets::SecretStore> = Arc::new(secrets::SqliteSecretStore(db.clone()));
```

---

## Fase 7 — Gatilhos mobile e UI adaptada

### Gatilhos de sync no Android

No desktop o sync é disparado por abertura/fechamento de processo de emulador
(`sysinfo` watcher). No Android isso é impossível (sandbox). Em vez disso, o app
escuta os eventos de ciclo de vida do Tauri:

| Evento Tauri | Direção | Trigger |
| --- | --- | --- |
| `tauri://resume` | Bidirecional | `foreground` |
| `tauri://pause` | `LocalToDrive` | `background` |

Registrado em `lib.rs` dentro de `#[cfg(mobile)]`:

```rust
app_handle.listen("tauri://resume", move |_| {
    spawn_sync(app_handle.clone(), SyncDirection::Bidirectional, TRIGGER_FOREGROUND);
});
app_handle.listen("tauri://pause", move |_| {
    spawn_sync(app_handle.clone(), SyncDirection::LocalToDrive, TRIGGER_BACKGROUND);
});
```

Novas constantes em `constants.rs`:
```rust
pub const TRIGGER_FOREGROUND: &str = "foreground";
pub const TRIGGER_BACKGROUND: &str = "background";
```

### Comando `pick_emulator_folder`

No Android a seleção de pasta usa o **SAF** (`ACTION_OPEN_DOCUMENT_TREE`), não um
seletor de filesystem nativo. O comando `pick_emulator_folder` expõe isso ao frontend:

```rust
#[cfg(mobile)]
pub async fn pick_emulator_folder(app: AppHandle) -> AppResult<String> {
    mobile_storage::pick_folder(&app).await
}
#[cfg(desktop)]
pub async fn pick_emulator_folder() -> AppResult<String> {
    Err(AppError::Other("use o seletor de pasta nativo no desktop".into()))
}
```

A URI da árvore SAF concedida é armazenada como `root_path` do emulador no SQLite.

### UI adaptada

**`src/hooks/usePlatform.ts`** — detecta a plataforma via `healthCheck().isMobile`:
```typescript
export function usePlatform() {
  const [isMobile, setIsMobile] = useState(false);
  useEffect(() => { healthCheck().then(h => setIsMobile(h.isMobile)).catch(() => {}); }, []);
  return { isMobile };
}
```

**`HealthStatus`** — campo `isMobile: boolean` adicionado (Rust: `cfg!(mobile)`).

**`SettingsModal`** — seção de autostart oculta no mobile (`{!isMobile ? ... : null}`).

**`AddEmulatorModal`** — comportamento por plataforma:

| Funcionalidade | Desktop | Mobile |
| --- | --- | --- |
| Seção "Recomendados" | exibe (scan de filesystem) | oculta |
| Botão "Selecionar pasta" | `openDialog({ directory: true })` → `detectEmulator` | `pickEmulatorFolder()` (SAF) |
| Detecção automática | tenta; abre formulário manual se falhar | pula — vai direto ao manual |
| Subpastas (saves/savestates/config) | seletor de subpasta via `openDialog` | campos de texto com caminhos relativos |
| Preenchimento inteligente | — | ao digitar o nome, preenche defaults por emulador (PPSSPP, PCSX2) |

Defaults de caminhos por emulador no mobile:

| Emulador | saves | savestates | config |
| --- | --- | --- | --- |
| PPSSPP | `PSP/SAVEDATA` | `PSP/PPSSPP_STATE` | `PSP/SYSTEM` |
| PCSX2 | `memcards` | `sstates` | `inis` |

---

## Fase 5 (revisão) — OAuth via Worker redirect

### Problema com os tipos de client OAuth

A implementação inicial tentou dois tipos de client:

| Tentativa | Problema |
| --- | --- |
| Desktop app | Aceita apenas `http://127.0.0.1` — rejeita custom URI schemes |
| Web application (UI) | Rejeita custom URI schemes na UI do Console |
| Android | Projetado para Google Sign-In SDK; browser-based PKCE retorna `invalid_request` |

### Solução final: um único client Web application + redirect via Worker

```
Android: App → Custom Tab → Google → https://<worker>/oauth/callback
         → Worker faz 302 para com.retrosync.app:/oauth2redirect?code=...
         → deep link chega no app → app chama Worker /token para trocar o code
```

Um único client **Web application** com duas URIs registradas no Google Console:
```
http://127.0.0.1                          ← desktop (qualquer porta)
https://<worker-url>/oauth/callback       ← Android
```

### Worker — novo endpoint `/oauth/callback`

```javascript
// GET /oauth/callback — chamado pelo Google (sem X-Proxy-Secret)
if (url.pathname === "/oauth/callback") {
  const code  = url.searchParams.get("code");
  const state = url.searchParams.get("state");
  const deepLink = `com.retrosync.app:/oauth2redirect?code=...&state=...`;
  return Response.redirect(deepLink, 302);
}
```

O endpoint `/token` passou a aceitar dois formatos de `redirect_uri`:
- `http://127.0.0.1:` (desktop, loopback)
- `{worker_origin}/oauth/callback` (Android)

### Rust — `OAuthConfig` unificado

`OAuthConfig::from_env()` voltou a ser único (sem `#[cfg]`): desktop e mobile leem
as mesmas variáveis `RETROSYNC_GOOGLE_CLIENT_ID` / `RETROSYNC_TOKEN_PROXY_URL` /
`RETROSYNC_PROXY_SECRET`.

O redirect URI mobile é calculado em runtime:
```rust
let redirect_uri = format!("{}{MOBILE_REDIRECT_SUFFIX}", config.token_proxy_url);
// MOBILE_REDIRECT_SUFFIX = "/oauth/callback"
```

`RETROSYNC_GOOGLE_CLIENT_ID_ANDROID` foi removido — não é mais necessário.

---

## Fase 8 — Assinatura e CI Android

### Assinatura local

A keystore é gerada uma única vez com `keytool` e nunca vai para o git:

```cmd
keytool -genkey -v -keystore retrosync.jks -alias retrosync -keyalg RSA -keysize 2048 -validity 10000
```

O `build.gradle.kts` lê as credenciais via `System.getenv()` — não há nada hardcodado:

```kotlin
val keystorePath    = System.getenv("ANDROID_KEYSTORE_PATH")
val storePassword   = System.getenv("ANDROID_STORE_PASSWORD")
val keyAlias        = System.getenv("ANDROID_KEY_ALIAS") ?: "retrosync"
val keyPassword     = System.getenv("ANDROID_KEY_PASSWORD")
val hasSigningConfig = keystorePath != null && storePassword != null && keyPassword != null

if (hasSigningConfig) {
    signingConfigs { create("release") { ... } }
}
buildTypes {
    getByName("release") {
        if (hasSigningConfig) signingConfig = signingConfigs.getByName("release")
    }
}
```

Para buildar localmente (cmd):
```cmd
set ANDROID_KEYSTORE_PATH=C:\...\retrosync.jks
set ANDROID_STORE_PASSWORD=<senha>
set ANDROID_KEY_ALIAS=retrosync
set ANDROID_KEY_PASSWORD=<senha>
npm run tauri android build -- --target aarch64
```

> As variáveis de assinatura **não** vão para o `.env` — o `.env` só é lido pelo
> `build.rs` do Rust. O Gradle lê apenas variáveis de ambiente do shell.

### `.gitignore`

O `src-tauri/gen/` continua ignorado no geral, mas o `build.gradle.kts` (que contém
a configuração de assinatura) é exposto seletivamente:

```gitignore
src-tauri/gen/**
!src-tauri/gen/android/
!src-tauri/gen/android/app/
!src-tauri/gen/android/app/build.gradle.kts

*.jks
*.keystore
```

### CI — job `android` no `release.yml`

O job roda em `ubuntu-latest` e publica o APK assinado junto com os instaladores desktop:

```yaml
android:
  needs: version
  runs-on: ubuntu-latest
  steps:
    - Setup Java 17, Android SDK, NDK 26.3.11579264, Rust aarch64-linux-android
    - Validar secrets obrigatórios
    - Definir versão (mesmo jq do job desktop)
    - Decodificar keystore: base64 -d → retrosync.jks
    - npm run tauri android build -- --target aarch64
    - Remover keystore (if: always())
    - Upload APK + AAB para a release via softprops/action-gh-release
```

### GitHub Secrets necessários

| Secret | Como obter |
| --- | --- |
| `ANDROID_KEYSTORE_BASE64` | PowerShell: `[Convert]::ToBase64String([IO.File]::ReadAllBytes("retrosync.jks")) \| clip` |
| `ANDROID_STORE_PASSWORD` | senha definida no `keytool` |
| `ANDROID_KEY_PASSWORD` | mesma senha (se não definiu senha separada para a chave) |

Os secrets de credenciais OAuth (`RETROSYNC_GOOGLE_CLIENT_ID`, `RETROSYNC_TOKEN_PROXY_URL`,
`RETROSYNC_PROXY_SECRET`) já existiam para o job desktop e são reaproveitados no job Android.

---

## Estado atual

- APK release assinado compilando e instalando em device físico (`aarch64`). ✅
- OAuth funcionando: login via browser → Worker redirect → deep link → token. ✅
- Job `android` no `release.yml` implementado; aguarda cadastro dos secrets no GitHub. 🟡
- Storage SAF (`StoragePlugin.kt`): esqueleto escrito, validação em device pendente. 🟡
- iOS: toda a estrutura está gateada por `#[cfg(any(target_os = "android", target_os = "ios"))]`; implementação nativa exige macOS/Xcode. ⬜

### Secrets ainda não cadastrados no GitHub

Para o CI Android funcionar, cadastre em **Settings → Secrets and variables → Actions**:

- [ ] `ANDROID_KEYSTORE_BASE64`
- [ ] `ANDROID_STORE_PASSWORD`
- [ ] `ANDROID_KEY_PASSWORD`
