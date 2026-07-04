# Como adicionar código por plataforma

Guia prático de **onde e como** escrever código específico de plataforma no RetroSync, sem
quebrar o build dos outros SOs. Complementa o [checklist faseado](./multiplataforma-checklist.md).

Este guia mostra os passos concretos para adicionar um comando ou módulo Rust que seja:
- **Geral** — roda em desktop e mobile sem diferença
- **Desktop-only** — Windows, macOS, Linux (inclusive Steam Deck)
- **Mobile-only** — Android, iOS

---

## Conceitos base

O Tauri 2 expõe dois predicados de compilação:

| Predicado | Quando é verdadeiro |
| --- | --- |
| `#[cfg(desktop)]` | Windows, macOS, Linux, Steam Deck |
| `#[cfg(mobile)]` | Android, iOS |

Eles são mutuamente exclusivos e cobrem 100% dos targets suportados pelo Tauri 2.
Para separar desktop de mobile — use `cfg(desktop)` / `cfg(mobile)`.

`desktop` e `mobile` são flags de `cfg` **definidas pelo build script do Tauri** — funcionam
em atributos de item no código-fonte (`#[cfg(desktop)]`), mas **não** na resolução de
dependências do Cargo (ver seção 6).

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

---

## 1. Comando geral (desktop + mobile)

Funciona da mesma forma em qualquer plataforma. Nenhuma marcação especial.

### Rust — `src-tauri/src/commands.rs`

```rust
#[tauri::command]
pub async fn meu_comando(state: State<'_, AppState>) -> AppResult<String> {
    // lógica aqui
    Ok("ok".into())
}
```

### `lib.rs` — registrar no handler

```rust
.invoke_handler(tauri::generate_handler![
    // ... outros comandos ...
    commands::meu_comando,
])
```

### Frontend — `src/lib/ipc.ts`

```ts
export async function meuComando(): Promise<string> {
    return invoke('meu_comando');
}
```

### Frontend — `src/types/ipc.ts`

Adicione o tipo de retorno se for uma struct Rust (ver
[Referência IPC](./referencia-ipc.md)).

---

## 2. Comando desktop-only

Use `#[cfg(desktop)]` na definição **e** no registro. O comando simplesmente não
existe no binário mobile — o frontend mobile nunca deve chamá-lo.

### Rust — `src-tauri/src/commands.rs`

```rust
#[cfg(desktop)]
use tauri_plugin_autostart::ManagerExt; // import condicional se necessário

#[cfg(desktop)]
#[tauri::command]
pub async fn meu_comando_desktop(app: AppHandle) -> AppResult<()> {
    // usa APIs só-desktop (keyring nativo, autostart, explorer, etc.)
    Ok(())
}
```

### `lib.rs` — registrar no handler

```rust
.invoke_handler(tauri::generate_handler![
    // ... comandos gerais ...
    #[cfg(desktop)]
    commands::meu_comando_desktop,
])
```

### Frontend — chamar com guarda

```ts
// src/lib/ipc.ts
export async function meuComandoDesktop(): Promise<void> {
    return invoke('meu_comando_desktop');
}
```

> No frontend, garanta que o botão/hook que chama esse comando só apareça em builds
> desktop. Por ora o frontend tem um único build; quando o build mobile existir, use
> a detecção de plataforma do Tauri:
> ```ts
> import { platform } from '@tauri-apps/plugin-os';
> if ((await platform()) !== 'android' && (await platform()) !== 'ios') { ... }
> ```

---

## 3. Comando mobile-only

Espelho do padrão anterior, com `#[cfg(mobile)]`.

### Rust — `src-tauri/src/commands.rs`

```rust
#[cfg(mobile)]
use tauri::Listener; // import condicional se necessário

#[cfg(mobile)]
#[tauri::command]
pub async fn meu_comando_mobile(app: AppHandle) -> AppResult<String> {
    // usa APIs só-mobile (SAF, deep link, opener, etc.)
    Ok("caminho escolhido".into())
}

// Stub para desktop: evita erro de "comando não encontrado" em dev se o
// frontend chamar sem guarda. Remova se tiver certeza que não acontece.
#[cfg(not(mobile))]
#[tauri::command]
pub async fn meu_comando_mobile(_app: AppHandle) -> AppResult<String> {
    Err(crate::error::AppError::Other(
        "meu_comando_mobile não disponível no desktop".into(),
    ))
}
```

### `lib.rs` — registrar no handler

```rust
.invoke_handler(tauri::generate_handler![
    // ... outros comandos ...
    commands::meu_comando_mobile, // sempre presente (stub no desktop)
])
```

---

## 4. Módulo Rust desktop-only

Para código maior (ex.: bandeja, autostart, watcher), use `platform/desktop.rs`.

### `src-tauri/src/platform/desktop.rs` — adicionar função

```rust
/// Faz algo exclusivo do desktop.
pub fn minha_funcao_desktop(app: &AppHandle) {
    // ...
}
```

### Chamar no setup de `lib.rs`

```rust
#[cfg(desktop)]
platform::desktop::setup(app, db.clone(), engine.clone())?;
// setup() chama internamente as sub-funções de desktop.rs
```

Se for uma nova responsabilidade separada, adicione-a como chamada direta dentro
de `platform::desktop::setup()`.

---

## 5. Módulo Rust mobile-only

Use `platform/mobile.rs`.

```rust
// src-tauri/src/platform/mobile.rs
pub fn setup(_app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    minha_init_mobile();
    Ok(())
}

fn minha_init_mobile() {
    // registrar listeners, configurar deep link, etc.
}
```

---

## 6. Dependência só de uma plataforma

`cfg(desktop)`/`cfg(mobile)` **não** existem para o Cargo na resolução de dependências —
use predicados padrão do Rust (`target_os`) em `src-tauri/Cargo.toml`:

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

---

## 7. Código inline com cfg (blocos pequenos)

Para diferenças pontuais dentro de uma função já existente:

```rust
pub async fn get_settings(app: AppHandle, state: State<'_, AppState>) -> AppResult<Settings> {
    let mut settings = state.db.with(settings::load).await?;

    #[cfg(desktop)]
    {
        settings.autostart = autostart_enabled(&app)?;
    }
    #[cfg(not(desktop))]
    {
        let _ = &app; // suprime warning de variável não usada
    }

    Ok(settings)
}
```

---

## Checklist ao adicionar suporte a uma plataforma ou recurso

1. O recurso é geral, desktop-only ou mobile-only? Marque com `cfg` ou deixe sem.
2. Toca I/O local de saves? Passe pelo trait `LocalStorage` — **nunca** `std::fs` direto.
3. Toca segredos (token, `device_id`)? Passe pelo `SecretStore`.
4. É um comando novo? Garanta a mesma assinatura nas duas plataformas (no-op onde não
   se aplica) e espelhe em `src/types/ipc.ts` + `src/lib/ipc.ts`.
5. Precisa de dependência nova? Coloque na seção `[target.*.dependencies]` correta.
6. `cargo build` verde para Windows, Linux e mobile na CI (é o critério de aceite da
   [Fase 0](./multiplataforma-checklist.md)).
