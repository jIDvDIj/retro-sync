# BUG-005 — Validações de filesystem incompatíveis com URIs SAF (mobile)

**Status:** ✅ resolvido — validação desacoplada via `LocalStorage`
(`is_valid_root`/`subdir_exists`); os `#[cfg(not(mobile))]` de validação foram removidos.

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

## Solução desacoplada (aplicada)

O trait `LocalStorage` ganhou dois métodos de validação:

```rust
pub trait LocalStorage: Send + Sync {
    // ... métodos existentes ...

    /// O locador aponta para um diretório válido e acessível?
    async fn is_valid_root(&self, loc: &FileLoc) -> bool;

    /// Existe a subpasta `rel` (separador `/`) sob `root`?
    async fn subdir_exists(&self, root: &FileLoc, rel: &str) -> bool;
}
```

`DesktopStorage` implementa com `Path::is_dir` (via `tokio::fs::metadata`); `MobileStorage`
delega ao plugin nativo reusando o comando `exists` (`DocumentFile` a partir da URI SAF),
sem exigir código Kotlin novo.

`AppState` passou a expor o `Arc<dyn LocalStorage>`. Os comandos `detect_emulator`,
`add_emulator` e `add_emulator_manual` chamam `is_valid_root`/`subdir_exists` em vez de
`PathBuf::is_dir` — a validação de existência saiu do `build_manual_profile` (agora puro,
só segurança de caminho) e todos os `#[cfg(not(mobile))]` de validação foram removidos. O
conhecimento de "como acessar arquivos" ficou centralizado no trait.

## Status

- [x] Paliativo aplicado (`#[cfg(not(mobile))]`) — mobile funcionava sem validação antecipada
- [x] Refatoração desacoplada (`is_valid_root`/`subdir_exists` no trait; comandos via `AppState`)
