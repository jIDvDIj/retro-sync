# Portabilidade multiplataforma — checklist faseado

Plano de portar o RetroSync para além do Windows: Linux, macOS, Steam Deck, Android e iOS.
As fases não são estritamente sequenciais — algumas abstrações (storage, segredos, OAuth)
foram feitas cedo para o código passar a **compilar** para mobile antes de a plataforma
estar de fato validada em device.

> Complementa o [17 — Suporte Android](./17-suporte-android.md) (detalhes do Android) e o
> [Como adicionar por plataforma](./plataformas-como-adicionar.md) (guia prático de código).

## Status por fase

| Fase | Descrição | Status |
| --- | --- | --- |
| 0 | Código compilável para mobile: `#[cfg(desktop)]` em tray, autostart e process watcher | ✅ Concluído |
| 1 | Desktop não-Windows: descoberta Steam Deck/Flatpak + empacotamento Flatpak/macOS | 🟡 Descoberta feita; empacotamento precisa de máquina Linux/macOS |
| 2 | Abstração de storage: trait `LocalStorage` + `FileLoc`; todo I/O do engine isolado | ✅ Concluído |
| 3 | Scaffolding Android: SDK/NDK, `tauri android init`, APK debug em device físico | ✅ Concluído |
| 4 | Storage mobile SAF: `MobileStorage` + plugin nativo (`StoragePlugin.kt`) | 🟡 Interface pronta; validação em device pendente |
| 5 | OAuth via Worker redirect: client Web único, `/oauth/callback`, deep link | ✅ Concluído |
| 6 | `SecretStore` trait: `KeyringStore` (desktop) / `SqliteSecretStore` (mobile) | ✅ Concluído |
| 7 | Gatilhos lifecycle (`resume`/`pause`) + UI mobile | ✅ Concluído |
| 8 | APK assinado (`retrosync.jks`) + job `android` no CI | 🟡 Secrets do GitHub pendentes |

## Fase 0 — Compilar para mobile

Recursos que só existem no desktop ficam atrás de `#[cfg(desktop)]`, e os equivalentes
mobile atrás de `#[cfg(mobile)]`:

- **Tray, janela escondível, autostart**: só-desktop (`platform::desktop`).
- **Process watcher** (`sysinfo`): só-desktop — no mobile não há como inspecionar processos
  de outros apps; os gatilhos automáticos viram `resume`/`pause` (Fase 7).
- **Plugins**: `tauri_plugin_autostart` só-desktop; `deep_link`/`opener` só-mobile.

O critério da fase é `cargo build` verde para os três SOs na CI.

## Fase 2 — Abstração de storage (`LocalStorage` + `FileLoc`)

O `SyncEngine` nunca toca `std::fs`/`tokio::fs`/`filetime` diretamente: todo I/O local
passa pelo trait [`LocalStorage`](../src-tauri/src/sync/storage.rs). Os arquivos são
endereçados por [`FileLoc`] **opaco** — um caminho nativo no desktop, um `{tree, rel}` do
SAF no mobile — para que a mesma lógica de sync sirva às duas plataformas.

- `DesktopStorage`: filesystem nativo.
- `MobileStorage`: traduz cada operação numa chamada ao plugin nativo (Fase 4).

Métodos: `scan`, `join`, `read`, `write_atomic`, `copy_to`, `exists`, `mtime_ms`,
`is_valid_root`, `subdir_exists` (os dois últimos adicionados por
[BUG-005](./bugs/bug-005-validacao-filesystem-mobile.md)).

## Fase 4 — Storage mobile (SAF)

No Android o acesso aos saves de **outro** app passa por uma concessão de pasta do usuário
(Storage Access Framework). O contrato Rust↔plugin vive em
[`sync/mobile_storage.rs`](../src-tauri/src/sync/mobile_storage.rs): comandos `listFiles`,
`stat`, `exists`, `read`, `write`, `copy`, `pickFolder`. O lado nativo (`StoragePlugin.kt`)
está escrito; falta validar em device físico (leitura/escrita/mtime via `DocumentFile`).
No iOS o equivalente seria document picker + security-scoped bookmark
(`register_ios_plugin` ainda `todo!()`).

## Fase 5 — OAuth mobile

Sem loopback TCP (RFC 8252) no mobile, o retorno do consentimento chega por **deep link**
(`com.retrosync.app:/oauth2redirect`). O Cloudflare Worker (ver
[15](./15-proxy-worker-oauth.md) e [FEATURE-005](./features/feature-005-cloudflare-worker-proxy.md))
expõe `/oauth/callback`, e `connect_google_drive` (mobile) registra o listener do deep link
**antes** de abrir o browser, para não perder o redirect se o app já estava em background.

## Fase 6 — Segredos mobile (`SecretStore`)

O keyring do SO não está disponível no mobile. O trait `SecretStore` abstrai o cofre:
`KeyringStore` (desktop, keychain do SO) e `SqliteSecretStore` (mobile, tabela `secrets` no
SQLite privado do app — SCHEMA_V5). Guarda o refresh token e o `device_id` estável.

## Fase 7 — Gatilhos e UI mobile

- **Gatilhos**: `tauri://resume` → sync bidirecional (`foreground`); `tauri://pause` →
  upload (`background`). Substituem watcher e o sync de despedida do desktop.
- **UI**: `AddEmulatorModal`/`SettingsModal` usam `pick_emulator_folder` (SAF) no lugar do
  seletor de ficheiros; recursos só-desktop (autostart) ficam ocultos no mobile.

## Fase 1 e 8 — Pendências

- **Fase 1**: a descoberta automática já cobre caminhos Flatpak do Steam Deck
  (`~/.var/app/...` em `profiles.toml`); falta empacotar Flatpak/`.dmg`, o que exige uma
  máquina Linux/macOS.
- **Fase 8**: o job `android` do `release.yml` está pronto; faltam os secrets no GitHub
  (`ANDROID_KEYSTORE_BASE64`, `ANDROID_STORE_PASSWORD`, `ANDROID_KEY_PASSWORD`).
