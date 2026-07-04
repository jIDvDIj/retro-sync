# Como adicionar funcionalidades por plataforma

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
Para separar desktop de mobile — use `cfg(desktop)` /
`cfg(mobile)`.

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

Adicione em `src-tauri/Cargo.toml` na seção correta:

```toml
# Desktop (Windows + macOS + Linux + Steam Deck)
[target.'cfg(desktop)'.dependencies]
minha-crate-desktop = "1"

# Mobile (Android + iOS)
[target.'cfg(mobile)'.dependencies]
minha-crate-mobile = "1"

# Windows apenas (dentro do desktop)
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
