# Como adicionar código por plataforma

Guia prático de **onde e como** escrever código específico de plataforma no RetroSync, sem
quebrar o build dos outros SOs. Complementa o [checklist faseado](./multiplataforma-checklist.md).

## As três categorias de código

| Categoria | Quando | Como marcar |
| --- | --- | --- |
| **Geral** | Vale para todas as plataformas | Sem `cfg` |
| **Desktop-only** | Tray, autostart, watcher, janela, keyring do SO | `#[cfg(desktop)]` |
| **Mobile-only** | SAF, deep link, lifecycle, segredos no SQLite | `#[cfg(mobile)]` |

`desktop` e `mobile` são flags de `cfg` **definidas pelo build script do Tauri** — funcionam
em atributos de item no código-fonte (`#[cfg(desktop)]`), mas **não** na resolução de
dependências do Cargo (ver abaixo).

## Manter a boundary IPC idêntica entre plataformas

Um comando exposto ao frontend deve existir em **todas** as plataformas, mesmo que seja
no-op em alguma — assim o `src/lib/ipc.ts` não precisa de ramos por SO. Padrão de duas
implementações com a mesma assinatura:

```rust
#[cfg(desktop)]
#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> AppResult<()> { /* real */ }

#[cfg(mobile)]
#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> AppResult<()> {
    let _ = (&app, enabled);
    Ok(()) // no-op: "subir com o sistema" não existe no mobile
}
```

Quando um comando **só** existe numa plataforma (ex.: `pick_emulator_folder` no mobile,
`open_backup_folder` no desktop), registre-o no `invoke_handler` com `#[cfg(...)]` e faça o
frontend chamá-lo só quando `HealthStatus.isMobile` indicar a plataforma certa.

## Onde mora o código de plataforma

- **`platform/mod.rs`** apenas declara os submódulos por `cfg`:
  ```rust
  #[cfg(desktop)] pub mod desktop;
  #[cfg(mobile)]  pub mod mobile;
  ```
- **`platform/desktop.rs`**: tray, `on_close_requested` (fechar-esconde), `setup` do watcher.
- **`platform/mobile.rs`**: `setup` mobile (webview único já exibido pelo sistema).
- **`sync/mobile_storage.rs`**: implementação `LocalStorage` sobre o plugin SAF (só-mobile).
- **`secrets.rs`**: `KeyringStore` (desktop) e `SqliteSecretStore` (mobile), atrás do trait
  `SecretStore` — escolhido no `setup` por `cfg`.

O `lib.rs` faz a montagem por plataforma no `setup`: escolhe `DesktopStorage`/`MobileStorage`
e `KeyringStore`/`SqliteSecretStore`, registra plugins só-desktop/só-mobile e liga os
gatilhos (`resume`/`pause` no mobile; watcher no desktop).

## Dependências condicionais (`Cargo.toml`)

`cfg(desktop)`/`cfg(mobile)` **não** existem para o Cargo na resolução de dependências —
use predicados padrão do Rust (`target_os`):

```toml
# Desktop (não-Android/iOS): watcher, autostart, keyring do SO.
[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]
tauri-plugin-autostart = "2"
sysinfo = "0.33"
keyring = { version = "3", features = ["windows-native", "apple-native", "sync-secret-service"] }

# Mobile: deep link (OAuth) + opener (browser no sandbox Android).
[target.'cfg(any(target_os = "android", target_os = "ios"))'.dependencies]
tauri-plugin-deep-link = "2"
tauri-plugin-opener = "2"

# Só Windows: leitura de registro para a descoberta de instalações.
[target.'cfg(windows)'.dependencies]
winreg = "0.55"
```

## Checklist ao adicionar suporte a uma plataforma ou recurso

1. O recurso é geral, desktop-only ou mobile-only? Marque com `cfg` ou deixe sem.
2. Toca I/O local de saves? Passe pelo trait `LocalStorage` — **nunca** `std::fs` direto.
3. Toca segredos (token, `device_id`)? Passe pelo `SecretStore`.
4. É um comando novo? Garanta a mesma assinatura nas duas plataformas (no-op onde não
   se aplica) e espelhe em `src/types/ipc.ts` + `src/lib/ipc.ts`.
5. Precisa de dependência nova? Coloque na seção `[target.*.dependencies]` correta.
6. `cargo build` verde para Windows, Linux e mobile na CI (é o critério de aceite da
   [Fase 0](./multiplataforma-checklist.md)).
