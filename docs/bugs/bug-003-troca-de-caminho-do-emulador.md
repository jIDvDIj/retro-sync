# BUG-003 — Sobrescrita do Drive ao trocar o caminho de um emulador já configurado

**Status:** ✅ resolvido (correção aplicada em `add_emulator` + `storage/emulators.rs`)  
**Severidade:** alta (sobrescrita de saves no Drive por uma instalação mais antiga)  
**Componente:** `src-tauri/src/storage/emulators.rs`, `src-tauri/src/commands.rs`

> **Resolução:** o `add_emulator` passou a usar `emulators::upsert_resetting_on_path_change`, que
> detecta quando o `root_path` de um emulador já registrado mudou e, na mesma transação do upsert,
> zera o estado de sync daquele emulador (`sync_manifest`, `sync_conflicts` e `pending_ops`). Sem as
> âncoras de mtime do caminho antigo, o próximo sync trata tudo como primeiro sync e cai em
> `DownloadWithBackup` (BUG-001): o Drive vence e o local é copiado para `<app_data>/backups/...`
> antes de ser sobrescrito — nada é perdido. O `add_emulator_manual` já era seguro: o guard `exists`
> bloqueia a regravação (`EmulatorExists`), então lá a troca de caminho só ocorre via remove+add,
> que já limpa o manifest.

---

## Descrição

Quando o usuário troca o `root_path` de um emulador **já registrado** — por exemplo, mantém duas
instâncias do PPSSPP (uma instalada no sistema, outra portátil num pendrive) e alterna entre elas —
o sync seguinte pode subir os saves da nova instalação por cima dos que estavam no Drive, mesmo que
estes sejam mais recentes/valiosos.

A raiz é que o `sync_manifest` ancora, por arquivo, o par `(local_mtime, drive_mtime)` do **último
sync bem-sucedido**. Esses mtimes locais se referem aos arquivos do caminho **antigo**. Ao apontar
o emulador para outra instalação, o diff compara o estado local novo contra âncoras de outra árvore
de arquivos e conclui erroneamente "o local mudou".

## Reprodução

1. Usuário registra o **PPSSPP portátil** (pendrive, `E:\PPSSPP`) e sincroniza ao sair. O manifest
   registra, para cada save, `(local_mtime=T_pendrive, drive_mtime=T_drive)`. O Drive fica com os
   saves da portátil.

2. Usuário troca para o **PPSSPP instalado** (`C:\...\PPSSPP`) chamando `add_emulator` na nova pasta.
   `emulators::upsert` faz `INSERT OR REPLACE` na linha do emulador `PPSSPP`, atualizando o
   `root_path` — **mas o `sync_manifest` não é tocado.**

3. Sync (`Bidirectional`) é disparado. Para um save que existe nas duas instalações, `decide()`
   recebe:
   - `local_mtime_ms` = `T_local` (mtime do arquivo na instalação do sistema — cópia distinta)
   - `drive_mtime_ms` = `T_drive` (inalterado desde o sync da portátil)
   - `last_synced` = `Some((T_pendrive, T_drive))` (âncora herdada da portátil)

4. `decide()` avalia: `local_changed = T_local ≠ T_pendrive` → **true**; `drive_changed =
   T_drive = T_drive` → **false** → **`Upload`**.

5. Os saves da instalação do sistema (possivelmente mais antigos) **sobem por cima** dos saves da
   portátil no Drive. Se aquela era a versão com mais progresso, ela é perdida no Drive.

## Causa raiz

`emulators::upsert` regrava o perfil via `INSERT OR REPLACE` sem nenhuma ciência de que o
`root_path` mudou. O estado de sync ancorado (`sync_manifest`) continua válido na tabela, mas passa
a descrever arquivos de **outra** instalação. O algoritmo de diff (`conflict::decide`) confia nesse
par de mtimes para distinguir "não mudou" de "mudou de um lado" de "conflito real" — e com a âncora
errada, a comparação fica inválida.

Diferente do BUG-001/BUG-002, aqui o `last_synced` **não** é `None`: ele existe, só que aponta para
o lugar errado. Por isso o caminho conservador de primeiro sync (`DownloadWithBackup`) não era
alcançado.

## Impacto

- Sobrescrita do Drive por uma instalação mais antiga ao alternar entre cópias do mesmo emulador
  (portátil ↔ instalada, ou após mover a pasta do emulador).
- Silenciosa: o diff vê um `Upload` legítimo; nenhum evento de erro é emitido.
- Afeta qualquer emulador suportado e qualquer categoria (saves, savestates, config).
- Não cobre o caso de detecção: as duas instâncias do mesmo emulador colidem no mesmo nome de
  perfil (`PPSSPP`), então a segunda sempre regrava a primeira.

## Solução adotada

Invalidar o estado de sync do emulador quando, e somente quando, o `root_path` muda em relação ao
já gravado. Implementado como uma função de storage `upsert`-ciente-de-troca, executada na mesma
transação para garantir atomicidade:

