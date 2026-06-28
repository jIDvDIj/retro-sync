# BUG-005 — Validações de filesystem incompatíveis com URIs SAF (mobile)

## Sintoma

Ao adicionar um emulador no Android, o app retorna erros como:

```
pasta não encontrada: content://com.android.externalstorage.documents/tree/primary%3APPSSPP
pasta não encontrada sob a raiz: PSP/SAVEDATA
```

## Causa raiz

O código de validação foi escrito assumindo que caminhos são caminhos de filesystem
(`PathBuf`, `Path::is_dir`, `Path::join`). No Android, o SAF (Storage Access Framework)
endereça arquivos e pastas por **URIs opacas** do tipo `content://...`, que não são
caminhos de filesystem e não podem ser verificadas com `std::fs`.

Os pontos afetados eram:

| Arquivo | Linha | Validação quebrada |
| --- | --- | --- |
| `commands.rs` | `detect_emulator` | `root.is_dir()` |
| `commands.rs` | `add_emulator` | `root.is_dir()` |
| `commands.rs` | `add_emulator_manual` | `root.is_dir()` |
| `emulator/mod.rs` | `validate_rel_dirs` | `root.join(&rel).is_dir()` |

## Solução aplicada (paliativa)

Todas as validações foram gateadas com `#[cfg(not(mobile))]`. No mobile elas são
simplesmente puladas — a validação real fica a cargo do plugin nativo (`StoragePlugin`)
quando o sync de fato tenta acessar os arquivos.

```rust
#[cfg(not(mobile))]
if !root.is_dir() {
    return Err(...);
}
```

## Problema estrutural

A solução paliativa funciona mas **inverte a responsabilidade**: no mobile não há
nenhuma validação antecipada — erros de caminho errado ou URI inválida só aparecem
na hora do sync, com mensagens genéricas de I/O.

O problema raiz é que `PathBuf` / `std::fs` vazam para fora da abstração `LocalStorage`.
Idealmente **nenhum código fora de `sync/storage.rs`** deveria manipular caminhos
diretamente — todo acesso a "existe essa pasta?" deveria passar pelo trait `LocalStorage`,
que sabe como tratar tanto `PathBuf` quanto URIs SAF.

## Solução desacoplada (a implementar)

Adicionar ao trait `LocalStorage` um método de validação:

```rust
pub trait LocalStorage: Send + Sync {
    // ... métodos existentes ...

    /// Verifica se o locador aponta para um diretório válido e acessível.
    async fn is_valid_root(&self, loc: &FileLoc) -> bool;

    /// Verifica se um caminho relativo existe sob a raiz.
    async fn subdir_exists(&self, root: &FileLoc, rel: &str) -> bool;
}
```

`DesktopStorage` implementaria com `Path::is_dir`; `MobileStorage` delegaria ao
`StoragePlugin` (que usaria `DocumentFile.isDirectory()` via SAF).

Os comandos `add_emulator` / `add_emulator_manual` receberiam o `LocalStorage` via
`AppState` e chamariam `is_valid_root` / `subdir_exists` em vez de `PathBuf::is_dir`.
Isso eliminaria todos os `#[cfg(not(mobile))]` de validação e centralizaria o
conhecimento de "como acessar arquivos" no trait.

## Status

- [x] Paliativo aplicado (`#[cfg(not(mobile))]`) — mobile funciona sem validação antecipada
- [ ] Refatoração desacoplada pendente (adicionar `is_valid_root`/`subdir_exists` ao trait)