```rust
// storage/emulators.rs
pub fn upsert_resetting_on_path_change(
    conn: &Connection,
    profile: &EmulatorProfile,
) -> AppResult<bool> {
    let previous_root: Option<String> = /* SELECT root_path WHERE name = ? */;
    let new_root = profile.root_path.to_string_lossy().into_owned();
    let path_changed = previous_root.as_ref().is_some_and(|old| *old != new_root);

    upsert(conn, profile)?;

    if path_changed {
        manifest::remove_for_emulator(conn, &profile.name)?;
        conflicts::remove_for_emulator(conn, &profile.name)?;
        queue::remove_for_emulator(conn, &profile.name)?;
    }
    Ok(path_changed)
}
```

Com o manifest zerado, o próximo sync vê `last_synced = None` para os arquivos presentes nos dois
lados e cai em `SyncAction::DownloadWithBackup` (ver [BUG-001](./bug-001-perda-save-primeiro-sync.md)):
o Drive vence e o save local é copiado para a pasta de backup **antes** de ser sobrescrito. Resultado
não-destrutivo: a versão que estava no Drive (a recém-sincronizada da portátil) é preservada, e os
saves da instalação local ficam recuperáveis no backup.

### Por que não normalizar/comparar caminhos com mais esperteza

Um falso-positivo de troca de caminho (ex.: `E:\PPSSPP` vs `E:/PPSSPP`) apenas dispara o reset, que é
não-destrutivo (Drive-vence-com-backup). Não há perda nem corrupção; no pior caso, um sync extra
reconstrói o manifest. Por isso não se investiu em normalização de path — o custo do erro é baixo.

### Alternativas consideradas

- **Bloquear a regravação (como o `add_emulator_manual`)** — exigiria remove+add explícito do
  usuário. Preterida: o reset automático é mais ergonômico e igualmente seguro.
- **Ancorar o manifest por hash de conteúdo em vez de mtime** — resolveria a classe inteira de
  problemas de mtime, mas é uma mudança estrutural grande no diff/engine; fora de escopo.

## Arquivos modificados

| Arquivo | Mudança |
|---|---|
| [`src-tauri/src/storage/emulators.rs`](../../src-tauri/src/storage/emulators.rs) | Nova `upsert_resetting_on_path_change` — detecta troca de `root_path` e zera manifest/conflitos/fila na mesma transação |
| [`src-tauri/src/commands.rs`](../../src-tauri/src/commands.rs) | `add_emulator` passa a usar a nova função e loga distinto quando o caminho muda |

A boundary IPC não mudou de shape: `add_emulator` continua recebendo `path` e devolvendo
`EmulatorProfile`; o `bool` de reset é interno (só log). Sem espelho TS a atualizar.

## Testes

Cobertos em `storage/emulators.rs` (`cargo test`):

- `upsert_com_caminho_novo_reseta_estado_de_sync` — troca de `root_path` zera o `sync_manifest` e
  grava o novo caminho.
- `upsert_com_mesmo_caminho_preserva_o_manifest` — re-detectar a mesma pasta **não** apaga o estado.
- `upsert_de_emulador_novo_nao_reseta` — primeiro registro de um emulador não dispara reset.

Suíte completa: **88 testes** Rust passando; `clippy` limpo.

## Como testar

```bash
# (no WSL) export CARGO_TARGET_DIR=$HOME/.cache/retro-sync-target
cargo test --manifest-path src-tauri/Cargo.toml emulators::
```

Manual (console F12, com um emulador `PPSSPP` já sincronizado):

```js
// Aponta o mesmo emulador para outra pasta → o próximo sync deve reportar
// backed_up > 0 (Drive vence com backup), não uploads silenciosos.
await window.__TAURI__.core.invoke("add_emulator", { path: "C:\\caminho\\outra-instalacao-ppsspp" });
await window.__TAURI__.core.invoke("sync_now");
```

## Relação com outros bugs

- [BUG-001](./bug-001-perda-save-primeiro-sync.md) — a correção reaproveita o caminho
  `DownloadWithBackup` do primeiro sync para tornar o reset não-destrutivo.
- [BUG-002](./bug-002-conflito-edicao-simultanea.md) — mesma raiz conceitual: a confiabilidade do
  diff depende das âncoras de mtime no `sync_manifest` descreverem o estado correto.

## Referências

- `upsert_resetting_on_path_change`: [`src-tauri/src/storage/emulators.rs:98`](../../src-tauri/src/storage/emulators.rs)
- `add_emulator`: [`src-tauri/src/commands.rs:88`](../../src-tauri/src/commands.rs)
- `decide` → `DownloadWithBackup`: [`src-tauri/src/sync/conflict.rs:27`](../../src-tauri/src/sync/conflict.rs)
- Manifest como fonte de verdade operacional: [`docs/05-sincronizacao.md`](../05-sincronizacao.md)
